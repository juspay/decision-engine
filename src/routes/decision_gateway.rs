use crate::custom_extractors::{RequestIds, ScopedJson, ScopedJsonRejection};
use std::time::Instant;

use crate::decider::gatewaydecider::{
    flows::decider_full_payload_hs_function,
    types::{DecidedGateway, DomainDeciderRequest, ErrorResponse, UnifiedError},
};
use crate::logger;
use crate::redis::feature::{
    check_redis_comp_merchant_flag, is_feature_enabled, RedisCompressionConfig,
    RedisCompressionConfigCombined,
};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
        crate::logger::error!(
            error_message = %self.error_message,
            error_code = %self.error_code,
            error_info = %serde_json::json!({
                "code": self.error_info.code,
                "user_message": self.error_info.user_message,
                "developer_message": self.error_info.developer_message,
            }),
            "Routing request failed"
        );
        // `status` carries the HTTP code. A client error (4xx) is answered with that code; anything
        // else is answered 400, which is what every decider error has always been answered with.
        let status = StatusCode::from_bytes(self.status.as_bytes())
            .ok()
            .filter(StatusCode::is_client_error)
            .unwrap_or(StatusCode::BAD_REQUEST);
        let body = serde_json::to_string(&self).unwrap();
        axum::http::Response::builder()
            .status(status)
            .header("Content-Type", "application/json")
            .body(axum::body::Body::from(body))
            .unwrap()
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct DecidedGatewayResponse {
    pub decided_gateway: DecidedGateway,
    pub filter_list: Vec<(String, Vec<String>)>,
    pub latency: Option<u64>,
}

#[axum::debug_handler]
pub async fn decision_gateway(
    headers: axum::http::HeaderMap,
    uri: axum::http::Uri,
    request: Result<ScopedJson<DomainDeciderRequest>, ScopedJsonRejection>,
) -> Result<DecidedGatewayResponse, ErrorResponse>
where
    DecidedGatewayResponse: IntoResponse,
    ErrorResponse: IntoResponse,
{
    let cpu_start = Instant::now();

    let original_url = uri.to_string();
    let ids = RequestIds::from_headers(&headers);
    let x_request_id = ids.request_id.as_deref().unwrap_or("unknown");
    let start_time = std::time::Instant::now();
    let request_time = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|_| "unknown".to_string());
    let query_params = original_url
        .split_once('?')
        .map(|x| x.1)
        .unwrap_or("")
        .to_string();
    // Extract request headers as a JSON string
    // let req_headers = serde_json::to_string(&headers).unwrap_or("{}".to_string());

    let (body, api_decider_request) = match request {
        Ok(ScopedJson { payload, body, .. }) => (body, Ok(payload)),
        Err(ScopedJsonRejection::Parse { body, error }) => (body, Err(error)),
        Err(ScopedJsonRejection::Scope(violation)) => {
            return Err(violation.into());
        }
        Err(ScopedJsonRejection::Read(e)) => {
            let error_response = e.into_error_response();

            let latency = start_time.elapsed().as_millis() as u64;
            let cpu_time = cpu_start.elapsed().as_millis() as u64;

            // Log the error with req_body and req_headers
            logger::error!(
                url = original_url,
                method = "POST",
                error_category = "API_ERROR",
                request_time = request_time,
                query_params = query_params,
                req_headers = format!("{:?}", masking::Secret::<_>::new(&headers)),
                latency = latency.to_string(),
                x_request_id = x_request_id,
                request_cputime = cpu_time.to_string(),
                env = crate::logger::env::app_env(),
                action = "POST",
                error_code = error_response.error_code.clone(),
                error_message = error_response.error_message.clone(),
                developer_message = error_response.error_info.developer_message.clone(),
                user_message = error_response.error_info.user_message.clone(),
                req_body = "",
                // req_headers = req_headers,
                category = "INCOMING_API",
                "Error occurred while parsing request body"
            );

            return Err(error_response);
        }
    };

    match api_decider_request {
        Ok(payload) => {
            let merchant_id = payload.orderReference.merchantId.clone();
            let merchant_id_string =
                crate::types::merchant::id::merchant_id_to_text(merchant_id.clone());

            let redis_comp_config: Option<HashMap<String, RedisCompressionConfig>> =
                check_redis_comp_merchant_flag(merchant_id_string.clone()).await;

            let redis_comp_enabled_de = is_feature_enabled(
                "REDIS_COMPRESSION_ENABLED_MERCHANT_DE".to_string(),
                merchant_id_string.clone(),
                "kv_redis".to_string(),
            )
            .await;

            let redis_comp_config_final = Some(RedisCompressionConfigCombined {
                redisCompressionConfig: redis_comp_config,
                isRedisCompEnabled: redis_comp_enabled_de,
            });

            let merchant_id_txt = crate::types::merchant::id::merchant_id_to_text(merchant_id);
            tracing::Span::current().record("merchant_id", merchant_id_txt.clone());
            jemalloc_ctl::epoch::advance().unwrap();
            let allocated_before = jemalloc_ctl::stats::allocated::read().unwrap_or(0);

            let result =
                decider_full_payload_hs_function(payload.clone(), redis_comp_config_final).await;

            jemalloc_ctl::epoch::advance().unwrap();
            let allocated_after = jemalloc_ctl::stats::allocated::read().unwrap_or(0);
            let bytes_allocated = allocated_after.saturating_sub(allocated_before);

            let final_result = match result {
                Ok((decided_gateway, filter_list)) => {
                    let cpu_time = cpu_start.elapsed().as_millis() as u64;
                    let response = DecidedGatewayResponse {
                        decided_gateway,
                        filter_list,
                        latency: Some(cpu_time),
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
                        env = crate::logger::env::app_env(),
                        action = "POST",
                        req_body = format!("{:?}", payload),
                        req_headers = format!("{:?}", masking::Secret::<_>::new(&headers)),
                        res_body = res_body,
                        res_code = 200,
                        // res_headers = res_headers,
                        category = "INCOMING_API",
                        "Successfully processed request"
                    );

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
                        env = crate::logger::env::app_env(),
                        action = "POST",
                        error_code = e.error_code,
                        error_message = e.error_message,
                        developer_message = e.error_info.developer_message,
                        user_message = e.error_info.user_message,
                        req_body = String::from_utf8_lossy(&body).to_string(),
                        req_headers = format!("{:?}", masking::Secret::<_>::new(&headers)),
                        category = "INCOMING_API",
                        "Error occurred while processing decider function"
                    );

                    Err(e)
                }
            };

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
                env = crate::logger::env::app_env(),
                action = "POST",
                error_code = error_response.error_code,
                error_message = error_response.error_message,
                developer_message = error_response.error_info.developer_message,
                user_message = error_response.error_info.user_message,
                req_body = String::from_utf8_lossy(&body).to_string(),
                // req_headers = req_headers,
                category = "INCOMING_API",
                req_headers = format!("{:?}", masking::Secret::<_>::new(&headers)),
                "Error occurred while parsing request payload"
            );

            Err(error_response)
        }
    }
}

// SELECT `merchant_iframe_preferences`.`id`, `merchant_iframe_preferences`.`merchant_id`, `merchant_iframe_preferences`.`dynamic_switching_enabled`, `merchant_iframe_preferences`.`isin_routing_enabled`, `merchant_iframe_preferences`.`issuer_routing_enabled`, `merchant_iframe_preferences`.`txn_failure_gateway_penality`, `merchant_iframe_preferences`.`card_brand_routing_enabled` FROM `merchant_iframe_preferences` WHERE (`merchant_iframe_preferences`.`merchant_id` = ?)
