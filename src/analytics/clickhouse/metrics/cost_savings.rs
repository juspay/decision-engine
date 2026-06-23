use clickhouse::Row;
use serde::Deserialize;

use crate::analytics::flow::FlowType;
use crate::analytics::models::{
    AnalyticsAvailableCurrency, AnalyticsCostSavingsTotals, AnalyticsCostSavingsTrendPoint,
    AnalyticsQuery,
};
use crate::error::ApiError;

use super::super::common::{fetch_all, fetch_one, DOMAIN_TABLE};
use super::super::filters::{analytics_dimension_filters, base_window_filters, merchant_filter};
use super::super::query::{BoundQueryBuilder, FilterClause, OrderClause};
use super::super::time::{effective_window_bounds, query_bucket_select_expr};

const COST_SAVED_BPS_EXPR: &str =
    "JSONExtractFloat(assumeNotNull(details), 'response', 'multi_objective_info', 'costSavedBps')";
const MO_OUTCOME_EXPR: &str =
    "JSONExtractString(assumeNotNull(details), 'response', 'multi_objective_info', 'outcome')";
const PAYMENT_AMOUNT_EXPR: &str =
    "JSONExtractFloat(assumeNotNull(details), 'request', 'paymentInfo', 'amount')";
// Currency lives in the request JSON, not the top-level `currency` column (which is NULL for
// /decide-gateway events).
const PAYMENT_CURRENCY_EXPR: &str =
    "JSONExtractString(assumeNotNull(details), 'request', 'paymentInfo', 'currency')";

fn decision_flow_filter() -> FilterClause {
    FilterClause::raw(format!(
        "flow_type = '{}'",
        FlowType::DecideGatewayDecision.as_str()
    ))
}

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
    let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);
    builder.extend_selects([
        format!("{PAYMENT_CURRENCY_EXPR} AS currency"),
        "count() AS decision_count".to_string(),
    ]);
    builder.extend_filters(base_window_filters(start_ms, end_ms));
    builder.extend_filters(merchant_filter(&query.merchant_id));
    builder.add_filter(decision_flow_filter());
    builder.add_filter(FilterClause::raw(format!("{MO_OUTCOME_EXPR} = 'COST_WON'")));
    builder.add_filter(FilterClause::raw(format!("{COST_SAVED_BPS_EXPR} > 0")));
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
    let mut scoped = query.clone();
    scoped.currency = None;
    let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);
    builder.extend_selects([
        query_bucket_select_expr(query, start_ms, end_ms),
        format!("sum(({COST_SAVED_BPS_EXPR} / 10000.0) * {PAYMENT_AMOUNT_EXPR}) AS saved_value"),
    ]);
    builder.extend_filters(base_window_filters(start_ms, end_ms));
    builder.extend_filters(merchant_filter(&query.merchant_id));
    builder.extend_filters(analytics_dimension_filters(&scoped));
    builder.add_filter(decision_flow_filter());
    builder.add_filter(FilterClause::raw(format!("{MO_OUTCOME_EXPR} = 'COST_WON'")));
    builder.add_filter(FilterClause::raw(format!("{COST_SAVED_BPS_EXPR} > 0")));
    builder.add_filter(FilterClause::new(
        format!("{PAYMENT_CURRENCY_EXPR} = ?"),
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
    let mut scoped = query.clone();
    scoped.currency = None;
    let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);
    builder.extend_selects([
        format!(
            "sumIf(({COST_SAVED_BPS_EXPR} / 10000.0) * {PAYMENT_AMOUNT_EXPR}, {MO_OUTCOME_EXPR} = 'COST_WON') AS saved_value"
        ),
        format!("countIf({MO_OUTCOME_EXPR} = 'COST_WON') AS cost_won_count"),
        format!("countIf({MO_OUTCOME_EXPR} != '') AS total_decisions"),
    ]);
    builder.extend_filters(base_window_filters(start_ms, end_ms));
    builder.extend_filters(merchant_filter(&query.merchant_id));
    builder.extend_filters(analytics_dimension_filters(&scoped));
    builder.add_filter(decision_flow_filter());
    builder.add_filter(FilterClause::new(
        format!("{PAYMENT_CURRENCY_EXPR} = ?"),
        vec![currency.to_string().into()],
    ));

    let row = fetch_one::<TotalsRow>(builder.build(client)).await?;
    Ok(AnalyticsCostSavingsTotals {
        saved_value: row.saved_value,
        cost_won_count: row.cost_won_count,
        total_decisions: row.total_decisions,
    })
}
