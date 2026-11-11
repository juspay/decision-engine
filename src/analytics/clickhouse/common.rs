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
    FlowType::RoutingHybridError,
];
pub const ALL_DECISION_FLOW_TYPES: &[FlowType] = &[
    FlowType::DecideGatewayDecision,
    FlowType::RoutingHybridDecision,
];
pub const ROUTE_HIT_FLOW_TYPES: &[FlowType] = &[
    FlowType::DecideGatewayRequestHit,
    FlowType::UpdateGatewayScoreRequestHit,
    FlowType::RoutingEvaluateRequestHit,
    FlowType::RoutingHybridRequestHit,
];
/// The overview's request-hit cards, in the order they are rendered.
pub const ROUTE_HIT_ROUTES: &[AnalyticsRoute] = &[
    AnalyticsRoute::Decide,
    AnalyticsRoute::DecideGateway,
    AnalyticsRoute::UpdateGatewayScore,
    AnalyticsRoute::RoutingEvaluate,
    AnalyticsRoute::RoutingHybrid,
];
pub const ROUTE_HIT_ENTRY_POINT_FLOW_TYPES: &[FlowType] = &[
    FlowType::DecideGatewayRequestHit,
    FlowType::RoutingEvaluateRequestHit,
    FlowType::RoutingHybridRequestHit,
];
pub use crate::analytics::flow::{
    PAYMENT_AUDIT_HYBRID_FLOW_TYPES, PAYMENT_AUDIT_MULTI_OBJECTIVE_FLOW_TYPES,
    PAYMENT_AUDIT_PREVIEW_FLOW_TYPES,
};

pub const fn payment_audit_flow_type_groups(
    scope: PaymentAuditScope,
) -> &'static [&'static [FlowType]] {
    match scope {
        PaymentAuditScope::All => &[
            PAYMENT_AUDIT_PREVIEW_FLOW_TYPES,
            PAYMENT_AUDIT_MULTI_OBJECTIVE_FLOW_TYPES,
            PAYMENT_AUDIT_HYBRID_FLOW_TYPES,
        ],
        PaymentAuditScope::Dynamic => &[
            PAYMENT_AUDIT_MULTI_OBJECTIVE_FLOW_TYPES,
            PAYMENT_AUDIT_HYBRID_FLOW_TYPES,
        ],
        PaymentAuditScope::Preview => &[PAYMENT_AUDIT_PREVIEW_FLOW_TYPES],
    }
}

/// The routing approach a hybrid call records when its static half made the decision. Written by
/// `routes::hybrid_routing`, read back by the hybrid metrics.
pub const STATIC_ROUTING_APPROACH: &str = "STATIC_ROUTING";

/// Payment amount on a decide event, inside the `details` JSON. Shared by every metric that
/// sums volume so the request shape has one place to move.
pub const PAYMENT_AMOUNT_EXPR: &str =
    "JSONExtractFloat(assumeNotNull(details), 'request', 'paymentInfo', 'amount')";

/// The same amount on a hybrid decision event, where the decider request is nested under the
/// hybrid envelope instead of sitting at the root of `details.request`.
pub const HYBRID_PAYMENT_AMOUNT_EXPR: &str =
    "JSONExtractFloat(assumeNotNull(details), 'request', 'dynamic_routing_request', 'paymentInfo', 'amount')";

/// The same amount multiplied onto the caller's own scale, for callers that compare traffic
/// against figures held in a different unit. `scale` of `1.0` reads the amount as it arrives,
/// which is what every metric that only reports traffic back wants.
pub fn payment_amount_expr(scale: f64) -> String {
    if scale == 1.0 {
        PAYMENT_AMOUNT_EXPR.to_string()
    } else {
        format!("({PAYMENT_AMOUNT_EXPR} * {scale})")
    }
}

pub async fn fetch_all<T>(query: Query) -> Result<Vec<T>, ApiError>
where
    T: Row + for<'de> Deserialize<'de>,
{
    super::guard::run(query.fetch_all::<T>())
        .await?
        .map_err(|error| {
            crate::logger::error!(?error, "clickhouse fetch_all failed");
            ApiError::DatabaseError
        })
}

pub async fn fetch_one<T>(query: Query) -> Result<T, ApiError>
where
    T: Row + for<'de> Deserialize<'de>,
{
    super::guard::run(query.fetch_one::<T>())
        .await?
        .map_err(|error| {
            crate::logger::error!(?error, "clickhouse fetch_one failed");
            ApiError::DatabaseError
        })
}

