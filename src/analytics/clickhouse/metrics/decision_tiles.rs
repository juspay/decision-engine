use clickhouse::Row;
use serde::Deserialize;

use crate::analytics::flow::FlowType;
use crate::analytics::models::AnalyticsQuery;
use crate::error::ApiError;

use super::super::common::{fetch_one, DOMAIN_TABLE};
use super::super::filters::{base_window_filters, merchant_filter};
use super::super::query::{BoundQueryBuilder, FilterClause};
use super::super::time::effective_window_bounds;

#[derive(Debug, Clone, Deserialize, Row)]
struct CountTileRow {
    total: u64,
    failures: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct DecisionTileSummary {
    pub total: u64,
    pub failures: u64,
}

pub async fn load(
    client: &clickhouse::Client,
    query: &AnalyticsQuery,
) -> Result<DecisionTileSummary, ApiError> {
    let (start_ms, end_ms) = effective_window_bounds(query);
    let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);
    builder.extend_selects([
        "count() AS total",
        "countIf(lowerUTF8(ifNull(status, '')) = 'failure') AS failures",
    ]);
    builder.extend_filters(base_window_filters(start_ms, end_ms));
    builder.extend_filters(merchant_filter(query.merchant_id.as_deref()));
    builder.add_filter(FilterClause::raw(format!(
        "flow_type = '{}'",
        FlowType::DecideGatewayDecision.as_str()
    )));

    let row = fetch_one::<CountTileRow>(builder.build(client)).await?;
    Ok(DecisionTileSummary {
        total: row.total,
        failures: row.failures,
    })
}
