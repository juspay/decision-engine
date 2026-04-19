use crate::analytics::models::*;
use crate::error;
use crate::metrics::{ANALYTICS_EVENT_COUNTER, ROUTING_DECISION_COUNTER, ROUTING_RULE_HIT_COUNTER};
use async_bb8_diesel::AsyncRunQueryDsl;
use diesel::prelude::*;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use time::OffsetDateTime;

#[cfg(feature = "mysql")]
use crate::storage::schema;
#[cfg(feature = "postgres")]
use crate::storage::schema_pg;

#[cfg(feature = "mysql")]
use crate::storage::schema::analytics_event::dsl as analytics_dsl;
#[cfg(feature = "postgres")]
use crate::storage::schema_pg::analytics_event::dsl as analytics_dsl;

#[derive(Debug, Clone, Insertable)]
#[cfg_attr(feature = "mysql", diesel(table_name = schema::analytics_event))]
#[cfg_attr(feature = "postgres", diesel(table_name = schema_pg::analytics_event))]
pub struct NewAnalyticsEvent {
    pub event_type: String,
    pub merchant_id: Option<String>,
    pub payment_id: Option<String>,
    pub request_id: Option<String>,
    pub payment_method_type: Option<String>,
    pub payment_method: Option<String>,
    pub card_network: Option<String>,
    pub card_is_in: Option<String>,
    pub currency: Option<String>,
    pub country: Option<String>,
    pub auth_type: Option<String>,
    pub gateway: Option<String>,
    pub event_stage: Option<String>,
    pub routing_approach: Option<String>,
    pub rule_name: Option<String>,
    pub status: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub score_value: Option<f64>,
    pub sigma_factor: Option<f64>,
    pub average_latency: Option<f64>,
    pub tp99_latency: Option<f64>,
    pub transaction_count: Option<i64>,
    pub route: Option<String>,
    pub details: Option<String>,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone, Queryable, Selectable)]
#[cfg_attr(feature = "mysql", diesel(table_name = schema::analytics_event))]
#[cfg_attr(feature = "postgres", diesel(table_name = schema_pg::analytics_event))]
pub struct AnalyticsEvent {
    pub id: i32,
    pub event_type: String,
    pub merchant_id: Option<String>,
    pub payment_id: Option<String>,
    pub request_id: Option<String>,
    pub payment_method_type: Option<String>,
    pub payment_method: Option<String>,
    pub card_network: Option<String>,
    pub card_is_in: Option<String>,
    pub currency: Option<String>,
    pub country: Option<String>,
    pub auth_type: Option<String>,
    pub gateway: Option<String>,
    pub event_stage: Option<String>,
    pub routing_approach: Option<String>,
    pub rule_name: Option<String>,
    pub status: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub score_value: Option<f64>,
    pub sigma_factor: Option<f64>,
    pub average_latency: Option<f64>,
    pub tp99_latency: Option<f64>,
    pub transaction_count: Option<i64>,
    pub route: Option<String>,
    pub details: Option<String>,
    pub created_at_ms: i64,
}

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

fn spawn_persist(event: NewAnalyticsEvent) {
    let label = event_type_label(event.event_type.as_str());
    ANALYTICS_EVENT_COUNTER.with_label_values(&[label]).inc();

    tokio::spawn(async move {
        if let Err(err) = persist_event(event).await {
            crate::logger::debug!(error = %err, "Failed to persist analytics event");
        }
    });
}

