//! Layered A/B arms and their per-endpoint projection.
//!
//! An experiment has one control arm and one or more variants. Each arm is a bundle of layers
//! (rule, SR with its autopilot / cost-savings overrides). Each endpoint applies only the layers
//! it supports, so one experiment measures a different effect on each endpoint:
//!
//! | endpoint        | layers applied |
//! |-----------------|----------------|
//! | hybrid_routing  | rule + SR      |
//! | decide_gateway  | SR             |
//! | evaluate        | rule           |
//!
//! Every variant owns a fixed slice of the payment-id hash space and control takes the rest. A
//! variant's payments are split on an endpoint only when it is in the experiment's endpoint scope,
//! the variant has not been stopped by the guardrail, and the variant routes differently from
//! control there. Otherwise the control projection is served untracked, so an experiment that only
//! changes rules never runs an A/A test on `/decide-gateway`. Control payments are tracked on an
//! endpoint when at least one variant is split there.

use crate::euclid::types::{
    ABTestData, ExperimentArm, ExperimentEndpoint, SrConfigOverride, CONTROL_ARM_ID,
    LEGACY_VARIANT_ARM_ID, MAX_EXPERIMENT_VARIANTS, MAX_TOTAL_VARIANT_SPLIT_PCT, SR_ROUTING_ARM_ID,
};

/// The arm a payment is assigned to: control, or the variant with this id.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ArmSide {
    Control,
    Variant(String),
}

impl ArmSide {
    /// The id the arm's decisions are recorded under (`variant_arm` in analytics).
    pub fn as_str(&self) -> &str {
        match self {
            Self::Control => CONTROL_ARM_ID,
            Self::Variant(id) => id,
        }
    }
}

/// One variant with its single-variant fields already read into the multi-variant shape.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedVariant {
    pub id: String,
    pub split_pct: u8,
    pub arm: ExperimentArm,
    pub stopped: bool,
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

/// Reads a single-strategy arm (`sr_routing` or a saved algorithm id) as an arm with that layer.
fn single_strategy_arm(
    algorithm_id: Option<&str>,
    sr_config: Option<&SrConfigOverride>,
) -> ExperimentArm {
    match algorithm_id {
        Some(SR_ROUTING_ARM_ID) => ExperimentArm {
            rule_algorithm_id: None,
            sr: Some(sr_config.cloned().unwrap_or_default()),
        },
        Some(id) if !id.is_empty() => ExperimentArm {
            rule_algorithm_id: Some(id.to_string()),
            sr: None,
        },
        _ => ExperimentArm::default(),
    }
}

/// The control arm, reading the single-strategy format as an arm with just that one layer.
pub fn resolved_control(data: &ABTestData) -> ExperimentArm {
    data.control.clone().unwrap_or_else(|| {
        single_strategy_arm(
            data.control_algorithm_id.as_deref(),
            data.control_sr_config.as_ref(),
        )
    })
}

/// The experiment's variants in hash-slice order. The single-variant format reads as one variant
/// with the id `variant`.
pub fn resolved_variants(data: &ABTestData) -> Vec<ResolvedVariant> {
    if let Some(variants) = &data.variants {
        return variants
            .iter()
            .map(|variant| ResolvedVariant {
                id: variant.id.clone(),
                split_pct: variant.split_pct,
                arm: variant.arm.clone(),
                stopped: variant.stopped,
            })
            .collect();
    }
    let arm = data.variant.clone().unwrap_or_else(|| {
        single_strategy_arm(
            data.variant_algorithm_id.as_deref(),
            data.variant_sr_config.as_ref(),
        )
    });
    vec![ResolvedVariant {
        id: LEGACY_VARIANT_ARM_ID.to_string(),
        split_pct: data.variant_split_pct,
        arm,
        stopped: false,
    }]
}

