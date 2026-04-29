use clickhouse::Row;
use serde::Deserialize;

use crate::analytics::flow::FlowType;
use crate::analytics::models::{AnalyticsGatewaySharePoint, AnalyticsQuery};
use crate::error::ApiError;

use super::super::common::{fetch_all, DOMAIN_TABLE};
use super::super::filters::{analytics_dimension_filters, base_window_filters, merchant_filter};
use super::super::query::{BoundQueryBuilder, FilterClause, OrderClause};
use super::super::time::{effective_window_bounds, query_bucket_select_expr};

#[derive(Debug, Clone, Deserialize, Row)]
struct GatewaySharePointRow {
    bucket_ms: i64,
    gateway: Option<String>,
    count: u64,
}

pub async fn load(
    client: &clickhouse::Client,
    query: &AnalyticsQuery,
) -> Result<Vec<AnalyticsGatewaySharePoint>, ApiError> {
    let (start_ms, end_ms) = effective_window_bounds(query);
    let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);
    builder.extend_selects([
        query_bucket_select_expr(query, start_ms, end_ms),
        "gateway".to_string(),
        "count() AS count".to_string(),
    ]);
    builder.extend_filters(base_window_filters(start_ms, end_ms));
    builder.extend_filters(merchant_filter(&query.merchant_id));
    builder.extend_filters(analytics_dimension_filters(query));
    builder.add_filter(FilterClause::raw(format!(
        "flow_type = '{}'",
        FlowType::DecideGatewayDecision.as_str()
    )));
    builder.extend_group_bys(["bucket_ms", "gateway"]);
    builder.add_order_by(OrderClause::asc("bucket_ms"));
    builder.add_order_by(OrderClause::asc("gateway"));

    let rows = fetch_all::<GatewaySharePointRow>(builder.build(client)).await?;
    Ok(rows
        .into_iter()
        .map(|row| AnalyticsGatewaySharePoint {
            bucket_ms: row.bucket_ms,
            gateway: row.gateway,
            count: row.count as i64,
        })
        .collect())
}
