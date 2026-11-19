use std::time::Instant;

use crate::custom_extractors::{Json, RequestIds, ScopedJson, ScopedJsonRejection};
use crate::euclid::types::ExperimentEndpoint;
use crate::{
    analytics::{
        serialize_details, AnalyticsFlowContext, AnalyticsRoute, ApiFlow, DomainAnalyticsEvent,
        FlowType,
    },
    decider::gatewaydecider::{
        flow_new::decider_full_payload_hs_function,
        types::{
            DecidedGateway, DomainDeciderRequestForApiCallV2, ErrorResponse,
            GatewayDeciderApproach, RankingAlgorithm, ResetApproach, UnifiedError,
        },
    },
    logger,
};
use axum::response::IntoResponse;
use serde::Serialize;

// impl IntoResponse for ErrorResponse {
//     fn into_response(self) -> axum::http::Response<axum::body::Body> {
//         let body = serde_json::to_string(&self).unwrap();
//         axum::http::Response::builder()
//             .status(StatusCode::BAD_REQUEST)
//             .header("Content-Type", "application/json")
//             .body(axum::body::Body::from(body))
//             .unwrap()
//     }
// }

#[derive(Debug, Serialize)]
struct DecideGatewayReadFailureDetail<'a> {
    request_id: &'a str,
    response: &'a ErrorResponse,
}

#[derive(Debug, Serialize)]
struct DecideGatewaySelectionReason<'a> {
    decided_gateway: &'a str,
    routing_approach: &'a GatewayDeciderApproach,
    gateway_before_evaluation: Option<&'a str>,
    priority_logic_tag: Option<&'a str>,
    reset_approach: &'a ResetApproach,
}

#[derive(Debug, Serialize)]
struct DecideGatewaySuccessDetail<'a> {
    request: &'a DomainDeciderRequestForApiCallV2,
    response: &'a DecidedGateway,
    score_context: Option<&'a serde_json::Value>,
    selection_reason: DecideGatewaySelectionReason<'a>,
}

#[derive(Debug, Serialize)]
struct DecideGatewayFailureDetail<'a> {
    request_id: &'a str,
    request: &'a DomainDeciderRequestForApiCallV2,
    routing_approach: Option<&'a GatewayDeciderApproach>,
    response: &'a ErrorResponse,
}

#[derive(Debug, Serialize)]
struct DecideGatewayParseFailureDetail<'a> {
    request_id: &'a str,
    raw_request: masking::Secret<String>,
    response: &'a ErrorResponse,
}

pub(crate) fn requested_routing_approach(
    ranking_algorithm: Option<&RankingAlgorithm>,
    response_routing_approach: Option<&GatewayDeciderApproach>,
) -> Option<String> {
    response_routing_approach
        .map(ToString::to_string)
        .or_else(|| ranking_algorithm.map(ToString::to_string))
}

fn request_parse_error_response(error: impl ToString) -> ErrorResponse {
    ErrorResponse {
        status: "400".to_string(),
        error_code: "400".to_string(),
        error_message: "Error parsing request".to_string(),
        priority_logic_tag: None,
        routing_approach: None,
        filter_wise_gateways: None,
        error_info: UnifiedError {
            code: "INVALID_INPUT".to_string(),
            user_message: "Invalid request params. Please verify your input.".to_string(),
            developer_message: error.to_string(),
        },
        priority_logic_output: None,
        is_dynamic_mga_enabled: false,
    }
}

fn invalid_payment_method_response(developer_message: String) -> ErrorResponse {
    ErrorResponse {
        status: "400".to_string(),
        error_code: "400".to_string(),
        error_message: "Invalid payment method".to_string(),
        priority_logic_tag: None,
        routing_approach: None,
        filter_wise_gateways: None,
        error_info: UnifiedError {
            code: "INVALID_PAYMENT_METHOD".to_string(),
            user_message:
                "paymentMethod and paymentMethodType are missing, swapped or do not fit together."
                    .to_string(),
            developer_message,
        },
        priority_logic_output: None,
        is_dynamic_mga_enabled: false,
    }
}

impl IntoResponse for DecidedGateway {
    fn into_response(self) -> axum::http::Response<axum::body::Body> {
        Json(self).into_response()
    }
}

