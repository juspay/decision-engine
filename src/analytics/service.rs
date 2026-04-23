use crate::analytics::events::DomainAnalyticsEvent;
use crate::analytics::flow::{AnalyticsFlowContext, AnalyticsRoute};
use crate::analytics::models::*;
use crate::error;
use crate::metrics::{ANALYTICS_EVENT_COUNTER, ROUTING_DECISION_COUNTER, ROUTING_RULE_HIT_COUNTER};
use axum::http::HeaderMap;
use time::OffsetDateTime;

pub fn now_ms() -> i64 {
    (OffsetDateTime::now_utc()
        .unix_timestamp_nanos()
        .div_euclid(1_000_000)) as i64
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
    ROUTING_DECISION_COUNTER
        .with_label_values(&[approach.as_str(), status_label.as_str()])
        .inc();
    enqueue_domain_event(DomainAnalyticsEvent {
        event_id: crate::analytics::next_event_id(now_ms()),
        api_flow: flow.api_flow,
        flow_type: flow.flow_type,
        merchant_id,
        payment_id,
        request_id,
        global_request_id,
        trace_id,
        payment_method_type,
        payment_method,
        card_network: None,
        card_is_in: None,
        currency: None,
        country: None,
        auth_type,
        gateway,
        event_stage,
        routing_approach,
        rule_name,
        status,
        error_code: None,
        error_message: None,
        score_value: None,
        sigma_factor: None,
        average_latency: None,
        tp99_latency: None,
        transaction_count: None,
        route: Some(route.as_str().to_string()),
        details,
        created_at_ms: now_ms(),
    });
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
    enqueue_domain_event(DomainAnalyticsEvent {
        event_id: crate::analytics::next_event_id(now_ms()),
        api_flow: flow.api_flow,
        flow_type: flow.flow_type,
        merchant_id,
        payment_id,
        request_id,
        global_request_id,
        trace_id,
        payment_method_type,
        payment_method,
        card_network,
        card_is_in,
        currency,
        country,
        auth_type,
        gateway,
        event_stage,
        routing_approach: None,
        rule_name: None,
        status: Some("snapshot".to_string()),
        error_code: None,
        error_message: None,
        score_value,
        sigma_factor,
        average_latency,
        tp99_latency,
        transaction_count,
        route: Some(route.as_str().to_string()),
        details,
        created_at_ms: now_ms(),
    });
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
    enqueue_domain_event(DomainAnalyticsEvent {
        event_id: crate::analytics::next_event_id(now_ms()),
        api_flow: flow.api_flow,
        flow_type: flow.flow_type,
        merchant_id,
        payment_id,
        request_id,
        global_request_id,
        trace_id,
        payment_method_type: None,
        payment_method: None,
        card_network: None,
        card_is_in: None,
        currency: None,
        country: None,
        auth_type: None,
        gateway,
        event_stage,
        routing_approach: None,
        rule_name: None,
        status,
        error_code: None,
        error_message: None,
        score_value: None,
        sigma_factor: None,
        average_latency: None,
        tp99_latency: None,
        transaction_count: None,
        route: Some(route.as_str().to_string()),
        details,
        created_at_ms: now_ms(),
    });
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
    enqueue_domain_event(DomainAnalyticsEvent {
        event_id: crate::analytics::next_event_id(now_ms()),
        api_flow: flow.api_flow,
        flow_type: flow.flow_type,
        merchant_id,
        payment_id,
        request_id,
        global_request_id,
        trace_id,
        payment_method_type: None,
        payment_method: None,
        card_network: None,
        card_is_in: None,
        currency: None,
        country: None,
        auth_type: None,
        gateway,
        event_stage,
        routing_approach,
        rule_name: Some(rule_name),
        status: Some("hit".to_string()),
        error_code: None,
        error_message: None,
        score_value: None,
        sigma_factor: None,
        average_latency: None,
        tp99_latency: None,
        transaction_count: None,
        route: Some(route.as_str().to_string()),
        details,
        created_at_ms: now_ms(),
    });
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
    enqueue_domain_event(DomainAnalyticsEvent {
        event_id: crate::analytics::next_event_id(now_ms()),
        api_flow: flow.api_flow,
        flow_type: flow.flow_type,
        merchant_id,
        payment_id,
        request_id,
        global_request_id,
        trace_id,
        payment_method_type: None,
        payment_method: None,
        card_network: None,
        card_is_in: None,
        currency: None,
        country: None,
        auth_type: None,
        gateway,
        event_stage: Some("preview_evaluated".to_string()),
        routing_approach: Some("RULE_EVALUATE_PREVIEW".to_string()),
        rule_name,
        status,
        error_code: None,
        error_message: None,
        score_value: None,
        sigma_factor: None,
        average_latency: None,
        tp99_latency: None,
        transaction_count: None,
        route: Some(AnalyticsRoute::RoutingEvaluate.as_str().to_string()),
        details,
        created_at_ms: now_ms(),
    });
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
    enqueue_domain_event(DomainAnalyticsEvent {
        event_id: crate::analytics::next_event_id(now_ms()),
        api_flow: flow.api_flow,
        flow_type: flow.flow_type,
        merchant_id,
        payment_id,
        request_id,
        global_request_id,
        trace_id,
        payment_method_type: None,
        payment_method: None,
        card_network: None,
        card_is_in: None,
        currency: None,
        country: None,
        auth_type,
        gateway,
        event_stage,
        routing_approach,
        rule_name: None,
        status: Some("failure".to_string()),
        error_code: Some(error_code),
        error_message: Some(error_message),
        score_value: None,
        sigma_factor: None,
        average_latency: None,
        tp99_latency: None,
        transaction_count: None,
        route: Some(route.as_str().to_string()),
        details,
        created_at_ms: now_ms(),
    });
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
    enqueue_domain_event(DomainAnalyticsEvent {
        event_id: crate::analytics::next_event_id(now_ms()),
        api_flow: flow.api_flow,
        flow_type: flow.flow_type,
        merchant_id,
        payment_id,
        request_id,
        global_request_id,
        trace_id,
        payment_method_type: None,
        payment_method: None,
        card_network: None,
        card_is_in: None,
        currency: None,
        country: None,
        auth_type,
        gateway: None,
        event_stage: Some("request_received".to_string()),
        routing_approach: None,
        rule_name: None,
        status: Some("received".to_string()),
        error_code: None,
        error_message: None,
        score_value: None,
        sigma_factor: None,
        average_latency: None,
        tp99_latency: None,
        transaction_count: None,
        route: Some(route.as_str().to_string()),
        details: None,
        created_at_ms: now_ms(),
    });
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
    enqueue_domain_event(DomainAnalyticsEvent {
        event_id: crate::analytics::next_event_id(now_ms()),
        api_flow: flow.api_flow,
        flow_type: flow.flow_type,
        merchant_id,
        payment_id,
        request_id,
        global_request_id,
        trace_id,
        payment_method_type: None,
        payment_method: None,
        card_network: None,
        card_is_in: None,
        currency: None,
        country: None,
        auth_type: None,
        gateway: None,
        event_stage,
        routing_approach: None,
        rule_name: None,
        status,
        error_code: None,
        error_message: None,
        score_value: None,
        sigma_factor: None,
        average_latency: None,
        tp99_latency: None,
        transaction_count: None,
        route: Some(route.as_str().to_string()),
        details,
        created_at_ms: now_ms(),
    });
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

fn normalise_gateways(raw: Option<String>) -> Vec<String> {
    raw.into_iter()
        .flat_map(|value| value.split(',').map(str::to_owned).collect::<Vec<_>>())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

fn normalise_payment_audit_route_filter(route: Option<String>) -> Option<String> {
    route.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return None;
        }

        AnalyticsRoute::from_filter_value(trimmed).map(|route| route.as_str().to_string())
    })
}

