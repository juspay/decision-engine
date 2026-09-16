//! A/B experiment configuration: what an experiment is, what each of its two arms does, and
//! which arm a given payment belongs to.
//!
//! Data only, and deliberately so. These types are read by the euclid rule layer (an experiment
//! is stored as a `StaticRoutingAlgorithm` variant), by the decider (which executes the assigned
//! arm), by the routing handlers (which create and preview experiments) and by the analytics
//! endpoints (which report on them) — so they can depend on none of those. The behaviour that
//! uses them lives in `crate::decider::gatewaydecider::ab_test`.

/// Deterministic arm assignment using djb2 hash of payment_id.
/// Returns "variant" if hash % 100 < variant_split_pct, else "control".
/// Same payment_id always returns the same arm — retries land on the same gateway.
pub fn assign_arm(payment_id: &str, variant_split_pct: u8) -> &'static str {
    let hash = payment_id.bytes().fold(5381u64, |acc, b| {
        acc.wrapping_mul(33).wrapping_add(b as u64)
    });
    if (hash % 100) < variant_split_pct as u64 {
        "variant"
    } else {
        "control"
    }
}

/// The experiment arm this payment was routed under, echoed on the decide response.
///
/// Without it a client cannot tell a variant (SR) arm payment from a payment outside the
/// experiment — both come back as ordinary SR decisions. Only the control/static arm is
/// self-evident, via `routing_approach = AB_TEST_STATIC_ALGORITHM`. Absent when no A/B test
/// applied to the payment.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AbTestInfo {
    pub experiment_id: String,
    /// `"control"` or `"variant"`, as assigned by [`assign_arm`].
    pub arm: String,
    /// The arm's routing algorithm id — `"sr_routing"` for the dynamic arm, otherwise the
    /// stored static algorithm's id.
    pub arm_algorithm: String,
}

/// Per-arm routing overrides for A/B experiments. Applied inside scoring_flow / the
/// multi-objective post-step before gateway selection, so no isolated score pools are
/// needed. All fields are optional — an absent field falls through to the merchant
/// config / feature flag / default, exactly as if no override were present.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SrConfigOverride {
    /// Share of traffic (0–100) sent to non-top gateways to keep scores fresh (explore-exploit).
    /// Overrides `defaultHedgingPercent` from the merchant's SR config.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hedging_percent: Option<f64>,
    /// SR score threshold (0–1) below which a gateway is eliminated from routing.
    /// Overrides the merchant's elimination rule threshold.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elimination_threshold: Option<f64>,
    /// Whether the multi-objective (cost-aware) post-step runs for this arm. Overrides the
    /// merchant `multi_objective_routing_enabled` flag / per-request value. Used by the
    /// "Turn cost on" experiment (control = false, variant = true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_multi_objective: Option<bool>,
    /// Merchant margin (fraction of ticket) the multi-objective EV ranking applies for this
    /// arm. Overrides `SuccessRateData.margin`. Lower margin lets cost win more often.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub margin: Option<f64>,
    /// Whether this arm honors autopilot-calibrated (source = "autopilot") bucket/hedging
    /// sub-level config. When `false`, autopilot-sourced entries are skipped and the arm
    /// falls back to the merchant's manual/default config. Used by the "Autopilot value"
    /// experiment (control = false → manual, variant = true → live autopilot).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_autopilot: Option<bool>,
    /// Whether the volume-commitment steering nudge runs for this arm. Overrides the merchant
    /// `volume_commitment_routing_enabled` feature flag. Both arms read the same commitment
    /// document from the `volume_commitment` slot, so the experiment compares steering on
    /// against steering off against one set of targets. An arm with steering requested but no
    /// active plan simply routes without it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enable_volume_commitment: Option<bool>,
}

/// The reserved arm algorithm id meaning "no stored rule, run the dynamic decider". Legacy
/// `ABTestData` rows carry it in `control_algorithm_id` / `variant_algorithm_id`; [`ArmStrategy`]
/// encodes the same thing as a variant.
pub const SR_ROUTING_ARM: &str = "sr_routing";