/// The arm for `side`. A variant id the experiment does not define reads as an empty arm.
pub fn resolved_arm(data: &ABTestData, side: &ArmSide) -> ExperimentArm {
    match side {
        ArmSide::Control => resolved_control(data),
        ArmSide::Variant(id) => resolved_variants(data)
            .into_iter()
            .find(|variant| variant.id == *id)
            .map(|variant| variant.arm)
            .unwrap_or_default(),
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

fn routes_differently(a: &ExperimentArm, b: &ExperimentArm, endpoint: ExperimentEndpoint) -> bool {
    routing_behaviour(a, endpoint) != routing_behaviour(b, endpoint)
}

/// Whether `variant`'s payments are split from control on `endpoint`.
fn variant_splits_on(
    data: &ABTestData,
    control: &ExperimentArm,
    variant: &ResolvedVariant,
    endpoint: ExperimentEndpoint,
) -> bool {
    !variant.stopped
        && in_scope(data, endpoint)
        && routes_differently(control, &variant.arm, endpoint)
}

/// Whether traffic on `endpoint` is split between control and at least one running variant.
pub fn splits_on(data: &ABTestData, endpoint: ExperimentEndpoint) -> bool {
    let control = resolved_control(data);
    resolved_variants(data)
        .iter()
        .any(|variant| variant_splits_on(data, &control, variant, endpoint))
}

/// The variants still receiving their traffic, i.e. not stopped by the guardrail.
pub fn running_variant_ids(data: &ABTestData) -> Vec<String> {
    resolved_variants(data)
        .into_iter()
        .filter(|variant| !variant.stopped)
        .map(|variant| variant.id)
        .collect()
}

pub fn plan(data: &ABTestData, endpoint: ExperimentEndpoint, payment_id: &str) -> EndpointPlan {
    let control = resolved_control(data);
    let variants = resolved_variants(data);
    let untracked_control = || EndpointPlan {
        side: ArmSide::Control,
        projection: project(&control, endpoint),
        in_experiment: false,
    };
    match super::common::assign_arm(payment_id, &variants) {
        ArmSide::Control => {
            let tracked = variants
                .iter()
                .any(|variant| variant_splits_on(data, &control, variant, endpoint));
            EndpointPlan {
                side: ArmSide::Control,
                projection: project(&control, endpoint),
                in_experiment: tracked,
            }
        }
        ArmSide::Variant(id) => match variants.iter().find(|variant| variant.id == id) {
            Some(variant) if variant_splits_on(data, &control, variant, endpoint) => EndpointPlan {
                side: ArmSide::Variant(id),
                projection: project(&variant.arm, endpoint),
                in_experiment: true,
            },
            _ => untracked_control(),
        },
    }
}

fn is_valid_variant_id(id: &str) -> bool {
    (1..=32).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

/// Write-time checks for an experiment definition. Returns `(field, message)` pairs.
pub fn validate(data: &ABTestData) -> Vec<(String, String)> {
    let mut errors: Vec<(String, String)> = Vec::new();
    let multi = data.variants.is_some();
    let field = |index: usize| {
        if multi {
            format!("variants[{index}]")
        } else {
            "variant".to_string()
        }
    };

    let control = resolved_control(data);
    if control.is_empty() {
        errors.push((
            "control".into(),
            "control arm needs a rule layer, an SR layer, or both".into(),
        ));
    }

    if multi
        && (data.variant.is_some()
            || data.variant_algorithm_id.is_some()
            || data.variant_sr_config.is_some()
            || data.variant_split_pct != 0)
    {
        errors.push((
            "variants".into(),
            "use either variants or the single-variant fields (variant, variant_algorithm_id, variant_sr_config, variant_split_pct), not both".into(),
        ));
    }

    let variants = resolved_variants(data);
    if variants.is_empty() {
        errors.push(("variants".into(), "add at least one variant".into()));
    }
    if variants.len() > MAX_EXPERIMENT_VARIANTS {
        errors.push((
            "variants".into(),
            format!("an experiment can have at most {MAX_EXPERIMENT_VARIANTS} variants"),
        ));
    }
    for (index, variant) in variants.iter().enumerate() {
        if multi {
            if !is_valid_variant_id(&variant.id) {
                errors.push((
                    format!("{}.id", field(index)),
                    "variant id must be 1–32 lowercase letters, digits, '_' or '-'".into(),
                ));
            } else if variant.id == CONTROL_ARM_ID {
                errors.push((
                    format!("{}.id", field(index)),
                    "variant id cannot be 'control'".into(),
                ));
            } else if variants[..index].iter().any(|other| other.id == variant.id) {
                errors.push((
                    format!("{}.id", field(index)),
                    format!("variant id '{}' is used more than once", variant.id),
                ));
            }
        }
        if variant.split_pct == 0 {
            let split_field = if multi {
                format!("{}.split_pct", field(index))
            } else {
                "variant_split_pct".to_string()
            };
            errors.push((split_field, "variant traffic must be at least 1%".into()));
        }
        if variant.arm.is_empty() {
            errors.push((
                field(index),
                format!(
                    "{} arm needs a rule layer, an SR layer, or both",
                    variant.id
                ),
            ));
        }
    }
    let total_split: u32 = variants.iter().map(|v| u32::from(v.split_pct)).sum();
    if total_split > MAX_TOTAL_VARIANT_SPLIT_PCT {
        let split_field = if multi {
            "variants"
        } else {
            "variant_split_pct"
        };
        errors.push((
            split_field.into(),
            format!(
                "variants together can take at most {MAX_TOTAL_VARIANT_SPLIT_PCT}% of traffic, so control keeps the majority (got {total_split}%)"
            ),
        ));
    }

    if let Some(endpoints) = &data.endpoints {
        if endpoints.is_empty() {
            errors.push((
                "endpoints".into(),
                "select at least one endpoint, or omit endpoints to apply everywhere".into(),
            ));
        }
        if endpoints.contains(&ExperimentEndpoint::Unknown) {
            errors.push((
                "endpoints".into(),
                "endpoints must be hybrid_routing, decide_gateway or evaluate".into(),
            ));
        }
    }

    if errors.is_empty() {
        let scoped: Vec<ExperimentEndpoint> = ExperimentEndpoint::ALL
            .into_iter()
            .filter(|endpoint| in_scope(data, *endpoint))
            .collect();
        for (index, variant) in variants.iter().enumerate() {
            if !scoped
                .iter()
                .any(|endpoint| routes_differently(&control, &variant.arm, *endpoint))
            {
                let message = if multi {
                    format!(
                        "{} applies the same layers as control on every selected endpoint",
                        variant.id
                    )
                } else {
                    "control and variant apply the same layers on every selected endpoint".into()
                };
                errors.push((field(index), message));
                continue;
            }
            if let Some(twin) = variants[..index].iter().find(|other| {
                !scoped
                    .iter()
                    .any(|endpoint| routes_differently(&other.arm, &variant.arm, *endpoint))
            }) {
                errors.push((
                    field(index),
                    format!(
                        "{} applies the same layers as {} on every selected endpoint",
                        variant.id, twin.id
                    ),
                ));
            }
        }
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::euclid::types::ExperimentVariant;

    fn sr(autopilot: bool, cost: bool) -> SrConfigOverride {
        SrConfigOverride {
            use_autopilot: Some(autopilot),
            enable_multi_objective: Some(cost),
            ..Default::default()
        }
    }

    fn rule(id: &str) -> ExperimentArm {
        ExperimentArm {
            rule_algorithm_id: Some(id.into()),
            sr: None,
        }
    }

    fn layered(control: ExperimentArm, variant: ExperimentArm) -> ABTestData {
        ABTestData {
            control: Some(control),
            variants: None,
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

    fn variant(id: &str, split_pct: u8, arm: ExperimentArm) -> ExperimentVariant {
        ExperimentVariant {
            id: id.into(),
            name: None,
            split_pct,
            arm,
            stopped: false,
        }
    }

    fn multi(control: ExperimentArm, variants: Vec<ExperimentVariant>) -> ABTestData {
        ABTestData {
            variants: Some(variants),
            variant: None,
            variant_split_pct: 0,
            ..layered(control, ExperimentArm::default())
        }
    }

    fn variant_side(id: &str) -> ArmSide {
        ArmSide::Variant(id.into())
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

    /// Control rule₁ against three rule variants at 10% each.
    fn three_variant_experiment() -> ABTestData {
        multi(
            rule("rule_1"),
            vec![
                variant("rule_a", 10, rule("rule_a")),
                variant("rule_b", 10, rule("rule_b")),
                variant("rule_c", 10, rule("rule_c")),
            ],
        )
    }

    #[test]
    fn each_endpoint_applies_only_its_layers() {
        let data = rule_and_sr_experiment();
        let variant = resolved_arm(&data, &variant_side(LEGACY_VARIANT_ARM_ID));

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

        assert_eq!(resolved_control(&data), rule("rule_1"));
        assert_eq!(
            resolved_variants(&data),
            vec![ResolvedVariant {
                id: LEGACY_VARIANT_ARM_ID.into(),
                split_pct: 10,
                arm: ExperimentArm {
                    rule_algorithm_id: None,
                    sr: Some(sr(false, true)),
                },
                stopped: false,
            }]
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
            resolved_control(&data).sr,
            Some(SrConfigOverride::default())
        );
        assert!(splits_on(&data, ExperimentEndpoint::DecideGateway));
        assert!(!splits_on(&data, ExperimentEndpoint::Evaluate));
    }

    #[test]
    fn validation_rejects_empty_and_identical_arms() {
        let fields = |data: &ABTestData| -> Vec<String> {
            validate(data).into_iter().map(|(f, _)| f).collect()
        };
        let empty = layered(ExperimentArm::default(), ExperimentArm::default());
        assert_eq!(fields(&empty), vec!["control", "variant"]);

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

        let mut no_traffic = rule_and_sr_experiment();
        no_traffic.variant_split_pct = 0;
        assert_eq!(fields(&no_traffic), vec!["variant_split_pct"]);

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

    #[test]
    fn variants_deserialize_with_flattened_layers() {
        let data: ABTestData = serde_json::from_value(serde_json::json!({
            "control": { "rule_algorithm_id": "rule_1" },
            "variants": [
                { "id": "rule_a", "split_pct": 10, "rule_algorithm_id": "rule_a" },
                { "id": "sr_cost", "name": "SR + cost", "split_pct": 15,
                  "sr": { "enable_multi_objective": true } }
            ],
            "min_sample_size": 1000,
            "guardrail_threshold_pp": 3.0
        }))
        .unwrap();
        let variants = resolved_variants(&data);
        assert_eq!(variants.len(), 2);
        assert_eq!(variants[0].arm, rule("rule_a"));
        assert_eq!(variants[1].split_pct, 15);
        assert_eq!(
            variants[1]
                .arm
                .sr
                .as_ref()
                .and_then(|s| s.enable_multi_objective),
            Some(true)
        );
        assert!(validate(&data).is_empty());
        // `stopped` is only written once the guardrail sets it.
        let stored = serde_json::to_value(&data).unwrap();
        assert!(stored["variants"][0].get("stopped").is_none());
        // The single-variant split is not written for multi-variant experiments.
        assert!(stored.get("variant_split_pct").is_none());
    }

    #[test]
    fn variants_take_their_slices_and_control_the_rest() {
        let data = three_variant_experiment();
        let mut counts = std::collections::HashMap::<String, u32>::new();
        for i in 0..10_000 {
            let planned = plan(&data, ExperimentEndpoint::Evaluate, &format!("pay_{i}"));
            assert!(planned.in_experiment);
            assert_eq!(
                planned.projection.rule_algorithm_id.as_deref(),
                Some(match &planned.side {
                    ArmSide::Control => "rule_1",
                    ArmSide::Variant(id) => id.as_str(),
                })
            );
            *counts.entry(planned.side.as_str().to_string()).or_default() += 1;
        }
        for id in ["rule_a", "rule_b", "rule_c"] {
            let share = counts[id];
            assert!((800..1200).contains(&share), "{id}: {share}");
        }
        assert!(counts[CONTROL_ARM_ID] > 6_500);
    }

    #[test]
    fn single_variant_slice_is_unchanged() {
        // The first variant keeps the hash slice the single-variant format always used, so
        // `variants: [v]` assigns every payment exactly as `variant: v` did.
        let legacy = layered(rule("rule_1"), rule("rule_2"));
        let as_list = multi(
            rule("rule_1"),
            vec![variant(LEGACY_VARIANT_ARM_ID, 10, rule("rule_2"))],
        );
        for i in 0..500 {
            let payment_id = format!("pay_{i}");
            assert_eq!(
                plan(&legacy, ExperimentEndpoint::Evaluate, &payment_id),
                plan(&as_list, ExperimentEndpoint::Evaluate, &payment_id),
            );
        }
    }

    #[test]
    fn a_stopped_variant_serves_control_untracked_and_keeps_other_slices() {
        let running = three_variant_experiment();
        let mut stopped = running.clone();
        stopped.variants.as_mut().unwrap()[1].stopped = true;
        assert_eq!(running_variant_ids(&stopped), vec!["rule_a", "rule_c"]);

        for i in 0..2_000 {
            let payment_id = format!("pay_{i}");
            let before = plan(&running, ExperimentEndpoint::Evaluate, &payment_id);
            let after = plan(&stopped, ExperimentEndpoint::Evaluate, &payment_id);
            if before.side == variant_side("rule_b") {
                assert_eq!(after.side, ArmSide::Control);
                assert!(!after.in_experiment);
                assert_eq!(
                    after.projection.rule_algorithm_id.as_deref(),
                    Some("rule_1")
                );
            } else {
                assert_eq!(before, after, "{payment_id}");
            }
        }
    }

    #[test]
    fn a_variant_identical_to_control_on_an_endpoint_is_untracked_there() {
        // rule_a only differs from control in its rule, sr_cost only in its SR layer.
        let control = ExperimentArm {
            rule_algorithm_id: Some("rule_1".into()),
            sr: Some(sr(false, false)),
        };
        let data = multi(
            control.clone(),
            vec![
                variant(
                    "rule_a",
                    20,
                    ExperimentArm {
                        rule_algorithm_id: Some("rule_a".into()),
                        sr: control.sr.clone(),
                    },
                ),
                variant(
                    "sr_cost",
                    20,
                    ExperimentArm {
                        rule_algorithm_id: control.rule_algorithm_id.clone(),
                        sr: Some(sr(false, true)),
                    },
                ),
            ],
        );
        let mut seen = std::collections::HashSet::new();
        for i in 0..500 {
            let payment_id = format!("pay_{i}");
            let assigned = plan(&data, ExperimentEndpoint::HybridRouting, &payment_id).side;
            let on_decide = plan(&data, ExperimentEndpoint::DecideGateway, &payment_id);
            let on_evaluate = plan(&data, ExperimentEndpoint::Evaluate, &payment_id);
            match assigned.as_str() {
                "rule_a" => {
                    assert!(!on_decide.in_experiment);
                    assert_eq!(on_evaluate.side, assigned);
                }
                "sr_cost" => {
                    assert_eq!(on_decide.side, assigned);
                    assert!(!on_evaluate.in_experiment);
                }
                _ => {
                    // Control is tracked wherever some variant is split.
                    assert!(on_decide.in_experiment && on_evaluate.in_experiment);
                }
            }
            seen.insert(assigned);
        }
        assert_eq!(seen.len(), 3);
    }

    #[test]
    fn validation_checks_each_variant() {
        let fields = |data: &ABTestData| -> Vec<String> {
            validate(data).into_iter().map(|(f, _)| f).collect()
        };
        assert!(validate(&three_variant_experiment()).is_empty());

        let no_variants = multi(rule("rule_1"), vec![]);
        assert_eq!(fields(&no_variants), vec!["variants"]);

        let too_many = multi(
            rule("rule_1"),
            (0..6)
                .map(|i| variant(&format!("v{i}"), 5, rule(&format!("rule_{i}x"))))
                .collect(),
        );
        assert_eq!(fields(&too_many), vec!["variants"]);

        let over_half = multi(
            rule("rule_1"),
            vec![
                variant("rule_a", 30, rule("rule_a")),
                variant("rule_b", 21, rule("rule_b")),
            ],
        );
        assert_eq!(fields(&over_half), vec!["variants"]);

        let bad_ids = multi(
            rule("rule_1"),
            vec![
                variant("control", 5, rule("rule_a")),
                variant("Rule B", 5, rule("rule_b")),
                variant("rule_c", 5, rule("rule_c")),
                variant("rule_c", 5, rule("rule_d")),
            ],
        );
        assert_eq!(
            fields(&bad_ids),
            vec!["variants[0].id", "variants[1].id", "variants[3].id"]
        );

        let no_split = multi(rule("rule_1"), vec![variant("rule_a", 0, rule("rule_a"))]);
        assert_eq!(fields(&no_split), vec!["variants[0].split_pct"]);

        let empty_arm = multi(
            rule("rule_1"),
            vec![variant("rule_a", 10, ExperimentArm::default())],
        );
        assert_eq!(fields(&empty_arm), vec!["variants[0]"]);

        let same_as_control = multi(
            rule("rule_1"),
            vec![
                variant("rule_a", 10, rule("rule_a")),
                variant("rule_b", 10, rule("rule_1")),
            ],
        );
        assert_eq!(fields(&same_as_control), vec!["variants[1]"]);

        let twins = multi(
            rule("rule_1"),
            vec![
                variant("rule_a", 10, rule("rule_a")),
                variant("rule_a_again", 10, rule("rule_a")),
            ],
        );
        assert_eq!(fields(&twins), vec!["variants[1]"]);

        let mut mixed = three_variant_experiment();
        mixed.variant_split_pct = 10;
        assert_eq!(fields(&mixed), vec!["variants"]);
    }
}
