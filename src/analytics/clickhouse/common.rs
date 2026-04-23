use clickhouse::{query::Query, Row};
use serde::Deserialize;

use crate::analytics::flow::{AnalyticsRoute, FlowType};
use crate::analytics::models::AnalyticsRouteHit;
use crate::error::ApiError;

pub const DOMAIN_TABLE: &str = "analytics_domain_events";
pub const PAYMENT_AUDIT_SUMMARY_BUCKET_TABLE: &str = "analytics_payment_audit_summary_buckets";
pub const PAYMENT_AUDIT_SUMMARY_KIND_DYNAMIC: &str = "dynamic";
pub const PAYMENT_AUDIT_SUMMARY_KIND_PREVIEW: &str = "preview";
pub const OVERVIEW_SCORE_FLOW_TYPES: &[FlowType] = &[
    FlowType::UpdateGatewayScoreScoreSnapshot,
    FlowType::UpdateScoreLegacyScoreSnapshot,
];
pub const OVERVIEW_ERROR_FLOW_TYPES: &[FlowType] = &[
    FlowType::DecideGatewayError,
    FlowType::UpdateGatewayScoreError,
    FlowType::UpdateScoreLegacyError,
    FlowType::RoutingEvaluateError,
];
pub const ROUTE_HIT_FLOW_TYPES: &[FlowType] = &[
    FlowType::DecideGatewayRequestHit,
    FlowType::UpdateGatewayScoreRequestHit,
    FlowType::RoutingEvaluateRequestHit,
];
pub const PAYMENT_AUDIT_PREVIEW_FLOW_TYPES: &[FlowType] = &[
    FlowType::RoutingEvaluateSingle,
    FlowType::RoutingEvaluatePriority,
    FlowType::RoutingEvaluateVolumeSplit,
    FlowType::RoutingEvaluateAdvanced,
    FlowType::RoutingEvaluatePreview,
    FlowType::RoutingEvaluateError,
];
pub const PAYMENT_AUDIT_DYNAMIC_FLOW_TYPES: &[FlowType] = &[
    FlowType::DecideGatewayDecision,
    FlowType::UpdateGatewayScoreUpdate,
    FlowType::UpdateScoreLegacyScoreSnapshot,
    FlowType::DecideGatewayRuleHit,
    FlowType::DecideGatewayError,
    FlowType::UpdateGatewayScoreError,
    FlowType::UpdateScoreLegacyError,
];

pub async fn fetch_all<T>(query: Query) -> Result<Vec<T>, ApiError>
where
    T: Row + for<'de> Deserialize<'de>,
{
    query
        .fetch_all::<T>()
        .await
        .map_err(|_| ApiError::DatabaseError)
}

pub async fn fetch_one<T>(query: Query) -> Result<T, ApiError>
where
    T: Row + for<'de> Deserialize<'de>,
{
    query
        .fetch_one::<T>()
        .await
        .map_err(|_| ApiError::DatabaseError)
}

pub fn static_flow_type_in_sql(flow_types: &[FlowType]) -> String {
    format!(
        "({})",
        flow_types
            .iter()
            .map(|flow_type| format!("'{}'", flow_type.as_str()))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

pub fn ordered_route_hits_from_counts(
    route_counts: impl IntoIterator<Item = (Option<String>, i64)>,
) -> Vec<AnalyticsRouteHit> {
    let mut counts = std::collections::HashMap::new();
    for (route, count) in route_counts {
        counts.insert(route.unwrap_or_else(|| "unknown".to_string()), count);
    }

    [
        AnalyticsRoute::DecideGateway,
        AnalyticsRoute::UpdateGatewayScore,
        AnalyticsRoute::RoutingEvaluate,
    ]
    .into_iter()
    .map(|route| AnalyticsRouteHit {
        route: route.overview_label().unwrap_or(route.as_str()).to_string(),
        count: counts.get(route.as_str()).copied().unwrap_or(0),
    })
    .collect()
}

pub fn payment_audit_stage_label(stage: String) -> String {
    match stage.as_str() {
        "gateway_decided" => "Decide Gateway".to_string(),
        "score_updated" => "Update Gateway".to_string(),
        "rule_applied" => "Rule Evaluate".to_string(),
        "preview_evaluated" => "Preview Result".to_string(),
        other => other.to_string(),
    }
}

pub fn payment_audit_route_label(route: String) -> String {
    AnalyticsRoute::from_stored_value(&route)
        .map(|route| route.payment_audit_label().to_string())
        .unwrap_or(route)
}

pub const fn payment_audit_summary_kind(preview_only: bool) -> &'static str {
    if preview_only {
        PAYMENT_AUDIT_SUMMARY_KIND_PREVIEW
    } else {
        PAYMENT_AUDIT_SUMMARY_KIND_DYNAMIC
    }
}
