use crate::analytics::clickhouse::common::STATIC_ROUTING_APPROACH;
use crate::analytics::{
    global_request_id_from_headers, serialize_details, trace_id_from_headers, AnalyticsFlowContext,
    AnalyticsRoute, ApiFlow, DomainAnalyticsEvent, FlowType,
};
use crate::decider::gatewaydecider::flow_new::{
    decider_full_payload_hs_function, store_scoring_context_without_decider,
};
use crate::decider::gatewaydecider::types::DecidedGateway;
use crate::error::ContainerError;
use crate::euclid::ast::ConnectorInfo;
use crate::euclid::errors::EuclidErrors;
use crate::euclid::handlers::routing_rules::{
    routing_evaluate_with_analytics, RoutingEvaluateAnalyticsFlows,
};
use crate::euclid::types::ExperimentEndpoint;
use crate::feedback::constants::kvRedis;
use crate::metrics::{API_LATENCY_HISTOGRAM, API_REQUEST_COUNTER, API_REQUEST_TOTAL_COUNTER};
use crate::redis::feature::is_feature_enabled;
use crate::types::hybrid_routing::HybridRoutingRequest;
use axum::{response::IntoResponse, Json};
use serde::Serialize;
use std::time::Instant;

fn to_json_value_or_invalid<T: Serialize>(
    value: &T,
    context: &str,
) -> Result<serde_json::Value, ContainerError<EuclidErrors>> {
    serde_json::to_value(value).map_err(|err| {
        crate::logger::error!(
            serialization_context = context,
            "Failed to serialize hybrid routing response field: {}",
            err
        );
        EuclidErrors::FailedToSerializeJsonToString.into()
    })
}

fn insert_serialized<T: Serialize>(
    map: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    value: &T,
    context: &str,
) -> Result<(), ContainerError<EuclidErrors>> {
    to_json_value_or_invalid(value, context).map(|json_value| {
        map.insert(key.to_string(), json_value);
    })
}

fn log_serializable_response<T: Serialize>(label: &str, value: &T) {
    match serde_json::to_value(value) {
        Ok(response_value) => {
            crate::logger::debug!(decision_engine_response_label = label, decision_engine_response = ?response_value)
        }
        Err(err) => crate::logger::warn!(
            decision_engine_response_label = label,
            "Failed to serialize response for logging: {}",
            err
        ),
    }
}

/// The client-facing connector list for a static routing result. A straight clone of
/// `evaluated_output`, which is already the routing answer — selection at the head, eligible
/// fallbacks behind it (see `routing_evaluate`).
///
/// Not `eligible_connectors`, which is what the rule *could* pick, in declaration order, not what
/// it *chose*.
fn extract_static_eligible_gateways(
    response: &crate::euclid::types::RoutingEvaluateResponse,
) -> Vec<ConnectorInfo> {
    response.evaluated_output.clone()
}

fn extract_gateway_identifiers(connectors: &[ConnectorInfo]) -> Vec<String> {
    connectors
        .iter()
        .map(|connector| match &connector.gateway_id {
            Some(gateway_id) => format!("{}:{}", connector.gateway_name, gateway_id),
            None => connector.gateway_name.clone(),
        })
        .collect::<Vec<String>>()
}

fn parse_dynamic_connector(connector_with_id: &str) -> ConnectorInfo {
    match connector_with_id.split_once(':') {
        Some((gateway_name, gateway_id)) => ConnectorInfo {
            gateway_name: gateway_name.to_string(),
            gateway_id: Some(gateway_id.to_string()),
        },
        None => ConnectorInfo {
            gateway_name: connector_with_id.to_string(),
            gateway_id: None,
        },
    }
}

pub const SR_ROUTING_FEATURE_FLAG: &str = "sr_routing_enabled";
/// Whether outcomes of hybrid payments decided by their rule output (SR skipped) update SR scores.
pub const SR_SCORES_FROM_RULE_ROUTING_FEATURE_FLAG: &str = "sr_scores_from_rule_routing_enabled";

#[derive(Serialize)]
struct DynamicRoutingEnvelope {
    status: &'static str,
    decision: Option<DecidedGateway>,
    fallback_connectors: Option<Vec<ConnectorInfo>>,
}