/// What one arm of an experiment actually does with a payment.
///
/// An arm is a routing *configuration*, not a single algorithm: the two legs the engine has
/// (a stored rule, and the dynamic success-rate decider) can each be present or absent, and a
/// merchant on hybrid routing needs both in the same arm. [`Self::Hybrid`] is the general case
/// — the rule narrows the candidate set, SR picks among the survivors, which is the same
/// composition `/routing/hybrid` performs at the route level.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ArmStrategy {
    /// Rule only. The rule's first output is the decision and SR never runs.
    Rule { algorithm_id: String },
    /// SR only, over whatever the decider's own filters admit.
    Sr {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sr_config: Option<SrConfigOverride>,
    },
    /// Rule then SR: the rule's outputs become the SR candidate set.
    Hybrid {
        algorithm_id: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        sr_config: Option<SrConfigOverride>,
    },
    /// "Whatever this merchant runs right now." Carries no settings of its own: both legs are
    /// read off the merchant's live configuration when the experiment is created, and the arm is
    /// stored as the concrete `Rule`/`Sr`/`Hybrid` that resolution produced. A control arm is
    /// therefore never typed out — which is the point, since a control that is not the merchant's
    /// production behaviour makes the whole comparison meaningless.
    Current,
    /// A shape written by a newer version of the engine. Payments assigned to it are excluded
    /// from the experiment and routed normally rather than guessed at.
    #[serde(other)]
    Unknown,
}

impl ArmStrategy {
    /// The stored rule this arm evaluates, if it has one.
    pub fn algorithm_id(&self) -> Option<&str> {
        match self {
            Self::Rule { algorithm_id } | Self::Hybrid { algorithm_id, .. } => Some(algorithm_id),
            Self::Sr { .. } | Self::Current | Self::Unknown => None,
        }
    }

    /// The arm's SR overrides. `None` means "use the merchant's live SR config", which is what
    /// keeps a control arm a faithful reflection of production.
    pub fn sr_config(&self) -> Option<&SrConfigOverride> {
        match self {
            Self::Sr { sr_config } | Self::Hybrid { sr_config, .. } => sr_config.as_ref(),
            Self::Rule { .. } | Self::Current | Self::Unknown => None,
        }
    }

    /// Whether the dynamic decider runs for this arm. Drives whether the payment's gateway
    /// score feedback is real (`Hybrid`/`Sr`) or absent (`Rule`).
    pub fn runs_sr(&self) -> bool {
        matches!(self, Self::Sr { .. } | Self::Hybrid { .. })
    }

