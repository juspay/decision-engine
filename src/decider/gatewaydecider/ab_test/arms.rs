//! Layered A/B arms and their per-endpoint projection.
//!
//! An experiment arm is a bundle of layers (rule, SR with its autopilot / cost-savings
//! overrides). Each endpoint applies only the layers it supports, so one experiment measures a
//! different effect on each endpoint:
//!
//! | endpoint        | layers applied |
//! |-----------------|----------------|
//! | hybrid_routing  | rule + SR      |
//! | decide_gateway  | SR             |
//! | evaluate        | rule           |
//!
//! Traffic splits on an endpoint only when it is in the experiment's endpoint scope and the two
//! arms' projections differ there. Everywhere else the control projection is served untracked,
//! so an experiment that only changes rules never runs an A/A test on `/decide-gateway`.

use crate::euclid::types::{
    ABTestData, ExperimentArm, ExperimentEndpoint, SrConfigOverride, SR_ROUTING_ARM_ID,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArmSide {
    Control,
    Variant,
}

impl ArmSide {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Control => "control",
            Self::Variant => "variant",
        }
    }
}

/// The layers of one arm that a given endpoint applies.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ArmProjection {
    pub rule_algorithm_id: Option<String>,
    pub sr: Option<SrConfigOverride>,
}

/// What an endpoint serves for one payment.
#[derive(Debug, Clone, PartialEq)]
pub struct EndpointPlan {
    pub side: ArmSide,
    pub projection: ArmProjection,
    /// True when the payment is split on this endpoint and its decision and outcome are
    /// attributed to `side`. False when the control projection is served untracked.
    pub in_experiment: bool,
}

/// The arm for `side`, reading the single-strategy format (`sr_routing` or a saved algorithm id)
/// as an arm with just that one layer.
pub fn resolved_arm(data: &ABTestData, side: ArmSide) -> ExperimentArm {
    let (layered, algorithm_id, sr_config) = match side {
        ArmSide::Control => (
            &data.control,
            &data.control_algorithm_id,
            &data.control_sr_config,
        ),
        ArmSide::Variant => (
            &data.variant,
            &data.variant_algorithm_id,
            &data.variant_sr_config,
        ),
    };
    if let Some(arm) = layered {
        return arm.clone();
    }
    match algorithm_id.as_deref() {
        Some(SR_ROUTING_ARM_ID) => ExperimentArm {
            rule_algorithm_id: None,
            sr: Some(sr_config.clone().unwrap_or_default()),
        },
        Some(id) if !id.is_empty() => ExperimentArm {
            rule_algorithm_id: Some(id.to_string()),
            sr: None,
        },
        _ => ExperimentArm::default(),
    }
}

pub fn project(arm: &ExperimentArm, endpoint: ExperimentEndpoint) -> ArmProjection {
    match endpoint {
        ExperimentEndpoint::HybridRouting => ArmProjection {
            rule_algorithm_id: arm.rule_algorithm_id.clone(),
            sr: arm.sr.clone(),
        },
        ExperimentEndpoint::DecideGateway => ArmProjection {
            rule_algorithm_id: None,
            sr: arm.sr.clone(),
        },
        ExperimentEndpoint::Evaluate => ArmProjection {
            rule_algorithm_id: arm.rule_algorithm_id.clone(),
            sr: None,
        },
        ExperimentEndpoint::Unknown => ArmProjection::default(),
    }
}

pub fn in_scope(data: &ABTestData, endpoint: ExperimentEndpoint) -> bool {
    endpoint != ExperimentEndpoint::Unknown
        && data
            .endpoints
            .as_ref()
            .is_none_or(|endpoints| endpoints.contains(&endpoint))
}

/// Whether traffic on `endpoint` is split between the arms.
pub fn splits_on(data: &ABTestData, endpoint: ExperimentEndpoint) -> bool {
    in_scope(data, endpoint)
        && routing_behaviour(&resolved_arm(data, ArmSide::Control), endpoint)
            != routing_behaviour(&resolved_arm(data, ArmSide::Variant), endpoint)
}

/// The projection reduced to how the endpoint routes. `/decide-gateway` always runs SR, and an arm
/// without an SR layer runs it with the merchant's settings, the same as an SR layer with no
/// overrides. On `/routing/hybrid` the SR layer's presence decides whether SR runs at all.
fn routing_behaviour(arm: &ExperimentArm, endpoint: ExperimentEndpoint) -> ArmProjection {
    let mut projection = project(arm, endpoint);
    if endpoint == ExperimentEndpoint::DecideGateway {
        projection.sr = Some(projection.sr.unwrap_or_default());
    }
    projection
}

