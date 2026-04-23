use crate::analytics::models::{AnalyticsGatewayScoresResponse, AnalyticsQuery};
use crate::analytics::service::format_range;
use crate::error::ApiError;

use super::super::metrics;

pub async fn load(
    client: &clickhouse::Client,
    query: &AnalyticsQuery,
) -> Result<AnalyticsGatewayScoresResponse, ApiError> {
    Ok(AnalyticsGatewayScoresResponse {
        merchant_id: query.merchant_id.clone(),
        range: format_range(query),
        snapshots: metrics::score_snapshots::load(client, query, None).await?,
        series: metrics::score_series::load(client, query).await?,
    })
}
