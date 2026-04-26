use clickhouse::Row;
use serde::Deserialize;

use crate::analytics::models::{AnalyticsQuery, AnalyticsRouteHit};
use crate::error::ApiError;

use super::super::common::{
    fetch_all, ordered_route_hits_from_counts, static_flow_type_in_sql, DOMAIN_TABLE,
    ROUTE_HIT_FLOW_TYPES,
};
use super::super::filters::{base_window_filters, merchant_filter};
use super::super::query::BoundQueryBuilder;
use super::super::time::effective_window_bounds;

#[derive(Debug, Clone, Deserialize, Row)]
struct RouteHitRow {
    route: Option<String>,
    count: u64,
}

pub async fn load(
    client: &clickhouse::Client,
    query: &AnalyticsQuery,
) -> Result<Vec<AnalyticsRouteHit>, ApiError> {
    let (start_ms, end_ms) = effective_window_bounds(query);
    let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);
    builder.extend_selects(["route", "count() AS count"]);
    builder.extend_filters(base_window_filters(start_ms, end_ms));
    builder.extend_filters(merchant_filter(&query.merchant_id));
    builder.add_filter(super::super::query::FilterClause::raw(format!(
        "flow_type IN {}",
        static_flow_type_in_sql(ROUTE_HIT_FLOW_TYPES)
    )));
    builder.add_group_by("route");

    let rows = fetch_all::<RouteHitRow>(builder.build(client)).await?;
    Ok(ordered_route_hits_from_counts(
        rows.into_iter().map(|row| (row.route, row.count as i64)),
    ))
}
