use crate::analytics::events::DomainAnalyticsEvent;
use crate::analytics::flow::{AnalyticsFlowContext, AnalyticsRoute};
use crate::analytics::models::*;
use crate::error;
use crate::metrics::{ANALYTICS_EVENT_COUNTER, ROUTING_DECISION_COUNTER, ROUTING_RULE_HIT_COUNTER};
use axum::http::HeaderMap;
use serde::Serialize;
use time::OffsetDateTime;

pub fn now_ms() -> i64 {
    (OffsetDateTime::now_utc()
        .unix_timestamp_nanos()
        .div_euclid(1_000_000)) as i64
}

pub fn serialize_details<T: Serialize>(details: &T) -> Option<String> {
    serde_json::to_string(details).ok()
}

fn header_string(headers: &HeaderMap, name: &'static str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub fn normalize_trace_id(value: &str) -> Option<String> {
    let mut parts = value.split('-');
    let _version = parts.next()?;
    let trace_id = parts.next()?;
    let _parent_id = parts.next()?;
    let _flags = parts.next()?;

    if trace_id.len() == 32 && trace_id.chars().all(|ch| ch.is_ascii_hexdigit()) {
        Some(trace_id.to_string())
    } else {
        None
    }
}

pub fn global_request_id_from_headers(headers: &HeaderMap) -> Option<String> {
    header_string(headers, crate::storage::consts::X_GLOBAL_REQUEST_ID)
}

pub fn trace_id_from_headers(headers: &HeaderMap) -> Option<String> {
    header_string(headers, crate::storage::consts::TRACEPARENT)
        .and_then(|value| normalize_trace_id(&value))
        .or_else(|| header_string(headers, crate::storage::consts::X_TRACE_ID))
        .or_else(|| header_string(headers, crate::storage::consts::X_B3_TRACE_ID))
}

fn enqueue_domain_event(event: DomainAnalyticsEvent) {
    let event = truncate_domain_event_details(event);
    ANALYTICS_EVENT_COUNTER
        .with_label_values(&[event.flow_type.as_str()])
        .inc();

    if let Some(global_state) = crate::app::APP_STATE.get() {
        global_state.analytics_runtime.enqueue_domain_event(event);
    }
}

fn truncate_domain_event_details(mut event: DomainAnalyticsEvent) -> DomainAnalyticsEvent {
    let Some(details) = event.details.take() else {
        return event;
    };

    let max_bytes = crate::app::APP_STATE
        .get()
        .map(|state| state.analytics_runtime.details_max_bytes())
        .unwrap_or_else(|| crate::config::AnalyticsCaptureConfig::default().details_max_bytes);

    if details.len() <= max_bytes {
        event.details = Some(details);
        return event;
    }

    let mut truncated = details;
    truncated.truncate(max_bytes);
    event.details = Some(truncated);
    event
}

pub fn record_decision_event(
    flow: AnalyticsFlowContext,
    merchant_id: Option<String>,
    routing_approach: Option<String>,
    gateway: Option<String>,
    status: Option<String>,
    route: AnalyticsRoute,
    rule_name: Option<String>,
    details: Option<String>,
    payment_id: Option<String>,
    request_id: Option<String>,
    global_request_id: Option<String>,
    trace_id: Option<String>,
    event_stage: Option<String>,
    payment_method_type: Option<String>,
    payment_method: Option<String>,
    auth_type: Option<String>,
) {
    let approach = routing_approach
        .clone()
        .unwrap_or_else(|| "UNKNOWN".to_string());
    let status_label = status.clone().unwrap_or_else(|| "success".to_string());
    let created_at_ms = now_ms();
    ROUTING_DECISION_COUNTER
        .with_label_values(&[approach.as_str(), status_label.as_str()])
        .inc();
    enqueue_domain_event(DomainAnalyticsEvent::decision(
        flow,
        route,
        merchant_id,
        routing_approach,
        gateway,
        status,
        rule_name,
        details,
        payment_id,
        request_id,
        global_request_id,
        trace_id,
        event_stage,
        payment_method_type,
        payment_method,
        auth_type,
        created_at_ms,
    ));
}

