use clickhouse::{query::Query, Row};
use serde::Deserialize;

use crate::analytics::flow::{AnalyticsRoute, FlowType};
use crate::analytics::models::{AnalyticsRouteHit, AnalyticsRoutingKind, PaymentAuditScope};
use crate::error::ApiError;

pub const DOMAIN_TABLE: &str = "analytics_domain_events";
pub const PAYMENT_AUDIT_SUMMARY_BUCKET_TABLE: &str = "analytics_payment_audit_summary_buckets";
pub const PAYMENT_AUDIT_LOOKUP_SUMMARY_TABLE: &str = "analytics_payment_audit_lookup_summaries";
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
    FlowType::RoutingHybridRequestHit,
];
pub const ROUTE_HIT_ENTRY_POINT_FLOW_TYPES: &[FlowType] = &[
    FlowType::DecideGatewayRequestHit,
    FlowType::RoutingEvaluateRequestHit,
    FlowType::RoutingHybridRequestHit,
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
    FlowType::RoutingHybridDecision,
    FlowType::RoutingHybridError,
];
pub const PAYMENT_AUDIT_HYBRID_FLOW_TYPES: &[FlowType] = &[
    FlowType::RoutingHybridDecision,
    FlowType::RoutingHybridError,
];
pub const PAYMENT_AUDIT_MULTI_OBJECTIVE_FLOW_TYPES: &[FlowType] = &[
    FlowType::DecideGatewayDecision,
    FlowType::UpdateGatewayScoreUpdate,
    FlowType::UpdateScoreLegacyScoreSnapshot,
    FlowType::DecideGatewayRuleHit,
    FlowType::DecideGatewayError,
    FlowType::UpdateGatewayScoreError,
    FlowType::UpdateScoreLegacyError,
];
pub const PAYMENT_AUDIT_ALL_FLOW_TYPES: &[FlowType] = &[
    FlowType::RoutingEvaluateSingle,
    FlowType::RoutingEvaluatePriority,
    FlowType::RoutingEvaluateVolumeSplit,
    FlowType::RoutingEvaluateAdvanced,
    FlowType::RoutingEvaluatePreview,
    FlowType::RoutingEvaluateError,
    FlowType::DecideGatewayDecision,
    FlowType::UpdateGatewayScoreUpdate,
    FlowType::UpdateScoreLegacyScoreSnapshot,
    FlowType::DecideGatewayRuleHit,
    FlowType::DecideGatewayError,
    FlowType::UpdateGatewayScoreError,
    FlowType::UpdateScoreLegacyError,
    FlowType::RoutingHybridDecision,
    FlowType::RoutingHybridError,
];

pub const fn payment_audit_flow_types(scope: PaymentAuditScope) -> &'static [FlowType] {
    match scope {
        PaymentAuditScope::All => PAYMENT_AUDIT_ALL_FLOW_TYPES,
        PaymentAuditScope::Dynamic => PAYMENT_AUDIT_DYNAMIC_FLOW_TYPES,
        PaymentAuditScope::Preview => PAYMENT_AUDIT_PREVIEW_FLOW_TYPES,
    }
}

/// Payment amount on a decide event, inside the `details` JSON. Shared by every metric that
/// sums volume so the request shape has one place to move.
pub const PAYMENT_AMOUNT_EXPR: &str =
    "JSONExtractFloat(assumeNotNull(details), 'request', 'paymentInfo', 'amount')";

/// The same amount on a hybrid decision event, where the decider request is nested under the
/// hybrid envelope instead of sitting at the root of `details.request`.
pub const HYBRID_PAYMENT_AMOUNT_EXPR: &str =
    "JSONExtractFloat(assumeNotNull(details), 'request', 'dynamic_routing_request', 'paymentInfo', 'amount')";

pub async fn fetch_all<T>(query: Query) -> Result<Vec<T>, ApiError>
where
    T: Row + for<'de> Deserialize<'de>,
{
    query.fetch_all::<T>().await.map_err(|error| {
        crate::logger::error!(?error, "clickhouse fetch_all failed");
        ApiError::DatabaseError
    })
}

pub async fn fetch_one<T>(query: Query) -> Result<T, ApiError>
where
    T: Row + for<'de> Deserialize<'de>,
{
    query.fetch_one::<T>().await.map_err(|error| {
        crate::logger::error!(?error, "clickhouse fetch_one failed");
        ApiError::DatabaseError
    })
}

pub fn static_flow_type_in_sql(flow_types: &[FlowType]) -> String {
    format!("({})", static_flow_type_list_sql(flow_types))
}

pub fn static_flow_type_array_sql(flow_types: &[FlowType]) -> String {
    format!("[{}]", static_flow_type_list_sql(flow_types))
}