fn normalise_payment_audit_status_filter(status: Option<String>) -> Option<String> {
    status.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return None;
        }

        Some(match trimmed.to_ascii_lowercase().as_str() {
            "success" => "success".to_string(),
            "failure" => "FAILURE".to_string(),
            _ => trimmed.to_string(),
        })
    })
}

pub fn parse_query(
    merchant_id: Option<String>,
    scope: Option<String>,
    range: Option<String>,
    start_ms: Option<i64>,
    end_ms: Option<i64>,
    page: Option<u32>,
    page_size: Option<u32>,
    payment_method_type: Option<String>,
    payment_method: Option<String>,
    card_network: Option<String>,
    card_is_in: Option<String>,
    currency: Option<String>,
    country: Option<String>,
    auth_type: Option<String>,
    gateways: Option<String>,
) -> AnalyticsQuery {
    let scope = AnalyticsScope::from_query(scope.as_deref());
    let range = AnalyticsRange::from_query(range.as_deref());
    let (start_ms, end_ms) = match (start_ms, end_ms) {
        (Some(start_ms), Some(end_ms)) if start_ms >= 0 && end_ms > start_ms => {
            (Some(start_ms), Some(end_ms))
        }
        _ => (None, None),
    };
    let page = page.unwrap_or(1).max(1) as usize;
    let page_size = page_size.unwrap_or(10).clamp(1, 50) as usize;
    let gateways = normalise_gateways(gateways);
    let payment_method_type = if scope == AnalyticsScope::Current {
        payment_method_type.filter(|value| !value.is_empty())
    } else {
        None
    };
    let payment_method = if scope == AnalyticsScope::Current {
        payment_method.filter(|value| !value.is_empty())
    } else {
        None
    };
    let card_network = if scope == AnalyticsScope::Current {
        card_network.filter(|value| !value.is_empty())
    } else {
        None
    };
    let card_is_in = if scope == AnalyticsScope::Current {
        card_is_in.filter(|value| !value.is_empty())
    } else {
        None
    };
    let currency = if scope == AnalyticsScope::Current {
        currency.filter(|value| !value.is_empty())
    } else {
        None
    };
    let country = if scope == AnalyticsScope::Current {
        country.filter(|value| !value.is_empty())
    } else {
        None
    };
    let auth_type = if scope == AnalyticsScope::Current {
        auth_type.filter(|value| !value.is_empty())
    } else {
        None
    };

    AnalyticsQuery {
        merchant_id,
        scope,
        range,
        start_ms,
        end_ms,
        page,
        page_size,
        payment_method_type,
        payment_method,
        card_network,
        card_is_in,
        currency,
        country,
        auth_type,
        gateways,
    }
}

