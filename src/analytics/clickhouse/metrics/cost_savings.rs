use clickhouse::Row;
use serde::Deserialize;

use crate::analytics::models::{
    AnalyticsAvailableCurrency, AnalyticsCostSavingsTotals, AnalyticsCostSavingsTrendPoint,
    AnalyticsQuery,
};
use crate::error::ApiError;

use super::super::common::{decision_shape, fetch_all, fetch_one, DOMAIN_TABLE};
use super::super::filters::{analytics_dimension_filters, base_window_filters, merchant_filter};
use super::super::query::{BoundQueryBuilder, FilterClause, OrderClause};
use super::super::time::{effective_window_bounds, query_bucket_select_expr};

// The cost/amount/currency expressions all read the decider payload out of `details`, which sits
// one level deeper on a hybrid event than on a decide-gateway one, so they come from the routing
// kind's `DecisionShape` rather than module constants. Currency in particular is JSON-extracted
// because the top-level `currency` column is NULL for decider events.

#[derive(Debug, Deserialize, Row)]
struct AvailableCurrencyRow {
    // JSONExtractString returns non-nullable String (empty when key absent).
    currency: String,
    decision_count: u64,
}

/// Currencies seen in the window for decisions that ran multi-objective and had a positive saving.
/// Used to populate the chart's currency dropdown and pick a default ("top currency").
///
/// We intentionally bypass `analytics_dimension_filters` here — the dropdown should list every
/// available currency regardless of what's already selected.
pub async fn load_available_currencies(
    client: &clickhouse::Client,
    query: &AnalyticsQuery,
) -> Result<Vec<AnalyticsAvailableCurrency>, ApiError> {
    let (start_ms, end_ms) = effective_window_bounds(query);
    let shape = decision_shape(query.routing_kind);
    let currency_expr = shape.currency_expr;
    let outcome_expr = shape.multi_objective_outcome_expr;
    let bps_expr = shape.cost_saved_bps_expr;
    let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);
    builder.extend_selects([
        format!("{currency_expr} AS currency"),
        "count() AS decision_count".to_string(),
    ]);
    builder.extend_filters(base_window_filters(start_ms, end_ms));
    builder.extend_filters(merchant_filter(&query.merchant_id));
    builder.add_filter(shape.decision_filter());
    builder.add_filter(FilterClause::raw(format!("{outcome_expr} = 'COST_WON'")));
    builder.add_filter(FilterClause::raw(format!("{bps_expr} > 0")));
    builder.extend_group_bys(["currency"]);
    builder.add_order_by(OrderClause::desc("decision_count"));
    builder.set_limit(Some(20));

    let rows = fetch_all::<AvailableCurrencyRow>(builder.build(client)).await?;
    Ok(rows
        .into_iter()
        .filter(|row| !row.currency.is_empty())
        .map(|row| AnalyticsAvailableCurrency {
            currency: row.currency,
            decision_count: row.decision_count,
        })
        .collect())
}

#[derive(Debug, Deserialize, Row)]
struct TrendRow {
    bucket_ms: i64,
    saved_value: f64,
}

/// Time-bucketed cost savings in the active currency: sum of (cost_saved_bps / 10000) * amount.
///
/// Currency is JSON-extracted (not filtered through `query.currency` / dimension filters)
/// because the top-level `currency` column is NULL for /decide-gateway events.
pub async fn load_cost_savings_trend(
    client: &clickhouse::Client,
    query: &AnalyticsQuery,
    currency: &str,
) -> Result<Vec<AnalyticsCostSavingsTrendPoint>, ApiError> {
    let (start_ms, end_ms) = effective_window_bounds(query);
    let shape = decision_shape(query.routing_kind);
    let currency_expr = shape.currency_expr;
    let outcome_expr = shape.multi_objective_outcome_expr;
    let bps_expr = shape.cost_saved_bps_expr;
    let amount_expr = shape.amount_expr;
    let mut scoped = query.clone();
    scoped.currency = None;
    let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);
    builder.extend_selects([
        query_bucket_select_expr(query, start_ms, end_ms),
        format!("sum(({bps_expr} / 10000.0) * {amount_expr}) AS saved_value"),
    ]);
    builder.extend_filters(base_window_filters(start_ms, end_ms));
    builder.extend_filters(merchant_filter(&query.merchant_id));
    builder.extend_filters(analytics_dimension_filters(&scoped));
    builder.add_filter(shape.decision_filter());
    builder.add_filter(FilterClause::raw(format!("{outcome_expr} = 'COST_WON'")));
    builder.add_filter(FilterClause::raw(format!("{bps_expr} > 0")));
    builder.add_filter(FilterClause::new(
        format!("{currency_expr} = ?"),
        vec![currency.to_string().into()],
    ));
    builder.extend_group_bys(["bucket_ms"]);
    builder.add_order_by(OrderClause::asc("bucket_ms"));

    let rows = fetch_all::<TrendRow>(builder.build(client)).await?;
    Ok(rows
        .into_iter()
        .map(|row| AnalyticsCostSavingsTrendPoint {
            bucket_ms: row.bucket_ms,
            saved_value: row.saved_value,
        })
        .collect())
}

#[derive(Debug, Deserialize, Row)]
struct TotalsRow {
    saved_value: f64,
    cost_won_count: u64,
    total_decisions: u64,
}

/// Aggregate totals for the KPI tile.
/// `total_decisions` counts every multi-objective decision (where `cost_saved_bps` was computed),
/// `cost_won_count` is the subset where the cheaper PSP was picked.
pub async fn load_cost_savings_totals(
    client: &clickhouse::Client,
    query: &AnalyticsQuery,
    currency: &str,
) -> Result<AnalyticsCostSavingsTotals, ApiError> {
    let (start_ms, end_ms) = effective_window_bounds(query);
    let shape = decision_shape(query.routing_kind);
    let currency_expr = shape.currency_expr;
    let outcome_expr = shape.multi_objective_outcome_expr;
    let bps_expr = shape.cost_saved_bps_expr;
    let amount_expr = shape.amount_expr;
    let mut scoped = query.clone();
    scoped.currency = None;
    let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);
    builder.extend_selects([
        format!(
            "sumIf(({bps_expr} / 10000.0) * {amount_expr}, {outcome_expr} = 'COST_WON') AS saved_value"
        ),
        format!("countIf({outcome_expr} = 'COST_WON') AS cost_won_count"),
        format!("countIf({outcome_expr} != '') AS total_decisions"),
    ]);
    builder.extend_filters(base_window_filters(start_ms, end_ms));
    builder.extend_filters(merchant_filter(&query.merchant_id));
    builder.extend_filters(analytics_dimension_filters(&scoped));
    builder.add_filter(shape.decision_filter());
    builder.add_filter(FilterClause::new(
        format!("{currency_expr} = ?"),
        vec![currency.to_string().into()],
    ));

    let row = fetch_one::<TotalsRow>(builder.build(client)).await?;
    Ok(AnalyticsCostSavingsTotals {
        saved_value: row.saved_value,
        cost_won_count: row.cost_won_count,
        total_decisions: row.total_decisions,
    })
}
