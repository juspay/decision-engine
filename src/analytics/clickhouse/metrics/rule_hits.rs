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
struct RuleHitRow {
    rule_name: Option<String>,
    count: u64,
}

pub async fn load(
    client: &clickhouse::Client,
    query: &AnalyticsQuery,
    limit: Option<usize>,
) -> Result<Vec<AnalyticsRuleHit>, ApiError> {
    let (start_ms, end_ms) = effective_window_bounds(query);
    let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);
    builder.extend_selects(["rule_name".to_string(), "count() AS count".to_string()]);
    builder.extend_filters(base_window_filters(start_ms, end_ms));
    builder.extend_filters(merchant_filter(&query.merchant_id));
    builder.add_filter(FilterClause::raw(format!(
        "flow_type = '{}'",
        FlowType::DecideGatewayRuleHit.as_str()
    )));
    builder.add_group_by("rule_name");
    builder.add_order_by(OrderClause::desc("count"));
    builder.add_order_by(OrderClause::asc("rule_name"));
    builder.set_limit(limit.map(|value| value as u64));

    let rows = fetch_all::<RuleHitRow>(builder.build(client)).await?;
    Ok(rows
        .into_iter()
        .map(|row| AnalyticsRuleHit {
            rule_name: row.rule_name,
            count: row.count as i64,
        })
        .collect())
}
