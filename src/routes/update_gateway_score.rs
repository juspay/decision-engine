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

use crate::decider::gatewaydecider::types::ErrorResponse;
use crate::feedback::gateway_scoring_service::check_and_update_gateway_score_;
use crate::feedback::types::{UpdateScorePayload, UpdateScoreResponse};
use crate::metrics::API_LATENCY_HISTOGRAM;
use crate::metrics::API_REQUEST_COUNTER;
use crate::metrics::API_REQUEST_TOTAL_COUNTER;
use axum::extract::Json;
use serde::Serialize;

#[derive(Debug, Serialize)]
struct UpdateGatewayScoreRequestDetail<'a> {
    merchant_id: &'a str,
    gateway: &'a str,
    payment_id: &'a str,
    status: &'a str,
    gateway_reference_id: Option<&'a str>,
    enforce_dynamic_routing_failure: Option<bool>,
    txn_latency: Option<&'a crate::types::txn_details::types::TransactionLatency>,
}

#[derive(Debug, Serialize)]
struct UpdateGatewayScoreSelectionReason<'a> {
    transaction_status: &'a str,
    stage: &'static str,
}

#[derive(Debug, Serialize)]
struct UpdateGatewayScoreSuccessDetail<'a> {
    request: UpdateGatewayScoreRequestDetail<'a>,
    response: &'a UpdateScoreResponse,
    selection_reason: UpdateGatewayScoreSelectionReason<'a>,
}

#[derive(Debug, Serialize)]
struct UpdateGatewayScoreFailureDetail<'a> {
    payment_id: &'a str,
    request_id: Option<&'a str>,
}

fn request_parse_error_response(error: impl ToString) -> ErrorResponse {
    ErrorResponse {
        status: "400".to_string(),
        error_code: "400".to_string(),
        error_message: "Error parsing request".to_string(),
        priority_logic_tag: None,
        routing_approach: None,
        filter_wise_gateways: None,
        error_info: crate::decider::gatewaydecider::types::UnifiedError {
            code: "INVALID_INPUT".to_string(),
            user_message: "Invalid request params. Please verify your input.".to_string(),
            developer_message: error.to_string(),
        },
        priority_logic_output: None,
        is_dynamic_mga_enabled: false,
    }
}