pub fn parse_payment_audit_query(
    merchant_id: Option<String>,
    scope: Option<String>,
    range: Option<String>,
    start_ms: Option<i64>,
    end_ms: Option<i64>,
    page: Option<u32>,
    page_size: Option<u32>,
    payment_id: Option<String>,
    request_id: Option<String>,
    gateway: Option<String>,
    route: Option<String>,
    status: Option<String>,
    flow_type: Option<String>,
    error_code: Option<String>,
) -> PaymentAuditQuery {
    let scope = AnalyticsScope::from_query(scope.as_deref());
    let range = AnalyticsRange::from_query(range.as_deref());
    let (start_ms, end_ms) = match (start_ms, end_ms) {
        (Some(start_ms), Some(end_ms)) if start_ms >= 0 && end_ms > start_ms => {
            (Some(start_ms), Some(end_ms))
        }
        _ => (None, None),
    };
    let page = page.unwrap_or(1).max(1) as usize;
    let page_size = page_size.unwrap_or(12).clamp(1, 50) as usize;

    PaymentAuditQuery {
        merchant_id,
        scope,
        range,
        start_ms,
        end_ms,
        page,
        page_size,
        payment_id,
        request_id,
        gateway,
        route: normalise_payment_audit_route_filter(route),
        status: normalise_payment_audit_status_filter(status),
        flow_type,
        error_code,
    }
}

pub fn format_range(query: &AnalyticsQuery) -> String {
    if query.start_ms.is_some() && query.end_ms.is_some() {
        return "custom".to_string();
    }

    match query.range {
        AnalyticsRange::M15 => "15m".to_string(),
        AnalyticsRange::H1 => "1h".to_string(),
        AnalyticsRange::H24 => "24h".to_string(),
        AnalyticsRange::D30 => "30d".to_string(),
        AnalyticsRange::M18 => "18mo".to_string(),
    }
}
