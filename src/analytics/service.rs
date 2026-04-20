use crate::analytics::events::DomainAnalyticsEvent;
use crate::analytics::models::*;
use crate::error;
use crate::metrics::{ANALYTICS_EVENT_COUNTER, ROUTING_DECISION_COUNTER, ROUTING_RULE_HIT_COUNTER};
use time::OffsetDateTime;

pub fn now_ms() -> i64 {
    (OffsetDateTime::now_utc()
        .unix_timestamp_nanos()
        .div_euclid(1_000_000)) as i64
}

fn event_type_label(kind: &str) -> &'static str {
    match kind {
        "decision" => "decision",
        "gateway_update" => "gateway_update",
        "score_snapshot" => "score_snapshot",
        "rule_hit" => "rule_hit",
        "rule_evaluation_preview" => "rule_evaluation_preview",
        "error" => "error",
        "request_hit" => "request_hit",
        _ => "other",
    }
}

fn enqueue_domain_event(event: DomainAnalyticsEvent) {
    let label = event_type_label(event.event_type.as_str());
    ANALYTICS_EVENT_COUNTER.with_label_values(&[label]).inc();

    if let Some(global_state) = crate::app::APP_STATE.get() {
        global_state.analytics_runtime.enqueue_domain_event(event);
    }
}

pub fn record_decision_event(
    tenant_id: String,
    merchant_id: Option<String>,
    routing_approach: Option<String>,
    gateway: Option<String>,
    status: Option<String>,
    route: &str,
    rule_name: Option<String>,
    details: Option<String>,
    payment_id: Option<String>,
    request_id: Option<String>,
    event_stage: Option<String>,
    payment_method_type: Option<String>,
    payment_method: Option<String>,
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
        tenant_id,
        event_type: "decision".to_string(),
        merchant_id,
        payment_id,
        request_id,
        payment_method_type,
        payment_method,
        card_network: None,
        card_is_in: None,
        currency: None,
        country: None,
        auth_type: None,
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
        route: Some(route.to_string()),
        details,
        created_at_ms: now_ms(),
    });
}

pub fn record_score_snapshot_event(
    tenant_id: String,
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
    route: &str,
    details: Option<String>,
    payment_id: Option<String>,
    request_id: Option<String>,
    event_stage: Option<String>,
) {
    enqueue_domain_event(DomainAnalyticsEvent {
        event_id: crate::analytics::next_event_id(now_ms()),
        tenant_id,
        event_type: "score_snapshot".to_string(),
        merchant_id,
        payment_id,
        request_id,
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
        route: Some(route.to_string()),
        details,
        created_at_ms: now_ms(),
    });
}

pub fn record_gateway_update_event(
    tenant_id: String,
    merchant_id: Option<String>,
    gateway: Option<String>,
    status: Option<String>,
    route: &str,
    details: Option<String>,
    payment_id: Option<String>,
    request_id: Option<String>,
    event_stage: Option<String>,
) {
    enqueue_domain_event(DomainAnalyticsEvent {
        event_id: crate::analytics::next_event_id(now_ms()),
        tenant_id,
        event_type: "gateway_update".to_string(),
        merchant_id,
        payment_id,
        request_id,
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
        route: Some(route.to_string()),
        details,
        created_at_ms: now_ms(),
    });
}

pub fn record_rule_hit_event(
    tenant_id: String,
    merchant_id: Option<String>,
    rule_name: String,
    gateway: Option<String>,
    routing_approach: Option<String>,
    details: Option<String>,
    payment_id: Option<String>,
    request_id: Option<String>,
    event_stage: Option<String>,
) {
    ROUTING_RULE_HIT_COUNTER
        .with_label_values(&[rule_name.as_str()])
        .inc();
    enqueue_domain_event(DomainAnalyticsEvent {
        event_id: crate::analytics::next_event_id(now_ms()),
        tenant_id,
        event_type: "rule_hit".to_string(),
        merchant_id,
        payment_id,
        request_id,
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
        route: Some("routing".to_string()),
        details,
        created_at_ms: now_ms(),
    });
}

pub fn record_rule_evaluation_preview_event(
    tenant_id: String,
    merchant_id: Option<String>,
    payment_id: Option<String>,
    gateway: Option<String>,
    rule_name: Option<String>,
    status: Option<String>,
    details: Option<String>,
) {
    enqueue_domain_event(DomainAnalyticsEvent {
        event_id: crate::analytics::next_event_id(now_ms()),
        tenant_id,
        event_type: "rule_evaluation_preview".to_string(),
        merchant_id,
        payment_id,
        request_id: None,
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
        route: Some("routing_evaluate".to_string()),
        details,
        created_at_ms: now_ms(),
    });
}

