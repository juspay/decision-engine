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
    merchant_id: Option<String>,
    payment_method_type: Option<String>,
    payment_method: Option<String>,
    gateway: Option<String>,
    score_value: Option<f64>,
    sigma_factor: Option<f64>,
    average_latency: Option<f64>,
    tp99_latency: Option<f64>,
    transaction_count: Option<i64>,
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
        "merchant_id".to_string(),
        "payment_method_type".to_string(),
        "payment_method".to_string(),
        "gateway".to_string(),
        "argMax(score_value, created_at_ms) AS score_value".to_string(),
        "argMax(sigma_factor, created_at_ms) AS sigma_factor".to_string(),
        "argMax(average_latency, created_at_ms) AS average_latency".to_string(),
        "argMax(tp99_latency, created_at_ms) AS tp99_latency".to_string(),
        "argMax(transaction_count, created_at_ms) AS transaction_count".to_string(),
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
