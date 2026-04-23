use clickhouse::Row;
use serde::Deserialize;

use crate::analytics::models::{AnalyticsQuery, GatewayScoreSnapshot};
use crate::error::ApiError;

use super::super::common::{
    fetch_all, static_flow_type_in_sql, DOMAIN_TABLE, OVERVIEW_SCORE_FLOW_TYPES,
};
use super::super::filters::score_filters;
use super::super::query::{BoundQueryBuilder, FilterClause, OrderClause};
use super::super::time::effective_window_bounds;

#[derive(Debug, Clone, Deserialize, Row)]
struct ScoreSnapshotRow {
    merchant_id: String,
    payment_method_type: String,
    payment_method: String,
    gateway: String,
    score_value: f64,
    sigma_factor: f64,
    average_latency: f64,
    tp99_latency: f64,
    transaction_count: i64,
    last_updated_ms: i64,
}

pub async fn load(
    client: &clickhouse::Client,
    query: &AnalyticsQuery,
    limit: Option<usize>,
) -> Result<Vec<GatewayScoreSnapshot>, ApiError> {
    let (start_ms, end_ms) = effective_window_bounds(query);
    let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);
    builder.extend_selects([
        "ifNull(merchant_id, '') AS merchant_id".to_string(),
        "ifNull(payment_method_type, '') AS payment_method_type".to_string(),
        "ifNull(payment_method, '') AS payment_method".to_string(),
        "ifNull(gateway, '') AS gateway".to_string(),
        "ifNull(argMax(score_value, created_at_ms), 0.0) AS score_value".to_string(),
        "ifNull(argMax(sigma_factor, created_at_ms), 0.0) AS sigma_factor".to_string(),
        "ifNull(argMax(average_latency, created_at_ms), 0.0) AS average_latency".to_string(),
        "ifNull(argMax(tp99_latency, created_at_ms), 0.0) AS tp99_latency".to_string(),
        "ifNull(argMax(transaction_count, created_at_ms), 0) AS transaction_count".to_string(),
        "max(created_at_ms) AS last_updated_ms".to_string(),
    ]);
    builder.extend_filters(score_filters(query, start_ms, end_ms));
    builder.add_filter(FilterClause::raw(format!(
        "flow_type IN {}",
        static_flow_type_in_sql(OVERVIEW_SCORE_FLOW_TYPES)
    )));
    builder.extend_group_bys([
        "merchant_id",
        "payment_method_type",
        "payment_method",
        "gateway",
    ]);
    builder.add_order_by(OrderClause::desc("score_value"));
    builder.add_order_by(OrderClause::desc("last_updated_ms"));
    builder.set_limit(limit.map(|value| value as u64));

    let rows = fetch_all::<ScoreSnapshotRow>(builder.build(client)).await?;
    Ok(rows
        .into_iter()
        .map(|row| GatewayScoreSnapshot {
            merchant_id: row.merchant_id,
            payment_method_type: row.payment_method_type,
            payment_method: row.payment_method,
            gateway: row.gateway,
            score_value: row.score_value,
            sigma_factor: row.sigma_factor,
            average_latency: row.average_latency,
            tp99_latency: row.tp99_latency,
            transaction_count: row.transaction_count,
            last_updated_ms: row.last_updated_ms,
        })
        .collect())
}
