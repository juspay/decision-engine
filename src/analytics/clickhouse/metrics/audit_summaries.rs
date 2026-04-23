use clickhouse::Row;
use serde::Deserialize;

use crate::analytics::models::{PaymentAuditQuery, PaymentAuditSummary};
use crate::error::ApiError;

use super::super::common::{
    fetch_all, payment_audit_route_label, payment_audit_stage_label, DOMAIN_TABLE,
};
use super::super::filters::payment_audit_filters;
use super::super::query::{BindArg, BoundQueryBuilder, SqlFragment};

#[derive(Debug, Clone, Deserialize, Row)]
struct AuditSummaryRow {
    lookup_key: String,
    payment_id: Option<String>,
    request_id: Option<String>,
    merchant_id: Option<String>,
    first_seen_ms: i64,
    last_seen_ms: i64,
    event_count: u64,
    latest_status: Option<String>,
    latest_gateway: Option<String>,
    latest_stage: Option<String>,
    gateways: Vec<String>,
    routes: Vec<String>,
}

fn inner_fragment(query: &PaymentAuditQuery, preview_only: bool) -> SqlFragment {
    let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);
    builder.extend_selects([
        "ifNull(if(payment_id != '' AND payment_id IS NOT NULL, payment_id, request_id), '') AS lookup_key".to_string(),
        "payment_id".to_string(),
        "request_id".to_string(),
        "merchant_id".to_string(),
        "created_at_ms".to_string(),
        "status".to_string(),
        "gateway".to_string(),
        "event_stage".to_string(),
        "route".to_string(),
    ]);
    builder.extend_filters(payment_audit_filters(query, preview_only));
    builder.into_fragment()
}

pub async fn load(
    client: &clickhouse::Client,
    query: &PaymentAuditQuery,
    preview_only: bool,
) -> Result<Vec<PaymentAuditSummary>, ApiError> {
    let inner = inner_fragment(query, preview_only);
    let mut outer = BoundQueryBuilder::from_fragment(SqlFragment::with_binds(
        format!("({})", inner.sql()),
        inner.binds().to_vec(),
    ));
    outer.extend_selects([
        "lookup_key".to_string(),
        "argMax(payment_id, created_at_ms) AS payment_id".to_string(),
        "argMax(request_id, created_at_ms) AS request_id".to_string(),
        "argMax(merchant_id, created_at_ms) AS merchant_id".to_string(),
        "min(created_at_ms) AS first_seen_ms".to_string(),
        "max(created_at_ms) AS last_seen_ms".to_string(),
        "count() AS event_count".to_string(),
        "argMax(status, created_at_ms) AS latest_status".to_string(),
        "argMax(gateway, created_at_ms) AS latest_gateway".to_string(),
        "argMax(event_stage, created_at_ms) AS latest_stage".to_string(),
        "arrayFilter(x -> x != '', groupUniqArray(ifNull(gateway, ''))) AS gateways".to_string(),
        "arrayFilter(x -> x != '', groupUniqArray(ifNull(route, ''))) AS routes".to_string(),
    ]);
    outer.add_filter(super::super::query::FilterClause::new(
        "lookup_key != ?",
        vec![BindArg::from("")],
    ));
    outer.add_group_by("lookup_key");
    outer.add_order_by(super::super::query::OrderClause::desc("last_seen_ms"));
    outer.add_order_by(super::super::query::OrderClause::desc("event_count"));

    let rows = fetch_all::<AuditSummaryRow>(outer.build(client)).await?;
    Ok(rows
        .into_iter()
        .map(|row| PaymentAuditSummary {
            lookup_key: row.lookup_key,
            payment_id: row.payment_id,
            request_id: row.request_id,
            merchant_id: row.merchant_id,
            first_seen_ms: row.first_seen_ms,
            last_seen_ms: row.last_seen_ms,
            event_count: row.event_count as usize,
            latest_status: row.latest_status,
            latest_gateway: row.latest_gateway,
            latest_stage: row.latest_stage.map(payment_audit_stage_label),
            gateways: row.gateways,
            routes: row
                .routes
                .into_iter()
                .map(payment_audit_route_label)
                .collect(),
        })
        .collect())
}
