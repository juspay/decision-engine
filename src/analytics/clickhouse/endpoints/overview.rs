use crate::analytics::models::{AnalyticsKpi, AnalyticsOverviewResponse, AnalyticsQuery};
use crate::analytics::service::format_range;
use crate::error::ApiError;

use super::super::metrics;

pub async fn load(
    client: &clickhouse::Client,
    query: &AnalyticsQuery,
) -> Result<AnalyticsOverviewResponse, ApiError> {
    let counts = metrics::overview_counts::load(client, query).await?;
    let route_hits = metrics::route_hits::load(client, query).await?;
    let top_scores = metrics::score_snapshots::load(client, query, Some(5)).await?;
    let top_errors = metrics::error_summaries::load(client, query, Some(5)).await?;
    let top_rules = metrics::rule_hits::load(client, query, Some(5)).await?;

    Ok(AnalyticsOverviewResponse {
        merchant_id: query.merchant_id.clone(),
        kpis: vec![
            AnalyticsKpi {
                label: format!("Decisions / {}", format_range(query)),
                value: counts.total.to_string(),
                subtitle: Some("Recorded decision events".to_string()),
            },
            AnalyticsKpi {
                label: "Score snapshots".to_string(),
                value: counts.score_count.to_string(),
                subtitle: Some("Latest gateway score updates".to_string()),
            },
            AnalyticsKpi {
                label: "Rule hits".to_string(),
                value: counts.rule_hit_count.to_string(),
                subtitle: Some("Priority-logic hits".to_string()),
            },
            AnalyticsKpi {
                label: "Errors".to_string(),
                value: counts.error_count.to_string(),
                subtitle: Some("Structured failure summaries".to_string()),
            },
        ],
        route_hits,
        top_scores,
        top_errors,
        top_rules,
    })
}
