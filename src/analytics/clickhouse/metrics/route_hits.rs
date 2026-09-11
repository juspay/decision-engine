use clickhouse::Row;
use serde::Deserialize;

use crate::analytics::flow::FlowType;
use crate::analytics::models::{AnalyticsQuery, AnalyticsRouteHit};
use crate::error::ApiError;

use super::super::common::{
    decision_shape, fetch_all, fetch_one, ordered_route_hits_from_counts, static_flow_type_in_sql,
    DOMAIN_TABLE, ROUTE_HIT_ENTRY_POINT_FLOW_TYPES,
};
use super::super::filters::{base_window_filters, merchant_filter};
use super::super::query::{BoundQueryBuilder, FilterClause};
use super::super::time::effective_window_bounds;

#[derive(Debug, Clone, Deserialize, Row)]
struct RouteHitRow {
    flow_type: String,
    count: u64,
}

#[derive(Debug, Clone, Deserialize, Row)]
struct ScoreFeedbackRow {
    count: u64,
}

pub async fn load(
    client: &clickhouse::Client,
    query: &AnalyticsQuery,
) -> Result<Vec<AnalyticsRouteHit>, ApiError> {
    let (start_ms, end_ms) = effective_window_bounds(query);
    let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);
    builder.extend_selects(["flow_type", "count() AS count"]);
    builder.extend_filters(base_window_filters(start_ms, end_ms));
    builder.extend_filters(merchant_filter(&query.merchant_id));
    builder.add_filter(FilterClause::raw(format!(
        "flow_type IN {}",
        static_flow_type_in_sql(ROUTE_HIT_ENTRY_POINT_FLOW_TYPES)
    )));
    builder.add_group_by("flow_type");

    let rows = fetch_all::<RouteHitRow>(builder.build(client)).await?;
    let score_feedback = load_score_feedback(client, query, start_ms, end_ms).await?;

    Ok(ordered_route_hits_from_counts(
        rows.into_iter()
            .map(|row| (row.flow_type, row.count as i64))
            .chain(std::iter::once((
                FlowType::UpdateGatewayScoreRequestHit.as_str().to_string(),
                score_feedback,
            ))),
    ))
}

/// Score feedback for the payments *this routing kind* decided.
///
/// `/update-gateway-score` is shared: a merchant's hybrid and decide-gateway traffic both report
/// outcomes through it, so counting the endpoint's hits would show the same merchant-wide total on
/// every tab. Each event carries the `payment_id` it scores, which is the only thing that ties it
/// back to the decision that chose the gateway.
async fn load_score_feedback(
    client: &clickhouse::Client,
    query: &AnalyticsQuery,
    start_ms: i64,
    end_ms: i64,
) -> Result<i64, ApiError> {
    let builder = score_feedback_builder(query, start_ms, end_ms);
    let row = fetch_one::<ScoreFeedbackRow>(builder.build(client)).await?;
    Ok(row.count as i64)
}

fn score_feedback_builder(
    query: &AnalyticsQuery,
    start_ms: i64,
    end_ms: i64,
) -> BoundQueryBuilder {
    let decision_flow_type = decision_shape(query.routing_kind).decision_flow_type;

    let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);
    builder.extend_selects(["count() AS count"]);
    builder.extend_filters(base_window_filters(start_ms, end_ms));
    builder.extend_filters(merchant_filter(&query.merchant_id));
    builder.add_filter(FilterClause::raw(format!(
        "flow_type = '{}'",
        FlowType::UpdateGatewayScoreRequestHit.as_str()
    )));
    // Binds are emitted in the order filters are added, so this clause's three placeholders
    // follow the window and merchant ones above.
    builder.add_filter(FilterClause::new(
        format!(
            "payment_id IN (SELECT payment_id FROM {DOMAIN_TABLE} \
             WHERE created_at_ms >= ? AND created_at_ms <= ? AND merchant_id = ? \
             AND flow_type = '{}')",
            decision_flow_type.as_str()
        ),
        vec![
            start_ms.into(),
            end_ms.into(),
            query.merchant_id.clone().into(),
        ],
    ));

    builder
}
