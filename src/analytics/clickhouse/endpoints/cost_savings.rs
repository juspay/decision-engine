use crate::analytics::models::{
    AnalyticsCostSavingsResponse, AnalyticsCostSavingsTotals, AnalyticsQuery,
};
use crate::analytics::service::format_range;
use crate::error::ApiError;

use super::super::metrics;

pub async fn load(
    client: &clickhouse::Client,
    query: &AnalyticsQuery,
) -> Result<AnalyticsCostSavingsResponse, ApiError> {
    let available_currencies =
        metrics::cost_savings::load_available_currencies(client, query).await?;

    // Active currency: caller-provided wins; otherwise pick the one with the most decisions.
    let active_currency = query
        .currency
        .clone()
        .or_else(|| available_currencies.first().map(|c| c.currency.clone()));

    let (trend, totals) = match active_currency.as_deref() {
        Some(currency) => {
            let trend =
                metrics::cost_savings::load_cost_savings_trend(client, query, currency).await?;
            let totals =
                metrics::cost_savings::load_cost_savings_totals(client, query, currency).await?;
            (trend, totals)
        }
        None => (
            Vec::new(),
            AnalyticsCostSavingsTotals {
                saved_value: 0.0,
                cost_won_count: 0,
                total_decisions: 0,
            },
        ),
    };

    Ok(AnalyticsCostSavingsResponse {
        merchant_id: query.merchant_id.clone(),
        range: format_range(query),
        currency: active_currency,
        available_currencies,
        trend,
        totals,
    })
}
