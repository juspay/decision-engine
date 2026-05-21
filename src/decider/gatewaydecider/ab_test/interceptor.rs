use crate::analytics::{
    AnalyticsFlowContext, ApiFlow, FlowType,
    serialize_details,
};
use crate::decider::gatewaydecider::types::{
    DecidedGateway, DomainDeciderRequestForApiCallV2, GatewayDeciderApproach, ResetApproach,
};
use super::config::{self, AbTestConfig};
use super::evaluator;
use super::outcome;
use crate::logger;

pub enum AbTestIntercept {
    /// Feature flag off, no active AB test, or error — proceed normally.
    Disabled,
    /// This payment is assigned to the SR routing arm — proceed normally, carry experiment context.
    SrArm {
        experiment_id: String,
        variant_arm: String,
        /// SR hyperparameter overrides to apply during routing. Only set for the variant arm
        /// in SR Config Tuning experiments; None for control arm and standard A/B tests.
        sr_config_override: Option<crate::euclid::types::SrConfigOverride>,
    },
    /// This payment is assigned to a static algorithm arm — return this result directly.
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

    let Some(AbTestConfig { experiment_id, data }) =
        config::load_active_ab_test(&dreq.merchant_id).await
    else {
        return AbTestIntercept::Disabled;
    };

    let payment_id = dreq.payment_id();
    let arm = super::common::assign_arm(payment_id, data.variant_split_pct);
    let arm_algorithm_id = if arm == "variant" {
        &data.variant_algorithm_id
    } else {
        &data.control_algorithm_id
    };

    logger::debug!(
        "ab_test intercept: payment_id={} merchant={} experiment={} arm={}",
        payment_id, dreq.merchant_id, experiment_id, arm
    );

    // SR arm: gateway unknown until the decider runs — emit routing event without gateway.
    if arm_algorithm_id == "sr_routing" {
        let sr_config_override = if arm == "variant" {
            data.variant_sr_config.clone()
        } else {
            None
        };
        emit_routing_event(payment_id, &dreq.merchant_id, &experiment_id, arm, "sr_routing", None);
        outcome::store_inflight(payment_id, &experiment_id, arm, None, false).await;
        return AbTestIntercept::SrArm {
            experiment_id,
            variant_arm: arm.to_string(),
            sr_config_override,
        };
    }

    // Static arm: evaluate, emit routing event with decided gateway.
    match evaluator::evaluate_static_arm(arm_algorithm_id, payment_id).await {
        Some(static_result) => {
            emit_routing_event(
                payment_id,
                &dreq.merchant_id,
                &experiment_id,
                arm,
                arm_algorithm_id,
                Some(static_result.decided_gateway.as_str()),
            );
            outcome::store_inflight(
                payment_id,
                &experiment_id,
                arm,
                Some(static_result.decided_gateway.as_str()),
                true,
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
                }),
                experiment_id,
                variant_arm: arm.to_string(),
            }
        }
        None => {
            logger::warn!(
                "ab_test intercept: static arm evaluation failed for '{}', falling back to SR routing",
                arm_algorithm_id
            );
            emit_routing_event(payment_id, &dreq.merchant_id, &experiment_id, arm, arm_algorithm_id, None);
            outcome::store_inflight(payment_id, &experiment_id, arm, None, false).await;
            AbTestIntercept::SrArm {
                experiment_id,
                variant_arm: arm.to_string(),
                sr_config_override: None,
            }
        }
    }
}
