use crate::analytics::models::{
    AnalyticsKpi, AnalyticsOverviewResponse, AnalyticsOverviewTotals, AnalyticsQuery,
    AnalyticsRoutingKind, SmartRetryStats,
};
use crate::analytics::service::format_range;
use crate::error::ApiError;
use futures::future::try_join;

use super::super::metrics;
use super::super::metrics::overview_counts::OverviewCounts;

const OVERVIEW_SCORE_SNAPSHOT_LIMIT: usize = 100;

impl OverviewCounts {
    fn into_kpis(
        self,
        query: &AnalyticsQuery,
        totals: &AnalyticsOverviewTotals,
    ) -> Vec<AnalyticsKpi> {
        vec![
            AnalyticsKpi {
                label: format!("Requests / {}", format_range(query)),
                value: totals.request_count.to_string(),
                subtitle: Some("Across every routing entry point".to_string()),
            },
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
                value: totals.error_count.to_string(),
                subtitle: Some("Structured failure summaries".to_string()),
            },
        ]
    }
}

pub async fn load(
    client: &clickhouse::Client,
    query: &AnalyticsQuery,
) -> Result<AnalyticsOverviewResponse, ApiError> {
    let (
        counts,
        route_hits,
        top_scores,
        auth_rate,
        top_errors,
        error_total,
        endpoint_totals,
        merchant_wide,
        top_rules,
        by_trigger,
        by_fallback,
    ) = tokio::join!(
        metrics::overview_counts::load(client, query),
        metrics::route_hits::load(client, query),
        metrics::score_snapshots::load(client, query, Some(OVERVIEW_SCORE_SNAPSHOT_LIMIT)),
        metrics::auth_rate::load(client, query),
        metrics::error_summaries::load(client, query, Some(5)),
        metrics::error_summaries::load_total(client, query),
        metrics::route_hits::load_endpoint_totals(client, query),
        try_join(
            metrics::auth_rate::load_all(client, query),
            metrics::gateway_volumes::load(client, query),
        ),
        metrics::rule_hits::load(client, query, Some(5)),
        metrics::smart_retry_stats::load_by_trigger(client, query),
        metrics::smart_retry_stats::load_by_fallback(client, query),
    );
    let counts = counts?;
    let route_hits = route_hits?;

    let hybrid_split = match query.routing_kind {
        AnalyticsRoutingKind::Hybrid => Some(metrics::hybrid_split::load(client, query).await?),
        AnalyticsRoutingKind::MultiObjective => None,
    };

    let requests_by_route = endpoint_totals?;
    let (all_auth_rate, gateway_volumes) = merchant_wide?;
    let totals = AnalyticsOverviewTotals {
        request_count: requests_by_route.iter().map(|hit| hit.count).sum(),
        error_count: error_total?,
        auth_rate: all_auth_rate,
        gateway_volumes,
        requests_by_route,
    };

    Ok(AnalyticsOverviewResponse {
        merchant_id: query.merchant_id.clone(),
        kpis: counts.into_kpis(query, &totals),
        totals,
        route_hits,
        top_scores: top_scores?,
        auth_rate: auth_rate?,
        top_errors: top_errors?,
        top_rules: top_rules?,
        smart_retry_stats: SmartRetryStats {
            retried_count: counts.smart_retry_count,
            recovered_count: counts.smart_retry_recovered_count,
            by_trigger: by_trigger.unwrap_or_default(),
            by_fallback: by_fallback.unwrap_or_default(),
        },
        hybrid_split,
    })
}