/// Runs the decider and records the request hit, the decision and any failure. Callers that
/// already hold a parsed request use this directly; `decide_gateway` wraps it with the raw-body
/// read and parse-failure reporting that only an HTTP entry point can do.
pub(crate) async fn run_decider_with_analytics(
    payload: DomainDeciderRequestForApiCallV2,
    x_request_id: &str,
    global_request_id: Option<String>,
    trace_id: Option<String>,
    cpu_start: Instant,
) -> Result<DecidedGateway, ErrorResponse> {
    let auth_type = payload.auth_type();
    DomainAnalyticsEvent {
        merchant_id: Some(payload.merchant_id.clone()),
        payment_id: Some(payload.payment_id().to_string()),
        request_id: Some(x_request_id.to_string()),
        global_request_id: global_request_id.clone(),
        trace_id: trace_id.clone(),
        auth_type: auth_type.clone(),
        ..DomainAnalyticsEvent::request_hit(
            AnalyticsFlowContext::new(ApiFlow::DynamicRouting, FlowType::DecideGatewayRequestHit),
            AnalyticsRoute::DecideGateway,
        )
    }
    .emit();

    match decider_full_payload_hs_function(
        payload.clone(),
        cpu_start,
        ExperimentEndpoint::DecideGateway,
    )
    .await
    {
        Ok(decided_gateway) => {
            let routing_approach = decided_gateway.routing_approach.to_string();
            let (payment_method, payment_method_type) = payload.instrument().analytics_columns();

            DomainAnalyticsEvent {
                merchant_id: Some(payload.merchant_id.clone()),
                routing_approach: Some(routing_approach),
                gateway: Some(decided_gateway.decided_gateway.clone()),
                status: Some("success".to_string()),
                rule_name: decided_gateway.priority_logic_tag.clone(),
                details: serialize_details(&DecideGatewaySuccessDetail {
                    request: &payload,
                    response: &decided_gateway,
                    score_context: decided_gateway.gateway_priority_map.as_ref(),
                    selection_reason: DecideGatewaySelectionReason {
                        decided_gateway: &decided_gateway.decided_gateway,
                        routing_approach: &decided_gateway.routing_approach,
                        gateway_before_evaluation: decided_gateway
                            .gateway_before_evaluation
                            .as_deref(),
                        priority_logic_tag: decided_gateway.priority_logic_tag.as_deref(),
                        reset_approach: &decided_gateway.reset_approach,
                    },
                }),
                payment_id: Some(payload.payment_id().to_string()),
                request_id: Some(x_request_id.to_string()),
                global_request_id,
                trace_id,
                event_stage: Some("gateway_decided".to_string()),
                payment_method_type,
                payment_method,
                auth_type,
                card_network: payload.card_network(),
                currency: Some(payload.currency()),
                country: payload.country(),
                ..DomainAnalyticsEvent::new(
                    AnalyticsFlowContext::new(
                        ApiFlow::DynamicRouting,
                        FlowType::DecideGatewayDecision,
                    ),
                    AnalyticsRoute::DecideGateway,
                )
            }
            .emit_decision();
            Ok(decided_gateway)
        }
        Err(e) => {
            logger::debug!(tag = "DecideGateway", "Error: {:?}", e);
            let routing_approach = requested_routing_approach(
                payload.ranking_algorithm.as_ref(),
                e.routing_approach.as_ref(),
            );
            DomainAnalyticsEvent {
                merchant_id: Some(payload.merchant_id.clone()),
                payment_id: Some(payload.payment_id().to_string()),
                request_id: Some(x_request_id.to_string()),
                global_request_id,
                trace_id,
                routing_approach,
                details: serialize_details(&DecideGatewayFailureDetail {
                    request_id: x_request_id,
                    request: &payload,
                    routing_approach: e.routing_approach.as_ref(),
                    response: &e,
                }),
                event_stage: Some("request_failed".to_string()),
                auth_type,
                ..DomainAnalyticsEvent::error(
                    AnalyticsFlowContext::new(
                        ApiFlow::DynamicRouting,
                        FlowType::DecideGatewayError,
                    ),
                    AnalyticsRoute::DecideGateway,
                    e.error_code.clone(),
                    e.error_message.clone(),
                )
            }
            .emit();
            Err(e)
        }
    }
}

