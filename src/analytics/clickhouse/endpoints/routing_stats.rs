use crate::analytics::models::{AnalyticsQuery, AnalyticsRoutingStatsResponse};
use crate::analytics::service::format_range;
use crate::error::ApiError;

use super::super::metrics;

pub async fn load(
    client: &clickhouse::Client,
    query: &AnalyticsQuery,
) -> Result<AnalyticsRoutingStatsResponse, ApiError> {
    Ok(AnalyticsRoutingStatsResponse {
        scope: query.scope.as_str().to_string(),
        merchant_id: query.merchant_id.clone(),
        range: format_range(query),
        gateway_share: metrics::gateway_share::load(client, query).await?,
        top_rules: metrics::rule_hits::load(client, query, Some(10)).await?,
        sr_trend: metrics::score_series::load(client, query).await?,
        available_filters: metrics::filter_options::load(client, query).await?,
    })
}
