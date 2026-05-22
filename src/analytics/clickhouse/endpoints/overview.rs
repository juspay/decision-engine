use crate::analytics::models::{
    AnalyticsKpi, AnalyticsOverviewResponse, AnalyticsQuery, SmartRetryStats,
};
use crate::analytics::service::format_range;
use crate::error::ApiError;

use super::super::metrics;
use super::super::metrics::overview_counts::OverviewCounts;

impl OverviewCounts {
    fn into_kpis(self, query: &AnalyticsQuery) -> Vec<AnalyticsKpi> {
        vec![
            AnalyticsKpi {
                label: format!("Decision events / {}", format_range(query)),
                value: self.total.to_string(),
                subtitle: Some("Counts decision events, not unique payments".to_string()),
            },
            AnalyticsKpi {
                label: "Score snapshots".to_string(),
                value: self.score_count.to_string(),
                subtitle: Some("Latest gateway score updates".to_string()),
            },
            AnalyticsKpi {
                label: "Rule hits".to_string(),
                value: self.rule_hit_count.to_string(),
                subtitle: Some("Priority-logic hits".to_string()),
            },
            AnalyticsKpi {
                label: "Errors".to_string(),
                value: self.error_count.to_string(),
                subtitle: Some("Structured failure summaries".to_string()),
            },
        ]
    }
}

pub async fn load(
    client: &clickhouse::Client,
    query: &AnalyticsQuery,
) -> Result<AnalyticsOverviewResponse, ApiError> {
    let (counts, route_hits, top_scores, top_errors, top_rules, by_trigger, by_fallback) = tokio::join!(
        metrics::overview_counts::load(client, query),
        metrics::route_hits::load(client, query),
        metrics::score_snapshots::load(client, query, Some(5)),
        metrics::error_summaries::load(client, query, Some(5)),
        metrics::rule_hits::load(client, query, Some(5)),
        metrics::smart_retry_stats::load_by_trigger(client, query),
        metrics::smart_retry_stats::load_by_fallback(client, query),
    );
    let counts = counts?;

    Ok(AnalyticsOverviewResponse {
        merchant_id: query.merchant_id.clone(),
        kpis: counts.into_kpis(query),
        route_hits: route_hits?,
        top_scores: top_scores?,
        top_errors: top_errors?,
        top_rules: top_rules?,
        smart_retry_stats: SmartRetryStats {
            retried_count: counts.smart_retry_count,
            recovered_count: counts.smart_retry_recovered_count,
            by_trigger: by_trigger.unwrap_or_default(),
            by_fallback: by_fallback.unwrap_or_default(),
        },
    })
}
