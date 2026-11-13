use clickhouse::Row;
use serde::Deserialize;

use crate::analytics::models::{AnalyticsQuery, GatewayScoreSeriesPoint};
use crate::error::ApiError;

use super::super::common::{
    fetch_all, static_flow_type_in_sql, DOMAIN_TABLE, OVERVIEW_SCORE_FLOW_TYPES,
};
use super::super::filters::score_filters;
use super::super::query::{BoundQueryBuilder, FilterClause, OrderClause};
use super::super::time::{effective_window_bounds, query_bucket_select_expr};

#[derive(Debug, Clone, Deserialize, Row)]
struct ScoreSeriesRow {
    bucket_ms: i64,
    merchant_id: Option<String>,
    payment_method_type: Option<String>,
    payment_method: Option<String>,
    gateway: Option<String>,
    score_value: Option<f64>,
}

pub async fn load(
    client: &clickhouse::Client,
    query: &AnalyticsQuery,
) -> Result<Vec<GatewayScoreSeriesPoint>, ApiError> {
    let (start_ms, end_ms) = effective_window_bounds(query);
    let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);
    builder.extend_selects([
        query_bucket_select_expr(query, start_ms, end_ms),
        "merchant_id".to_string(),
        "payment_method_type".to_string(),
        "payment_method".to_string(),
        "gateway".to_string(),
        "avg(score_value) AS score_value".to_string(),
    ]);
    builder.extend_filters(score_filters(query, start_ms, end_ms));
    builder.add_filter(FilterClause::raw(format!(
        "flow_type IN {}",
        static_flow_type_in_sql(OVERVIEW_SCORE_FLOW_TYPES)
    )));
    builder.extend_group_bys([
        "bucket_ms",
        "merchant_id",
        "payment_method_type",
        "payment_method",
        "gateway",
    ]);
    builder.add_order_by(OrderClause::asc("bucket_ms"));
    builder.add_order_by(OrderClause::asc("gateway"));

    let rows = fetch_all::<ScoreSeriesRow>(builder.build(client)).await?;
    Ok(rows
        .into_iter()
        .map(|row| GatewayScoreSeriesPoint {
            bucket_ms: row.bucket_ms,
            merchant_id: row.merchant_id,
            payment_method_type: row.payment_method_type,
            payment_method: row.payment_method,
            gateway: row.gateway,
            score_value: row.score_value,
        })
        .collect())
}
