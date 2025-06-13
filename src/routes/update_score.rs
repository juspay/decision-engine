use crate::decider::gatewaydecider::types::{ErrorResponse, UnifiedError};
use crate::feedback::gateway_scoring_service::check_and_update_gateway_score;
use crate::logger;
use crate::types::card::txn_card_info::TxnCardInfo;
use crate::types::txn_details::types::TxnDetail;
use axum::body::to_bytes;
use cpu_time::ProcessTime;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, PartialEq)]
struct UpdateScoreRequest {
    txn_detail: TxnDetail,
    txn_card_info: TxnCardInfo,
    log_message: String,
    enforce_dynaic_routing_failure: Option<bool>,
    gateway_reference_id: Option<String>,
}

#[axum::debug_handler]
pub async fn update_score(
    req: axum::http::Request<axum::body::Body>,
) -> Result<&'static str, ErrorResponse> {
    // Extract headers and URI
    let cpu_start = ProcessTime::now();
    let headers = req.headers().clone();
    // let req_headers = serde_json::to_string(&headers).unwrap_or("{}".to_string());
    let original_url = req.uri().to_string();
    let x_request_id = headers
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown");
    let start_time = std::time::Instant::now();
    let request_time = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "unknown".to_string());
    let query_params = original_url.splitn(2, '?').nth(1).unwrap_or("").to_string();

    // Buffer the body into memory
    let body_bytes = match to_bytes(req.into_body(), usize::MAX).await {
        Ok(bytes) => bytes,
        Err(e) => {
            let error_response = ErrorResponse {
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
            };

            // Log the error
            let latency = start_time.elapsed().as_millis() as u64;
            let cpu_time = cpu_start.elapsed().as_millis() as u64;
            logger::error!(
                url = original_url,
                method = "POST",
                request_time = request_time,
                query_params = query_params,
                error_category = "API_ERROR",
                latency = latency.to_string(),
                request_cputime = cpu_time.to_string(),
                x_request_id = x_request_id,
                env = std::env::var("APP_ENV").unwrap_or_else(|_| "development".to_string()),
                action = "POST",
                error_code = error_response.error_code,
                error_message = error_response.error_message,
                developer_message = error_response.error_info.developer_message,
                user_message = error_response.error_info.user_message,
                req_body = "Failed to parse body",
                req_headers = format!("{:?}", headers),
                level = "Error",
                category = "INCOMING_API",
                "Error occurred while parsing request"
            );

            return Err(error_response);
        }
    };

    // Convert the body to a string for logging
    let req_body = String::from_utf8_lossy(&body_bytes).to_string();

    // Deserialize the body into the expected type
    let update_score_request: Result<UpdateScoreRequest, _> = serde_json::from_slice(&body_bytes);
    match update_score_request {
        Ok(payload) => {
            let merchant_id = payload.txn_detail.merchantId.clone();
            let merchant_id_txt = crate::types::merchant::id::merchant_id_to_text(merchant_id);
            tracing::Span::current().record("merchant_id", merchant_id_txt.clone());
            if payload.txn_detail.gateway.is_none() {
                let error_response = ErrorResponse {
                    status: "400".to_string(),
                    error_code: "400".to_string(),
                    error_message: "Gateway is empty".to_string(),
                    priority_logic_tag: None,
                    routing_approach: None,
                    filter_wise_gateways: None,
                    error_info: UnifiedError {
                        code: "GATEWAY_NOT_FOUND".to_string(),
                        user_message: "Request params does not have gateway, please provide the gateway to update score.".to_string(),
                        developer_message: "Gateway field is empty. Not able to update score.".to_string(),
                    },
                    priority_logic_output: None,
                    is_dynamic_mga_enabled: false,
                };

                // Log the error
                let latency = start_time.elapsed().as_millis() as u64;
                let cpu_time = cpu_start.elapsed().as_millis() as u64;
                logger::error!(
                    url = original_url,
                    method = "POST",
                    error_category = "API_ERROR",
                    latency = latency.to_string(),
                    request_cputime = cpu_time.to_string(),
                    request_time = request_time,
                    query_params = query_params,
                    x_request_id = x_request_id,
                    env = std::env::var("APP_ENV").unwrap_or_else(|_| "development".to_string()),
                    action = "POST",
                    error_code = error_response.error_code,
                    error_message = error_response.error_message,
                    developer_message = error_response.error_info.developer_message,
                    user_message = error_response.error_info.user_message,
                    req_body = req_body,
                    req_headers = format!("{:?}", headers),
                    category = "INCOMING_API",
                    "Gateway field is empty"
                );

                return Err(error_response);
            }

            // Process the request
            let txn_detail = payload.txn_detail;
            let txn_card_info = payload.txn_card_info;
            let log_message = payload.log_message;
            let enforce_failure = payload.enforce_dynaic_routing_failure.unwrap_or(false);
            let gateway_reference_id = payload.gateway_reference_id;

            jemalloc_ctl::epoch::advance().unwrap();
            let allocated_before = jemalloc_ctl::stats::allocated::read().unwrap_or(0);

            check_and_update_gateway_score(
                txn_detail,
                txn_card_info,
                log_message.as_str(),
                enforce_failure,
                gateway_reference_id,
            )
            .await;

            jemalloc_ctl::epoch::advance().unwrap();
            let allocated_after = jemalloc_ctl::stats::allocated::read().unwrap_or(0);
            let bytes_allocated = allocated_after.saturating_sub(allocated_before);

            // Log the successful response
            let latency = start_time.elapsed().as_millis() as u64;
            let cpu_time = cpu_start.elapsed().as_millis() as u64;
            logger::info!(
                url = original_url,
                method = "POST",
                query_params = query_params,
                error_category = "NONE",
                latency = latency.to_string(),
                request_cputime = cpu_time.to_string(),
                bytes_allocated = bytes_allocated.to_string(),
                x_request_id = x_request_id,
                request_time = request_time,
                env = std::env::var("APP_ENV").unwrap_or_else(|_| "development".to_string()),
                action = "POST",
                req_body = req_body,
                category = "INCOMING_API",
                req_headers = format!("{:?}", headers),
                "Successfully updated score"
            );

            return Ok("Success");
        }
        Err(e) => {
            let error_response = ErrorResponse {
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
            };

            let latency = start_time.elapsed().as_millis() as u64;
            let cpu_time = cpu_start.elapsed().as_millis() as u64;
            // Log the error
            logger::error!(
                url = original_url,
                method = "POST",
                error_category = "API_ERROR",
                latency = latency.to_string(),
                request_time = request_time,
                request_cputime = cpu_time.to_string(),
                query_params = query_params,
                x_request_id = x_request_id,
                env = std::env::var("APP_ENV").unwrap_or_else(|_| "development".to_string()),
                action = "POST",
                error_code = error_response.error_code,
                error_message = error_response.error_message,
                developer_message = error_response.error_info.developer_message,
                user_message = error_response.error_info.user_message,
                req_body = req_body,
                req_headers = format!("{:?}", headers),
                category = "INCOMING_API",
                "Error occurred while parsing request payload"
            );

            return Err(error_response);
        }
    }
}