    /// Human-readable arm algorithm label for analytics and the `ab_test_info` response field.
    pub fn label(&self) -> String {
        match self {
            Self::Rule { algorithm_id } => algorithm_id.clone(),
            Self::Sr { .. } => SR_ROUTING_ARM.to_string(),
            Self::Hybrid { algorithm_id, .. } => format!("{algorithm_id}+{SR_ROUTING_ARM}"),
            // Only reachable if an unresolved arm was stored, which `routing_create` prevents.
            Self::Current => "current".to_string(),
            Self::Unknown => "unknown".to_string(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(from = "AbTestDataRepr", into = "AbTestDataRepr")]
pub struct ABTestData {
    pub control: ArmStrategy,
    pub variant: ArmStrategy,
    /// Percentage of traffic routed to the variant arm (1–49).
    pub variant_split_pct: u8,
    /// Minimum transactions to collect before reporting a significance verdict.
    pub min_sample_size: u32,
    /// Threshold at which the results view flags the variant as underperforming: variant auth
    /// rate more than this many pp below control. Advisory — nothing pauses routing on it.
    pub guardrail_threshold_pp: f64,
}

impl ABTestData {
    /// The arm `payment_id` is assigned to, paired with its name.
    pub fn arm_for(&self, payment_id: &str) -> (&'static str, &ArmStrategy) {
        let arm = assign_arm(payment_id, self.variant_split_pct);
        if arm == "variant" {
            (arm, &self.variant)
        } else {
            (arm, &self.control)
        }
    }
}

/// Wire/storage shape for [`ABTestData`], accepting both the current per-arm
/// [`ArmStrategy`] form and the original flat `*_algorithm_id` + `*_sr_config` form that
/// existing `routing_algorithm` rows hold. Serialization always writes the current form.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AbTestDataRepr {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    control: Option<ArmStrategy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    variant: Option<ArmStrategy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    control_algorithm_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    variant_algorithm_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    control_sr_config: Option<SrConfigOverride>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    variant_sr_config: Option<SrConfigOverride>,
    variant_split_pct: u8,
    min_sample_size: u32,
    guardrail_threshold_pp: f64,
}

/// A flat-form arm becomes `Sr` for the reserved id and `Rule` otherwise: before
/// [`ArmStrategy`] a stored-rule arm returned its rule's output directly and never reached SR,
/// so `Rule` — not `Hybrid` — is what those rows have always meant.
fn arm_from_legacy(
    algorithm_id: Option<String>,
    sr_config: Option<SrConfigOverride>,
) -> ArmStrategy {
    match algorithm_id.as_deref() {
        Some(SR_ROUTING_ARM) => ArmStrategy::Sr { sr_config },
        Some(id) => ArmStrategy::Rule {
            algorithm_id: id.to_string(),
        },
        None => ArmStrategy::Unknown,
    }
}

impl From<AbTestDataRepr> for ABTestData {
    fn from(repr: AbTestDataRepr) -> Self {
        Self {
            control: repr.control.unwrap_or_else(|| {
                arm_from_legacy(repr.control_algorithm_id, repr.control_sr_config)
            }),
            variant: repr.variant.unwrap_or_else(|| {
                arm_from_legacy(repr.variant_algorithm_id, repr.variant_sr_config)
            }),
            variant_split_pct: repr.variant_split_pct,
            min_sample_size: repr.min_sample_size,
            guardrail_threshold_pp: repr.guardrail_threshold_pp,
        }
    }
}

impl From<ABTestData> for AbTestDataRepr {
    fn from(data: ABTestData) -> Self {
        Self {
            control: Some(data.control),
            variant: Some(data.variant),
            control_algorithm_id: None,
            variant_algorithm_id: None,
            control_sr_config: None,
            variant_sr_config: None,
            variant_split_pct: data.variant_split_pct,
            min_sample_size: data.min_sample_size,
            guardrail_threshold_pp: data.guardrail_threshold_pp,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ABTestData, ArmStrategy, SrConfigOverride};
    use crate::euclid::types::StaticRoutingAlgorithm;

    /// Rows written before `ArmStrategy` existed must keep routing the way they always have:
    /// the reserved id is the dynamic arm, anything else is a rule that answers on its own.
    #[test]
    fn legacy_flat_shape_still_parses() {
        let json = r#"{
            "control_algorithm_id": "routing_abc",
            "variant_algorithm_id": "sr_routing",
            "variant_split_pct": 5,
            "min_sample_size": 1000,
            "guardrail_threshold_pp": 3.0,
            "variant_sr_config": { "enable_multi_objective": true }
        }"#;
        let data: ABTestData = serde_json::from_str(json).unwrap();

        assert_eq!(
            data.control,
            ArmStrategy::Rule {
                algorithm_id: "routing_abc".to_string()
            }
        );
        assert!(!data.control.runs_sr());
        assert_eq!(
            data.variant,
            ArmStrategy::Sr {
                sr_config: Some(SrConfigOverride {
                    hedging_percent: None,
                    elimination_threshold: None,
                    enable_multi_objective: Some(true),
                    margin: None,
                    use_autopilot: None,
                    enable_volume_commitment: None,
                })
            }
        );
        assert_eq!(data.variant_split_pct, 5);
    }

    #[test]
    fn hybrid_arms_round_trip() {
        let data = ABTestData {
            control: ArmStrategy::Hybrid {
                algorithm_id: "routing_r1".to_string(),
                sr_config: None,
            },
            variant: ArmStrategy::Hybrid {
                algorithm_id: "routing_r2".to_string(),
                sr_config: Some(SrConfigOverride {
                    hedging_percent: None,
                    elimination_threshold: None,
                    enable_multi_objective: Some(true),
                    margin: Some(0.012),
                    use_autopilot: Some(true),
                    enable_volume_commitment: Some(true),
                }),
            },
            variant_split_pct: 5,
            min_sample_size: 1000,
            guardrail_threshold_pp: 3.0,
        };

        let encoded = serde_json::to_string(&data).unwrap();
        // Serialization writes only the current shape.
        assert!(!encoded.contains("control_algorithm_id"));

        let decoded: ABTestData = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded.control, data.control);
        assert_eq!(decoded.variant, data.variant);
        assert!(decoded.control.runs_sr());
        assert_eq!(decoded.variant.algorithm_id(), Some("routing_r2"));
        assert_eq!(
            decoded.variant.sr_config().and_then(|c| c.margin),
            Some(0.012)
        );
    }

    /// An arm shape from a newer build decodes to `Unknown` rather than failing the whole
    /// experiment, so the interceptor can exclude those payments and route them normally.
    #[test]
    fn unrecognized_arm_kind_is_unknown() {
        let json = r#"{
            "control": { "kind": "rule", "algorithm_id": "routing_abc" },
            "variant": { "kind": "some_future_shape", "whatever": 1 },
            "variant_split_pct": 5,
            "min_sample_size": 1000,
            "guardrail_threshold_pp": 3.0
        }"#;
        let data: ABTestData = serde_json::from_str(json).unwrap();
        assert_eq!(data.variant, ArmStrategy::Unknown);
        assert_eq!(data.variant.algorithm_id(), None);
        assert!(!data.variant.runs_sr());
    }

