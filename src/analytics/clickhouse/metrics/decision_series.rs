use clickhouse::Row;
use serde::Deserialize;

use crate::analytics::flow::FlowType;
use crate::analytics::models::{AnalyticsDecisionPoint, AnalyticsQuery};
use crate::error::ApiError;

use super::super::common::{fetch_all, DOMAIN_TABLE};
use super::super::filters::{base_window_filters, merchant_filter};
use super::super::query::{BoundQueryBuilder, FilterClause, OrderClause};
use super::super::time::{effective_window_bounds, query_bucket_select_expr};

#[derive(Debug, Clone, Deserialize, Row)]
struct DecisionPointRow {
    bucket_ms: i64,
    routing_approach: Option<String>,
    count: u64,
}

pub async fn load(
    client: &clickhouse::Client,
    query: &AnalyticsQuery,
) -> Result<Vec<AnalyticsDecisionPoint>, ApiError> {
    let (start_ms, end_ms) = effective_window_bounds(query);
    let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);
    builder.extend_selects([
        query_bucket_select_expr(query, start_ms, end_ms),
        "routing_approach".to_string(),
        "count() AS count".to_string(),
    ]);
    builder.extend_filters(base_window_filters(start_ms, end_ms));
    builder.extend_filters(merchant_filter(&query.merchant_id));
    builder.add_filter(FilterClause::raw(format!(
        "flow_type = '{}'",
        FlowType::DecideGatewayDecision.as_str()
    )));
    builder.extend_group_bys(["bucket_ms", "routing_approach"]);
    builder.add_order_by(OrderClause::asc("bucket_ms"));
    builder.add_order_by(OrderClause::asc("routing_approach"));

    let rows = fetch_all::<DecisionPointRow>(builder.build(client)).await?;
    Ok(rows
        .into_iter()
        .map(|row| AnalyticsDecisionPoint {
            bucket_ms: row.bucket_ms,
            routing_approach: row.routing_approach,
            count: row.count as i64,
        })
        .collect())
}
