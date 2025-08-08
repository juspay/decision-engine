use crate::decider::gatewaydecider::{
    flows::deciderFullPayloadHSFunction,
    types::{DecidedGateway, DomainDeciderRequest, ErrorResponse, UnifiedError},
};
use crate::metrics::{API_LATENCY_HISTOGRAM, API_REQUEST_COUNTER, API_REQUEST_TOTAL_COUNTER};
use crate::{logger, metrics};
use axum::body::to_bytes;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use cpu_time::ProcessTime;
use serde::{Deserialize, Serialize};

impl IntoResponse for DecidedGatewayResponse {
    fn into_response(self) -> axum::http::Response<axum::body::Body> {
        let body = serde_json::to_string(&self).unwrap();
        axum::http::Response::builder()
            .status(StatusCode::OK)
            .header("Content-Type", "application/json")
            .body(axum::body::Body::from(body))
            .unwrap()
    }
}

impl IntoResponse for ErrorResponse {
    fn into_response(self) -> axum::http::Response<axum::body::Body> {
        let body = serde_json::to_string(&self).unwrap();
        axum::http::Response::builder()
            .status(StatusCode::BAD_REQUEST)
            .header("Content-Type", "application/json")
            .body(axum::body::Body::from(body))
            .unwrap()
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct DecidedGatewayResponse {
    pub decided_gateway: DecidedGateway,
    pub filter_list: Vec<(String, Vec<String>)>,
}

#[axum::debug_handler]
pub async fn decision_gateway(
    req: axum::http::Request<axum::body::Body>,
) -> Result<DecidedGatewayResponse, ErrorResponse>
where
    DecidedGatewayResponse: IntoResponse,
    ErrorResponse: IntoResponse,
{
    let cpu_start = ProcessTime::now();
    let timer = API_LATENCY_HISTOGRAM
        .with_label_values(&["decision_gateway"])
        .start_timer();
    API_REQUEST_TOTAL_COUNTER
        .with_label_values(&["decision_gateway"])
        .inc();

    // Clone the headers and URI before consuming `req`
    let headers = req.headers().clone();
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
    // Extract request headers as a JSON string
    // let req_headers = serde_json::to_string(&headers).unwrap_or("{}".to_string());

    // Now consume `req` to get the body
    let body = match to_bytes(req.into_body(), usize::MAX).await {
        Ok(body) => body,
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

            // Log the error with req_body and req_headers
            logger::error!(
                url = original_url,
                method = "POST",
                error_category = "API_ERROR",
                request_time = request_time,
                query_params = query_params,
                req_headers = format!("{:?}", headers),
                latency = latency.to_string(),
                x_request_id = x_request_id,
                request_cputime = cpu_time.to_string(),
                env = std::env::var("APP_ENV").unwrap_or_else(|_| "development".to_string()),
                action = "POST",
                error_code = error_response.error_code,
                error_message = error_response.error_message,
                developer_message = error_response.error_info.developer_message,
                user_message = error_response.error_info.user_message,
                req_body = "",
                // req_headers = req_headers,
                category = "INCOMING_API",
                "Error occurred while parsing request body"
            );

            API_REQUEST_COUNTER
                .with_label_values(&["decision_gateway", "failure", "400"])
                .inc();
            timer.observe_duration();

            return Err(error_response);
        }
    };

    let api_decider_request: Result<DomainDeciderRequest, _> = serde_json::from_slice(&body);
    match api_decider_request {
        Ok(payload) => {
            let merchant_id = payload.orderReference.merchantId.clone();
            let merchant_id_txt = crate::types::merchant::id::merchant_id_to_text(merchant_id);
            tracing::Span::current().record("merchant_id", merchant_id_txt.clone());
            jemalloc_ctl::epoch::advance().unwrap();
            let allocated_before = jemalloc_ctl::stats::allocated::read().unwrap_or(0);

            let result = deciderFullPayloadHSFunction(payload.clone()).await;

            jemalloc_ctl::epoch::advance().unwrap();
            let allocated_after = jemalloc_ctl::stats::allocated::read().unwrap_or(0);
            let bytes_allocated = allocated_after.saturating_sub(allocated_before);

            let final_result = match result {
                Ok((decided_gateway, filter_list)) => {
                    let response = DecidedGatewayResponse {
                        decided_gateway,
                        filter_list,
                    };

                    // Serialize response body and headers for logging
                    let res_body = serde_json::to_string(&response).unwrap_or("{}".to_string());
                    // let res_headers = r#"{"Content-Type": "application/json"}"#;

                    // Log the successful response
                    let latency = start_time.elapsed().as_millis() as u64;
                    let cpu_time = cpu_start.elapsed().as_millis() as u64;
                    logger::info!(
                        url = original_url,
                        method = "POST",
                        error_category = "NONE",
                        request_time = request_time,
                        query_params = query_params,
                        latency = latency.to_string(),
                        x_request_id = x_request_id,
                        request_cputime = cpu_time.to_string(),
                        bytes_allocated = bytes_allocated.to_string(),
                        env =
                            std::env::var("APP_ENV").unwrap_or_else(|_| "development".to_string()),
                        action = "POST",
                        req_body = String::from_utf8_lossy(&body).to_string(),
                        req_headers = format!("{:?}", headers),
                        res_body = res_body,
                        res_code = 200,
                        // res_headers = res_headers,
                        category = "INCOMING_API",
                        "Successfully processed request"
                    );

                    API_REQUEST_COUNTER
                        .with_label_values(&["decision_gateway", "success", "200"])
                        .inc();
                    Ok(response)
                }
                Err(e) => {
                    let latency = start_time.elapsed().as_millis() as u64;
                    let cpu_time = cpu_start.elapsed().as_millis() as u64;
                    logger::error!(
                        url = original_url,
                        method = "POST",
                        error_category = "API_ERROR",
                        request_time = request_time,
                        query_params = query_params,
                        latency = latency.to_string(),
                        request_cputime = cpu_time.to_string(),
                        bytes_allocated = bytes_allocated.to_string(),
                        x_request_id = x_request_id,
                        env =
                            std::env::var("APP_ENV").unwrap_or_else(|_| "development".to_string()),
                        action = "POST",
                        error_code = e.error_code,
                        error_message = e.error_message,
                        developer_message = e.error_info.developer_message,
                        user_message = e.error_info.user_message,
                        req_body = String::from_utf8_lossy(&body).to_string(),
                        req_headers = format!("{:?}", headers),
                        category = "INCOMING_API",
                        "Error occurred while processing decider function"
                    );
                    let status_code = e.status.clone();
                    API_REQUEST_COUNTER
                        .with_label_values(&["decision_gateway", "failure", &status_code])
                        .inc();
                    Err(e)
                }
            };

            timer.observe_duration();
            final_result
        }
        Err(e) => {
            let error_response = ErrorResponse {
                status: "400".to_string(),
                error_code: "400".to_string(),
                error_message: "Error parsing request payload".to_string(),
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

            // Log the error with req_body and req_headers
            logger::error!(
                url = original_url,
                method = "POST",
                error_category = "API_ERROR",
                request_time = request_time,
                query_params = query_params,
                latency = latency.to_string(),
                x_request_id = x_request_id,
                request_cputime = cpu_time.to_string(),
                env = std::env::var("APP_ENV").unwrap_or_else(|_| "development".to_string()),
                action = "POST",
                error_code = error_response.error_code,
                error_message = error_response.error_message,
                developer_message = error_response.error_info.developer_message,
                user_message = error_response.error_info.user_message,
                req_body = String::from_utf8_lossy(&body).to_string(),
                // req_headers = req_headers,
                category = "INCOMING_API",
                req_headers = format!("{:?}", headers),
                "Error occurred while parsing request payload"
            );

            API_REQUEST_COUNTER
                .with_label_values(&["decision_gateway", "failure", "400"])
                .inc();
            timer.observe_duration();
            Err(error_response)
        }
    }
}

// SELECT `merchant_iframe_preferences`.`id`, `merchant_iframe_preferences`.`merchant_id`, `merchant_iframe_preferences`.`dynamic_switching_enabled`, `merchant_iframe_preferences`.`isin_routing_enabled`, `merchant_iframe_preferences`.`issuer_routing_enabled`, `merchant_iframe_preferences`.`txn_failure_gateway_penality`, `merchant_iframe_preferences`.`card_brand_routing_enabled` FROM `merchant_iframe_preferences` WHERE (`merchant_iframe_preferences`.`merchant_id` = ?)
