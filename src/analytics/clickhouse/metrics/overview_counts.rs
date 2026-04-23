use clickhouse::Row;
use serde::Deserialize;

use crate::analytics::flow::FlowType;
use crate::analytics::models::AnalyticsQuery;
use crate::error::ApiError;

use super::super::common::{
    fetch_one, static_flow_type_in_sql, DOMAIN_TABLE, OVERVIEW_ERROR_FLOW_TYPES,
    OVERVIEW_SCORE_FLOW_TYPES,
};
use super::super::filters::{base_window_filters, merchant_filter};
use super::super::query::BoundQueryBuilder;
use super::super::time::effective_window_bounds;

#[derive(Debug, Clone, Deserialize, Row)]
struct OverviewCountRow {
    total: u64,
    score_count: u64,
    rule_hit_count: u64,
    error_count: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct OverviewCounts {
    pub total: u64,
    pub score_count: u64,
    pub rule_hit_count: u64,
    pub error_count: u64,
}

pub async fn load(
    client: &clickhouse::Client,
    query: &AnalyticsQuery,
) -> Result<OverviewCounts, ApiError> {
    let (start_ms, end_ms) = effective_window_bounds(query);
    let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);
    builder.extend_selects([
        format!(
            "countIf(flow_type = '{}') AS total",
            FlowType::DecideGatewayDecision.as_str()
        ),
        format!(
            "countIf(flow_type IN {}) AS score_count",
            static_flow_type_in_sql(OVERVIEW_SCORE_FLOW_TYPES)
        ),
        format!(
            "countIf(flow_type = '{}') AS rule_hit_count",
            FlowType::DecideGatewayRuleHit.as_str()
        ),
        format!(
            "countIf(flow_type IN {}) AS error_count",
            static_flow_type_in_sql(OVERVIEW_ERROR_FLOW_TYPES)
        ),
    ]);
    builder.extend_filters(base_window_filters(start_ms, end_ms));
    builder.extend_filters(merchant_filter(query.merchant_id.as_deref()));

    let row = fetch_one::<OverviewCountRow>(builder.build(client)).await?;
    Ok(OverviewCounts {
        total: row.total,
        score_count: row.score_count,
        rule_hit_count: row.rule_hit_count,
        error_count: row.error_count,
    })
}