pub fn plan(data: &ABTestData, endpoint: ExperimentEndpoint, payment_id: &str) -> EndpointPlan {
    if !splits_on(data, endpoint) {
        return EndpointPlan {
            side: ArmSide::Control,
            projection: project(&resolved_arm(data, ArmSide::Control), endpoint),
            in_experiment: false,
        };
    }
    let side = super::common::assign_arm(payment_id, data.variant_split_pct);
    EndpointPlan {
        side,
        projection: project(&resolved_arm(data, side), endpoint),
        in_experiment: true,
    }
}

/// Write-time checks for an experiment definition. Returns `(field, message)` pairs.
pub fn validate(data: &ABTestData) -> Vec<(&'static str, String)> {
    let mut errors = Vec::new();
    let control = resolved_arm(data, ArmSide::Control);
    let variant = resolved_arm(data, ArmSide::Variant);
    if control.is_empty() {
        errors.push((
            "control",
            "control arm needs a rule layer, an SR layer, or both".to_string(),
        ));
    }
    if variant.is_empty() {
        errors.push((
            "variant",
            "variant arm needs a rule layer, an SR layer, or both".to_string(),
        ));
    }
    if let Some(endpoints) = &data.endpoints {
        if endpoints.is_empty() {
            errors.push((
                "endpoints",
                "select at least one endpoint, or omit endpoints to apply everywhere".to_string(),
            ));
        }
        if endpoints.contains(&ExperimentEndpoint::Unknown) {
            errors.push((
                "endpoints",
                "endpoints must be hybrid_routing, decide_gateway or evaluate".to_string(),
            ));
        }
    }
    if errors.is_empty() && !ExperimentEndpoint::ALL.iter().any(|e| splits_on(data, *e)) {
        errors.push((
            "variant",
            "control and variant apply the same layers on every selected endpoint".to_string(),
        ));
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sr(autopilot: bool, cost: bool) -> SrConfigOverride {
        SrConfigOverride {
            use_autopilot: Some(autopilot),
            enable_multi_objective: Some(cost),
            ..Default::default()
        }
    }

    fn layered(control: ExperimentArm, variant: ExperimentArm) -> ABTestData {
        ABTestData {
            control: Some(control),
            variant: Some(variant),
            endpoints: None,
            control_algorithm_id: None,
            variant_algorithm_id: None,
            variant_split_pct: 10,
            min_sample_size: 1000,
            guardrail_threshold_pp: 3.0,
            variant_sr_config: None,
            control_sr_config: None,
        }
    }

    /// A: rule₁ + SR autopilot, B: rule₂ + SR autopilot + cost savings.
    fn rule_and_sr_experiment() -> ABTestData {
        layered(
            ExperimentArm {
                rule_algorithm_id: Some("rule_1".into()),
                sr: Some(sr(true, false)),
            },
            ExperimentArm {
                rule_algorithm_id: Some("rule_2".into()),
                sr: Some(sr(true, true)),
            },
        )
    }

    #[test]
    fn each_endpoint_applies_only_its_layers() {
        let data = rule_and_sr_experiment();
        let variant = resolved_arm(&data, ArmSide::Variant);

        let hybrid = project(&variant, ExperimentEndpoint::HybridRouting);
        assert_eq!(hybrid.rule_algorithm_id.as_deref(), Some("rule_2"));
        assert_eq!(hybrid.sr, Some(sr(true, true)));

        let decide = project(&variant, ExperimentEndpoint::DecideGateway);
        assert_eq!(decide.rule_algorithm_id, None);
        assert_eq!(decide.sr, Some(sr(true, true)));

        let evaluate = project(&variant, ExperimentEndpoint::Evaluate);
        assert_eq!(evaluate.rule_algorithm_id.as_deref(), Some("rule_2"));
        assert_eq!(evaluate.sr, None);

        for endpoint in ExperimentEndpoint::ALL {
            assert!(splits_on(&data, endpoint), "{endpoint:?} should split");
        }
    }

    #[test]
    fn rule_only_difference_does_not_split_decide_gateway() {
        let data = layered(
            ExperimentArm {
                rule_algorithm_id: Some("rule_1".into()),
                sr: Some(sr(true, false)),
            },
            ExperimentArm {
                rule_algorithm_id: Some("rule_2".into()),
                sr: Some(sr(true, false)),
            },
        );
        assert!(splits_on(&data, ExperimentEndpoint::HybridRouting));
        assert!(splits_on(&data, ExperimentEndpoint::Evaluate));
        assert!(!splits_on(&data, ExperimentEndpoint::DecideGateway));

        // Every payment is served the shared SR layer, untracked.
        let served = plan(&data, ExperimentEndpoint::DecideGateway, "pay_1");
        assert!(!served.in_experiment);
        assert_eq!(served.side, ArmSide::Control);
        assert_eq!(served.projection.sr, Some(sr(true, false)));
    }

    #[test]
    fn no_sr_layer_and_empty_sr_layer_route_alike_on_decide_gateway() {
        let data = layered(
            ExperimentArm {
                rule_algorithm_id: Some("rule_1".into()),
                sr: None,
            },
            ExperimentArm {
                rule_algorithm_id: None,
                sr: Some(SrConfigOverride::default()),
            },
        );
        assert!(!splits_on(&data, ExperimentEndpoint::DecideGateway));
        assert!(splits_on(&data, ExperimentEndpoint::HybridRouting));
    }

    #[test]
    fn endpoint_scope_limits_the_split() {
        let mut data = rule_and_sr_experiment();
        data.endpoints = Some(vec![ExperimentEndpoint::HybridRouting]);
        assert!(splits_on(&data, ExperimentEndpoint::HybridRouting));
        assert!(!splits_on(&data, ExperimentEndpoint::DecideGateway));
        assert!(!splits_on(&data, ExperimentEndpoint::Evaluate));

        let served = plan(&data, ExperimentEndpoint::Evaluate, "pay_1");
        assert!(!served.in_experiment);
        assert_eq!(
            served.projection.rule_algorithm_id.as_deref(),
            Some("rule_1")
        );
    }

    #[test]
    fn a_payment_lands_on_the_same_arm_on_every_endpoint() {
        let data = rule_and_sr_experiment();
        for i in 0..200 {
            let payment_id = format!("pay_{i}");
            let sides: Vec<ArmSide> = ExperimentEndpoint::ALL
                .iter()
                .map(|e| plan(&data, *e, &payment_id).side)
                .collect();
            assert!(sides.windows(2).all(|w| w[0] == w[1]), "{payment_id}");
        }
    }

    #[test]
    fn single_strategy_arms_read_as_one_layer() {
        let mut data = layered(ExperimentArm::default(), ExperimentArm::default());
        data.control = None;
        data.variant = None;
        data.control_algorithm_id = Some("rule_1".into());
        data.variant_algorithm_id = Some(SR_ROUTING_ARM_ID.into());
        data.variant_sr_config = Some(sr(false, true));

        assert_eq!(
            resolved_arm(&data, ArmSide::Control),
            ExperimentArm {
                rule_algorithm_id: Some("rule_1".into()),
                sr: None,
            }
        );
        assert_eq!(
            resolved_arm(&data, ArmSide::Variant),
            ExperimentArm {
                rule_algorithm_id: None,
                sr: Some(sr(false, true)),
            }
        );
        // Rule vs SR: decide_gateway compares "no SR layer" with "SR + cost".
        assert!(splits_on(&data, ExperimentEndpoint::DecideGateway));
    }

    #[test]
    fn single_strategy_sr_arm_without_overrides_is_plain_sr() {
        let mut data = layered(ExperimentArm::default(), ExperimentArm::default());
        data.control = None;
        data.variant = None;
        data.control_algorithm_id = Some(SR_ROUTING_ARM_ID.into());
        data.variant_algorithm_id = Some(SR_ROUTING_ARM_ID.into());
        data.variant_sr_config = Some(SrConfigOverride {
            hedging_percent: Some(10.0),
            ..Default::default()
        });
        assert_eq!(
            resolved_arm(&data, ArmSide::Control).sr,
            Some(SrConfigOverride::default())
        );
        assert!(splits_on(&data, ExperimentEndpoint::DecideGateway));
        assert!(!splits_on(&data, ExperimentEndpoint::Evaluate));
    }

    #[test]
    fn validation_rejects_empty_and_identical_arms() {
        let empty = layered(ExperimentArm::default(), ExperimentArm::default());
        let fields: Vec<_> = validate(&empty).into_iter().map(|(f, _)| f).collect();
        assert_eq!(fields, vec!["control", "variant"]);

        let arm = ExperimentArm {
            rule_algorithm_id: Some("rule_1".into()),
            sr: Some(sr(true, false)),
        };
        let identical = layered(arm.clone(), arm);
        assert_eq!(validate(&identical).len(), 1);

        // Only the rule differs, but the experiment is scoped to decide_gateway where rules
        // are not applied — nothing would ever split.
        let mut rules_on_decide = rule_and_sr_experiment();
        rules_on_decide.variant.as_mut().unwrap().sr = Some(sr(true, false));
        rules_on_decide.endpoints = Some(vec![ExperimentEndpoint::DecideGateway]);
        assert_eq!(validate(&rules_on_decide).len(), 1);

        let mut no_endpoints = rule_and_sr_experiment();
        no_endpoints.endpoints = Some(vec![]);
        assert_eq!(validate(&no_endpoints)[0].0, "endpoints");

        assert!(validate(&rule_and_sr_experiment()).is_empty());
    }

    #[test]
    fn unknown_endpoint_names_deserialize() {
        let endpoints: Vec<ExperimentEndpoint> =
            serde_json::from_str(r#"["hybrid_routing","payouts"]"#).unwrap();
        assert_eq!(
            endpoints,
            vec![
                ExperimentEndpoint::HybridRouting,
                ExperimentEndpoint::Unknown
            ]
        );
    }
}
