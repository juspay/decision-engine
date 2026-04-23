use std::{borrow::Cow, time::Instant};

use crate::{
    analytics::{
        global_request_id_from_headers, serialize_details, trace_id_from_headers,
        AnalyticsFlowContext, AnalyticsRoute, ApiFlow, DomainAnalyticsEvent, FlowType,
    },
    decider::gatewaydecider::{
        flow_new::decider_full_payload_hs_function,
        types::{
            DecidedGateway, DomainDeciderRequestForApiCallV2, ErrorResponse,
            GatewayDeciderApproach, ResetApproach, UnifiedError,
        },
    },
    logger, metrics,
};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use serde::Serialize;

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

#[derive(Debug, Serialize)]
struct DecideGatewayReadFailureDetail<'a> {
    request_id: &'a str,
    response: &'a ErrorResponse,
}

#[derive(Debug, Serialize)]
struct DecideGatewaySelectionReason<'a> {
    decided_gateway: &'a str,
    routing_approach: &'a GatewayDeciderApproach,
    gateway_before_evaluation: Option<&'a str>,
    priority_logic_tag: Option<&'a str>,
    reset_approach: &'a ResetApproach,
}

#[derive(Debug, Serialize)]
struct DecideGatewaySuccessDetail<'a> {
    request: &'a DomainDeciderRequestForApiCallV2,
    response: &'a DecidedGateway,
    score_context: Option<&'a serde_json::Value>,
    selection_reason: DecideGatewaySelectionReason<'a>,
}

#[derive(Debug, Serialize)]
struct DecideGatewayFailureDetail<'a> {
    request_id: &'a str,
    request: &'a DomainDeciderRequestForApiCallV2,
    routing_approach: Option<&'a GatewayDeciderApproach>,
    response: &'a ErrorResponse,
}

#[derive(Debug, Serialize)]
struct DecideGatewayParseFailureDetail<'a> {
    request_id: &'a str,
    raw_request: Cow<'a, str>,
    response: &'a ErrorResponse,
}

