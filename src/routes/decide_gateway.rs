use std::time::Instant;

use crate::{
    decider::gatewaydecider::{
        flow_new::decider_full_payload_hs_function,
        types::{DecidedGateway, DomainDeciderRequestForApiCallV2, ErrorResponse, UnifiedError},
    },
    logger, metrics,
};
use axum::body::to_bytes;
use axum::http::StatusCode;
use axum::response::IntoResponse;

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

impl IntoResponse for DecidedGateway {
    fn into_response(self) -> axum::http::Response<axum::body::Body> {
        let body = serde_json::to_string(&self).unwrap();
        axum::http::Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/json")
            .body(axum::body::Body::from(body))
            .unwrap()
    }
}

#[axum::debug_handler]
pub async fn decide_gateway(
    req: axum::http::Request<axum::body::Body>,
) -> Result<DecidedGateway, ErrorResponse> {
    let cpu_start = Instant::now();
    let timer = metrics::API_LATENCY_HISTOGRAM
        .with_label_values(&["decide_gateway"])
        .start_timer();
    metrics::API_REQUEST_TOTAL_COUNTER
        .with_label_values(&["decide_gateway"])
        .inc();

    let headers = req.headers().clone();
    let x_request_id = headers
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("unknown")
        .to_string();
    for (name, value) in headers.iter() {
        logger::debug!(tag = "DecideGateway", "Header: {}: {:?}", name, value);
    }
    let body = match to_bytes(req.into_body(), usize::MAX).await {
        Ok(body) => {
            logger::debug!("Body: {:?}", body);
            body
        }
        Err(e) => {
            logger::debug!(tag = "DecideGateway", "Error: {:?}", e);
            crate::analytics::record_error_event(
                "decide_gateway",
                None,
                None,
                Some(x_request_id.clone()),
                None,
                None,
                "400".to_string(),
                "Error parsing request".to_string(),
                serde_json::to_string(&serde_json::json!({
                    "request_id": x_request_id,
                    "response": {
                        "status": "400",
                        "error_code": "400",
                        "error_message": "Error parsing request",
                        "error_info": {
                            "code": "INVALID_INPUT",
                            "user_message": "Invalid request params. Please verify your input.",
                            "developer_message": e.to_string(),
                        }
                    }
                }))
                .ok(),
                Some("request_parse_failed".to_string()),
            );
            metrics::API_REQUEST_COUNTER
                .with_label_values(&["decide_gateway", "failure"])
                .inc();
            timer.observe_duration();
            return Err(ErrorResponse {
                status: "400".to_string(),
                error_code: "400".to_string(),
                error_message: "Error parsing request".to_string(),
                priority_logic_tag: None,
                routing_approach: None,
                filter_wise_gateways: None,
                error_info: UnifiedError {
                    code: "INVALID_INPUT".to_string(),
                    user_message: "Invalid request params. Please verify your input.".to_string(),
                    developer_message: e.to_string(),
                },
                priority_logic_output: None,
                is_dynamic_mga_enabled: false,
            });
        }
    };
    let api_decider_request: Result<DomainDeciderRequestForApiCallV2, _> =
        serde_json::from_slice(&body);
    let result = match api_decider_request {
        Ok(payload) => match decider_full_payload_hs_function(payload.clone(), cpu_start).await {
            Ok(decided_gateway) => {
                let routing_approach = serde_json::to_string(&decided_gateway.routing_approach)
                    .unwrap_or_else(|_| format!("{:?}", decided_gateway.routing_approach))
                    .trim_matches('"')
                    .to_string();

                crate::analytics::record_decision_event(
                    Some(payload.merchant_id.clone()),
                    Some(routing_approach),
                    Some(decided_gateway.decided_gateway.clone()),
                    Some("success".to_string()),
                    "decide_gateway",
                    decided_gateway.priority_logic_tag.clone(),
                    serde_json::to_string(&serde_json::json!({
                        "request": &payload,
                        "response": &decided_gateway,
                        "score_context": decided_gateway.gateway_priority_map.clone(),
                        "selection_reason": {
                            "decided_gateway": decided_gateway.decided_gateway.clone(),
                            "routing_approach": decided_gateway.routing_approach.clone(),
                            "gateway_before_evaluation": decided_gateway.gateway_before_evaluation.clone(),
                            "priority_logic_tag": decided_gateway.priority_logic_tag.clone(),
                            "reset_approach": decided_gateway.reset_approach.clone(),
                        }
                    }))
                    .ok(),
                    Some(payload.payment_id().to_string()),
                    Some(x_request_id.clone()),
                    Some("gateway_decided".to_string()),
                    Some(payload.payment_method_type().to_string()),
                    Some(payload.payment_method().to_string()),
                );
                metrics::API_REQUEST_COUNTER
                    .with_label_values(&["decide_gateway", "success"])
                    .inc();
                Ok(decided_gateway)
            }
            Err(e) => {
                logger::debug!(tag = "DecideGateway", "Error: {:?}", e);
                crate::analytics::record_error_event(
                    "decide_gateway",
                    Some(payload.merchant_id.clone()),
                    Some(payload.payment_id().to_string()),
                    Some(x_request_id.clone()),
                    None,
                    e.routing_approach.clone().map(|approach| {
                        serde_json::to_string(&approach)
                            .unwrap_or_else(|_| format!("{:?}", approach))
                            .trim_matches('"')
                            .to_string()
                    }),
                    e.error_code.clone(),
                    e.error_message.clone(),
                    serde_json::to_string(&serde_json::json!({
                        "request_id": x_request_id,
                        "request": &payload,
                        "routing_approach": e.routing_approach.clone(),
                        "response": {
                            "status": e.status.clone(),
                            "error_code": e.error_code.clone(),
                            "error_message": e.error_message.clone(),
                            "priority_logic_tag": e.priority_logic_tag.clone(),
                            "routing_approach": e.routing_approach.clone(),
                            "filter_wise_gateways": e.filter_wise_gateways.clone(),
                            "priority_logic_output": e.priority_logic_output.clone(),
                            "is_dynamic_mga_enabled": e.is_dynamic_mga_enabled,
                            "error_info": {
                                "code": e.error_info.code.clone(),
                                "user_message": e.error_info.user_message.clone(),
                                "developer_message": e.error_info.developer_message.clone(),
                            }
                        }
                    }))
                    .ok(),
                    Some("request_failed".to_string()),
                );
                metrics::API_REQUEST_COUNTER
                    .with_label_values(&["decide_gateway", "failure"])
                    .inc();
                Err(e)
            }
        },
        Err(e) => {
            logger::debug!(tag = "DecideGateway", "Error: {:?}", e);
            crate::analytics::record_error_event(
                "decide_gateway",
                None,
                None,
                Some(x_request_id.clone()),
                None,
                None,
                "400".to_string(),
                "Error parsing request".to_string(),
                serde_json::to_string(&serde_json::json!({
                    "request_id": x_request_id,
                    "raw_request": String::from_utf8_lossy(&body).to_string(),
                    "response": {
                        "status": "400",
                        "error_code": "400",
                        "error_message": "Error parsing request",
                        "error_info": {
                            "code": "INVALID_INPUT",
                            "user_message": "Invalid request params. Please verify your input.",
                            "developer_message": e.to_string(),
                        }
                    }
                }))
                .ok(),
                Some("request_parse_failed".to_string()),
            );
            metrics::API_REQUEST_COUNTER
                .with_label_values(&["decide_gateway", "failure"])
                .inc();
            Err(ErrorResponse {
                status: "400".to_string(),
                error_code: "400".to_string(),
                error_message: "Error parsing request".to_string(),
                priority_logic_tag: None,
                routing_approach: None,
                filter_wise_gateways: None,
                error_info: UnifiedError {
                    code: "INVALID_INPUT".to_string(),
                    user_message: "Invalid request params. Please verify your input.".to_string(),
                    developer_message: e.to_string(),
                },
                priority_logic_output: None,
                is_dynamic_mga_enabled: false,
            })
        }
    };
    timer.observe_duration();

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
