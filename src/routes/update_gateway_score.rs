// use std::sync::Arc;
//
// use axum::{routing::post, Json};
//
// #[cfg(feature = "limit")]
// use axum::{error_handling::HandleErrorLayer, response::IntoResponse};
//
// use crate::{
//     crypto::{
//         hash_manager::managers::sha::Sha512,
//         keymanager::{self, KeyProvider},
//     },
//     custom_extractors::TenantStateResolver,
//     error::{self, ContainerError, ResultContainerExt},
//     logger,
//     storage::{FingerprintInterface, HashInterface, OpenRouterInterface},
//     tenant::GlobalAppState,
//     utils,
// };

use crate::decider::gatewaydecider::types::{ErrorResponse, UnifiedError};
use crate::feedback::gateway_scoring_service::check_and_update_gateway_score_;
use crate::feedback::types::{UpdateScorePayload, UpdateScoreResponse};
use crate::metrics::API_LATENCY_HISTOGRAM;
use crate::metrics::API_REQUEST_COUNTER;
use crate::metrics::API_REQUEST_TOTAL_COUNTER;
use axum::body::to_bytes;
use axum::extract::Json;

#[axum::debug_handler]
pub async fn update_gateway_score(
    req: axum::http::Request<axum::body::Body>,
) -> Result<Json<UpdateScoreResponse>, ErrorResponse> {
    let timer = API_LATENCY_HISTOGRAM
        .with_label_values(&["update_gateway_score"])
        .start_timer();
    API_REQUEST_TOTAL_COUNTER
        .with_label_values(&["update_gateway_score"])
        .inc();

    let headers = req.headers();
    for (name, value) in headers.iter() {
        crate::logger::debug!(tag = "UpdateGatewayScore", "Header: {}: {:?}", name, value);
    }
    let body = match to_bytes(req.into_body(), usize::MAX).await {
        Ok(body) => {
            crate::logger::debug!(tag = "UpdateGatewayScore", "Body: {:?}", body);

            body
        }
        Err(e) => {
            crate::logger::debug!(tag = "UpdateGatewayScore", "Error: {:?}", e);
            API_REQUEST_COUNTER
                .with_label_values(&["update_gateway_score", "failure"])
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

    let update_score_request: Result<UpdateScorePayload, _> = serde_json::from_slice(&body);
    match update_score_request {
        Ok(payload) => {
            let merchant_id = payload.merchantId.clone();
            let gateway = payload.gateway.clone();
            let payment_id = payload.paymentId.clone();
            let result = check_and_update_gateway_score_(payload).await;
            match result {
                Ok(success) => {
                    API_REQUEST_COUNTER
                        .with_label_values(&["update_gateway_score", "success"])
                        .inc();
                    timer.observe_duration();
                    Ok(Json(UpdateScoreResponse {
                        message: "Gateway score updated successfully".to_string(),
                        merchant_id,
                        gateway,
                        payment_id,
                    }))
                }
                Err(e) => {
                    API_REQUEST_COUNTER
                        .with_label_values(&["update_gateway_score", "failure"])
                        .inc();
                    timer.observe_duration();
                    println!("Error: {:?}", e);
                    Err(e)
                }
            }
        }
        Err(e) => {
            crate::logger::debug!(tag = "UpdateScoreRequest", "Error: {:?}", e);
            API_REQUEST_COUNTER
                .with_label_values(&["update_gateway_score", "failure"])
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
    }
}