pub fn record_score_snapshot_event(
    flow: AnalyticsFlowContext,
    merchant_id: Option<String>,
    payment_method_type: Option<String>,
    payment_method: Option<String>,
    card_network: Option<String>,
    card_is_in: Option<String>,
    currency: Option<String>,
    country: Option<String>,
    auth_type: Option<String>,
    gateway: Option<String>,
    score_value: Option<f64>,
    sigma_factor: Option<f64>,
    average_latency: Option<f64>,
    tp99_latency: Option<f64>,
    transaction_count: Option<i64>,
    route: AnalyticsRoute,
    details: Option<String>,
    payment_id: Option<String>,
    request_id: Option<String>,
    global_request_id: Option<String>,
    trace_id: Option<String>,
    event_stage: Option<String>,
) {
    enqueue_domain_event(DomainAnalyticsEvent::score_snapshot(
        flow,
        route,
        merchant_id,
        payment_method_type,
        payment_method,
        card_network,
        card_is_in,
        currency,
        country,
        auth_type,
        gateway,
        score_value,
        sigma_factor,
        average_latency,
        tp99_latency,
        transaction_count,
        details,
        payment_id,
        request_id,
        global_request_id,
        trace_id,
        event_stage,
        now_ms(),
    ));
}

pub fn record_gateway_update_event(
    flow: AnalyticsFlowContext,
    merchant_id: Option<String>,
    gateway: Option<String>,
    status: Option<String>,
    route: AnalyticsRoute,
    details: Option<String>,
    payment_id: Option<String>,
    request_id: Option<String>,
    global_request_id: Option<String>,
    trace_id: Option<String>,
    event_stage: Option<String>,
) {
    enqueue_domain_event(DomainAnalyticsEvent::gateway_update(
        flow,
        route,
        merchant_id,
        gateway,
        status,
        details,
        payment_id,
        request_id,
        global_request_id,
        trace_id,
        event_stage,
        now_ms(),
    ));
}

pub fn record_rule_hit_event(
    flow: AnalyticsFlowContext,
    route: AnalyticsRoute,
    merchant_id: Option<String>,
    rule_name: String,
    gateway: Option<String>,
    routing_approach: Option<String>,
    details: Option<String>,
    payment_id: Option<String>,
    request_id: Option<String>,
    global_request_id: Option<String>,
    trace_id: Option<String>,
    event_stage: Option<String>,
) {
    ROUTING_RULE_HIT_COUNTER
        .with_label_values(&[rule_name.as_str()])
        .inc();
    enqueue_domain_event(DomainAnalyticsEvent::rule_hit(
        flow,
        route,
        merchant_id,
        rule_name,
        gateway,
        routing_approach,
        details,
        payment_id,
        request_id,
        global_request_id,
        trace_id,
        event_stage,
        now_ms(),
    ));
}

pub fn record_rule_evaluation_preview_event(
    flow: AnalyticsFlowContext,
    merchant_id: Option<String>,
    payment_id: Option<String>,
    gateway: Option<String>,
    rule_name: Option<String>,
    status: Option<String>,
    details: Option<String>,
    request_id: Option<String>,
    global_request_id: Option<String>,
    trace_id: Option<String>,
) {
    enqueue_domain_event(DomainAnalyticsEvent::rule_evaluation_preview(
        flow,
        merchant_id,
        payment_id,
        gateway,
        rule_name,
        status,
        details,
        request_id,
        global_request_id,
        trace_id,
        now_ms(),
    ));
}

pub fn record_error_event(
    flow: AnalyticsFlowContext,
    route: AnalyticsRoute,
    merchant_id: Option<String>,
    payment_id: Option<String>,
    request_id: Option<String>,
    global_request_id: Option<String>,
    trace_id: Option<String>,
    gateway: Option<String>,
    routing_approach: Option<String>,
    error_code: String,
    error_message: String,
    details: Option<String>,
    event_stage: Option<String>,
    auth_type: Option<String>,
) {
    enqueue_domain_event(DomainAnalyticsEvent::error(
        flow,
        route,
        merchant_id,
        payment_id,
        request_id,
        global_request_id,
        trace_id,
        gateway,
        routing_approach,
        error_code,
        error_message,
        details,
        event_stage,
        auth_type,
        now_ms(),
    ));
}

