use crate::analytics::models::{AnalyticsGatewayScoresResponse, AnalyticsQuery, AnalyticsScope};
use crate::analytics::service::format_range;
use crate::error::ApiError;

use super::super::metrics;

pub async fn load(
    client: &clickhouse::Client,
    query: &AnalyticsQuery,
) -> Result<AnalyticsGatewayScoresResponse, ApiError> {
    if query.scope == AnalyticsScope::All {
        return Ok(AnalyticsGatewayScoresResponse {
            scope: query.scope.as_str().to_string(),
            merchant_id: query.merchant_id.clone(),
            range: format_range(query),
            snapshots: Vec::new(),
            series: Vec::new(),
        });
    }

    Ok(AnalyticsGatewayScoresResponse {
        scope: query.scope.as_str().to_string(),
        merchant_id: query.merchant_id.clone(),
        range: format_range(query),
        snapshots: metrics::score_snapshots::load(client, query, None).await?,
        series: metrics::score_series::load(client, query).await?,
    })
}
