use crate::custom_extractors::{Json, RequestIds, ScopedJson, ScopedJsonRejection};
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
use crate::feedback::gateway_scoring_service::{
    check_and_update_gateway_score_, SCORE_UPDATE_SKIPPED,
};
use crate::feedback::smart_retry::RetryConnectors;
use crate::feedback::types::{UpdateScorePayload, UpdateScoreResponse};
use crate::gsm::{GatewayIdentifier, Gsm};
use once_cell::sync::Lazy;
use serde::Serialize;
use tokio_util::task::TaskTracker;

// Tracks in-flight async score update tasks. Checked during graceful shutdown
// to ensure no feedback writes are dropped when the pod receives SIGTERM.
pub static FEEDBACK_TASK_TRACKER: Lazy<TaskTracker> = Lazy::new(TaskTracker::new);

#[derive(Debug, Serialize)]
struct UpdateGatewayScoreRequestDetail<'a> {
    merchant_id: &'a str,
    gateway: &'a str,
    payment_id: &'a str,
    status: &'a str,
    gateway_reference_id: Option<&'a str>,
    enforce_dynamic_routing_failure: Option<bool>,
    txn_latency: Option<&'a crate::types::txn_details::types::TransactionLatency>,
    error_info: Option<&'a crate::feedback::types::GsmErrorInfo>,
    is_smart_retry: Option<bool>,
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
struct UpdateGatewayScoreSkippedDetail<'a> {
    request: UpdateGatewayScoreRequestDetail<'a>,
    selection_reason: UpdateGatewayScoreSelectionReason<'a>,
}