    /// `current` asks for nothing and answers nothing: both legs come from the merchant's live
    /// configuration when `routing_create` resolves it into a concrete arm.
    #[test]
    fn current_arm_names_no_configuration() {
        let json = r#"{
            "control": { "kind": "current" },
            "variant": { "kind": "sr" },
            "variant_split_pct": 10,
            "min_sample_size": 500,
            "guardrail_threshold_pp": 2.0
        }"#;
        let data: ABTestData = serde_json::from_str(json).unwrap();
        assert_eq!(data.control, ArmStrategy::Current);
        assert_eq!(data.control.algorithm_id(), None);
        assert_eq!(data.control.sr_config(), None);
        assert!(!data.control.runs_sr());
    }

    /// The identical-arms guard in `routing_create` leans on this, so an arm's equality has to
    /// account for both legs and not just the rule.
    #[test]
    fn arms_differing_only_in_their_sr_leg_are_not_equal() {
        let rule_only = ArmStrategy::Rule {
            algorithm_id: "routing_abc".into(),
        };
        let hybrid = ArmStrategy::Hybrid {
            algorithm_id: "routing_abc".into(),
            sr_config: None,
        };
        assert_ne!(rule_only, hybrid);
        assert_eq!(hybrid.algorithm_id(), rule_only.algorithm_id());
    }

    #[test]
    fn arm_labels_name_both_legs() {
        assert_eq!(
            ArmStrategy::Rule {
                algorithm_id: "routing_abc".into()
            }
            .label(),
            "routing_abc"
        );
        assert_eq!(ArmStrategy::Sr { sr_config: None }.label(), "sr_routing");
        assert_eq!(
            ArmStrategy::Hybrid {
                algorithm_id: "routing_abc".into(),
                sr_config: None
            }
            .label(),
            "routing_abc+sr_routing"
        );
    }

    /// The experiment rides inside `StaticRoutingAlgorithm`, so a stored row's outer tagging
    /// has to survive the arm rework too.
    #[test]
    fn legacy_row_parses_through_the_algorithm_envelope() {
        let json = r#"{
            "type": "ab_test",
            "data": {
                "control_algorithm_id": "sr_routing",
                "variant_algorithm_id": "sr_routing",
                "variant_split_pct": 20,
                "min_sample_size": 2000,
                "guardrail_threshold_pp": 1.5,
                "control_sr_config": { "use_autopilot": false },
                "variant_sr_config": { "use_autopilot": true }
            }
        }"#;
        let algorithm: StaticRoutingAlgorithm = serde_json::from_str(json).unwrap();
        let StaticRoutingAlgorithm::AbTest(data) = algorithm else {
            panic!("expected an ab_test algorithm");
        };
        assert!(data.control.runs_sr() && data.variant.runs_sr());
        assert_eq!(
            data.control.sr_config().and_then(|c| c.use_autopilot),
            Some(false)
        );
        assert_eq!(
            data.variant.sr_config().and_then(|c| c.use_autopilot),
            Some(true)
        );
    }
}
