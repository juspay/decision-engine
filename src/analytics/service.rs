use crate::analytics::events::{derive_lookup_key, DomainAnalyticsEvent};
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
    masking::masked_serialize(details)
        .ok()
        .and_then(|masked| serde_json::to_string(&masked).ok())
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
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let event = limit_domain_event_details(event);
        ANALYTICS_EVENT_COUNTER
            .with_label_values(&[event.flow_type.as_str()])
            .inc();

        if let Some(global_state) = crate::app::APP_STATE.get() {
            global_state.analytics_runtime.enqueue_domain_event(event);
        }
    }))
    .is_err()
    {
        crate::logger::warn!("Skipping analytics event after an unexpected capture failure");
    }
}

fn limit_domain_event_details(mut event: DomainAnalyticsEvent) -> DomainAnalyticsEvent {
    let max_bytes = crate::app::APP_STATE
        .get()
        .map(|state| state.analytics_runtime.details_max_bytes())
        .unwrap_or_else(|| crate::config::AnalyticsCaptureConfig::default().details_max_bytes);

    event.details = event.details.filter(|details| details.len() <= max_bytes);
    event
}

impl DomainAnalyticsEvent {
    /// Queues the event for the analytics sink; a no-op when analytics writes are disabled.
    pub fn emit(mut self) {
        if self.lookup_key.is_none() {
            self.lookup_key =
                derive_lookup_key(self.payment_id.as_deref(), self.request_id.as_deref());
        }
        enqueue_domain_event(self);
    }

    /// Counts the decision by routing approach and status, then queues it.
    pub fn emit_decision(self) {
        ROUTING_DECISION_COUNTER
            .with_label_values(&[
                self.routing_approach.as_deref().unwrap_or("UNKNOWN"),
                self.status.as_deref().unwrap_or("success"),
            ])
            .inc();
        self.emit();
    }

    /// Counts the rule hit by rule name, then queues it.
    pub fn emit_rule_hit(self) {
        ROUTING_RULE_HIT_COUNTER
            .with_label_values(&[self.rule_name.as_deref().unwrap_or_default()])
            .inc();
        self.emit();
    }
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

pub async fn cost_savings(
    _state: &crate::app::TenantAppState,
    query: &AnalyticsQuery,
) -> Result<AnalyticsCostSavingsResponse, error::ApiError> {
    let global_state = crate::app::APP_STATE
        .get()
        .ok_or(error::ApiError::DatabaseError)?;
    global_state
        .analytics_runtime
        .read_store()
        .cost_savings(query)
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

pub async fn payment_audit_filter_options(
    _state: &crate::app::TenantAppState,
    query: &PaymentAuditQuery,
) -> Result<PaymentAuditFilterOptions, error::ApiError> {
    let global_state = crate::app::APP_STATE
        .get()
        .ok_or(error::ApiError::DatabaseError)?;
    global_state
        .analytics_runtime
        .read_store()
        .payment_audit_filter_options(query)
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

pub async fn experiment_results(
    _state: &crate::app::TenantAppState,
    query: &ExperimentResultsQuery,
) -> Result<ExperimentResultsResponse, error::ApiError> {
    let global_state = crate::app::APP_STATE
        .get()
        .ok_or(error::ApiError::DatabaseError)?;
    global_state
        .analytics_runtime
        .read_store()
        .experiment_results(query)
        .await
}

pub async fn experiment_transactions(
    _state: &crate::app::TenantAppState,
    query: &ExperimentTransactionsQuery,
) -> Result<ExperimentTransactionsResponse, error::ApiError> {
    let global_state = crate::app::APP_STATE
        .get()
        .ok_or(error::ApiError::DatabaseError)?;
    global_state
        .analytics_runtime
        .read_store()
        .experiment_transactions(query)
        .await
}

pub async fn routing_events(
    _state: &crate::app::TenantAppState,
    query: &RoutingEventsQuery,
) -> Result<RoutingEventsResponse, error::ApiError> {
    let global_state = crate::app::APP_STATE
        .get()
        .ok_or(error::ApiError::DatabaseError)?;
    global_state
        .analytics_runtime
        .read_store()
        .routing_events(query)
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
