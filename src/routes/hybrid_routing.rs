use crate::decider::gatewaydecider::flow_new::decider_full_payload_hs_function;
use crate::error::ContainerError;
use crate::euclid::ast::ConnectorInfo;
use crate::euclid::errors::EuclidErrors;
use crate::euclid::handlers::routing_rules::routing_evaluate;
use crate::types::hybrid_routing::HybridRoutingRequest;
use axum::{response::IntoResponse, Json};
use serde::Serialize;
use std::time::Instant;

fn to_json_value_or_invalid<T: Serialize>(
    value: &T,
    context: &str,
) -> Result<serde_json::Value, ContainerError<EuclidErrors>> {
    serde_json::to_value(value).map_err(|err| {
        EuclidErrors::InvalidRequest(format!("Failed to serialize {context}: {err}")).into()
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

fn to_logged_success_response(
    map: serde_json::Map<String, serde_json::Value>,
) -> axum::response::Response {
    let response_value = serde_json::Value::Object(map);
    crate::logger::debug!(decision_engine_success_response = ?response_value);
    Json(response_value).into_response()
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

/// Extracts ordered connector blobs from static routing output.
///
/// Order is preserved intentionally because static routing can return
/// connector priority, and dynamic routing consumes this as candidate order.
fn extract_static_eligible_gateways(
    response: &crate::euclid::types::RoutingEvaluateResponse,
) -> Vec<ConnectorInfo> {
    response.eligible_connectors.clone()
}

fn extract_gateway_names(connectors: &[ConnectorInfo]) -> Vec<String> {
    connectors
        .iter()
        .map(|connector| connector.gateway_name.clone())
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

#[axum::debug_handler]
pub async fn hybrid_routing_evaluate(
    Json(payload): Json<HybridRoutingRequest>,
) -> Result<axum::response::Response, ContainerError<EuclidErrors>> {
    let HybridRoutingRequest {
        static_routing_request,
        dynamic_routing_request,
    } = payload;

    let is_empty_request = static_routing_request.is_none() && dynamic_routing_request.is_none();

    let (static_routing_response, static_routing_error, static_fallback_gateways) =
        match static_routing_request {
            Some(req) => {
                // Preserve static fallback connectors even when static evaluation fails,
                // so dynamic can still run with a bounded candidate set.
                let fallback_gateways = req.fallback_output.clone();

                match routing_evaluate(Json(req)).await {
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
            // Request-provided dynamic list has precedence.
            // Static-derived list is only used when request list is absent/empty.
            let request_eligible_gateways = match req.eligible_gateway_list.take() {
                Some(gateways) if gateways.is_empty() => None,
                Some(gateways) => Some(gateways),
                None => None,
            };
            let fallback_eligible_gateways = dynamic_fallback_gateways
                .clone()
                .map(|connectors| extract_gateway_names(&connectors));
            req.eligible_gateway_list = request_eligible_gateways.or(fallback_eligible_gateways);
            Some(decider_full_payload_hs_function(req, Instant::now()).await)
        }
        None => None,
    };

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

    let response_result = match (
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
            let dynamic_connector = vec![parse_dynamic_connector(&dynamic_ok.decided_gateway)];
            insert_serialized(&mut res, "dynamic_routing", &dynamic_ok, "dynamic_routing")
                .and_then(|_| insert_evaluated_connectors(&mut res, &dynamic_connector))
                .map(|_| to_logged_success_response(res))
        }
        (false, Some(Err(_dynamic_err)), Some(dynamic_fallback), _, _) => {
            // Graceful degradation: when dynamic fails but fallback connectors exist,
            // return fallback connectors instead of hard-failing.
            insert_serialized(
                &mut res,
                "dynamic_routing",
                &dynamic_fallback,
                "dynamic_routing_fallback",
            )
            .and_then(|_| insert_evaluated_connectors(&mut res, &dynamic_fallback))
            .map(|_| to_logged_success_response(res))
        }
        (false, Some(Err(dynamic_err)), None, _, _) => {
            log_serializable_response("dynamic_error_response", &dynamic_err);
            Ok(dynamic_err.into_response())
        }
        (false, None, _, Some(static_gateways), _) => {
            insert_evaluated_connectors(&mut res, &static_gateways)
                .map(|_| to_logged_success_response(res))
        }
        (false, None, Some(dynamic_fallback), None, _) => {
            insert_evaluated_connectors(&mut res, &dynamic_fallback)
                .map(|_| to_logged_success_response(res))
        }
        (false, None, _, None, Some(static_err)) => {
            crate::logger::debug!(decision_engine_response_label = "static_error_response", error = ?static_err);
            Ok(static_err.into_response())
        }
        (false, None, _, None, None) => Ok(to_logged_success_response(res)),
    };

    static_insert_result.and(response_result)
}
