use clickhouse::Row;
use serde::Deserialize;

use crate::analytics::models::{AnalyticsErrorSummary, AnalyticsQuery};
use crate::error::ApiError;

use super::super::common::{
    fetch_all, static_flow_type_in_sql, DOMAIN_TABLE, OVERVIEW_ERROR_FLOW_TYPES,
};
use super::super::filters::{analytics_dimension_filters, base_window_filters, merchant_filter};
use super::super::query::{BoundQueryBuilder, FilterClause, OrderClause};
use super::super::time::effective_window_bounds;

#[derive(Debug, Clone, Deserialize, Row)]
struct ErrorSummaryRow {
    route: Option<String>,
    error_code: Option<String>,
    error_message: Option<String>,
    count: u64,
    last_seen_ms: i64,
}

pub async fn load(
    client: &clickhouse::Client,
    query: &AnalyticsQuery,
    limit: Option<usize>,
) -> Result<Vec<AnalyticsErrorSummary>, ApiError> {
    let (start_ms, end_ms) = effective_window_bounds(query);
    let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);
    builder.extend_selects([
        "route".to_string(),
        "error_code".to_string(),
        "error_message".to_string(),
        "count() AS count".to_string(),
        "max(created_at_ms) AS last_seen_ms".to_string(),
    ]);
    builder.extend_filters(base_window_filters(start_ms, end_ms));
    builder.extend_filters(merchant_filter(&query.merchant_id));
    builder.extend_filters(analytics_dimension_filters(query));
    builder.add_filter(FilterClause::raw(format!(
        "flow_type IN {}",
        static_flow_type_in_sql(OVERVIEW_ERROR_FLOW_TYPES)
    )));
    builder.extend_group_bys(["route", "error_code", "error_message"]);
    builder.add_order_by(OrderClause::desc("count"));
    builder.add_order_by(OrderClause::desc("last_seen_ms"));
    builder.set_limit(limit.map(|value| value as u64));

    let rows = fetch_all::<ErrorSummaryRow>(builder.build(client)).await?;
    Ok(rows
        .into_iter()
        .map(|row| AnalyticsErrorSummary {
            route: row.route,
            error_code: row.error_code,
            error_message: row.error_message,
            count: row.count as i64,
            last_seen_ms: row.last_seen_ms,
        })
        .collect())
}
