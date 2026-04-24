use crate::analytics::models::{AnalyticsDecisionResponse, AnalyticsKpi, AnalyticsQuery};
use crate::analytics::service::format_range;
use crate::error::ApiError;

use super::super::metrics;

pub async fn load(
    client: &clickhouse::Client,
    query: &AnalyticsQuery,
) -> Result<AnalyticsDecisionResponse, ApiError> {
    let counts = metrics::decision_tiles::load(client, query).await?;
    let series = metrics::decision_series::load(client, query).await?;
    let approaches = metrics::decision_approaches::load(client, query).await?;

    let error_rate = if counts.total > 0 {
        (counts.failures as f64 / counts.total as f64) * 100.0
    } else {
        0.0
    };

    Ok(AnalyticsDecisionResponse {
        merchant_id: query.merchant_id.clone(),
        range: format_range(query),
        tiles: vec![
            AnalyticsKpi {
                label: "Decisions".to_string(),
                value: counts.total.to_string(),
                subtitle: Some(format!("Failures: {}", counts.failures)),
            },
            AnalyticsKpi {
                label: "Error rate".to_string(),
                value: format!("{error_rate:.2}%"),
                subtitle: Some("From recorded decision events".to_string()),
            },
        ],
        series,
        approaches,
    })
}
