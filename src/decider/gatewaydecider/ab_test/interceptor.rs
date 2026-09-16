use super::config::{self, AbTestConfig};
use super::evaluator;
use super::outcome;
use crate::analytics::{serialize_details, AnalyticsFlowContext, ApiFlow, FlowType};
use crate::decider::gatewaydecider::types::{
    DecidedGateway, DomainDeciderRequestForApiCallV2, GatewayDeciderApproach, ResetApproach,
};
use crate::logger;
use crate::types::ab_test::ArmStrategy;

pub enum AbTestIntercept {
    /// Feature flag off, no active AB test, an arm shape this build does not understand, or an
    /// arm that failed to evaluate — proceed normally, outside the experiment.
    Disabled,
    /// This payment's arm runs the dynamic decider — proceed into it carrying experiment context.
    /// Covers both a pure SR arm and a hybrid arm whose rule leg has already run.
    SrArm {
        experiment_id: String,
        variant_arm: String,
        /// The arm's algorithm label, for the response and the audit trail.
        arm_algorithm: String,
        /// SR hyperparameter overrides to apply during routing. `None` means the arm uses the
        /// merchant's live SR config, which is what keeps a control arm faithful to production.
        sr_config_override: Option<crate::types::ab_test::SrConfigOverride>,
        /// Gateways the arm's rule leg admitted, to be enforced as SR's candidate set.
        /// `None` for a pure SR arm, which routes over everything the decider's filters allow.
        eligible_gateways: Option<Vec<String>>,
    },
    /// This payment is assigned to a rule-only arm — return this result directly, SR never runs.
    StaticArm {
        result: Box<DecidedGateway>,
        experiment_id: String,
        variant_arm: String,
    },
}

/// Emit a routing-time `RoutingEvaluateAbTest` event so the rule-based Decision Audit
/// shows the actual decision details (arm, gateway, algorithm) rather than just the
/// thin outcome event emitted later by outcome::emit_if_in_flight.
fn emit_routing_event(
    payment_id: &str,
    merchant_id: &str,
    experiment_id: &str,
    variant_arm: &str,
    arm_algorithm_id: &str,
    gateway: Option<&str>,
) {
    let details = serialize_details(&serde_json::json!({
        "experiment_id": experiment_id,
        "variant_arm": variant_arm,
        "arm_algorithm": arm_algorithm_id,
        "preview_kind": "routing_evaluate_ab_test",
        "routing_source": "real_payment_intercept",
    }));

    crate::analytics::DomainAnalyticsEvent::record_rule_evaluation_preview(
        AnalyticsFlowContext::new(ApiFlow::RuleBasedRouting, FlowType::RoutingEvaluateAbTest),
        Some(merchant_id.to_string()),
        Some(payment_id.to_string()),
        gateway.map(str::to_string),
        Some(format!("ab_test_{variant_arm}_{arm_algorithm_id}")),
        Some("ab_test_decision".to_string()),
        details,
        None,
        None,
        None,
    );
}