/// Stores the normalized client-facing connector field.
///
/// Clients should rely on `evaluated_connectors` for connector consumption
/// instead of branching on static/dynamic payload shapes.
fn insert_evaluated_connectors(
    map: &mut serde_json::Map<String, serde_json::Value>,
    connectors: &[ConnectorInfo],
) -> Result<(), ContainerError<EuclidErrors>> {
    insert_serialized(
        map,
        "evaluated_connectors",
        &connectors.to_vec(),
        "evaluated_connectors",
    )
}

/// The audit record of one hybrid call. The whole request and the whole response are captured
/// once, with the pieces the audit page reads: connector scores and the selection reason.
#[derive(Serialize)]
struct HybridRoutingSuccessDetail<'a> {
    request: &'a HybridRoutingRequest,
    response: &'a serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    score_context: Option<&'a serde_json::Value>,
    selection_reason: &'a HybridDecisionSummary,
}

#[derive(Serialize)]
struct HybridRoutingFailureDetail<'a> {
    request: &'a HybridRoutingRequest,
    error: serde_json::Value,
}

/// What a successful hybrid call resolved to. This is the `selection_reason` of its audit event,
/// so the field order here is the order the audit page reads. `score_context` rides along for the
/// detail's own field and stays out of the reason.
#[derive(Serialize)]
struct HybridDecisionSummary {
    decided_gateway: Option<String>,
    routing_approach: String,
    dynamic_status: &'static str,
    #[serde(
        serialize_with = "serialize_connector_list",
        skip_serializing_if = "Vec::is_empty"
    )]
    static_connectors: Vec<String>,
    priority_logic_tag: Option<String>,
    #[serde(skip_serializing)]
    score_context: Option<serde_json::Value>,
}

impl HybridDecisionSummary {
    fn static_only(dynamic_status: &'static str, connectors: Vec<String>) -> Self {
        Self {
            decided_gateway: connectors.first().cloned(),
            routing_approach: STATIC_ROUTING_APPROACH.to_string(),
            dynamic_status,
            static_connectors: connectors,
            priority_logic_tag: None,
            score_context: None,
        }
    }
}

fn serialize_connector_list<S>(connectors: &[String], serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    serializer.serialize_str(&connectors.join(", "))
}

/// A hybrid call that answers with an error response.
struct HybridFailure {
    response: axum::response::Response,
    error_code: String,
    error_message: String,
    error: serde_json::Value,
}

