use crate::analytics::models::{
    AnalyticsKpi, AnalyticsOverviewResponse, AnalyticsQuery, AnalyticsScope,
};
use crate::analytics::service::{format_range, now_ms};
use crate::error::ApiError;

use super::super::metrics;

pub async fn load(
    client: &clickhouse::Client,
    query: &AnalyticsQuery,
) -> Result<AnalyticsOverviewResponse, ApiError> {
    if query.scope == AnalyticsScope::All {
        return Ok(AnalyticsOverviewResponse {
            generated_at_ms: now_ms(),
            scope: query.scope.as_str().to_string(),
            merchant_id: query.merchant_id.clone(),
            kpis: vec![
                AnalyticsKpi {
                    label: format!("Decisions / {}", format_range(query)),
                    value: "0".to_string(),
                    subtitle: Some(
                        "Global mode is limited to connector-level analytics".to_string(),
                    ),
                },
                AnalyticsKpi {
                    label: "Score snapshots".to_string(),
                    value: "0".to_string(),
                    subtitle: Some("Global mode hides merchant-specific data".to_string()),
                },
                AnalyticsKpi {
                    label: "Rule hits".to_string(),
                    value: "0".to_string(),
                    subtitle: Some("Global mode hides merchant-specific data".to_string()),
                },
                AnalyticsKpi {
                    label: "Errors".to_string(),
                    value: "0".to_string(),
                    subtitle: Some("Global mode hides merchant-specific data".to_string()),
                },
            ],
            route_hits: vec![
                crate::analytics::models::AnalyticsRouteHit {
                    route: "/decide_gateway".to_string(),
                    count: 0,
                },
                crate::analytics::models::AnalyticsRouteHit {
                    route: "/update_gateway".to_string(),
                    count: 0,
                },
                crate::analytics::models::AnalyticsRouteHit {
                    route: "/rule_evaluate".to_string(),
                    count: 0,
                },
            ],
            top_scores: Vec::new(),
            top_errors: Vec::new(),
            top_rules: Vec::new(),
        });
    }

    let counts = metrics::overview_counts::load(client, query).await?;
    let route_hits = metrics::route_hits::load(client, query).await?;
    let top_scores = metrics::score_snapshots::load(client, query, Some(5)).await?;
    let top_errors = metrics::error_summaries::load(client, query, Some(5)).await?;
    let top_rules = metrics::rule_hits::load(client, query, Some(5)).await?;

    Ok(AnalyticsOverviewResponse {
        generated_at_ms: now_ms(),
        scope: query.scope.as_str().to_string(),
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
