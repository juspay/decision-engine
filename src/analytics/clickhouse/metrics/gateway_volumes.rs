use clickhouse::Row;
use serde::Deserialize;

use crate::analytics::models::{AnalyticsGatewayVolume, AnalyticsQuery};
use crate::error::ApiError;

use super::super::common::{all_decisions_filter, fetch_all, DOMAIN_TABLE};
use super::super::filters::{analytics_dimension_filters, base_window_filters, merchant_filter};
use super::super::query::{BoundQueryBuilder, OrderClause};
use super::super::time::effective_window_bounds;

#[derive(Debug, Clone, Deserialize, Row)]
struct GatewayVolumeRow {
    gateway: Option<String>,
    count: u64,
}

/// Decisions per gateway over the whole window, under either routing kind.
///
/// `gateway_share` answers the same question for one routing kind and buckets by time for a chart;
/// the overview has no routing-kind tab and no chart, so it counts both kinds and skips the buckets.
pub async fn load(
    client: &clickhouse::Client,
    query: &AnalyticsQuery,
) -> Result<Vec<AnalyticsGatewayVolume>, ApiError> {
    let (start_ms, end_ms) = effective_window_bounds(query);
    let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);
    builder.extend_selects(["gateway", "count() AS count"]);
    builder.extend_filters(base_window_filters(start_ms, end_ms));
    builder.extend_filters(merchant_filter(&query.merchant_id));
    builder.extend_filters(analytics_dimension_filters(query));
    builder.add_filter(all_decisions_filter());
    builder.add_group_by("gateway");
    builder.add_order_by(OrderClause::desc("count"));

    let rows = fetch_all::<GatewayVolumeRow>(builder.build(client)).await?;
    Ok(rows
        .into_iter()
        .filter_map(|row| {
            row.gateway
                .filter(|gateway| !gateway.is_empty())
                .map(|gateway| AnalyticsGatewayVolume {
                    gateway,
                    count: row.count as i64,
                })
        })
        .collect())
}