fn static_flow_type_list_sql(flow_types: &[FlowType]) -> String {
    flow_types
        .iter()
        .map(|flow_type| format!("'{}'", flow_type.as_str()))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Roll request-hit rows up into the fixed card set the overview renders.
///
/// Keyed by `flow_type`, never by `route`: hybrid request hits were recorded under the
/// `decide_gateway` route before they got their own, so grouping by route would fold historical
/// hybrid traffic into the decide-gateway card and split the hybrid card at the cutover date.
pub fn ordered_route_hits_from_counts(
    flow_type_counts: impl IntoIterator<Item = (String, i64)>,
) -> Vec<AnalyticsRouteHit> {
    let mut counts = std::collections::HashMap::new();
    for (flow_type, count) in flow_type_counts {
        counts.insert(flow_type, count);
    }

    ROUTE_HIT_FLOW_TYPES
        .iter()
        .map(|flow_type| {
            let route = route_for_request_hit(*flow_type);
            AnalyticsRouteHit {
                route: route.overview_label().unwrap_or(route.as_str()).to_string(),
                count: counts.get(flow_type.as_str()).copied().unwrap_or(0),
            }
        })
        .collect()
}

const fn route_for_request_hit(flow_type: FlowType) -> AnalyticsRoute {
    match flow_type {
        FlowType::UpdateGatewayScoreRequestHit => AnalyticsRoute::UpdateGatewayScore,
        FlowType::RoutingEvaluateRequestHit => AnalyticsRoute::RoutingEvaluate,
        FlowType::RoutingHybridRequestHit => AnalyticsRoute::RoutingHybrid,
        _ => AnalyticsRoute::DecideGateway,
    }
}

/// The flow type and `details` JSON paths a decision-based metric needs for one routing kind.
///
/// A hybrid call runs the same decider as `/decide_gateway`, so its decision event carries the
/// same typed columns (`gateway`, `routing_approach`, dimensions). Only the flow type and the
/// depth of the decider payload inside `details` differ, and both live here.
#[derive(Debug, Clone, Copy)]
pub struct DecisionShape {
    pub decision_flow_type: FlowType,
    pub request_hit_flow_type: FlowType,
    pub error_flow_type: FlowType,
    /// Predicate for "the rule step decided this". `/decide_gateway` records a dedicated
    /// rule-hit event; hybrid records no separate event, so its static half is identified by the
    /// routing approach on the decision row itself.
    pub rule_hit_predicate: &'static str,
    pub amount_expr: &'static str,
    pub currency_expr: &'static str,
    pub cost_saved_bps_expr: &'static str,
    pub multi_objective_outcome_expr: &'static str,
}

const MULTI_OBJECTIVE_SHAPE: DecisionShape = DecisionShape {
    decision_flow_type: FlowType::DecideGatewayDecision,
    request_hit_flow_type: FlowType::DecideGatewayRequestHit,
    error_flow_type: FlowType::DecideGatewayError,
    rule_hit_predicate: "flow_type = 'decide_gateway_rule_hit'",
    amount_expr: PAYMENT_AMOUNT_EXPR,
    currency_expr: "JSONExtractString(assumeNotNull(details), 'request', 'paymentInfo', 'currency')",
    cost_saved_bps_expr:
        "JSONExtractFloat(assumeNotNull(details), 'response', 'multi_objective_info', 'costSavedBps')",
    multi_objective_outcome_expr:
        "JSONExtractString(assumeNotNull(details), 'response', 'multi_objective_info', 'outcome')",
};

const HYBRID_SHAPE: DecisionShape = DecisionShape {
    decision_flow_type: FlowType::RoutingHybridDecision,
    request_hit_flow_type: FlowType::RoutingHybridRequestHit,
    error_flow_type: FlowType::RoutingHybridError,
    rule_hit_predicate:
        "flow_type = 'routing_hybrid_decision' AND routing_approach = 'STATIC_ROUTING'",
    amount_expr: HYBRID_PAYMENT_AMOUNT_EXPR,
    currency_expr: "JSONExtractString(assumeNotNull(details), 'request', 'dynamic_routing_request', 'paymentInfo', 'currency')",
    cost_saved_bps_expr:
        "JSONExtractFloat(assumeNotNull(details), 'response', 'dynamic_routing', 'decision', 'multi_objective_info', 'costSavedBps')",
    multi_objective_outcome_expr:
        "JSONExtractString(assumeNotNull(details), 'response', 'dynamic_routing', 'decision', 'multi_objective_info', 'outcome')",
};

pub const fn decision_shape(kind: AnalyticsRoutingKind) -> DecisionShape {
    match kind {
        AnalyticsRoutingKind::MultiObjective => MULTI_OBJECTIVE_SHAPE,
        AnalyticsRoutingKind::Hybrid => HYBRID_SHAPE,
    }
}

impl DecisionShape {
    pub fn decision_filter(&self) -> super::query::FilterClause {
        super::query::FilterClause::raw(format!(
            "flow_type = '{}'",
            self.decision_flow_type.as_str()
        ))
    }
}

pub fn payment_audit_stage_label(stage: String) -> String {
    match stage.as_str() {
        "gateway_decided" => "Decide Gateway".to_string(),
        "hybrid_routed" => "Hybrid Routing".to_string(),
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