pub fn static_flow_type_in_sql(flow_types: &[FlowType]) -> String {
    format!("({})", flow_type_list_sql(flow_types))
}

pub fn static_flow_type_array_sql(flow_types: &[FlowType]) -> String {
    format!("[{}]", flow_type_list_sql(flow_types))
}

/// The `IN (…)` list for a scope, over its groups in the order `payment_audit_flow_type_groups`
/// holds them.
pub fn payment_audit_flow_types_in_sql(scope: PaymentAuditScope) -> String {
    format!(
        "({})",
        flow_type_list_sql(
            payment_audit_flow_type_groups(scope)
                .iter()
                .copied()
                .flatten()
        )
    )
}

fn flow_type_list_sql<'a>(flow_types: impl IntoIterator<Item = &'a FlowType>) -> String {
    flow_types
        .into_iter()
        .map(|flow_type| format!("'{}'", flow_type.as_str()))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Roll request-hit rows up into the fixed card set the overview renders.
pub fn ordered_route_hits_from_counts(
    route_counts: impl IntoIterator<Item = (AnalyticsRoute, i64)>,
) -> Vec<AnalyticsRouteHit> {
    let mut counts: std::collections::HashMap<AnalyticsRoute, i64> =
        std::collections::HashMap::new();
    for (route, count) in route_counts {
        *counts.entry(route).or_default() += count;
    }

    ROUTE_HIT_ROUTES
        .iter()
        .map(|route| AnalyticsRouteHit {
            route: route.overview_label().unwrap_or(route.as_str()).to_string(),
            count: counts.get(route).copied().unwrap_or(0),
        })
        .collect()
}

/// The request-hit flow type a stored value names, when the overview counts that flow type.
pub(crate) fn request_hit_flow_type(value: &str) -> Option<FlowType> {
    ROUTE_HIT_FLOW_TYPES
        .iter()
        .copied()
        .find(|flow_type| flow_type.as_str() == value)
}

/// The card a request-hit row belongs to.
///
/// Only the hybrid flow type has two entry points, so only it reads the stored route.
pub(crate) fn route_for_request_hit(
    flow_type: FlowType,
    stored_route: Option<&str>,
) -> AnalyticsRoute {
    let default_route = match flow_type {
        FlowType::UpdateGatewayScoreRequestHit => AnalyticsRoute::UpdateGatewayScore,
        FlowType::RoutingEvaluateRequestHit => AnalyticsRoute::RoutingEvaluate,
        FlowType::RoutingHybridRequestHit => AnalyticsRoute::RoutingHybrid,
        _ => AnalyticsRoute::DecideGateway,
    };
    if !matches!(flow_type, FlowType::RoutingHybridRequestHit) {
        return default_route;
    }
    match stored_route.and_then(AnalyticsRoute::from_stored_value) {
        Some(route @ (AnalyticsRoute::Decide | AnalyticsRoute::RoutingHybrid)) => route,
        _ => default_route,
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
    pub rule_hit_predicate: &'static str,
    pub error_flow_types: &'static [FlowType],
    pub amount_expr: &'static str,
    pub currency_expr: &'static str,
    pub cost_saved_bps_expr: &'static str,
    pub multi_objective_outcome_expr: &'static str,
}

const MULTI_OBJECTIVE_ERROR_FLOW_TYPES: &[FlowType] = &[
    FlowType::DecideGatewayError,
    FlowType::UpdateGatewayScoreError,
    FlowType::UpdateScoreLegacyError,
];

const HYBRID_ERROR_FLOW_TYPES: &[FlowType] = &[
    FlowType::RoutingHybridError,
    FlowType::UpdateGatewayScoreError,
    FlowType::UpdateScoreLegacyError,
];

const MULTI_OBJECTIVE_SHAPE: DecisionShape = DecisionShape {
    decision_flow_type: FlowType::DecideGatewayDecision,
    rule_hit_predicate: "flow_type = 'decide_gateway_rule_hit'",
    error_flow_types: MULTI_OBJECTIVE_ERROR_FLOW_TYPES,
    amount_expr: PAYMENT_AMOUNT_EXPR,
    currency_expr: "JSONExtractString(assumeNotNull(details), 'request', 'paymentInfo', 'currency')",
    cost_saved_bps_expr:
        "JSONExtractFloat(assumeNotNull(details), 'response', 'multi_objective_info', 'costSavedBps')",
    multi_objective_outcome_expr:
        "JSONExtractString(assumeNotNull(details), 'response', 'multi_objective_info', 'outcome')",
};

const HYBRID_SHAPE: DecisionShape = DecisionShape {
    decision_flow_type: FlowType::RoutingHybridDecision,
    rule_hit_predicate:
        "flow_type = 'routing_hybrid_decision' AND routing_approach = 'STATIC_ROUTING'",
    error_flow_types: HYBRID_ERROR_FLOW_TYPES,
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

pub fn all_decisions_filter() -> super::query::FilterClause {
    super::query::FilterClause::raw(format!(
        "flow_type IN {}",
        static_flow_type_in_sql(ALL_DECISION_FLOW_TYPES)
    ))
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
        "gateway_decided" => "Auth Rate".to_string(),
        "hybrid_routed" | "decide_routed" => "Multi Objective".to_string(),
        "score_updated" | "score_skipped" => "Update Gateway".to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decisions_are_staged_by_routing_kind_and_routed_by_endpoint_path() {
        assert_eq!(
            payment_audit_stage_label("decide_routed".into()),
            "Multi Objective"
        );
        assert_eq!(
            payment_audit_stage_label("gateway_decided".into()),
            "Auth Rate"
        );
        assert_eq!(
            payment_audit_stage_label("hybrid_routed".into()),
            "Multi Objective"
        );
        assert_eq!(payment_audit_route_label("decide".into()), "/decide");
        assert_eq!(
            payment_audit_route_label("decide_gateway".into()),
            "/decide-gateway"
        );
        assert_eq!(
            payment_audit_route_label("routing_hybrid".into()),
            "/routing/hybrid"
        );
    }

    #[test]
    fn the_stored_route_tells_the_two_hybrid_entry_points_apart() {
        let hybrid = FlowType::RoutingHybridRequestHit;
        assert_eq!(
            route_for_request_hit(hybrid, Some("decide")),
            AnalyticsRoute::Decide
        );
        assert_eq!(
            route_for_request_hit(hybrid, Some("routing_hybrid")),
            AnalyticsRoute::RoutingHybrid
        );
    }

    #[test]
    fn hybrid_hits_stored_under_the_old_decide_gateway_route_stay_on_the_hybrid_card() {
        let hybrid = FlowType::RoutingHybridRequestHit;
        assert_eq!(
            route_for_request_hit(hybrid, Some("decide_gateway")),
            AnalyticsRoute::RoutingHybrid
        );
        assert_eq!(
            route_for_request_hit(hybrid, None),
            AnalyticsRoute::RoutingHybrid
        );
    }

    #[test]
    fn every_other_flow_type_keeps_its_own_card_whatever_the_row_says() {
        assert_eq!(
            route_for_request_hit(FlowType::DecideGatewayRequestHit, Some("decide")),
            AnalyticsRoute::DecideGateway
        );
        assert_eq!(
            route_for_request_hit(FlowType::RoutingEvaluateRequestHit, Some("decide")),
            AnalyticsRoute::RoutingEvaluate
        );
        assert_eq!(
            route_for_request_hit(FlowType::UpdateGatewayScoreRequestHit, Some("decide")),
            AnalyticsRoute::UpdateGatewayScore
        );
    }

    #[test]
    fn the_two_hybrid_entry_points_are_counted_on_separate_cards() {
        let hits = ordered_route_hits_from_counts([
            (AnalyticsRoute::Decide, 348),
            (AnalyticsRoute::RoutingHybrid, 1),
            (AnalyticsRoute::UpdateGatewayScore, 348),
        ]);
        let count_for = |label: &str| {
            hits.iter()
                .find(|hit| hit.route == label)
                .map(|hit| hit.count)
        };

        assert_eq!(count_for("/decide"), Some(348));
        assert_eq!(count_for("/routing_hybrid"), Some(1));
        assert_eq!(count_for("/update_gateway"), Some(348));
        assert_eq!(count_for("/decide_gateway"), Some(0));
    }

    #[test]
    fn rows_landing_on_the_same_card_are_summed_rather_than_overwritten() {
        let hits = ordered_route_hits_from_counts([
            (AnalyticsRoute::RoutingHybrid, 19563),
            (AnalyticsRoute::RoutingHybrid, 206),
        ]);

        assert_eq!(
            hits.iter()
                .find(|hit| hit.route == "/routing_hybrid")
                .map(|hit| hit.count),
            Some(19769)
        );
    }
}