#[derive(Debug, Serialize)]
struct UpdateGatewayScoreFailureDetail<'a> {
    payment_id: &'a str,
    request_id: Option<&'a str>,
    /// The reported transaction status, so the payment's outcome still reaches the A/B
    /// experiment results when the score update itself failed.
    status: &'a str,
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
    headers: axum::http::HeaderMap,
    request: Result<ScopedJson<UpdateScorePayload>, ScopedJsonRejection>,
) -> Result<Json<UpdateScoreResponse>, ErrorResponse> {
    let RequestIds {
        request_id: x_request_id,
        global_request_id,
        trace_id,
    } = RequestIds::from_headers(&headers);

    // Mask values by default and consolidate header diagnostics into one event.
    crate::logger::debug!(
        tag = "UpdateGatewayScore",
        headers = ?headers
            .keys()
            .map(|name| (name.as_str(), "**MASKED**"))
            .collect::<std::collections::BTreeMap<_, _>>(),
        "Incoming request headers"
    );
    let update_score_request = match request {
        Ok(ScopedJson { payload, body, .. }) => {
            crate::logger::debug!(tag = "UpdateGatewayScore", "Body: {:?}", body);
            Ok(payload)
        }
        Err(ScopedJsonRejection::Parse { body, error }) => {
            crate::logger::debug!(tag = "UpdateGatewayScore", "Body: {:?}", body);
            Err(error)
        }
        Err(ScopedJsonRejection::Scope(violation)) => {
            return Err(violation.into());
        }
        Err(ScopedJsonRejection::Read(e)) => {
            let (error_code, error_message) = e.analytics_code_and_message();
            crate::analytics::DomainAnalyticsEvent {
                global_request_id: global_request_id.clone(),
                trace_id: trace_id.clone(),
                details: Some("request body parse failure".to_string()),
                event_stage: Some(e.analytics_stage().to_string()),
                ..crate::analytics::DomainAnalyticsEvent::error(
                    crate::analytics::AnalyticsFlowContext::new(
                        crate::analytics::ApiFlow::DynamicRouting,
                        crate::analytics::FlowType::UpdateGatewayScoreError,
                    ),
                    crate::analytics::AnalyticsRoute::UpdateGatewayScore,
                    error_code,
                    error_message.to_string(),
                )
            }
            .emit();
            return Err(e.into_error_response());
        }
    };

    match update_score_request {
        Ok(payload) => {
            let merchant_id = payload.merchant_id.clone();
            let gateway = payload.gateway.clone();
            let payment_id = payload.payment_id.clone();
            let span = tracing::Span::current();
            if !merchant_id.is_empty() {
                span.record("merchant_id", merchant_id.as_str());
            }
            if !payment_id.is_empty() {
                span.record("payment_id", payment_id.as_str());
            }
            crate::analytics::DomainAnalyticsEvent {
                merchant_id: Some(merchant_id.clone()),
                payment_id: Some(payment_id.clone()),
                request_id: x_request_id.clone(),
                global_request_id: global_request_id.clone(),
                trace_id: trace_id.clone(),
                ..crate::analytics::DomainAnalyticsEvent::request_hit(
                    crate::analytics::AnalyticsFlowContext::new(
                        crate::analytics::ApiFlow::DynamicRouting,
                        crate::analytics::FlowType::UpdateGatewayScoreRequestHit,
                    ),
                    crate::analytics::AnalyticsRoute::UpdateGatewayScore,
                )
            }
            .emit();
            // GSM (pod rules or a Hyperswitch call) is looked up before responding, only with errorInfo.
            let mut gsm_info = match payload.error_info.as_ref() {
                Some(error_info) => {
                    Gsm::source()
                        .find_rule(payload.gateway.connector_name(), error_info)
                        .await
                }
                None => None,
            };
            // A retryable failure gets the connectors to retry on, so the caller need not ask again.
            if let Some(info) = gsm_info.as_mut() {
                if info.decision == crate::gsm::GsmDecision::Retry.to_string() {
                    info.retry_connectors = Some(RetryConnectors::new(&payload).find().await);
                }
            }
            // Marks a response enriched with a GSM rule, like `binEnrichmentFromCardInfo` for card info.
            if gsm_info.is_some() {
                crate::logger::debug!(
                    action = "ruleEnrichmentFromGsm",
                    tag = "ruleEnrichmentFromGsm",
                    "Enriched gsm_info from GSM rule"
                );
            }

            let analytics_gsm_info = gsm_info.clone();
            let response = UpdateScoreResponse {
                message: "Gateway score updated successfully".to_string(),
                merchant_id: merchant_id.clone(),
                gateway: gateway.clone(),
                payment_id: payment_id.clone(),
                gsm_info,
            };

            // Spawn the Redis score update so the caller is not blocked waiting for it.
            // Analytics and outcome metrics are recorded inside the task.
            let spawn_payment_id = payment_id.clone();
            let spawn_merchant_id = merchant_id.clone();
            let handle = FEEDBACK_TASK_TRACKER.spawn(async move {
                let transaction_status = serde_json::to_string(&payload.status)
                    .unwrap_or_else(|_| format!("{:?}", payload.status))
                    .trim_matches('"')
                    .to_string();

                match check_and_update_gateway_score_(payload.clone()).await {
                    Ok(outcome) => {
                        // A payment without a scoring context had no score to update.
                        if outcome != SCORE_UPDATE_SKIPPED {
                            crate::analytics::DomainAnalyticsEvent {
                                merchant_id: Some(merchant_id.clone()),
                                gateway: Some(gateway.clone()),
                                status: Some(transaction_status.clone()),
                                details: crate::analytics::serialize_details(
                                    &UpdateGatewayScoreSuccessDetail {
                                        request: UpdateGatewayScoreRequestDetail {
                                            merchant_id: &merchant_id,
                                            gateway: &gateway,
                                            payment_id: &payment_id,
                                            status: &transaction_status,
                                            gateway_reference_id: payload
                                                .gateway_reference_id
                                                .as_deref(),
                                            enforce_dynamic_routing_failure: payload
                                                .enforce_dynamic_routing_failure,
                                            txn_latency: payload.txn_latency.as_ref(),
                                            error_info: payload.error_info.as_ref(),
                                            is_smart_retry: payload.is_smart_retry,
                                        },
                                        response: &UpdateScoreResponse {
                                            message: "Gateway score updated successfully"
                                                .to_string(),
                                            merchant_id: merchant_id.clone(),
                                            gateway: gateway.clone(),
                                            payment_id: payment_id.clone(),
                                            gsm_info: analytics_gsm_info.clone(),
                                        },
                                        selection_reason: UpdateGatewayScoreSelectionReason {
                                            transaction_status: &transaction_status,
                                            stage: "gateway score updated",
                                        },
                                    },
                                ),
                                payment_id: Some(payment_id.clone()),
                                request_id: x_request_id.clone(),
                                global_request_id: global_request_id.clone(),
                                trace_id: trace_id.clone(),
                                event_stage: Some("score_updated".to_string()),
                                ..crate::analytics::DomainAnalyticsEvent::new(
                                    crate::analytics::AnalyticsFlowContext::new(
                                        crate::analytics::ApiFlow::DynamicRouting,
                                        crate::analytics::FlowType::UpdateGatewayScoreUpdate,
                                    ),
                                    crate::analytics::AnalyticsRoute::UpdateGatewayScore,
                                )
                            }
                            .emit();
                        } else {
                            crate::analytics::DomainAnalyticsEvent {
                                merchant_id: Some(merchant_id.clone()),
                                gateway: Some(gateway.clone()),
                                status: Some(transaction_status.clone()),
                                details: crate::analytics::serialize_details(
                                    &UpdateGatewayScoreSkippedDetail {
                                        request: UpdateGatewayScoreRequestDetail {
                                            merchant_id: &merchant_id,
                                            gateway: &gateway,
                                            payment_id: &payment_id,
                                            status: &transaction_status,
                                            gateway_reference_id: payload
                                                .gateway_reference_id
                                                .as_deref(),
                                            enforce_dynamic_routing_failure: payload
                                                .enforce_dynamic_routing_failure,
                                            txn_latency: payload.txn_latency.as_ref(),
                                            error_info: payload.error_info.as_ref(),
                                            is_smart_retry: payload.is_smart_retry,
                                        },
                                        selection_reason: UpdateGatewayScoreSelectionReason {
                                            transaction_status: &transaction_status,
                                            stage: "no scoring context, nothing to update",
                                        },
                                    },
                                ),
                                payment_id: Some(payment_id.clone()),
                                request_id: x_request_id.clone(),
                                global_request_id: global_request_id.clone(),
                                trace_id: trace_id.clone(),
                                event_stage: Some("score_skipped".to_string()),
                                ..crate::analytics::DomainAnalyticsEvent::new(
                                    crate::analytics::AnalyticsFlowContext::new(
                                        crate::analytics::ApiFlow::DynamicRouting,
                                        crate::analytics::FlowType::UpdateGatewayScoreSkipped,
                                    ),
                                    crate::analytics::AnalyticsRoute::UpdateGatewayScore,
                                )
                            }
                            .emit();
                        }
                    }
                    Err(e) => {
                        crate::analytics::DomainAnalyticsEvent {
                            merchant_id: Some(merchant_id.clone()),
                            payment_id: Some(payment_id.clone()),
                            request_id: x_request_id,
                            global_request_id,
                            trace_id,
                            gateway: Some(gateway.clone()),
                            details: crate::analytics::serialize_details(
                                &UpdateGatewayScoreFailureDetail {
                                    payment_id: &payment_id,
                                    request_id: None,
                                    status: &transaction_status,
                                },
                            ),
                            event_stage: Some("score_update_failed".to_string()),
                            ..crate::analytics::DomainAnalyticsEvent::error(
                                crate::analytics::AnalyticsFlowContext::new(
                                    crate::analytics::ApiFlow::DynamicRouting,
                                    crate::analytics::FlowType::UpdateGatewayScoreError,
                                ),
                                crate::analytics::AnalyticsRoute::UpdateGatewayScore,
                                e.error_code.clone(),
                                e.error_message.clone(),
                            )
                        }
                        .emit();
                        crate::logger::error!(
                            merchant_id = %merchant_id,
                            gateway = %gateway,
                            payment_id = %payment_id,
                            error = ?e,
                            "Async gateway score update failed"
                        );
                    }
                }
            });

            // Log if the spawned task panicked — otherwise panics are silently swallowed.
            tokio::spawn(async move {
                if let Err(e) = handle.await {
                    if e.is_panic() {
                        crate::logger::error!(
                            merchant_id = %spawn_merchant_id,
                            payment_id = %spawn_payment_id,
                            "Async gateway score update task panicked"
                        );
                    }
                }
            });

            Ok(Json(response))
        }
        Err(e) => {
            let error_response = request_parse_error_response(&e);
            crate::analytics::DomainAnalyticsEvent {
                global_request_id,
                trace_id,
                details: Some("request body parse failure".to_string()),
                event_stage: Some("request_parse_failed".to_string()),
                ..crate::analytics::DomainAnalyticsEvent::error(
                    crate::analytics::AnalyticsFlowContext::new(
                        crate::analytics::ApiFlow::DynamicRouting,
                        crate::analytics::FlowType::UpdateGatewayScoreError,
                    ),
                    crate::analytics::AnalyticsRoute::UpdateGatewayScore,
                    "400".to_string(),
                    "Error parsing request".to_string(),
                )
            }
            .emit();
            Err(error_response)
        }
    }
}
