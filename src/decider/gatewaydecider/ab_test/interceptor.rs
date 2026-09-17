use super::arms;
use super::config::{self, AbTestConfig};
use super::outcome::{self, ExperimentAssignment};
use crate::analytics::{serialize_details, AnalyticsFlowContext, ApiFlow, FlowType};
use crate::decider::gatewaydecider::types::DomainDeciderRequestForApiCallV2;
use crate::euclid::types::ExperimentEndpoint;
use crate::logger;

pub enum AbTestIntercept {
    /// Feature flag off, no active AB test, or the payment is not covered or excluded — proceed
    /// normally.
    Disabled,
    /// Run SR routing with this arm's SR layer; the decision is attributed to `experiment`'s arm.
    SrArm {
        /// SR overrides (hedging / elimination / margin / multi-objective / autopilot). `None`
        /// when the served arm has no SR layer, which routes with the live SR config.
        sr_config_override: Option<crate::euclid::types::SrConfigOverride>,
        experiment: ExperimentAssignment,
    },
}

/// Emit a routing-time `RoutingEvaluateAbTest` event so the rule-based Decision Audit
/// shows the actual decision details (arm, endpoint, layers, gateway) rather than just the
/// thin outcome event emitted later by outcome::emit_if_in_flight.
pub fn emit_routing_event(
    payment_id: &str,
    merchant_id: &str,
    assignment: &ExperimentAssignment,
    projection: &arms::ArmProjection,
    gateway: Option<&str>,
) {
    let variant_arm = assignment.side.as_str();
    let arm_label = match (&projection.rule_algorithm_id, &projection.sr) {
        (Some(rule), Some(_)) => format!("{rule}+sr_routing"),
        (Some(rule), None) => rule.clone(),
        (None, _) => crate::euclid::types::SR_ROUTING_ARM_ID.to_string(),
    };
    let details = serialize_details(&serde_json::json!({
        "experiment_id": assignment.experiment_id,
        "variant_arm": variant_arm,
        "endpoint": assignment.endpoint.as_str(),
        "arm_algorithm": arm_label,
        "rule_algorithm_id": projection.rule_algorithm_id,
        "sr_config": projection.sr,
        "preview_kind": "routing_evaluate_ab_test",
        "routing_source": "real_payment_intercept",
    }));

    crate::analytics::DomainAnalyticsEvent::record_rule_evaluation_preview(
        AnalyticsFlowContext::new(ApiFlow::RuleBasedRouting, FlowType::RoutingEvaluateAbTest),
        Some(merchant_id.to_string()),
        Some(payment_id.to_string()),
        gateway.map(str::to_string),
        Some(format!("ab_test_{variant_arm}_{arm_label}")),
        Some("ab_test_decision".to_string()),
        details,
        None,
        None,
        None,
    );
}

/// Whether the arm served on `/routing/hybrid` for this payment has an SR layer, when an active
/// experiment applies to the merchant. The experiment decides for its own traffic: an arm with an
/// SR layer runs SR even when the merchant's `sr-routing` setting is off, and an arm without one
/// is decided by its rule output alone. `None` leaves the decision to `sr-routing`.
pub async fn hybrid_arm_has_sr_layer(merchant_id: &str, payment_id: &str) -> Option<bool> {
    if !config::is_enabled(merchant_id).await {
        return None;
    }
    let AbTestConfig { data, .. } = config::load_active_ab_test(merchant_id).await?;
    let plan = arms::plan(&data, ExperimentEndpoint::HybridRouting, payment_id);
    // A payment the experiment doesn't cover is routed by the live setup.
    plan.in_experiment.then_some(plan.projection.sr.is_some())
}

/// Applies the active experiment's SR layer for a decider call on `endpoint`
/// (`DecideGateway`, or `HybridRouting` for the dynamic half of `/routing/hybrid`). The rule
/// layer is applied by `/routing/evaluate`, which on a hybrid call has already narrowed
/// `eligible_gateway_list` to the arm's rule output.
pub async fn intercept(
    dreq: &DomainDeciderRequestForApiCallV2,
    endpoint: ExperimentEndpoint,
) -> AbTestIntercept {
    if !config::is_enabled(&dreq.merchant_id).await {
        return AbTestIntercept::Disabled;
    }

    let Some(AbTestConfig {
        experiment_id,
        data,
    }) = config::load_active_ab_test(&dreq.merchant_id).await
    else {
        return AbTestIntercept::Disabled;
    };

    let payment_id = dreq.payment_id();
    let plan = arms::plan(&data, endpoint, payment_id);

    logger::debug!(
        "ab_test intercept: payment_id={} merchant={} experiment={} endpoint={} arm={} in_experiment={}",
        payment_id,
        dreq.merchant_id,
        experiment_id,
        endpoint.as_str(),
        plan.side.as_str(),
        plan.in_experiment
    );

    // A payment the experiment doesn't cover is routed by the live SR settings.
    if !plan.in_experiment {
        return AbTestIntercept::Disabled;
    }
    let assignment = ExperimentAssignment {
        experiment_id,
        side: plan.side,
        endpoint,
    };

    // `/routing/hybrid` skips the decider for an arm without an SR layer (see
    // `hybrid_arm_has_sr_layer`). Reaching here with one means there was no rule output to serve;
    // routing it via SR under the rule arm's attribution would corrupt the experiment, so the
    // payment is excluded and routes normally.
    if endpoint == ExperimentEndpoint::HybridRouting && plan.projection.sr.is_none() {
        logger::warn!(
            "ab_test intercept: hybrid arm without an SR layer reached the decider, excluding payment {} from the experiment",
            payment_id
        );
        return AbTestIntercept::Disabled;
    }

    // SR layer (or none): the gateway is unknown until the decider runs, so the routing event
    // carries no gateway and the decided gateway is backfilled by `record_cost_outcome`.
    emit_routing_event(
        payment_id,
        &dreq.merchant_id,
        &assignment,
        &plan.projection,
        None,
    );
    outcome::store_inflight(
        payment_id,
        &assignment,
        None,
        false,
        Some(dreq.payment_info.amount),
    )
    .await;

    AbTestIntercept::SrArm {
        sr_config_override: plan.projection.sr,
        experiment: assignment,
    }
}
