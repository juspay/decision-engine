use clickhouse::Row;
use serde::Deserialize;

use crate::analytics::flow::FlowType;
use crate::analytics::models::{AnalyticsQuery, SmartRetryFallback, SmartRetryTrigger};
use crate::error::ApiError;

use super::super::common::{fetch_all, DOMAIN_TABLE};
use super::super::filters::{base_window_filters, merchant_filter};
use super::super::query::{BoundQueryBuilder, FilterClause, OrderClause};
use super::super::time::effective_window_bounds;

#[derive(Debug, Deserialize, Row)]
struct TriggerRow {
    // gateway is Nullable(String) in the table — GROUP BY preserves nullability
    gateway: Option<String>,
    // JSONExtractString returns non-nullable String (empty string when key absent)
    error_code: String,
    count: u64,
}

#[derive(Debug, Deserialize, Row)]
struct FallbackRow {
    // gateway is Nullable(String) in the table — GROUP BY preserves nullability
    gateway: Option<String>,
    retried: u64,
    recovered: u64,
}

/// Which gateway+error combinations triggered smart retries.
pub async fn load_by_trigger(
    client: &clickhouse::Client,
    query: &AnalyticsQuery,
) -> Result<Vec<SmartRetryTrigger>, ApiError> {
    let (start_ms, end_ms) = effective_window_bounds(query);
    let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);
    builder.extend_selects([
        "gateway".to_string(),
        "JSONExtractString(assumeNotNull(details), 'response', 'gsm_info', 'standardisedCode') AS error_code".to_string(),
        "count() AS count".to_string(),
    ]);
    builder.extend_filters(base_window_filters(start_ms, end_ms));
    builder.extend_filters(merchant_filter(&query.merchant_id));
    builder.add_filter(FilterClause::raw(format!(
        "flow_type = '{}'",
        FlowType::UpdateGatewayScoreUpdate.as_str()
    )));
    builder.add_filter(FilterClause::raw(
        "lowerUTF8(status) = 'failure'".to_string(),
    ));
    builder.add_filter(FilterClause::raw(
        "JSONExtractString(assumeNotNull(details), 'response', 'gsm_info', 'decision') = 'retry'".to_string(),
    ));
    builder.extend_group_bys(["gateway", "error_code"]);
    builder.add_order_by(OrderClause::desc("count"));
    builder.set_limit(Some(20));

    let rows = fetch_all::<TriggerRow>(builder.build(client)).await?;
    Ok(rows
        .into_iter()
        .filter_map(|row| {
            let gateway = row.gateway.filter(|g| !g.is_empty())?;
            Some(SmartRetryTrigger {
                gateway,
                error_code: if row.error_code.is_empty() { None } else { Some(row.error_code) },
                count: row.count,
            })
        })
        .collect())
}

/// Which fallback gateways were used for smart retries and their recovery rate.
pub async fn load_by_fallback(
    client: &clickhouse::Client,
    query: &AnalyticsQuery,
) -> Result<Vec<SmartRetryFallback>, ApiError> {
    let (start_ms, end_ms) = effective_window_bounds(query);
    let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);
    builder.extend_selects([
        "gateway".to_string(),
        "count() AS retried".to_string(),
        "countIf(lowerUTF8(status) = 'charged') AS recovered".to_string(),
    ]);
    builder.extend_filters(base_window_filters(start_ms, end_ms));
    builder.extend_filters(merchant_filter(&query.merchant_id));
    builder.add_filter(FilterClause::raw(format!(
        "flow_type = '{}'",
        FlowType::UpdateGatewayScoreUpdate.as_str()
    )));
    builder.add_filter(FilterClause::raw(
        "JSONExtractBool(assumeNotNull(details), 'request', 'is_smart_retry') = true".to_string(),
    ));
    builder.extend_group_bys(["gateway"]);
    builder.add_order_by(OrderClause::desc("retried"));
    builder.set_limit(Some(20));

    let rows = fetch_all::<FallbackRow>(builder.build(client)).await?;
    Ok(rows
        .into_iter()
        .filter_map(|row| {
            let gateway = row.gateway.filter(|g| !g.is_empty())?;
            Some(SmartRetryFallback {
                gateway,
                retried: row.retried,
                recovered: row.recovered,
            })
        })
        .collect())
}
