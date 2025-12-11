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

    let headers = req.headers();
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
        Ok(payload) => match decider_full_payload_hs_function(payload, cpu_start).await {
            Ok(decided_gateway) => {
                metrics::API_REQUEST_COUNTER
                    .with_label_values(&["decide_gateway", "success"])
                    .inc();
                Ok(decided_gateway)
            }
            Err(e) => {
                logger::debug!(tag = "DecideGateway", "Error: {:?}", e);
                metrics::API_REQUEST_COUNTER
                    .with_label_values(&["decide_gateway", "failure"])
                    .inc();
                Err(e)
            }
        },
        Err(e) => {
            logger::debug!(tag = "DecideGateway", "Error: {:?}", e);
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