pub fn record_request_hit_event(
    flow: AnalyticsFlowContext,
    route: AnalyticsRoute,
    merchant_id: Option<String>,
    payment_id: Option<String>,
    request_id: Option<String>,
    global_request_id: Option<String>,
    trace_id: Option<String>,
    auth_type: Option<String>,
) {
    enqueue_domain_event(DomainAnalyticsEvent::request_hit(
        flow,
        route,
        merchant_id,
        payment_id,
        request_id,
        global_request_id,
        trace_id,
        auth_type,
        now_ms(),
    ));
}

pub fn record_operation_event(
    flow: AnalyticsFlowContext,
    route: AnalyticsRoute,
    merchant_id: Option<String>,
    payment_id: Option<String>,
    request_id: Option<String>,
    global_request_id: Option<String>,
    trace_id: Option<String>,
    status: Option<String>,
    details: Option<String>,
    event_stage: Option<String>,
) {
    enqueue_domain_event(DomainAnalyticsEvent::operation(
        flow,
        route,
        merchant_id,
        payment_id,
        request_id,
        global_request_id,
        trace_id,
        status,
        details,
        event_stage,
        now_ms(),
    ));
}

pub async fn overview(
    _state: &crate::app::TenantAppState,
    query: &AnalyticsQuery,
) -> Result<AnalyticsOverviewResponse, error::ApiError> {
    let global_state = crate::app::APP_STATE
        .get()
        .ok_or(error::ApiError::DatabaseError)?;
    global_state
        .analytics_runtime
        .read_store()
        .overview(query)
        .await
}

pub async fn gateway_scores(
    _state: &crate::app::TenantAppState,
    query: &AnalyticsQuery,
) -> Result<AnalyticsGatewayScoresResponse, error::ApiError> {
    let global_state = crate::app::APP_STATE
        .get()
        .ok_or(error::ApiError::DatabaseError)?;
    global_state
        .analytics_runtime
        .read_store()
        .gateway_scores(query)
        .await
}

pub async fn decisions(
    _state: &crate::app::TenantAppState,
    query: &AnalyticsQuery,
) -> Result<AnalyticsDecisionResponse, error::ApiError> {
    let global_state = crate::app::APP_STATE
        .get()
        .ok_or(error::ApiError::DatabaseError)?;
    global_state
        .analytics_runtime
        .read_store()
        .decisions(query)
        .await
}

pub async fn routing_stats(
    _state: &crate::app::TenantAppState,
    query: &AnalyticsQuery,
) -> Result<AnalyticsRoutingStatsResponse, error::ApiError> {
    let global_state = crate::app::APP_STATE
        .get()
        .ok_or(error::ApiError::DatabaseError)?;
    global_state
        .analytics_runtime
        .read_store()
        .routing_stats(query)
        .await
}

pub async fn log_summaries(
    _state: &crate::app::TenantAppState,
    query: &AnalyticsQuery,
) -> Result<AnalyticsLogSummariesResponse, error::ApiError> {
    let global_state = crate::app::APP_STATE
        .get()
        .ok_or(error::ApiError::DatabaseError)?;
    global_state
        .analytics_runtime
        .read_store()
        .log_summaries(query)
        .await
}

pub async fn payment_audit(
    _state: &crate::app::TenantAppState,
    query: &PaymentAuditQuery,
) -> Result<PaymentAuditResponse, error::ApiError> {
    let global_state = crate::app::APP_STATE
        .get()
        .ok_or(error::ApiError::DatabaseError)?;
    global_state
        .analytics_runtime
        .read_store()
        .payment_audit(query)
        .await
}

pub async fn preview_trace(
    _state: &crate::app::TenantAppState,
    query: &PaymentAuditQuery,
) -> Result<PaymentAuditResponse, error::ApiError> {
    let global_state = crate::app::APP_STATE
        .get()
        .ok_or(error::ApiError::DatabaseError)?;
    global_state
        .analytics_runtime
        .read_store()
        .preview_trace(query)
        .await
}

pub fn format_range(query: &AnalyticsQuery) -> String {
    if query.start_ms.is_some() && query.end_ms.is_some() {
        return "custom".to_string();
    }

    match query.range {
        AnalyticsRange::M15 => "15m".to_string(),
        AnalyticsRange::H1 => "1h".to_string(),
        AnalyticsRange::H12 => "12h".to_string(),
        AnalyticsRange::D1 => "1d".to_string(),
        AnalyticsRange::W1 => "1w".to_string(),
    }
}