#[axum::debug_handler]
pub async fn hybrid_routing_evaluate(
    headers: axum::http::HeaderMap,
    Json(payload): Json<HybridRoutingRequest>,
) -> Result<axum::response::Response, ContainerError<EuclidErrors>> {
    let timer = API_LATENCY_HISTOGRAM
        .with_label_values(&["hybrid_routing_evaluate"])
        .start_timer();
    API_REQUEST_TOTAL_COUNTER
        .with_label_values(&["hybrid_routing_evaluate"])
        .inc();

    let recorded_request = payload.clone();
    let HybridRoutingRequest {
        static_routing_request,
        dynamic_routing_request,
    } = payload;

    let request_id = headers
        .get(crate::storage::consts::X_REQUEST_ID)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
        .unwrap_or_else(crate::analytics::next_event_id);
    let global_request_id = global_request_id_from_headers(&headers);
    let trace_id = trace_id_from_headers(&headers);

    let merchant_id = dynamic_routing_request
        .as_ref()
        .map(|request| request.merchant_id.clone())
        .or_else(|| {
            static_routing_request
                .as_ref()
                .map(|request| request.created_by.clone())
        });
    let payment_id = dynamic_routing_request
        .as_ref()
        .map(|request| request.payment_id().to_string())
        .or_else(|| {
            static_routing_request
                .as_ref()
                .and_then(|request| request.payment_id.clone())
        });
    let auth_type = dynamic_routing_request
        .as_ref()
        .and_then(|request| request.auth_type());

    DomainAnalyticsEvent::record_request_hit(
        AnalyticsFlowContext::new(ApiFlow::DynamicRouting, FlowType::RoutingHybridRequestHit),
        AnalyticsRoute::RoutingHybrid,
        merchant_id.clone(),
        payment_id.clone(),
        Some(request_id.clone()),
        global_request_id.clone(),
        trace_id.clone(),
        auth_type.clone(),
    );

    let is_empty_request = static_routing_request.is_none() && dynamic_routing_request.is_none();

    // Whether the dynamic (SR) half runs. An active experiment decides for the payments it serves
    // (its arm's SR layer); otherwise the merchant's `sr-routing` setting does. An arm without an SR
    // layer needs the static half's rule output, so without one the setting decides.
    let run_dynamic_routing = match dynamic_routing_request.as_ref() {
        None => false,
        Some(req) => {
            let arm_has_sr_layer =
                crate::decider::gatewaydecider::ab_test::interceptor::hybrid_arm_has_sr_layer(
                    &req.merchant_id,
                    req.payment_id(),
                )
                .await;
            match arm_has_sr_layer {
                Some(true) => true,
                Some(false) if static_routing_request.is_some() => false,
                _ => {
                    is_feature_enabled(
                        SR_ROUTING_FEATURE_FLAG.to_string(),
                        req.merchant_id.clone(),
                        kvRedis(),
                    )
                    .await
                }
            }
        }
    };

    // The static half runs silently, this call records one event for both halves below.
    let (static_routing_response, static_routing_error, static_fallback_gateways) =
        match static_routing_request {
            Some(mut req) => {
                // Preserve static fallback connectors even when static evaluation fails,
                // so dynamic can still run with a bounded candidate set.
                let fallback_gateways = req.fallback_output.clone();

                // A/B arms are assigned by payment id; both halves must see the same one so the
                // rule layer and the SR layer come from the same arm. The dynamic request's id is
                // the one scores and outcomes are recorded under, so it wins.
                let dynamic_payment_id = dynamic_routing_request
                    .as_ref()
                    .map(|dynamic| dynamic.payment_id())
                    .filter(|id| !id.is_empty());
                if let Some(dynamic_payment_id) = dynamic_payment_id {
                    if req
                        .payment_id
                        .as_deref()
                        .is_some_and(|id| !id.is_empty() && id != dynamic_payment_id)
                    {
                        crate::logger::warn!(
                            "hybrid routing: static payment_id {:?} differs from dynamic payment_id {}; using the dynamic one",
                            req.payment_id,
                            dynamic_payment_id
                        );
                    }
                    req.payment_id = Some(dynamic_payment_id.to_string());
                }

                // The decider half records the experiment decision when it runs; otherwise the
                // rule output is the decision and is recorded here.
                match routing_evaluate_with_analytics(
                    headers.clone(),
                    Json(req),
                    RoutingEvaluateAnalyticsFlows::routing_hybrid(),
                    Some(request_id.clone()),
                    ExperimentEndpoint::HybridRouting,
                    !run_dynamic_routing,
                )
                .await
                {
                    Ok(response) => (Some(response.0), None, fallback_gateways),
                    Err(err) => (None, Some(err), fallback_gateways),
                }
            }
            None => (None, None, None),
        };

    // Prefer static evaluated connectors over static fallback connectors
    // when auto-populating dynamic candidates.
    let static_eligible_gateways = static_routing_response
        .as_ref()
        .map(extract_static_eligible_gateways);

    let dynamic_fallback_gateways = static_eligible_gateways
        .clone()
        .or(static_fallback_gateways);

    let dynamic_eval_result = match dynamic_routing_request {
        Some(mut req) => {
            if run_dynamic_routing {
                let request_eligible_gateways = match req.eligible_gateway_list.take() {
                    Some(gateways) if gateways.is_empty() => None,
                    Some(gateways) => Some(gateways),
                    None => None,
                };
                let static_eligible_gateway_ids = static_eligible_gateways
                    .as_ref()
                    .filter(|connectors| !connectors.is_empty())
                    .map(|connectors| extract_gateway_identifiers(connectors));
                let fallback_eligible_gateways = dynamic_fallback_gateways
                    .clone()
                    .map(|connectors| extract_gateway_identifiers(&connectors));
                req.eligible_gateway_list = static_eligible_gateway_ids
                    .or(request_eligible_gateways)
                    .or(fallback_eligible_gateways);
                let result = decider_full_payload_hs_function(
                    req,
                    Instant::now(),
                    ExperimentEndpoint::HybridRouting,
                )
                .await;
                API_REQUEST_COUNTER
                    .with_label_values(&[
                        "hybrid_routing_evaluate_dynamic",
                        if result.is_ok() { "success" } else { "failure" },
                    ])
                    .inc();
                Some(result)
            } else {
                crate::logger::debug!(
                    "dynamic routing skipped for merchant {}: sr_routing_enabled is off or the A/B arm has no SR layer",
                    req.merchant_id
                );
                // Saved before responding so the payment's `/update-gateway-score` finds it.
                if static_routing_response.is_some()
                    && is_feature_enabled(
                        SR_SCORES_FROM_RULE_ROUTING_FEATURE_FLAG.to_string(),
                        req.merchant_id.clone(),
                        kvRedis(),
                    )
                    .await
                {
                    if let Err(err) =
                        store_scoring_context_without_decider(&req, STATIC_ROUTING_APPROACH).await
                    {
                        crate::logger::warn!(
                            "failed to store scoring context for rule-routed payment {}: {}",
                            req.payment_id(),
                            err.error_message
                        );
                    }
                }
                None
            }
        }
        None => None,
    };

    let static_connector_ids = static_eligible_gateways
        .as_ref()
        .map(|connectors| extract_gateway_identifiers(connectors))
        .unwrap_or_default();

    let mut res = serde_json::Map::new();

    let static_insert_result = match static_routing_response.as_ref() {
        Some(static_response) => insert_serialized(
            &mut res,
            "static_routing",
            static_response,
            "static_routing",
        ),
        None => Ok(()),
    };

    // Resolve the call to its response body plus the summary its audit event needs.
    let resolve = move || -> Result<
        Result<(serde_json::Map<String, serde_json::Value>, HybridDecisionSummary), HybridFailure>,
        ContainerError<EuclidErrors>,
    > {
        static_insert_result?;
        match (
            is_empty_request,
            dynamic_eval_result,
            dynamic_fallback_gateways,
            static_eligible_gateways,
            static_routing_error,
        ) {
            (true, _, _, _, _) => Err(EuclidErrors::InvalidRequest(
                "At least one of static_routing_request or dynamic_routing_request must be provided."
                    .to_string(),
            )
            .into()),
            (false, Some(Ok(dynamic_ok)), _, _, _) => {
                // Dynamic winner is the first-class output for normalized connector field.
                let summary = HybridDecisionSummary {
                    decided_gateway: Some(dynamic_ok.decided_gateway.clone()),
                    routing_approach: dynamic_ok.routing_approach.to_string(),
                    priority_logic_tag: dynamic_ok.priority_logic_tag.clone(),
                    score_context: dynamic_ok.gateway_priority_map.clone(),
                    dynamic_status: "success",
                    static_connectors: static_connector_ids,
                };
                let dynamic_connector = vec![parse_dynamic_connector(&dynamic_ok.decided_gateway)];
                let dynamic_payload = DynamicRoutingEnvelope {
                    status: "success",
                    decision: Some(dynamic_ok),
                    fallback_connectors: None,
                };
                insert_serialized(
                    &mut res,
                    "dynamic_routing",
                    &dynamic_payload,
                    "dynamic_routing",
                )?;
                insert_evaluated_connectors(&mut res, &dynamic_connector)?;
                Ok(Ok((res, summary)))
            }
            (false, Some(Err(_dynamic_err)), Some(dynamic_fallback), _, _) => {
                // when dynamic fails but fallback connectors exist,
                // return fallback connectors instead of hard-failing.
                let dynamic_payload = DynamicRoutingEnvelope {
                    status: "fallback",
                    decision: None,
                    fallback_connectors: Some(dynamic_fallback.clone()),
                };
                insert_serialized(
                    &mut res,
                    "dynamic_routing",
                    &dynamic_payload,
                    "dynamic_routing",
                )?;
                insert_evaluated_connectors(&mut res, &dynamic_fallback)?;
                let fallback_ids = extract_gateway_identifiers(&dynamic_fallback);
                Ok(Ok((
                    res,
                    HybridDecisionSummary::static_only("fallback", fallback_ids),
                )))
            }
            (false, Some(Err(dynamic_err)), None, _, _) => {
                log_serializable_response("dynamic_error_response", &dynamic_err);
                let error = serde_json::to_value(&dynamic_err).unwrap_or(serde_json::Value::Null);
                Ok(Err(HybridFailure {
                    error_code: dynamic_err.error_code.clone(),
                    error_message: dynamic_err.error_message.clone(),
                    error,
                    response: dynamic_err.into_response(),
                }))
            }
            (false, None, _, Some(static_gateways), _) => {
                insert_evaluated_connectors(&mut res, &static_gateways)?;
                Ok(Ok((
                    res,
                    HybridDecisionSummary::static_only("skipped", static_connector_ids),
                )))
            }
            (false, None, Some(dynamic_fallback), None, _) => {
                insert_evaluated_connectors(&mut res, &dynamic_fallback)?;
                let fallback_ids = extract_gateway_identifiers(&dynamic_fallback);
                Ok(Ok((
                    res,
                    HybridDecisionSummary::static_only("skipped", fallback_ids),
                )))
            }
            (false, None, _, None, Some(static_err)) => {
                crate::logger::debug!(decision_engine_response_label = "static_error_response", error = ?static_err);
                let error_message = static_err.get_inner().to_string();
                let error = static_err
                    .downcast_ref::<crate::error::ApiErrorResponse>()
                    .and_then(|payload| serde_json::to_value(payload).ok())
                    .unwrap_or_else(|| serde_json::Value::String(error_message.clone()));
                Ok(Err(HybridFailure {
                    error_code: "HYBRID_STATIC_ROUTING_FAILED".to_string(),
                    error_message,
                    error,
                    response: static_err.into_response(),
                }))
            }
            (false, None, _, None, None) => Ok(Ok((
                res,
                HybridDecisionSummary::static_only("skipped", static_connector_ids),
            ))),
        }
    };

    let dynamic_request = recorded_request.dynamic_routing_request.as_ref();
    let record_failure = |error_code: String, error_message: String, error: serde_json::Value| {
        DomainAnalyticsEvent::record_error(
            AnalyticsFlowContext::new(ApiFlow::DynamicRouting, FlowType::RoutingHybridError),
            AnalyticsRoute::RoutingHybrid,
            merchant_id.clone(),
            payment_id.clone(),
            Some(request_id.clone()),
            global_request_id.clone(),
            trace_id.clone(),
            None,
            None,
            error_code,
            error_message,
            serialize_details(&HybridRoutingFailureDetail {
                request: &recorded_request,
                error,
            }),
            Some("hybrid_failed".to_string()),
            auth_type.clone(),
        );
        API_REQUEST_COUNTER
            .with_label_values(&["hybrid_routing_evaluate", "failure"])
            .inc();
    };

    let api_result = match resolve() {
        Ok(Ok((res, summary))) => {
            let response_value = serde_json::Value::Object(res);
            crate::logger::debug!(decision_engine_success_response = ?response_value);
            DomainAnalyticsEvent::record_decision(
                AnalyticsFlowContext::new(ApiFlow::DynamicRouting, FlowType::RoutingHybridDecision),
                merchant_id.clone(),
                Some(summary.routing_approach.clone()),
                summary.decided_gateway.clone(),
                Some("success".to_string()),
                AnalyticsRoute::RoutingHybrid,
                summary.priority_logic_tag.clone(),
                serialize_details(&HybridRoutingSuccessDetail {
                    request: &recorded_request,
                    response: &response_value,
                    score_context: summary.score_context.as_ref(),
                    selection_reason: &summary,
                }),
                payment_id.clone(),
                Some(request_id.clone()),
                global_request_id.clone(),
                trace_id.clone(),
                Some("hybrid_routed".to_string()),
                dynamic_request.map(|request| request.payment_method_type().to_string()),
                dynamic_request.map(|request| request.payment_method().to_string()),
                auth_type.clone(),
                dynamic_request.and_then(|request| request.card_network()),
                dynamic_request.map(|request| request.currency()),
                dynamic_request.and_then(|request| request.country()),
            );
            API_REQUEST_COUNTER
                .with_label_values(&["hybrid_routing_evaluate", "success"])
                .inc();
            Ok(Json(response_value).into_response())
        }
        Ok(Err(failure)) => {
            record_failure(failure.error_code, failure.error_message, failure.error);
            Ok(failure.response)
        }
        Err(err) => {
            let message = err.get_inner().to_string();
            record_failure(
                "HYBRID_ROUTING_FAILED".to_string(),
                message.clone(),
                serde_json::Value::String(message),
            );
            Err(err)
        }
    };
    timer.observe_duration();
    api_result
}