#[axum::debug_handler]
pub async fn update_gateway_score(
    req: axum::http::Request<axum::body::Body>,
) -> Result<Json<UpdateScoreResponse>, ErrorResponse> {
    let x_request_id = req
        .headers()
        .get(crate::storage::consts::X_REQUEST_ID)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let global_request_id = crate::analytics::global_request_id_from_headers(req.headers());
    let trace_id = crate::analytics::trace_id_from_headers(req.headers());
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
    let body = match crate::routes::body::read_request_body(req.into_body()).await {
        Ok(body) => {
            crate::logger::debug!(tag = "UpdateGatewayScore", "Body: {:?}", body);

            body
        }
        Err(e) => {
            crate::logger::debug!(tag = "UpdateGatewayScore", "Error: {:?}", e);
            let (error_code, error_message) = e.analytics_code_and_message();
            crate::analytics::DomainAnalyticsEvent::record_error(
                crate::analytics::AnalyticsFlowContext::new(
                    crate::analytics::ApiFlow::DynamicRouting,
                    crate::analytics::FlowType::UpdateGatewayScoreError,
                ),
                crate::analytics::AnalyticsRoute::UpdateGatewayScore,
                None,
                None,
                None,
                global_request_id.clone(),
                trace_id.clone(),
                None,
                None,
                error_code,
                error_message.to_string(),
                Some("request body parse failure".to_string()),
                Some(e.analytics_stage().to_string()),
                None,
            );
            API_REQUEST_COUNTER
                .with_label_values(&["update_gateway_score", "failure"])
                .inc();
            timer.observe_duration();
            return Err(e.into_error_response());
        }
    };

    let update_score_request: Result<UpdateScorePayload, _> = serde_json::from_slice(&body);
    match update_score_request {
        Ok(payload) => {
            let merchant_id = payload.merchant_id.clone();
            let gateway = payload.gateway.clone();
            let payment_id = payload.payment_id.clone();
            crate::analytics::DomainAnalyticsEvent::record_request_hit(
                crate::analytics::AnalyticsFlowContext::new(
                    crate::analytics::ApiFlow::DynamicRouting,
                    crate::analytics::FlowType::UpdateGatewayScoreRequestHit,
                ),
                crate::analytics::AnalyticsRoute::UpdateGatewayScore,
                Some(merchant_id.clone()),
                Some(payment_id.clone()),
                x_request_id.clone(),
                global_request_id.clone(),
                trace_id.clone(),
                None,
            );
            let result = check_and_update_gateway_score_(payload.clone()).await;
            match result {
                Ok(_success) => {
                    let transaction_status = serde_json::to_string(&payload.status)
                        .unwrap_or_else(|_| format!("{:?}", payload.status))
                        .trim_matches('"')
                        .to_string();
                    let response = UpdateScoreResponse {
                        message: "Gateway score updated successfully".to_string(),
                        merchant_id: merchant_id.clone(),
                        gateway: gateway.clone(),
                        payment_id: payment_id.clone(),
                    };
                    crate::analytics::DomainAnalyticsEvent::record_gateway_update(
                        crate::analytics::AnalyticsFlowContext::new(
                            crate::analytics::ApiFlow::DynamicRouting,
                            crate::analytics::FlowType::UpdateGatewayScoreUpdate,
                        ),
                        Some(merchant_id.clone()),
                        Some(gateway.clone()),
                        Some(transaction_status.clone()),
                        crate::analytics::AnalyticsRoute::UpdateGatewayScore,
                        crate::analytics::serialize_details(&UpdateGatewayScoreSuccessDetail {
                            request: UpdateGatewayScoreRequestDetail {
                                merchant_id: &merchant_id,
                                gateway: &gateway,
                                payment_id: &payment_id,
                                status: &transaction_status,
                                gateway_reference_id: payload.gateway_reference_id.as_deref(),
                                enforce_dynamic_routing_failure: payload
                                    .enforce_dynamic_routing_failure,
                                txn_latency: payload.txn_latency.as_ref(),
                            },
                            response: &response,
                            selection_reason: UpdateGatewayScoreSelectionReason {
                                transaction_status: &transaction_status,
                                stage: "gateway score updated",
                            },
                        }),
                        Some(response.payment_id.clone()),
                        x_request_id.clone(),
                        global_request_id.clone(),
                        trace_id.clone(),
                        Some("score_updated".to_string()),
                    );
                    API_REQUEST_COUNTER
                        .with_label_values(&["update_gateway_score", "success"])
                        .inc();
                    timer.observe_duration();
                    Ok(Json(response))
                }
                Err(e) => {
                    API_REQUEST_COUNTER
                        .with_label_values(&["update_gateway_score", "failure"])
                        .inc();
                    crate::analytics::DomainAnalyticsEvent::record_error(
                        crate::analytics::AnalyticsFlowContext::new(
                            crate::analytics::ApiFlow::DynamicRouting,
                            crate::analytics::FlowType::UpdateGatewayScoreError,
                        ),
                        crate::analytics::AnalyticsRoute::UpdateGatewayScore,
                        Some(merchant_id.clone()),
                        Some(payment_id.clone()),
                        x_request_id.clone(),
                        global_request_id.clone(),
                        trace_id.clone(),
                        Some(gateway.clone()),
                        None,
                        e.error_code.clone(),
                        e.error_message.clone(),
                        crate::analytics::serialize_details(&UpdateGatewayScoreFailureDetail {
                            payment_id: &payment_id,
                            request_id: x_request_id.as_deref(),
                        }),
                        Some("score_update_failed".to_string()),
                        None,
                    );
                    timer.observe_duration();
                    println!("Error: {:?}", e);
                    Err(e)
                }
            }
        }
        Err(e) => {
            crate::logger::debug!(tag = "UpdateScoreRequest", "Error: {:?}", e);
            let error_response = request_parse_error_response(&e);
            crate::analytics::DomainAnalyticsEvent::record_error(
                crate::analytics::AnalyticsFlowContext::new(
                    crate::analytics::ApiFlow::DynamicRouting,
                    crate::analytics::FlowType::UpdateGatewayScoreError,
                ),
                crate::analytics::AnalyticsRoute::UpdateGatewayScore,
                None,
                None,
                None,
                global_request_id,
                trace_id,
                None,
                None,
                "400".to_string(),
                "Error parsing request".to_string(),
                Some("request body parse failure".to_string()),
                Some("request_parse_failed".to_string()),
                None,
            );
            API_REQUEST_COUNTER
                .with_label_values(&["update_gateway_score", "failure"])
                .inc();
            timer.observe_duration();
            Err(error_response)
        }
    }
}
