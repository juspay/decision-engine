use clickhouse::Row;
use serde::Deserialize;

use crate::analytics::flow::FlowType;
use crate::analytics::models::{
    AnalyticsHybridConnectorPick, AnalyticsHybridSplit, AnalyticsQuery,
};
use crate::error::ApiError;

use super::super::common::{fetch_all, fetch_one, static_flow_type_in_sql, DOMAIN_TABLE};
use super::super::filters::{base_window_filters, merchant_filter};
use super::super::query::{BoundQueryBuilder, FilterClause, OrderClause};
use super::super::time::effective_window_bounds;

/// Both halves of a hybrid call are summarised on its single decision event: `dynamic_status`
/// says whether the SR decider answered, and `static_connectors` lists what the rule step chose.
const DYNAMIC_STATUS_EXPR: &str =
    "JSONExtractString(assumeNotNull(details), 'selection_reason', 'dynamic_status')";
const STATIC_CONNECTORS_EXPR: &str =
    "JSONExtractString(assumeNotNull(details), 'selection_reason', 'static_connectors')";
const HYBRID_EVENT_FLOW_TYPES: &[FlowType] = &[
    FlowType::RoutingHybridDecision,
    FlowType::RoutingHybridError,
];
const STATIC_ROUTING_APPROACH: &str = "STATIC_ROUTING";

#[derive(Debug, Clone, Deserialize, Row)]
struct SplitRow {
    decisions: u64,
    dynamic_success: u64,
    dynamic_fallback: u64,
    dynamic_skipped: u64,
    static_decided: u64,
    failed: u64,
}

#[derive(Debug, Clone, Deserialize, Row)]
struct ConnectorPickRow {
    connector: String,
    count: u64,
}

/// How hybrid calls resolved: the dynamic decider answering, falling back to the static
/// connectors, or being skipped entirely. This is the rule-based half of the tab — the same call
/// whose chosen connector the multi-objective charts plot.
pub async fn load(
    client: &clickhouse::Client,
    query: &AnalyticsQuery,
) -> Result<AnalyticsHybridSplit, ApiError> {
    let (start_ms, end_ms) = effective_window_bounds(query);
    let decision = FlowType::RoutingHybridDecision.as_str();
    let error = FlowType::RoutingHybridError.as_str();

    let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);
    builder.extend_selects([
        format!("countIf(flow_type = '{decision}') AS decisions"),
        format!(
            "countIf(flow_type = '{decision}' AND {DYNAMIC_STATUS_EXPR} = 'success') AS dynamic_success"
        ),
        format!(
            "countIf(flow_type = '{decision}' AND {DYNAMIC_STATUS_EXPR} = 'fallback') AS dynamic_fallback"
        ),
        format!(
            "countIf(flow_type = '{decision}' AND {DYNAMIC_STATUS_EXPR} = 'skipped') AS dynamic_skipped"
        ),
        format!(
            "countIf(flow_type = '{decision}' AND routing_approach = '{STATIC_ROUTING_APPROACH}') AS static_decided"
        ),
        format!("countIf(flow_type = '{error}') AS failed"),
    ]);
    builder.extend_filters(base_window_filters(start_ms, end_ms));
    builder.extend_filters(merchant_filter(&query.merchant_id));
    builder.add_filter(FilterClause::raw(format!(
        "flow_type IN {}",
        static_flow_type_in_sql(HYBRID_EVENT_FLOW_TYPES)
    )));

    let row = fetch_one::<SplitRow>(builder.build(client)).await?;
    let static_connectors = load_static_connectors(client, query).await?;

    Ok(AnalyticsHybridSplit {
        decisions: row.decisions as i64,
        dynamic_success: row.dynamic_success as i64,
        dynamic_fallback: row.dynamic_fallback as i64,
        dynamic_skipped: row.dynamic_skipped as i64,
        static_decided: row.static_decided as i64,
        failed: row.failed as i64,
        static_connectors,
    })
}

/// The connectors the static rule step shortlisted, one row per connector. `static_connectors`
/// is stored comma-joined (a scalar, so the audit page can render it as a plain value), so it is
/// split back out here.
async fn load_static_connectors(
    client: &clickhouse::Client,
    query: &AnalyticsQuery,
) -> Result<Vec<AnalyticsHybridConnectorPick>, ApiError> {
    let (start_ms, end_ms) = effective_window_bounds(query);
    let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);
    builder.extend_selects([
        format!(
            "arrayJoin(arrayFilter(candidate -> candidate != '', splitByString(', ', {STATIC_CONNECTORS_EXPR}))) AS connector"
        ),
        "count() AS count".to_string(),
    ]);
    builder.extend_filters(base_window_filters(start_ms, end_ms));
    builder.extend_filters(merchant_filter(&query.merchant_id));
    builder.add_filter(FilterClause::raw(format!(
        "flow_type = '{}'",
        FlowType::RoutingHybridDecision.as_str()
    )));
    builder.add_group_by("connector");
    builder.add_order_by(OrderClause::desc("count"));
    builder.add_order_by(OrderClause::asc("connector"));
    builder.set_limit(Some(10));

    let rows = fetch_all::<ConnectorPickRow>(builder.build(client)).await?;
    Ok(rows
        .into_iter()
        .map(|row| AnalyticsHybridConnectorPick {
            connector: row.connector,
            count: row.count as i64,
        })
        .collect())
}