fn request_parse_error_response(error: impl ToString) -> ErrorResponse {
    ErrorResponse {
        status: "400".to_string(),
        error_code: "400".to_string(),
        error_message: "Error parsing request".to_string(),
        priority_logic_tag: None,
        routing_approach: None,
        filter_wise_gateways: None,
        error_info: UnifiedError {
            code: "INVALID_INPUT".to_string(),
            user_message: "Invalid request params. Please verify your input.".to_string(),
            developer_message: error.to_string(),
        },
        priority_logic_output: None,
        is_dynamic_mga_enabled: false,
    }
}

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
        .get(crate::storage::consts::X_REQUEST_ID)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("unknown")
        .to_string();
    let global_request_id = global_request_id_from_headers(&headers);
    let trace_id = trace_id_from_headers(&headers);
    for (name, value) in headers.iter() {
        logger::debug!(tag = "DecideGateway", "Header: {}: {:?}", name, value);
    }
    let body = match crate::routes::body::read_request_body(req.into_body()).await {
        Ok(body) => {
            logger::debug!("Body: {:?}", body);
            body
        }
        Err(e) => {
            logger::debug!(tag = "DecideGateway", "Error: {:?}", e);
            let (error_code, error_message) = e.analytics_code_and_message();
            let analytics_stage = e.analytics_stage().to_string();
            let error_response = e.into_error_response();
            DomainAnalyticsEvent::record_error(
                AnalyticsFlowContext::new(ApiFlow::DynamicRouting, FlowType::DecideGatewayError),
                AnalyticsRoute::DecideGateway,
                None,
                None,
                Some(x_request_id.clone()),
                global_request_id.clone(),
                trace_id.clone(),
                None,
                None,
                error_code,
                error_message.to_string(),
                serialize_details(&DecideGatewayReadFailureDetail {
                    request_id: &x_request_id,
                    response: &error_response,
                }),
                Some(analytics_stage),
                None,
            );
            metrics::API_REQUEST_COUNTER
                .with_label_values(&["decide_gateway", "failure"])
                .inc();
            timer.observe_duration();
            return Err(error_response);
        }
    };
    let api_decider_request: Result<DomainDeciderRequestForApiCallV2, _> =
        serde_json::from_slice(&body);
    let result = match api_decider_request {
        Ok(payload) => {
            let auth_type = payload.auth_type();
            DomainAnalyticsEvent::record_request_hit(
                AnalyticsFlowContext::new(
                    ApiFlow::DynamicRouting,
                    FlowType::DecideGatewayRequestHit,
                ),
                AnalyticsRoute::DecideGateway,
                Some(payload.merchant_id.clone()),
                Some(payload.payment_id().to_string()),
                Some(x_request_id.clone()),
                global_request_id.clone(),
                trace_id.clone(),
                auth_type.clone(),
            );
            match decider_full_payload_hs_function(payload.clone(), cpu_start).await {
                Ok(decided_gateway) => {
                    let routing_approach = decided_gateway.routing_approach.to_string();

                    DomainAnalyticsEvent::record_decision(
                        AnalyticsFlowContext::new(
                            ApiFlow::DynamicRouting,
                            FlowType::DecideGatewayDecision,
                        ),
                        Some(payload.merchant_id.clone()),
                        Some(routing_approach),
                        Some(decided_gateway.decided_gateway.clone()),
                        Some("success".to_string()),
                        AnalyticsRoute::DecideGateway,
                        decided_gateway.priority_logic_tag.clone(),
                        serialize_details(&DecideGatewaySuccessDetail {
                            request: &payload,
                            response: &decided_gateway,
                            score_context: decided_gateway.gateway_priority_map.as_ref(),
                            selection_reason: DecideGatewaySelectionReason {
                                decided_gateway: &decided_gateway.decided_gateway,
                                routing_approach: &decided_gateway.routing_approach,
                                gateway_before_evaluation: decided_gateway
                                    .gateway_before_evaluation
                                    .as_deref(),
                                priority_logic_tag: decided_gateway.priority_logic_tag.as_deref(),
                                reset_approach: &decided_gateway.reset_approach,
                            },
                        }),
                        Some(payload.payment_id().to_string()),
                        Some(x_request_id.clone()),
                        global_request_id.clone(),
                        trace_id.clone(),
                        Some("gateway_decided".to_string()),
                        Some(payload.payment_method_type().to_string()),
                        Some(payload.payment_method().to_string()),
                        auth_type.clone(),
                    );
                    metrics::API_REQUEST_COUNTER
                        .with_label_values(&["decide_gateway", "success"])
                        .inc();
                    Ok(decided_gateway)
                }
                Err(e) => {
                    logger::debug!(tag = "DecideGateway", "Error: {:?}", e);
                    DomainAnalyticsEvent::record_error(
                        AnalyticsFlowContext::new(
                            ApiFlow::DynamicRouting,
                            FlowType::DecideGatewayError,
                        ),
                        AnalyticsRoute::DecideGateway,
                        Some(payload.merchant_id.clone()),
                        Some(payload.payment_id().to_string()),
                        Some(x_request_id.clone()),
                        global_request_id.clone(),
                        trace_id.clone(),
                        None,
                        e.routing_approach.as_ref().map(ToString::to_string),
                        e.error_code.clone(),
                        e.error_message.clone(),
                        serialize_details(&DecideGatewayFailureDetail {
                            request_id: &x_request_id,
                            request: &payload,
                            routing_approach: e.routing_approach.as_ref(),
                            response: &e,
                        }),
                        Some("request_failed".to_string()),
                        auth_type.clone(),
                    );
                    metrics::API_REQUEST_COUNTER
                        .with_label_values(&["decide_gateway", "failure"])
                        .inc();
                    Err(e)
                }
            }
        }
        Err(e) => {
            logger::debug!(tag = "DecideGateway", "Error: {:?}", e);
            let error_response = request_parse_error_response(&e);
            DomainAnalyticsEvent::record_error(
                AnalyticsFlowContext::new(ApiFlow::DynamicRouting, FlowType::DecideGatewayError),
                AnalyticsRoute::DecideGateway,
                None,
                None,
                Some(x_request_id.clone()),
                global_request_id,
                trace_id,
                None,
                None,
                "400".to_string(),
                "Error parsing request".to_string(),
                serialize_details(&DecideGatewayParseFailureDetail {
                    request_id: &x_request_id,
                    raw_request: String::from_utf8_lossy(&body),
                    response: &error_response,
                }),
                Some("request_parse_failed".to_string()),
                None,
            );
            metrics::API_REQUEST_COUNTER
                .with_label_values(&["decide_gateway", "failure"])
                .inc();
            Err(error_response)
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