#[axum::debug_handler]
pub async fn decide_gateway(
    headers: axum::http::HeaderMap,
    request: Result<ScopedJson<DomainDeciderRequestForApiCallV2>, ScopedJsonRejection>,
) -> Result<DecidedGateway, ErrorResponse> {
    let cpu_start = Instant::now();

    let RequestIds {
        request_id,
        global_request_id,
        trace_id,
    } = RequestIds::from_headers(&headers);
    let x_request_id = request_id.unwrap_or_else(|| "unknown".to_string());
    // Mask values by default and consolidate header diagnostics into one event.
    logger::debug!(
        tag = "DecideGateway",
        headers = ?headers
            .keys()
            .map(|name| (name.as_str(), "**MASKED**"))
            .collect::<std::collections::BTreeMap<_, _>>(),
        "Incoming request headers"
    );
    let (body, parsed) = match request {
        Ok(ScopedJson { payload, body, .. }) => {
            logger::debug!("Body: {:?}", masking::Secret::<_>::new(&body));
            (body, Ok(payload))
        }
        Err(ScopedJsonRejection::Parse { body, error }) => {
            logger::debug!("Body: {:?}", masking::Secret::<_>::new(&body));
            (body, Err(error))
        }
        Err(ScopedJsonRejection::Scope(violation)) => {
            return Err(violation.into());
        }
        Err(ScopedJsonRejection::Read(e)) => {
            let (error_code, error_message) = e.analytics_code_and_message();
            let analytics_stage = e.analytics_stage().to_string();
            let error_response = e.into_error_response();
            DomainAnalyticsEvent {
                request_id: Some(x_request_id.clone()),
                global_request_id: global_request_id.clone(),
                trace_id: trace_id.clone(),
                details: serialize_details(&DecideGatewayReadFailureDetail {
                    request_id: &x_request_id,
                    response: &error_response,
                }),
                event_stage: Some(analytics_stage),
                ..DomainAnalyticsEvent::error(
                    AnalyticsFlowContext::new(
                        ApiFlow::DynamicRouting,
                        FlowType::DecideGatewayError,
                    ),
                    AnalyticsRoute::DecideGateway,
                    error_code,
                    error_message.to_string(),
                )
            }
            .emit();
            return Err(error_response);
        }
    };
    let api_decider_request = match parsed {
        Ok(payload) => match payload.check_payment_method() {
            Ok(_) => Ok(payload),
            Err(error) => Err(invalid_payment_method_response(error.to_string())),
        },
        Err(e) => Err(request_parse_error_response(&e)),
    };
    let result = match api_decider_request {
        Ok(payload) => {
            let span = tracing::Span::current();
            if !payload.merchant_id.is_empty() {
                span.record("merchant_id", payload.merchant_id.as_str());
            }
            if !payload.payment_id().is_empty() {
                span.record("payment_id", payload.payment_id());
            }
            run_decider_with_analytics(
                payload,
                &x_request_id,
                global_request_id,
                trace_id,
                cpu_start,
            )
            .await
        }
        Err(error_response) => {
            DomainAnalyticsEvent {
                request_id: Some(x_request_id.clone()),
                global_request_id,
                trace_id,
                details: serialize_details(&DecideGatewayParseFailureDetail {
                    request_id: &x_request_id,
                    raw_request: masking::Secret::new(String::from_utf8_lossy(&body).into_owned()),
                    response: &error_response,
                }),
                event_stage: Some("request_parse_failed".to_string()),
                ..DomainAnalyticsEvent::error(
                    AnalyticsFlowContext::new(
                        ApiFlow::DynamicRouting,
                        FlowType::DecideGatewayError,
                    ),
                    AnalyticsRoute::DecideGateway,
                    error_response.error_code.clone(),
                    error_response.error_message.clone(),
                )
            }
            .emit();
            Err(error_response)
        }
    };

    // let connection = state.db.get_conn().await.unwrap();
    // println!("Starting Decision Gateway");
    // let res = crate::redis::feature::isFeatureEnabled("ENABLE_RESET_ON_SR_V3".to_string(), "jungleerummy".to_string(), "".to_string()).await;
    // println!("Decision Gateway expect true: {:?}", res);

    // let res = crate::redis::feature::isFeatureEnabled("ENABLE_RESET_ON_SR_V3".to_string(), "ClassicRummy".to_string(), "".to_string()).await;
    // println!("Decision Gateway expect false: {:?}", res);

    // let res = crate::redis::feature::isFeatureEnabled("ENABLE_RESET_ON_SR_V3".to_string(), "zeptomarketplace".to_string(), "".to_string()).await;
    // println!("Decision Gateway expect false: {:?}", res);

    result
}

// SELECT `merchant_iframe_preferences`.`id`, `merchant_iframe_preferences`.`merchant_id`, `merchant_iframe_preferences`.`dynamic_switching_enabled`, `merchant_iframe_preferences`.`isin_routing_enabled`, `merchant_iframe_preferences`.`issuer_routing_enabled`, `merchant_iframe_preferences`.`txn_failure_gateway_penality`, `merchant_iframe_preferences`.`card_brand_routing_enabled` FROM `merchant_iframe_preferences` WHERE (`merchant_iframe_preferences`.`merchant_id` = ?)