pub fn record_error_event(
    tenant_id: String,
    route: &str,
    merchant_id: Option<String>,
    payment_id: Option<String>,
    request_id: Option<String>,
    gateway: Option<String>,
    routing_approach: Option<String>,
    error_code: String,
    error_message: String,
    details: Option<String>,
    event_stage: Option<String>,
) {
    enqueue_domain_event(DomainAnalyticsEvent {
        event_id: crate::analytics::next_event_id(now_ms()),
        tenant_id,
        event_type: "error".to_string(),
        merchant_id,
        payment_id,
        request_id,
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
        rule_name: None,
        status: Some("failure".to_string()),
        error_code: Some(error_code),
        error_message: Some(error_message),
        score_value: None,
        sigma_factor: None,
        average_latency: None,
        tp99_latency: None,
        transaction_count: None,
        route: Some(route.to_string()),
        details,
        created_at_ms: now_ms(),
    });
}

pub fn record_request_hit_event(
    tenant_id: String,
    route: &str,
    merchant_id: Option<String>,
    payment_id: Option<String>,
    request_id: Option<String>,
) {
    enqueue_domain_event(DomainAnalyticsEvent {
        event_id: crate::analytics::next_event_id(now_ms()),
        tenant_id,
        event_type: "request_hit".to_string(),
        merchant_id,
        payment_id,
        request_id,
        payment_method_type: None,
        payment_method: None,
        card_network: None,
        card_is_in: None,
        currency: None,
        country: None,
        auth_type: None,
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
        route: Some(route.to_string()),
        details: None,
        created_at_ms: now_ms(),
    });
}

pub async fn overview(
    state: &crate::app::TenantAppState,
    query: &AnalyticsQuery,
) -> Result<AnalyticsOverviewResponse, error::ApiError> {
    let global_state = crate::app::APP_STATE
        .get()
        .ok_or(error::ApiError::DatabaseError)?;
    global_state
        .analytics_runtime
        .read_store()
        .overview(&state.config.tenant_id, query)
        .await
}

pub async fn gateway_scores(
    state: &crate::app::TenantAppState,
    query: &AnalyticsQuery,
) -> Result<AnalyticsGatewayScoresResponse, error::ApiError> {
    let global_state = crate::app::APP_STATE
        .get()
        .ok_or(error::ApiError::DatabaseError)?;
    global_state
        .analytics_runtime
        .read_store()
        .gateway_scores(&state.config.tenant_id, query)
        .await
}

pub async fn decisions(
    state: &crate::app::TenantAppState,
    query: &AnalyticsQuery,
) -> Result<AnalyticsDecisionResponse, error::ApiError> {
    let global_state = crate::app::APP_STATE
        .get()
        .ok_or(error::ApiError::DatabaseError)?;
    global_state
        .analytics_runtime
        .read_store()
        .decisions(&state.config.tenant_id, query)
        .await
}

pub async fn routing_stats(
    state: &crate::app::TenantAppState,
    query: &AnalyticsQuery,
) -> Result<AnalyticsRoutingStatsResponse, error::ApiError> {
    let global_state = crate::app::APP_STATE
        .get()
        .ok_or(error::ApiError::DatabaseError)?;
    global_state
        .analytics_runtime
        .read_store()
        .routing_stats(&state.config.tenant_id, query)
        .await
}

pub async fn log_summaries(
    state: &crate::app::TenantAppState,
    query: &AnalyticsQuery,
) -> Result<AnalyticsLogSummariesResponse, error::ApiError> {
    let global_state = crate::app::APP_STATE
        .get()
        .ok_or(error::ApiError::DatabaseError)?;
    global_state
        .analytics_runtime
        .read_store()
        .log_summaries(&state.config.tenant_id, query)
        .await
}

pub async fn payment_audit(
    state: &crate::app::TenantAppState,
    query: &PaymentAuditQuery,
) -> Result<PaymentAuditResponse, error::ApiError> {
    let global_state = crate::app::APP_STATE
        .get()
        .ok_or(error::ApiError::DatabaseError)?;
    global_state
        .analytics_runtime
        .read_store()
        .payment_audit(&state.config.tenant_id, query)
        .await
}

pub async fn preview_trace(
    state: &crate::app::TenantAppState,
    query: &PaymentAuditQuery,
) -> Result<PaymentAuditResponse, error::ApiError> {
    let global_state = crate::app::APP_STATE
        .get()
        .ok_or(error::ApiError::DatabaseError)?;
    global_state
        .analytics_runtime
        .read_store()
        .preview_trace(&state.config.tenant_id, query)
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

        Some(match trimmed {
            "Decide Gateway" => "decide_gateway".to_string(),
            "Update Gateway" => "update_gateway_score".to_string(),
            "Rule Evaluate" => "routing_evaluate".to_string(),
            other => other.to_string(),
        })
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
    event_type: Option<String>,
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
        event_type,
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
    }
}