pub async fn intercept(dreq: &DomainDeciderRequestForApiCallV2) -> AbTestIntercept {
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
    let (arm, strategy) = data.arm_for(payment_id);
    let arm_algorithm = strategy.label();

    logger::debug!(
        "ab_test intercept: payment_id={} merchant={} experiment={} arm={} algorithm={}",
        payment_id,
        dreq.merchant_id,
        experiment_id,
        arm,
        arm_algorithm
    );

    // The rule leg, for the arms that have one. `Rule` answers with it directly; `Hybrid`
    // reduces it to a candidate set and hands that to SR.
    let rule_result = match strategy.algorithm_id() {
        Some(algorithm_id) => {
            match evaluator::evaluate_static_arm(algorithm_id, payment_id, dreq).await {
                Some(result) => Some(result),
                None => {
                    // Do NOT fall back to unrestricted routing under this arm's attribution — the
                    // payment would route some other way but still get counted in this arm's
                    // auth/cost stats, silently corrupting the experiment (a broken rule config
                    // would make the "rule" arm's results actually be SR results). Opt this
                    // payment out of the experiment entirely: no routing/outcome events, no
                    // store_inflight, normal live-config routing.
                    logger::warn!(
                        "ab_test intercept: rule '{}' failed to evaluate for the '{}' arm, excluding payment {} from experiment {} and routing normally",
                        algorithm_id,
                        arm,
                        payment_id,
                        experiment_id
                    );
                    return AbTestIntercept::Disabled;
                }
            }
        }
        None => None,
    };

    match strategy {
        // Exclude the payment rather than guess at it: an arm shape this build cannot execute
        // would otherwise be routed as something else and still counted for the arm. `Current`
        // is resolved to a concrete arm by `routing_create`, so one reaching here means a row
        // was written by some other path and its real shape is unknown. Neither names an
        // algorithm, so no rule was evaluated above.
        ArmStrategy::Unknown | ArmStrategy::Current => {
            logger::warn!(
                "ab_test intercept: unresolved or unrecognized arm shape on the '{}' arm of experiment {}, excluding payment {}",
                arm,
                experiment_id,
                payment_id
            );
            AbTestIntercept::Disabled
        }

        // Rule only: the rule's pick is the decision and SR never runs, so the gateway is
        // known now and this payment gets no SR score feedback.
        ArmStrategy::Rule { .. } => {
            let static_result = rule_result.expect("Rule arm always evaluates a rule");
            emit_routing_event(
                payment_id,
                &dreq.merchant_id,
                &experiment_id,
                arm,
                &arm_algorithm,
                Some(static_result.decided_gateway.as_str()),
            );
            outcome::store_inflight(
                payment_id,
                &experiment_id,
                arm,
                Some(static_result.decided_gateway.as_str()),
                false,
                Some(dreq.payment_info.amount),
            )
            .await;
            AbTestIntercept::StaticArm {
                result: Box::new(DecidedGateway {
                    decided_gateway: static_result.decided_gateway,
                    fallback_gateways: static_result.fallback_gateways,
                    gateway_priority_map: None,
                    filter_wise_gateways: None,
                    priority_logic_tag: static_result.rule_name,
                    routing_approach: GatewayDeciderApproach::AbTestStaticAlgorithm,
                    gateway_before_evaluation: None,
                    priority_logic_output: None,
                    debit_routing_output: None,
                    reset_approach: ResetApproach::NoReset,
                    routing_dimension: None,
                    routing_dimension_level: None,
                    is_scheduled_outage: false,
                    is_dynamic_mga_enabled: false,
                    gateway_mga_id_map: None,
                    is_rust_based_decider: true,
                    latency: None,
                    multi_objective_info: None,
                    volume_steer_info: None,
                    ab_test_info: Some(crate::types::ab_test::AbTestInfo {
                        experiment_id: experiment_id.clone(),
                        arm: arm.to_string(),
                        arm_algorithm,
                    }),
                }),
                experiment_id,
                variant_arm: arm.to_string(),
            }
        }

        // SR only, and hybrid (rule narrows, SR picks): both continue into the decider, so the
        // gateway is unknown until it returns. `record_cost_outcome` backfills it afterwards.
        ArmStrategy::Sr { .. } | ArmStrategy::Hybrid { .. } => {
            let eligible_gateways = match rule_result {
                Some(result) if result.candidates.is_empty() => {
                    logger::warn!(
                        "ab_test intercept: rule on the '{}' arm admitted no gateway, excluding payment {} from experiment {}",
                        arm,
                        payment_id,
                        experiment_id
                    );
                    return AbTestIntercept::Disabled;
                }
                Some(result) => Some(result.candidates),
                None => None,
            };
            emit_routing_event(
                payment_id,
                &dreq.merchant_id,
                &experiment_id,
                arm,
                &arm_algorithm,
                None,
            );
            outcome::store_inflight(
                payment_id,
                &experiment_id,
                arm,
                None,
                true,
                Some(dreq.payment_info.amount),
            )
            .await;
            AbTestIntercept::SrArm {
                experiment_id,
                variant_arm: arm.to_string(),
                arm_algorithm,
                sr_config_override: strategy.sr_config().cloned(),
                eligible_gateways,
            }
        }
    }
}
