use clickhouse::Row;
use serde::Deserialize;

use crate::analytics::flow::FlowType;
use crate::analytics::models::{AnalyticsQuery, AnalyticsRuleHit};
use crate::error::ApiError;

use super::super::common::{fetch_all, DOMAIN_TABLE};
use super::super::filters::{base_window_filters, merchant_filter};
use super::super::query::{BoundQueryBuilder, FilterClause, OrderClause};
use super::super::time::effective_window_bounds;

#[derive(Debug, Clone, Deserialize, Row)]
struct ApproachRow {
    rule_name: String,
    count: u64,
}

pub async fn load(
    client: &clickhouse::Client,
    query: &AnalyticsQuery,
) -> Result<Vec<AnalyticsRuleHit>, ApiError> {
    let (start_ms, end_ms) = effective_window_bounds(query);
    let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);
    builder.extend_selects([
        "ifNull(routing_approach, 'UNKNOWN') AS rule_name",
        "count() AS count",
    ]);
    builder.extend_filters(base_window_filters(start_ms, end_ms));
    builder.extend_filters(merchant_filter(&query.merchant_id));
    builder.add_filter(FilterClause::raw(format!(
        "flow_type = '{}'",
        FlowType::DecideGatewayDecision.as_str()
    )));
    builder.add_group_by("rule_name");
    builder.add_order_by(OrderClause::desc("count"));
    builder.add_order_by(OrderClause::asc("rule_name"));

    let rows = fetch_all::<ApproachRow>(builder.build(client)).await?;
    Ok(rows
        .into_iter()
        .map(|row| AnalyticsRuleHit {
            rule_name: row.rule_name,
            count: row.count as i64,
        })
        .collect())
}
