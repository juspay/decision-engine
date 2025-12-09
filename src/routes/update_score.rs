use crate::decider::gatewaydecider::types::{ErrorResponse, UnifiedError};
use crate::feedback::gateway_scoring_service::{
    check_and_update_gateway_score, invalid_request_error,
};
use crate::logger;
use crate::metrics::{API_LATENCY_HISTOGRAM, API_REQUEST_COUNTER, API_REQUEST_TOTAL_COUNTER};
use crate::redis::feature::{check_redis_comp_merchant_flag, RedisCompressionConfig};
use crate::types::card::txn_card_info::{convert_safe_to_txn_card_info, SafeTxnCardInfo};
use crate::types::txn_details::types::{
    convert_safe_txn_detail_to_txn_detail, SafeTxnDetail, TransactionLatency,
};
use axum::body::to_bytes;
use cpu_time::ProcessTime;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
struct UpdateScoreRequest {
    txn_detail: SafeTxnDetail,
    txn_card_info: SafeTxnCardInfo,
    log_message: String,
    enforce_dynaic_routing_failure: Option<bool>,
    gateway_reference_id: Option<String>,
    txn_latency: Option<TransactionLatency>,
}

#[axum::debug_handler]
pub async fn update_score(
    req: axum::http::Request<axum::body::Body>,
) -> Result<&'static str, ErrorResponse> {
    // Extract headers and URI
    let cpu_start = ProcessTime::now();
    let timer = API_LATENCY_HISTOGRAM
        .with_label_values(&["update_score"])
        .start_timer();
    API_REQUEST_TOTAL_COUNTER
        .with_label_values(&["update_score"])
        .inc();

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
    tracing::Span::current().record("is_audit_trail_log", "true");
    // Buffer the body into memory
    let body_bytes = match to_bytes(req.into_body(), usize::MAX).await {
        Ok(bytes) => bytes,
        Err(e) => {
            API_REQUEST_COUNTER
                .with_label_values(&["update_score", "failure"])
                .inc();
            timer.observe_duration();
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

                API_REQUEST_COUNTER
                    .with_label_values(&["update_score", "failure"])
                    .inc();
                timer.observe_duration();
                return Err(error_response);
            }

            // Process the request
            let txn_detail = match convert_safe_txn_detail_to_txn_detail(payload.txn_detail.clone())
            {
                Ok(detail) => detail,
                Err(e) => {
                    return Err(invalid_request_error("transaction details", &e));
                }
            };
            let txn_card_info = match convert_safe_to_txn_card_info(payload.txn_card_info.clone()) {
                Ok(card_info) => card_info,
                Err(e) => {
                    return Err(invalid_request_error("transaction Card Info", &e));
                }
            };

            let log_message = payload.log_message.clone();
            let enforce_failure = payload.enforce_dynaic_routing_failure.unwrap_or(false);
            let gateway_reference_id = payload.gateway_reference_id.clone();
            let txn_latency = payload.txn_latency.clone();

            jemalloc_ctl::epoch::advance().unwrap();
            let allocated_before = jemalloc_ctl::stats::allocated::read().unwrap_or(0);

            let redis_comp_config: Option<HashMap<String, RedisCompressionConfig>> =
                check_redis_comp_merchant_flag(merchant_id_txt.clone()).await;

            check_and_update_gateway_score(
                txn_detail,
                txn_card_info,
                log_message.as_str(),
                enforce_failure,
                gateway_reference_id,
                txn_latency,
                redis_comp_config,
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
                req_body = format!("{:?}", payload.clone()),
                category = "INCOMING_API",
                req_headers = format!("{:?}", headers),
                "Successfully updated score"
            );

            API_REQUEST_COUNTER
                .with_label_values(&["update_score", "success"])
                .inc();
            timer.observe_duration();
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

            API_REQUEST_COUNTER
                .with_label_values(&["update_score", "failure"])
                .inc();
            timer.observe_duration();
            return Err(error_response);
        }
    }
}