pub fn record_decision_event(
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
    spawn_persist(NewAnalyticsEvent {
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
    spawn_persist(NewAnalyticsEvent {
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
    merchant_id: Option<String>,
    gateway: Option<String>,
    status: Option<String>,
    route: &str,
    details: Option<String>,
    payment_id: Option<String>,
    request_id: Option<String>,
    event_stage: Option<String>,
) {
    spawn_persist(NewAnalyticsEvent {
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
    spawn_persist(NewAnalyticsEvent {
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
    merchant_id: Option<String>,
    payment_id: Option<String>,
    gateway: Option<String>,
    rule_name: Option<String>,
    status: Option<String>,
    details: Option<String>,
) {
    spawn_persist(NewAnalyticsEvent {
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
    spawn_persist(NewAnalyticsEvent {
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
    route: &str,
    merchant_id: Option<String>,
    payment_id: Option<String>,
    request_id: Option<String>,
) {
    spawn_persist(NewAnalyticsEvent {
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

async fn persist_event(event: NewAnalyticsEvent) -> Result<(), error::ApiError> {
    let state = crate::app::get_tenant_app_state().await;
    let conn = &state
        .db
        .get_conn()
        .await
        .map_err(|_| error::ApiError::DatabaseError)?;

    diesel::insert_into(analytics_dsl::analytics_event)
        .values(event)
        .execute_async(&**conn)
        .await
        .map_err(|_| error::ApiError::DatabaseError)?;

    Ok(())
}

async fn load_events(
    state: &crate::app::TenantAppState,
    query: &AnalyticsQuery,
    event_types: &[&str],
) -> Result<Vec<AnalyticsEvent>, error::ApiError> {
    let conn = &state
        .db
        .get_conn()
        .await
        .map_err(|_| error::ApiError::DatabaseError)?;

    let mut builder = analytics_dsl::analytics_event
        .select(AnalyticsEvent::as_select())
        .into_boxed();
    let (start_ms, end_ms) = effective_window_bounds(query);
    builder = builder
        .filter(analytics_dsl::created_at_ms.ge(start_ms))
        .filter(analytics_dsl::created_at_ms.le(end_ms));

    if query.scope == AnalyticsScope::Current {
        if let Some(merchant_id) = &query.merchant_id {
            builder = builder.filter(analytics_dsl::merchant_id.eq(merchant_id.clone()));
        }
    }

    let event_types: Vec<String> = event_types
        .iter()
        .map(|event_type| (*event_type).to_string())
        .collect();

    if !event_types.is_empty() {
        builder = builder.filter(analytics_dsl::event_type.eq_any(event_types));
    }

    builder
        .order((analytics_dsl::created_at_ms.asc(), analytics_dsl::id.asc()))
        .load_async::<AnalyticsEvent>(&**conn)
        .await
        .map_err(|err| {
            crate::logger::error!(
                error = ?err,
                merchant_id = ?query.merchant_id,
                scope = query.scope.as_str(),
                "Analytics read failed; returning empty analytics state"
            );
            err
        })
        .or_else(|_| Ok(Vec::new()))
}

async fn load_payment_audit_events(
    state: &crate::app::TenantAppState,
    query: &PaymentAuditQuery,
) -> Result<Vec<AnalyticsEvent>, error::ApiError> {
    let conn = &state
        .db
        .get_conn()
        .await
        .map_err(|_| error::ApiError::DatabaseError)?;

    let mut builder = analytics_dsl::analytics_event
        .select(AnalyticsEvent::as_select())
        .into_boxed();
    let (start_ms, end_ms) = effective_payment_audit_window_bounds(query);
    builder = builder
        .filter(analytics_dsl::created_at_ms.ge(start_ms))
        .filter(analytics_dsl::created_at_ms.le(end_ms));
    builder = builder.filter(analytics_dsl::event_type.eq_any(vec![
        "decision".to_string(),
        "gateway_update".to_string(),
        "rule_hit".to_string(),
        "error".to_string(),
    ]));

    if query.scope == AnalyticsScope::Current {
        if let Some(merchant_id) = &query.merchant_id {
            builder = builder.filter(analytics_dsl::merchant_id.eq(merchant_id.clone()));
        }
    }

    if let Some(payment_id) = &query.payment_id {
        builder = builder.filter(analytics_dsl::payment_id.eq(payment_id.clone()));
    }

    if query.payment_id.is_none() {
        if let Some(request_id) = &query.request_id {
            builder = builder.filter(analytics_dsl::request_id.eq(request_id.clone()));
        }
    }

    if let Some(gateway) = &query.gateway {
        builder = builder.filter(analytics_dsl::gateway.eq(gateway.clone()));
    }

    if let Some(route) = &query.route {
        builder = builder.filter(analytics_dsl::route.eq(route.clone()));
    }

    if let Some(status) = &query.status {
        if status.eq_ignore_ascii_case("success") {
            builder = builder.filter(
                analytics_dsl::status
                    .eq("success".to_string())
                    .or(analytics_dsl::status
                        .eq("SUCCESS".to_string())
                        .or(analytics_dsl::status
                            .eq("CHARGED".to_string())
                            .or(analytics_dsl::status.eq("AUTHORIZED".to_string())))),
            );
        } else if status.eq_ignore_ascii_case("failure") || status.eq_ignore_ascii_case("FAILURE") {
            builder = builder.filter(
                analytics_dsl::status
                    .eq("FAILURE".to_string())
                    .or(analytics_dsl::status
                        .like("%FAILED%")
                        .or(analytics_dsl::status.like("%DECLINED%"))),
            );
        } else {
            builder = builder.filter(analytics_dsl::status.eq(status.clone()));
        }
    }

    if let Some(event_type) = &query.event_type {
        builder = builder.filter(analytics_dsl::event_type.eq(event_type.clone()));
    }

    if let Some(error_code) = &query.error_code {
        builder = builder.filter(analytics_dsl::error_code.eq(error_code.clone()));
    }

    builder
        .order((
            analytics_dsl::created_at_ms.desc(),
            analytics_dsl::id.desc(),
        ))
        .load_async::<AnalyticsEvent>(&**conn)
        .await
        .map_err(|err| {
            crate::logger::error!(
                error = ?err,
                merchant_id = ?query.merchant_id,
                payment_id = ?query.payment_id,
                request_id = ?query.request_id,
                "Payment audit read failed; returning empty audit state"
            );
            err
        })
        .or_else(|_| Ok(Vec::new()))
}

async fn load_preview_trace_events(
    state: &crate::app::TenantAppState,
    query: &PaymentAuditQuery,
) -> Result<Vec<AnalyticsEvent>, error::ApiError> {
    let conn = &state
        .db
        .get_conn()
        .await
        .map_err(|_| error::ApiError::DatabaseError)?;

    let mut builder = analytics_dsl::analytics_event
        .select(AnalyticsEvent::as_select())
        .into_boxed();
    let (start_ms, end_ms) = effective_payment_audit_window_bounds(query);
    builder = builder
        .filter(analytics_dsl::created_at_ms.ge(start_ms))
        .filter(analytics_dsl::created_at_ms.le(end_ms));
    builder = builder.filter(analytics_dsl::route.eq("routing_evaluate".to_string()));
    builder = builder.filter(analytics_dsl::event_type.eq_any(vec![
        "rule_evaluation_preview".to_string(),
        "error".to_string(),
    ]));

    if query.scope == AnalyticsScope::Current {
        if let Some(merchant_id) = &query.merchant_id {
            builder = builder.filter(analytics_dsl::merchant_id.eq(merchant_id.clone()));
        }
    }

    if let Some(payment_id) = &query.payment_id {
        builder = builder.filter(analytics_dsl::payment_id.eq(payment_id.clone()));
    }

    if query.payment_id.is_none() {
        if let Some(request_id) = &query.request_id {
            builder = builder.filter(analytics_dsl::request_id.eq(request_id.clone()));
        }
    }

    if let Some(gateway) = &query.gateway {
        builder = builder.filter(analytics_dsl::gateway.eq(gateway.clone()));
    }

    if let Some(status) = &query.status {
        if status.eq_ignore_ascii_case("success") {
            builder = builder.filter(
                analytics_dsl::status
                    .eq("success".to_string())
                    .or(analytics_dsl::status.eq("default_selection".to_string())),
            );
        } else if status.eq_ignore_ascii_case("failure") || status.eq_ignore_ascii_case("FAILURE") {
            builder = builder.filter(
                analytics_dsl::status
                    .eq("FAILURE".to_string())
                    .or(analytics_dsl::status
                        .like("%FAILED%")
                        .or(analytics_dsl::status.like("%DECLINED%"))),
            );
        } else {
            builder = builder.filter(analytics_dsl::status.eq(status.clone()));
        }
    }

    if let Some(event_type) = &query.event_type {
        builder = builder.filter(analytics_dsl::event_type.eq(event_type.clone()));
    }

    if let Some(error_code) = &query.error_code {
        builder = builder.filter(analytics_dsl::error_code.eq(error_code.clone()));
    }

    builder
        .order((
            analytics_dsl::created_at_ms.desc(),
            analytics_dsl::id.desc(),
        ))
        .load_async::<AnalyticsEvent>(&**conn)
        .await
        .map_err(|err| {
            crate::logger::error!(
                error = ?err,
                merchant_id = ?query.merchant_id,
                payment_id = ?query.payment_id,
                "Preview trace read failed; returning empty preview state"
            );
            err
        })
        .or_else(|_| Ok(Vec::new()))
}

fn parse_details_json(details: &Option<String>) -> Option<serde_json::Value> {
    details
        .as_ref()
        .and_then(|value| serde_json::from_str::<serde_json::Value>(value).ok())
}

fn transaction_status_from_details(event: &AnalyticsEvent) -> Option<String> {
    let details = parse_details_json(&event.details)?;
    details
        .get("request")
        .and_then(|request| request.get("status"))
        .and_then(|status| status.as_str())
        .map(str::to_string)
        .or_else(|| {
            details
                .get("selection_reason")
                .and_then(|reason| reason.get("transaction_status"))
                .and_then(|status| status.as_str())
                .map(str::to_string)
        })
}

fn effective_window_bounds(query: &AnalyticsQuery) -> (i64, i64) {
    let now = now_ms();
    let end_ms = query.end_ms.unwrap_or(now).min(now);
    let start_ms = query
        .start_ms
        .filter(|start_ms| *start_ms >= 0 && *start_ms < end_ms)
        .unwrap_or_else(|| end_ms.saturating_sub(query.range.window_ms()));

    (start_ms, end_ms)
}

fn effective_payment_audit_window_bounds(query: &PaymentAuditQuery) -> (i64, i64) {
    let now = now_ms();
    let end_ms = query.end_ms.unwrap_or(now).min(now);
    let start_ms = query
        .start_ms
        .filter(|start_ms| *start_ms >= 0 && *start_ms < end_ms)
        .unwrap_or_else(|| end_ms.saturating_sub(query.range.window_ms()));

    (start_ms, end_ms)
}

fn query_bucket_size_ms(query: &AnalyticsQuery) -> i64 {
    let (start_ms, end_ms) = effective_window_bounds(query);
    let window_ms = end_ms.saturating_sub(start_ms);

    match window_ms {
        0..=900_000 => 60 * 1000,
        900_001..=3_600_000 => 5 * 60 * 1000,
        3_600_001..=86_400_000 => 15 * 60 * 1000,
        86_400_001..=259_200_000 => 60 * 60 * 1000,
        _ => 3 * 60 * 60 * 1000,
    }
}

fn bucket_ms(created_at_ms: i64, query: &AnalyticsQuery) -> i64 {
    let bucket = query_bucket_size_ms(query).max(1);
    created_at_ms - (created_at_ms.rem_euclid(bucket))
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

fn payment_audit_stage_label(event: &AnalyticsEvent) -> &'static str {
    match event.event_type.as_str() {
        "decision" => "Decide Gateway",
        "gateway_update" => "Update Gateway",
        "rule_hit" => "Rule Evaluate",
        "rule_evaluation_preview" => "Preview Result",
        "error" => "Errors",
        _ => "Errors",
    }
}

fn payment_audit_route_label(route: &str) -> String {
    match route {
        "decision_gateway" | "decide_gateway" => "Decide Gateway".to_string(),
        "update_gateway_score" => "Update Gateway".to_string(),
        "routing_evaluate" => "Rule Evaluate".to_string(),
        other => other.to_string(),
    }
}

fn payment_dimension_filters_enabled(query: &AnalyticsQuery) -> bool {
    query.scope == AnalyticsScope::Current
}

struct RoutingDimensionSpec {
    key: &'static str,
    label: &'static str,
}

const ROUTING_DIMENSION_SPECS: [RoutingDimensionSpec; 7] = [
    RoutingDimensionSpec {
        key: "payment_method_type",
        label: "Payment Method Type",
    },
    RoutingDimensionSpec {
        key: "payment_method",
        label: "Payment Method",
    },
    RoutingDimensionSpec {
        key: "card_network",
        label: "Card Network",
    },
    RoutingDimensionSpec {
        key: "card_is_in",
        label: "Card ISIN",
    },
    RoutingDimensionSpec {
        key: "currency",
        label: "Currency",
    },
    RoutingDimensionSpec {
        key: "country",
        label: "Country",
    },
    RoutingDimensionSpec {
        key: "auth_type",
        label: "Auth Type",
    },
];

fn event_dimension_value<'a>(event: &'a AnalyticsEvent, key: &str) -> Option<&'a str> {
    match key {
        "payment_method_type" => event.payment_method_type.as_deref(),
        "payment_method" => event.payment_method.as_deref(),
        "card_network" => event.card_network.as_deref(),
        "card_is_in" => event.card_is_in.as_deref(),
        "currency" => event.currency.as_deref(),
        "country" => event.country.as_deref(),
        "auth_type" => event.auth_type.as_deref(),
        _ => None,
    }
}

fn query_dimension_value<'a>(query: &'a AnalyticsQuery, key: &str) -> Option<&'a str> {
    match key {
        "payment_method_type" => query.payment_method_type.as_deref(),
        "payment_method" => query.payment_method.as_deref(),
        "card_network" => query.card_network.as_deref(),
        "card_is_in" => query.card_is_in.as_deref(),
        "currency" => query.currency.as_deref(),
        "country" => query.country.as_deref(),
        "auth_type" => query.auth_type.as_deref(),
        _ => None,
    }
}

fn score_events_for_dimensions<'a>(events: &'a [AnalyticsEvent]) -> Vec<&'a AnalyticsEvent> {
    events
        .iter()
        .filter(|event| event.event_type == "score_snapshot")
        .collect()
}

fn active_routing_dimension_specs(
    events: &[&AnalyticsEvent],
) -> Vec<&'static RoutingDimensionSpec> {
    ROUTING_DIMENSION_SPECS
        .iter()
        .filter(|spec| {
            events.iter().any(|event| {
                event_dimension_value(event, spec.key).is_some_and(|value| !value.is_empty())
            })
        })
        .collect()
}

fn score_event_matches_dimension_filters(
    event: &AnalyticsEvent,
    query: &AnalyticsQuery,
    ignored_key: Option<&str>,
) -> bool {
    if !payment_dimension_filters_enabled(query) {
        return true;
    }

    for spec in ROUTING_DIMENSION_SPECS.iter() {
        if ignored_key == Some(spec.key) {
            continue;
        }

        let Some(selected_value) = query_dimension_value(query, spec.key) else {
            continue;
        };

        if event_dimension_value(event, spec.key) != Some(selected_value) {
            return false;
        }
    }

    true
}

fn score_event_matches_filters(event: &AnalyticsEvent, query: &AnalyticsQuery) -> bool {
    if !score_event_matches_dimension_filters(event, query, None) {
        return false;
    }

    if !query.gateways.is_empty() {
        let Some(gateway) = event.gateway.as_deref() else {
            return false;
        };
        if !query.gateways.iter().any(|selected| selected == gateway) {
            return false;
        }
    }

    true
}

fn derive_routing_filter_options(
    events: &[AnalyticsEvent],
    query: &AnalyticsQuery,
) -> RoutingFilterOptions {
    let score_events = score_events_for_dimensions(events);
    let active_dimensions = active_routing_dimension_specs(&score_events);
    let missing_dimensions = ROUTING_DIMENSION_SPECS
        .iter()
        .filter(|spec| {
            !active_dimensions
                .iter()
                .any(|active| active.key == spec.key)
        })
        .map(|spec| RoutingFilterDimensionHint {
            key: spec.key.to_string(),
            label: spec.label.to_string(),
        })
        .collect();

    let dimensions = active_dimensions
        .into_iter()
        .map(|spec| {
            let values = score_events
                .iter()
                .filter(|event| score_event_matches_dimension_filters(event, query, Some(spec.key)))
                .filter_map(|event| event_dimension_value(event, spec.key).map(str::to_string))
                .filter(|value| !value.is_empty())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();

            RoutingFilterDimension {
                key: spec.key.to_string(),
                label: spec.label.to_string(),
                values,
            }
        })
        .collect();

    let gateways = score_events
        .iter()
        .filter(|event| score_event_matches_dimension_filters(event, query, None))
        .filter_map(|event| event.gateway.clone())
        .filter(|value| !value.is_empty())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();

    RoutingFilterOptions {
        dimensions,
        missing_dimensions,
        gateways,
    }
}

fn summarise_filtered_scores(
    events: &[AnalyticsEvent],
    query: &AnalyticsQuery,
) -> Vec<GatewayScoreSeriesPoint> {
    let mut by_bucket_gateway: BTreeMap<(i64, String), (f64, i64, String, String, String)> =
        BTreeMap::new();

    for event in events
        .iter()
        .filter(|event| event.event_type == "score_snapshot")
        .filter(|event| score_event_matches_filters(event, query))
    {
        let gateway = event
            .gateway
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        let bucket = bucket_ms(event.created_at_ms, query);
        let entry = by_bucket_gateway
            .entry((bucket, gateway.clone()))
            .or_insert((
                0.0,
                0,
                event.merchant_id.clone().unwrap_or_default(),
                event.payment_method_type.clone().unwrap_or_default(),
                event.payment_method.clone().unwrap_or_default(),
            ));
        entry.0 += event.score_value.unwrap_or_default();
        entry.1 += 1;
    }

    by_bucket_gateway
        .into_iter()
        .map(
            |(
                (bucket_ms, gateway),
                (score_total, score_count, merchant_id, payment_method_type, payment_method),
            )| {
                GatewayScoreSeriesPoint {
                    bucket_ms,
                    merchant_id,
                    payment_method_type,
                    payment_method,
                    gateway,
                    score_value: if score_count > 0 {
                        score_total / score_count as f64
                    } else {
                        0.0
                    },
                }
            },
        )
        .collect()
}

fn summarise_errors(events: &[AnalyticsEvent]) -> Vec<AnalyticsErrorSummary> {
    let mut grouped: HashMap<(String, String, String), AnalyticsErrorSummary> = HashMap::new();

    for event in events.iter().filter(|event| event.event_type == "error") {
        let route = event.route.clone().unwrap_or_else(|| "unknown".to_string());
        let error_code = event
            .error_code
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        let error_message = event
            .error_message
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        let key = (route.clone(), error_code.clone(), error_message.clone());
        grouped
            .entry(key)
            .and_modify(|summary| {
                summary.count += 1;
                summary.last_seen_ms = summary.last_seen_ms.max(event.created_at_ms);
            })
            .or_insert_with(|| AnalyticsErrorSummary {
                route,
                error_code,
                error_message,
                count: 1,
                last_seen_ms: event.created_at_ms,
            });
    }

    let mut rows: Vec<_> = grouped.into_values().collect();
    rows.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| right.last_seen_ms.cmp(&left.last_seen_ms))
    });
    rows
}

fn summarise_rule_hits(events: &[AnalyticsEvent]) -> Vec<AnalyticsRuleHit> {
    let mut grouped: HashMap<String, i64> = HashMap::new();
    for event in events.iter().filter(|event| event.event_type == "rule_hit") {
        let rule_name = event
            .rule_name
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        *grouped.entry(rule_name).or_insert(0) += 1;
    }

    let mut rows: Vec<_> = grouped
        .into_iter()
        .map(|(rule_name, count)| AnalyticsRuleHit { rule_name, count })
        .collect();
    rows.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.rule_name.cmp(&right.rule_name))
    });
    rows
}

fn summarise_scores(
    events: &[AnalyticsEvent],
) -> (Vec<GatewayScoreSnapshot>, Vec<GatewayScoreSeriesPoint>) {
    let mut latest: HashMap<(String, String, String, String), AnalyticsEvent> = HashMap::new();
    let mut series = Vec::new();

    for event in events
        .iter()
        .filter(|event| event.event_type == "score_snapshot")
    {
        let merchant_id = event.merchant_id.clone().unwrap_or_default();
        let payment_method_type = event.payment_method_type.clone().unwrap_or_default();
        let payment_method = event.payment_method.clone().unwrap_or_default();
        let gateway = event.gateway.clone().unwrap_or_default();
        let key = (
            merchant_id.clone(),
            payment_method_type.clone(),
            payment_method.clone(),
            gateway.clone(),
        );
        series.push(GatewayScoreSeriesPoint {
            bucket_ms: event.created_at_ms,
            merchant_id: merchant_id.clone(),
            payment_method_type: payment_method_type.clone(),
            payment_method: payment_method.clone(),
            gateway: gateway.clone(),
            score_value: event.score_value.unwrap_or_default(),
        });

        latest
            .entry(key)
            .and_modify(|current| {
                if event.created_at_ms >= current.created_at_ms {
                    *current = event.clone();
                }
            })
            .or_insert_with(|| event.clone());
    }

    let mut snapshots: Vec<GatewayScoreSnapshot> = latest
        .into_iter()
        .map(
            |((merchant_id, payment_method_type, payment_method, gateway), event)| {
                GatewayScoreSnapshot {
                    merchant_id,
                    payment_method_type,
                    payment_method,
                    gateway,
                    score_value: event.score_value.unwrap_or_default(),
                    sigma_factor: event.sigma_factor.unwrap_or_default(),
                    average_latency: event.average_latency.unwrap_or_default(),
                    tp99_latency: event.tp99_latency.unwrap_or_default(),
                    transaction_count: event.transaction_count.unwrap_or_default(),
                    last_updated_ms: event.created_at_ms,
                }
            },
        )
        .collect();
    snapshots.sort_by(|left, right| {
        right
            .score_value
            .partial_cmp(&left.score_value)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| right.last_updated_ms.cmp(&left.last_updated_ms))
    });

    (snapshots, series)
}

fn summarise_decisions(
    events: &[AnalyticsEvent],
    query: &AnalyticsQuery,
) -> (
    Vec<AnalyticsDecisionPoint>,
    Vec<AnalyticsRuleHit>,
    Vec<AnalyticsKpi>,
) {
    let mut by_bucket: BTreeMap<(i64, String), i64> = BTreeMap::new();
    let mut by_approach: HashMap<String, i64> = HashMap::new();
    let mut total = 0_i64;
    let mut failures = 0_i64;

    for event in events.iter().filter(|event| event.event_type == "decision") {
        total += 1;
        if event.status.as_deref() == Some("failure") {
            failures += 1;
        }
        let bucket = bucket_ms(event.created_at_ms, query);
        let approach = event
            .routing_approach
            .clone()
            .unwrap_or_else(|| "UNKNOWN".to_string());
        *by_bucket.entry((bucket, approach.clone())).or_insert(0) += 1;
        *by_approach.entry(approach).or_insert(0) += 1;
    }

    let mut series: Vec<AnalyticsDecisionPoint> = by_bucket
        .into_iter()
        .map(
            |((bucket_ms, routing_approach), count)| AnalyticsDecisionPoint {
                bucket_ms,
                routing_approach,
                count,
            },
        )
        .collect();
    series.sort_by(|left, right| {
        left.bucket_ms
            .cmp(&right.bucket_ms)
            .then_with(|| left.routing_approach.cmp(&right.routing_approach))
    });

    let mut approaches: Vec<AnalyticsRuleHit> = by_approach
        .into_iter()
        .map(|(rule_name, count)| AnalyticsRuleHit { rule_name, count })
        .collect();
    approaches.sort_by(|left, right| right.count.cmp(&left.count));

    let error_rate = if total > 0 {
        (failures as f64 / total as f64) * 100.0
    } else {
        0.0
    };

    let tiles = vec![
        AnalyticsKpi {
            label: "Decisions".to_string(),
            value: total.to_string(),
            subtitle: Some(format!("Failures: {}", failures)),
        },
        AnalyticsKpi {
            label: "Error rate".to_string(),
            value: format!("{:.2}%", error_rate),
            subtitle: Some("From recorded decision events".to_string()),
        },
    ];

    (series, approaches, tiles)
}

fn summarise_gateway_share(
    events: &[AnalyticsEvent],
    query: &AnalyticsQuery,
) -> Vec<AnalyticsGatewaySharePoint> {
    let mut by_bucket_gateway: BTreeMap<(i64, String), i64> = BTreeMap::new();

    for event in events.iter().filter(|event| event.event_type == "decision") {
        let bucket = bucket_ms(event.created_at_ms, query);
        let gateway = event
            .gateway
            .clone()
            .unwrap_or_else(|| "unknown".to_string());
        *by_bucket_gateway.entry((bucket, gateway)).or_insert(0) += 1;
    }

    let mut points: Vec<_> = by_bucket_gateway
        .into_iter()
        .map(|((bucket_ms, gateway), count)| AnalyticsGatewaySharePoint {
            bucket_ms,
            gateway,
            count,
        })
        .collect();
    points.sort_by(|left, right| {
        left.bucket_ms
            .cmp(&right.bucket_ms)
            .then_with(|| left.gateway.cmp(&right.gateway))
    });
    points
}

fn overview_kpis(events: &[AnalyticsEvent], query: &AnalyticsQuery) -> Vec<AnalyticsKpi> {
    let total = events
        .iter()
        .filter(|event| event.event_type == "decision")
        .count() as i64;
    let score_count = events
        .iter()
        .filter(|event| event.event_type == "score_snapshot")
        .count() as i64;
    let rule_hit_count = events
        .iter()
        .filter(|event| event.event_type == "rule_hit")
        .count() as i64;
    let error_count = events
        .iter()
        .filter(|event| event.event_type == "error")
        .count() as i64;

    vec![
        AnalyticsKpi {
            label: format!("Decisions / {}", format_range(query)),
            value: total.to_string(),
            subtitle: Some("Recorded decision events".to_string()),
        },
        AnalyticsKpi {
            label: "Score snapshots".to_string(),
            value: score_count.to_string(),
            subtitle: Some("Latest gateway score updates".to_string()),
        },
        AnalyticsKpi {
            label: "Rule hits".to_string(),
            value: rule_hit_count.to_string(),
            subtitle: Some("Priority-logic hits".to_string()),
        },
        AnalyticsKpi {
            label: "Errors".to_string(),
            value: error_count.to_string(),
            subtitle: Some("Structured failure summaries".to_string()),
        },
    ]
}

fn summarise_route_hits(events: &[AnalyticsEvent]) -> Vec<AnalyticsRouteHit> {
    let mut counts: HashMap<String, i64> = HashMap::new();
    for event in events
        .iter()
        .filter(|event| event.event_type == "request_hit")
    {
        let route = event.route.clone().unwrap_or_else(|| "unknown".to_string());
        *counts.entry(route).or_insert(0) += 1;
    }

    [
        ("decide_gateway", "/decide_gateway"),
        ("update_gateway_score", "/update_gateway"),
        ("routing_evaluate", "/rule_evaluate"),
    ]
    .into_iter()
    .map(|(stored_route, display_route)| AnalyticsRouteHit {
        route: display_route.to_string(),
        count: counts.get(stored_route).copied().unwrap_or(0),
    })
    .collect()
}

fn empty_overview_response(query: &AnalyticsQuery) -> AnalyticsOverviewResponse {
    AnalyticsOverviewResponse {
        generated_at_ms: now_ms(),
        scope: query.scope.as_str().to_string(),
        merchant_id: query.merchant_id.clone(),
        kpis: vec![
            AnalyticsKpi {
                label: format!("Decisions / {}", format_range(query)),
                value: "0".to_string(),
                subtitle: Some("Global mode is limited to connector-level analytics".to_string()),
            },
            AnalyticsKpi {
                label: "Score snapshots".to_string(),
                value: "0".to_string(),
                subtitle: Some("Global mode hides merchant-specific data".to_string()),
            },
            AnalyticsKpi {
                label: "Rule hits".to_string(),
                value: "0".to_string(),
                subtitle: Some("Global mode hides merchant-specific data".to_string()),
            },
            AnalyticsKpi {
                label: "Errors".to_string(),
                value: "0".to_string(),
                subtitle: Some("Global mode hides merchant-specific data".to_string()),
            },
        ],
        route_hits: vec![
            AnalyticsRouteHit {
                route: "/decide_gateway".to_string(),
                count: 0,
            },
            AnalyticsRouteHit {
                route: "/update_gateway".to_string(),
                count: 0,
            },
            AnalyticsRouteHit {
                route: "/rule_evaluate".to_string(),
                count: 0,
            },
        ],
        top_scores: Vec::new(),
        top_errors: Vec::new(),
        top_rules: Vec::new(),
    }
}

fn empty_gateway_scores_response(query: &AnalyticsQuery) -> AnalyticsGatewayScoresResponse {
    AnalyticsGatewayScoresResponse {
        generated_at_ms: now_ms(),
        scope: query.scope.as_str().to_string(),
        merchant_id: query.merchant_id.clone(),
        range: format_range(query),
        snapshots: Vec::new(),
        series: Vec::new(),
    }
}

fn empty_decisions_response(query: &AnalyticsQuery) -> AnalyticsDecisionResponse {
    AnalyticsDecisionResponse {
        generated_at_ms: now_ms(),
        scope: query.scope.as_str().to_string(),
        merchant_id: query.merchant_id.clone(),
        range: format_range(query),
        tiles: vec![
            AnalyticsKpi {
                label: "Decisions".to_string(),
                value: "0".to_string(),
                subtitle: Some("Global mode hides merchant-specific traffic volumes".to_string()),
            },
            AnalyticsKpi {
                label: "Error rate".to_string(),
                value: "0.00%".to_string(),
                subtitle: Some("Global mode hides merchant-specific traffic volumes".to_string()),
            },
        ],
        series: Vec::new(),
        approaches: Vec::new(),
    }
}

fn empty_log_summaries_response(query: &AnalyticsQuery) -> AnalyticsLogSummariesResponse {
    AnalyticsLogSummariesResponse {
        generated_at_ms: now_ms(),
        scope: query.scope.as_str().to_string(),
        merchant_id: query.merchant_id.clone(),
        range: format_range(query),
        total_errors: 0,
        errors: Vec::new(),
        samples: Vec::new(),
        page: query.page.max(1),
        page_size: query.page_size.clamp(1, 50),
    }
}

fn empty_payment_audit_response(query: &PaymentAuditQuery) -> PaymentAuditResponse {
    PaymentAuditResponse {
        generated_at_ms: now_ms(),
        scope: query.scope.as_str().to_string(),
        merchant_id: query.merchant_id.clone(),
        range: format_payment_audit_range(query),
        payment_id: query.payment_id.clone(),
        request_id: query.request_id.clone(),
        gateway: query.gateway.clone(),
        route: query.route.clone(),
        status: query.status.clone(),
        event_type: query.event_type.clone(),
        error_code: query.error_code.clone(),
        page: query.page.max(1),
        page_size: query.page_size.clamp(1, 50),
        total_results: 0,
        results: Vec::new(),
        timeline: Vec::new(),
    }
}

fn summarise_payment_audit_results(events: &[AnalyticsEvent]) -> Vec<PaymentAuditSummary> {
    let mut grouped: HashMap<String, Vec<&AnalyticsEvent>> = HashMap::new();

    for event in events {
        let Some(lookup_key) = event
            .payment_id
            .clone()
            .or_else(|| event.request_id.clone())
        else {
            continue;
        };
        grouped.entry(lookup_key).or_default().push(event);
    }

    let mut rows: Vec<PaymentAuditSummary> = grouped
        .into_iter()
        .filter_map(|(lookup_key, events)| {
            let mut sorted = events;
            sorted.sort_by(|left, right| {
                left.created_at_ms
                    .cmp(&right.created_at_ms)
                    .then_with(|| left.id.cmp(&right.id))
            });
            let (first, last) = match (sorted.first().copied(), sorted.last().copied()) {
                (Some(first), Some(last)) => (first, last),
                _ => return None,
            };
            let gateways = sorted
                .iter()
                .filter_map(|event| event.gateway.clone())
                .fold(Vec::<String>::new(), |mut acc, gateway| {
                    if !acc.contains(&gateway) {
                        acc.push(gateway);
                    }
                    acc
                });
            let routes = sorted.iter().filter_map(|event| event.route.clone()).fold(
                Vec::<String>::new(),
                |mut acc, route| {
                    let route_label = payment_audit_route_label(&route);
                    if !acc.contains(&route_label) {
                        acc.push(route_label);
                    }
                    acc
                },
            );

            Some(PaymentAuditSummary {
                lookup_key,
                payment_id: last.payment_id.clone().or_else(|| first.payment_id.clone()),
                request_id: last.request_id.clone().or_else(|| first.request_id.clone()),
                merchant_id: last
                    .merchant_id
                    .clone()
                    .or_else(|| first.merchant_id.clone()),
                first_seen_ms: first.created_at_ms,
                last_seen_ms: last.created_at_ms,
                event_count: sorted.len(),
                latest_status: sorted
                    .iter()
                    .rev()
                    .find_map(|event| transaction_status_from_details(event))
                    .or_else(|| {
                        sorted
                            .iter()
                            .rev()
                            .find_map(|event| match event.event_type.as_str() {
                                "error" => Some("FAILURE".to_string()),
                                "decision" | "gateway_update" | "rule_evaluation_preview" => {
                                    event.status.clone()
                                }
                                _ => None,
                            })
                    }),
                latest_gateway: last.gateway.clone(),
                latest_stage: Some(payment_audit_stage_label(last).to_string()),
                gateways,
                routes,
            })
        })
        .collect();

    rows.sort_by(|left, right| {
        right
            .last_seen_ms
            .cmp(&left.last_seen_ms)
            .then_with(|| right.event_count.cmp(&left.event_count))
    });
    rows
}

fn build_payment_timeline(
    events: &[AnalyticsEvent],
    selected_payment_id: Option<&str>,
    selected_request_id: Option<&str>,
    selected_lookup_key: Option<&str>,
) -> Vec<PaymentAuditEvent> {
    let mut timeline: Vec<PaymentAuditEvent> = events
        .iter()
        .filter(|event| {
            if let Some(payment_id) = selected_payment_id {
                return event.payment_id.as_deref() == Some(payment_id);
            }
            if let Some(request_id) = selected_request_id {
                return event.request_id.as_deref() == Some(request_id);
            }
            if let Some(lookup_key) = selected_lookup_key {
                return event.payment_id.as_deref() == Some(lookup_key)
                    || event.request_id.as_deref() == Some(lookup_key);
            }
            false
        })
        .map(|event| PaymentAuditEvent {
            id: event.id,
            event_type: event.event_type.clone(),
            event_stage: event.event_stage.clone(),
            route: event.route.clone(),
            merchant_id: event.merchant_id.clone(),
            payment_id: event.payment_id.clone(),
            request_id: event.request_id.clone(),
            payment_method_type: event.payment_method_type.clone(),
            payment_method: event.payment_method.clone(),
            gateway: event.gateway.clone(),
            routing_approach: event.routing_approach.clone(),
            rule_name: event.rule_name.clone(),
            status: event.status.clone(),
            error_code: event.error_code.clone(),
            error_message: event.error_message.clone(),
            score_value: event.score_value,
            sigma_factor: event.sigma_factor,
            average_latency: event.average_latency,
            tp99_latency: event.tp99_latency,
            transaction_count: event.transaction_count,
            details: event.details.clone(),
            details_json: parse_details_json(&event.details),
            created_at_ms: event.created_at_ms,
        })
        .collect();

    timeline.sort_by(|left, right| {
        left.created_at_ms
            .cmp(&right.created_at_ms)
            .then_with(|| left.id.cmp(&right.id))
    });
    timeline
}

pub async fn overview(
    state: &crate::app::TenantAppState,
    query: &AnalyticsQuery,
) -> Result<AnalyticsOverviewResponse, error::ApiError> {
    if query.scope == AnalyticsScope::All {
        return Ok(empty_overview_response(query));
    }
    let events = load_events(
        state,
        query,
        &[
            "request_hit",
            "decision",
            "score_snapshot",
            "rule_hit",
            "error",
        ],
    )
    .await?;
    let (top_scores, _) = summarise_scores(&events);
    let top_errors = summarise_errors(&events);
    let top_rules = summarise_rule_hits(&events);

    Ok(AnalyticsOverviewResponse {
        generated_at_ms: now_ms(),
        scope: query.scope.as_str().to_string(),
        merchant_id: query.merchant_id.clone(),
        kpis: overview_kpis(&events, query),
        route_hits: summarise_route_hits(&events),
        top_scores: top_scores.into_iter().take(5).collect(),
        top_errors: top_errors.into_iter().take(5).collect(),
        top_rules: top_rules.into_iter().take(5).collect(),
    })
}

pub async fn gateway_scores(
    state: &crate::app::TenantAppState,
    query: &AnalyticsQuery,
) -> Result<AnalyticsGatewayScoresResponse, error::ApiError> {
    if query.scope == AnalyticsScope::All {
        return Ok(empty_gateway_scores_response(query));
    }
    let events = load_events(state, query, &["score_snapshot"]).await?;
    let (snapshots, series) = summarise_scores(&events);
    Ok(AnalyticsGatewayScoresResponse {
        generated_at_ms: now_ms(),
        scope: query.scope.as_str().to_string(),
        merchant_id: query.merchant_id.clone(),
        range: format_range(query),
        snapshots,
        series,
    })
}

pub async fn decisions(
    state: &crate::app::TenantAppState,
    query: &AnalyticsQuery,
) -> Result<AnalyticsDecisionResponse, error::ApiError> {
    if query.scope == AnalyticsScope::All {
        return Ok(empty_decisions_response(query));
    }
    let events = load_events(state, query, &["decision"]).await?;
    let (series, approaches, tiles) = summarise_decisions(&events, query);
    Ok(AnalyticsDecisionResponse {
        generated_at_ms: now_ms(),
        scope: query.scope.as_str().to_string(),
        merchant_id: query.merchant_id.clone(),
        range: format_range(query),
        tiles,
        series,
        approaches,
    })
}

pub async fn routing_stats(
    state: &crate::app::TenantAppState,
    query: &AnalyticsQuery,
) -> Result<AnalyticsRoutingStatsResponse, error::ApiError> {
    let events = load_events(state, query, &["decision", "score_snapshot", "rule_hit"]).await?;
    let gateway_share = summarise_gateway_share(&events, query);
    let top_rules = summarise_rule_hits(&events);
    let available_filters = derive_routing_filter_options(&events, query);
    let series = summarise_filtered_scores(&events, query);

    Ok(AnalyticsRoutingStatsResponse {
        generated_at_ms: now_ms(),
        scope: query.scope.as_str().to_string(),
        merchant_id: query.merchant_id.clone(),
        range: format_range(query),
        gateway_share,
        top_rules: top_rules.into_iter().take(10).collect(),
        sr_trend: series,
        available_filters,
    })
}

pub async fn log_summaries(
    state: &crate::app::TenantAppState,
    query: &AnalyticsQuery,
) -> Result<AnalyticsLogSummariesResponse, error::ApiError> {
    if query.scope == AnalyticsScope::All {
        return Ok(empty_log_summaries_response(query));
    }
    let events = load_events(
        state,
        query,
        &["error", "decision", "score_snapshot", "rule_hit"],
    )
    .await?;
    let mut errors = summarise_errors(&events);
    let total_errors = errors.iter().map(|entry| entry.count).sum();
    errors.truncate(10);

    let mut samples: Vec<AnalyticsLogSample> = events
        .into_iter()
        .filter(|event| event.event_type == "error")
        .map(|event| AnalyticsLogSample {
            route: event.route.unwrap_or_else(|| "unknown".to_string()),
            merchant_id: event.merchant_id,
            payment_id: event.payment_id,
            request_id: event.request_id,
            gateway: event.gateway,
            routing_approach: event.routing_approach,
            status: event.status,
            error_code: event.error_code,
            error_message: event.error_message,
            event_type: Some(event.event_type),
            created_at_ms: event.created_at_ms,
        })
        .collect();
    samples.sort_by(|left, right| right.created_at_ms.cmp(&left.created_at_ms));

    let page_size = query.page_size.clamp(1, 50);
    let page = query.page.max(1);
    let start = (page - 1) * page_size;
    let samples = samples.into_iter().skip(start).take(page_size).collect();

    Ok(AnalyticsLogSummariesResponse {
        generated_at_ms: now_ms(),
        scope: query.scope.as_str().to_string(),
        merchant_id: query.merchant_id.clone(),
        range: format_range(query),
        total_errors,
        errors,
        samples,
        page,
        page_size,
    })
}

pub async fn payment_audit(
    state: &crate::app::TenantAppState,
    query: &PaymentAuditQuery,
) -> Result<PaymentAuditResponse, error::ApiError> {
    if query.scope == AnalyticsScope::All {
        return Ok(empty_payment_audit_response(query));
    }
    let events = load_payment_audit_events(state, query).await?;
    let audit_events: Vec<AnalyticsEvent> = events
        .into_iter()
        .filter(|event| event.event_type != "request_hit" && event.event_type != "score_snapshot")
        .collect();
    let results = summarise_payment_audit_results(&audit_events);

    let page_size = query.page_size.clamp(1, 50);
    let page = query.page.max(1);
    let start = (page - 1) * page_size;
    let paged_results: Vec<PaymentAuditSummary> = results
        .iter()
        .skip(start)
        .take(page_size)
        .cloned()
        .collect();

    let selected_payment_id = query.payment_id.as_deref().or_else(|| {
        paged_results
            .first()
            .and_then(|row| row.payment_id.as_deref())
    });
    let selected_request_id = query.request_id.as_deref().or_else(|| {
        paged_results
            .first()
            .and_then(|row| row.request_id.as_deref())
    });
    let selected_lookup_key = paged_results.first().map(|row| row.lookup_key.as_str());
    let timeline = build_payment_timeline(
        &audit_events,
        selected_payment_id,
        selected_request_id,
        selected_lookup_key,
    );

    Ok(PaymentAuditResponse {
        generated_at_ms: now_ms(),
        scope: query.scope.as_str().to_string(),
        merchant_id: query.merchant_id.clone(),
        range: format_payment_audit_range(query),
        payment_id: query
            .payment_id
            .clone()
            .or_else(|| paged_results.first().and_then(|row| row.payment_id.clone())),
        request_id: query
            .request_id
            .clone()
            .or_else(|| paged_results.first().and_then(|row| row.request_id.clone())),
        gateway: query.gateway.clone(),
        route: query.route.clone(),
        status: query.status.clone(),
        event_type: query.event_type.clone(),
        error_code: query.error_code.clone(),
        page,
        page_size,
        total_results: results.len(),
        results: paged_results,
        timeline,
    })
}

pub async fn preview_trace(
    state: &crate::app::TenantAppState,
    query: &PaymentAuditQuery,
) -> Result<PaymentAuditResponse, error::ApiError> {
    if query.scope == AnalyticsScope::All {
        return Ok(empty_payment_audit_response(query));
    }

    let preview_events = load_preview_trace_events(state, query).await?;
    let results = summarise_payment_audit_results(&preview_events);

    let page_size = query.page_size.clamp(1, 50);
    let page = query.page.max(1);
    let start = (page - 1) * page_size;
    let paged_results: Vec<PaymentAuditSummary> = results
        .iter()
        .skip(start)
        .take(page_size)
        .cloned()
        .collect();

    let selected_payment_id = query.payment_id.as_deref().or_else(|| {
        paged_results
            .first()
            .and_then(|row| row.payment_id.as_deref())
    });
    let selected_request_id = query.request_id.as_deref().or_else(|| {
        paged_results
            .first()
            .and_then(|row| row.request_id.as_deref())
    });
    let selected_lookup_key = paged_results.first().map(|row| row.lookup_key.as_str());
    let timeline = build_payment_timeline(
        &preview_events,
        selected_payment_id,
        selected_request_id,
        selected_lookup_key,
    );

    Ok(PaymentAuditResponse {
        generated_at_ms: now_ms(),
        scope: query.scope.as_str().to_string(),
        merchant_id: query.merchant_id.clone(),
        range: format_payment_audit_range(query),
        payment_id: query
            .payment_id
            .clone()
            .or_else(|| paged_results.first().and_then(|row| row.payment_id.clone())),
        request_id: query
            .request_id
            .clone()
            .or_else(|| paged_results.first().and_then(|row| row.request_id.clone())),
        gateway: query.gateway.clone(),
        route: Some("routing_evaluate".to_string()),
        status: query.status.clone(),
        event_type: query.event_type.clone(),
        error_code: query.error_code.clone(),
        page,
        page_size,
        total_results: results.len(),
        results: paged_results,
        timeline,
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

fn format_payment_audit_range(query: &PaymentAuditQuery) -> String {
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
