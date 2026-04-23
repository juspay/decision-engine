use clickhouse::Row;
use serde::Deserialize;

use crate::analytics::models::{PaymentAuditEvent, PaymentAuditQuery};
use crate::error::ApiError;

use super::super::common::{fetch_all, DOMAIN_TABLE};
use super::super::filters::payment_audit_raw_filters;
use super::super::query::{BoundQueryBuilder, FilterClause, OrderClause};

#[derive(Debug, Clone, Deserialize, Row)]
struct AuditEventRow {
    event_id: u64,
    flow_type: String,
    event_stage: Option<String>,
    route: Option<String>,
    merchant_id: Option<String>,
    payment_id: Option<String>,
    request_id: Option<String>,
    global_request_id: Option<String>,
    trace_id: Option<String>,
    payment_method_type: Option<String>,
    payment_method: Option<String>,
    gateway: Option<String>,
    routing_approach: Option<String>,
    rule_name: Option<String>,
    status: Option<String>,
    error_code: Option<String>,
    error_message: Option<String>,
    score_value: Option<f64>,
    sigma_factor: Option<f64>,
    average_latency: Option<f64>,
    tp99_latency: Option<f64>,
    transaction_count: Option<i64>,
    details: Option<String>,
    created_at_ms: i64,
}

pub async fn load(
    client: &clickhouse::Client,
    query: &PaymentAuditQuery,
    preview_only: bool,
    lookup_key: &str,
) -> Result<Vec<PaymentAuditEvent>, ApiError> {
    let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);
    builder.extend_selects([
        "event_id".to_string(),
        "flow_type".to_string(),
        "event_stage".to_string(),
        "route".to_string(),
        "merchant_id".to_string(),
        "payment_id".to_string(),
        "request_id".to_string(),
        "global_request_id".to_string(),
        "trace_id".to_string(),
        "payment_method_type".to_string(),
        "payment_method".to_string(),
        "gateway".to_string(),
        "routing_approach".to_string(),
        "rule_name".to_string(),
        "status".to_string(),
        "error_code".to_string(),
        "error_message".to_string(),
        "score_value".to_string(),
        "sigma_factor".to_string(),
        "average_latency".to_string(),
        "tp99_latency".to_string(),
        "transaction_count".to_string(),
        "details".to_string(),
        "created_at_ms".to_string(),
    ]);
    builder.extend_filters(payment_audit_raw_filters(query, preview_only));
    builder.add_filter(FilterClause::eq("lookup_key", lookup_key.to_string()));
    builder.add_order_by(OrderClause::asc("created_at_ms"));
    builder.add_order_by(OrderClause::asc("event_id"));

    let rows = fetch_all::<AuditEventRow>(builder.build(client)).await?;
    Ok(rows
        .into_iter()
        .map(|row| PaymentAuditEvent {
            id: row.event_id as i64,
            flow_type: row.flow_type,
            event_stage: row.event_stage,
            route: row.route,
            merchant_id: row.merchant_id,
            payment_id: row.payment_id,
            request_id: row.request_id,
            global_request_id: row.global_request_id,
            trace_id: row.trace_id,
            payment_method_type: row.payment_method_type,
            payment_method: row.payment_method,
            gateway: row.gateway,
            routing_approach: row.routing_approach,
            rule_name: row.rule_name,
            status: row.status,
            error_code: row.error_code,
            error_message: row.error_message,
            score_value: row.score_value,
            sigma_factor: row.sigma_factor,
            average_latency: row.average_latency,
            tp99_latency: row.tp99_latency,
            transaction_count: row.transaction_count,
            details_json: row
                .details
                .as_ref()
                .and_then(|value| serde_json::from_str(value).ok()),
            details: row.details,
            created_at_ms: row.created_at_ms,
        })
        .collect())
}
