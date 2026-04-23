use crate::analytics::models::{AnalyticsLogSummariesResponse, AnalyticsQuery, AnalyticsScope};
use crate::analytics::service::format_range;
use crate::error::ApiError;

use super::super::metrics;

pub async fn load(
    client: &clickhouse::Client,
    query: &AnalyticsQuery,
) -> Result<AnalyticsLogSummariesResponse, ApiError> {
    if query.scope == AnalyticsScope::All {
        return Ok(AnalyticsLogSummariesResponse {
            scope: query.scope.as_str().to_string(),
            merchant_id: query.merchant_id.clone(),
            range: format_range(query),
            total_errors: 0,
            errors: Vec::new(),
            samples: Vec::new(),
            page: query.page,
            page_size: query.page_size,
        });
    }

    let errors = metrics::error_summaries::load(client, query, Some(10)).await?;
    let total_errors = errors.iter().map(|entry| entry.count).sum();
    let samples = metrics::log_samples::load(client, query).await?;

    Ok(AnalyticsLogSummariesResponse {
        scope: query.scope.as_str().to_string(),
        merchant_id: query.merchant_id.clone(),
        range: format_range(query),
        total_errors,
        errors,
        samples,
        page: query.page,
        page_size: query.page_size,
    })
}
