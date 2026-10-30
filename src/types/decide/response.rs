//! The `/decide` answer: one ranked connector list, an outcome that says how it was reached, and
//! one section per routing layer. It is written from the same [`RoutingEvaluation`] as the audit
//! record (which `/routing/hybrid` returns) and carries every fact that record does. The
//! conversions below destructure each decider type in full, so a field added there fails this
//! build until it is given a place here.
//!
//! The decider's own types serialize in camelCase for `/decide-gateway` and the analytics queries,
//! so the sections that hold them are snake_case copies. Types that already serialize in
//! snake_case are reused as they are.

use super::DecideConnector;
use crate::decider::gatewaydecider::multi_objective::{
    CostModel, CostSource, MultiObjectiveInfo, MultiObjectiveOutcome, PspSummary, RankedPsp,
};
use crate::decider::gatewaydecider::types::{
    DecidedGateway, GatewayDeciderApproach, GatewayPriorityLogicOutput, PriorityLogicData,
    ResetApproach,
};
use crate::decider::gatewaydecider::volume_commitment::nudge::{
    BlockedCommitment, SteerBlock, VolumeSteerInfo, VolumeSteerOutcome,
};
use crate::decider::network_decider::types::DebitRoutingOutput;
use crate::euclid::types::{RoutingEvaluateResponse, RuleSelection};
use crate::types::routing_evaluation::{RoutingEvaluation, SuccessRateOutcome};
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Serialize)]
pub struct DecideResponse {
    /// Unique per decision, so a retried payment's decisions stay apart.
    pub id: String,
    pub object: &'static str,
    pub payment_id: String,
    /// The call's `x-request-id`, or the one minted for it.
    pub request_id: String,
    pub created_at: String,
    /// The order to try connectors in: the first is the choice, the rest the retry order.
    pub connectors: Vec<DecideConnector>,
    pub outcome: Outcome,
    pub rule: Option<RuleSection>,
    pub success_rate: Option<SuccessRateSection>,
    pub cost: Option<CostSection>,
    pub volume_steering: Option<VolumeSteeringSection>,
    pub debit_routing: Option<DebitRoutingOutput>,
    /// Problems the decision routed around. Routing problems never fail the call.
    pub warnings: Vec<Warning>,
}

#[derive(Debug, Serialize)]
pub struct Outcome {
    #[serde(rename = "type")]
    pub kind: OutcomeType,
    pub strategy: Strategy,
    pub reason: Reason,
    /// For people reading logs. Not stable; branch on `reason`.
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeType {
    /// The configured routing produced the order.
    Routed,
    /// Routing could not apply, so the candidates, or the rule output unranked, answered.
    Fallback,
    /// No connector is left to try.
    NoConnector,
}

/// The layer that set the final order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Strategy {
    Rule,
    SuccessRate,
    Cost,
    VolumeCommitment,
    DebitNetwork,
    CandidateOrder,
    #[serde(rename = "none")]
    NotRouted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    RuleMatched,
    RuleDefaultSelection,
    RuleOutputPartiallyEligible,
    PriorityLogic,
    PriorityLogicDowntime,
    MerchantPreference,
    ExperimentRule,
    SuccessRateRanked,
    SuccessRateDowntime,
    SuccessRateExploration,
    DeciderDefault,
    PreferredConnector,
    CostOptimized,
    VolumeCommitment,
    DebitNetworkOptimized,
    NoRuleMatched,
    NoActiveRule,
    RuleOutputNotEligible,
    RuleInputInvalid,
    SuccessRateUnavailable,
    NoEligibleConnectors,
    NoActiveRuleNoCandidates,
}

impl Reason {
    fn message(self) -> &'static str {
        match self {
            Self::RuleMatched => "Routed by the active rule.",
            Self::RuleDefaultSelection => {
                "No rule matched; routed by the algorithm's default selection."
            }
            Self::RuleOutputPartiallyEligible => {
                "Routed by the active rule; connectors it chose that are not eligible were dropped."
            }
            Self::PriorityLogic => "Routed by priority logic.",
            Self::PriorityLogicDowntime => "Routed by priority logic, adjusted for downtime.",
            Self::MerchantPreference => "Routed by the merchant's preference.",
            Self::ExperimentRule => "Routed by the experiment arm's rule.",
            Self::SuccessRateRanked => "Ranked by success rate.",
            Self::SuccessRateDowntime => "Ranked by success rate, adjusted for downtime.",
            Self::SuccessRateExploration => "Ranked by success rate, exploring another connector.",
            Self::DeciderDefault => "The decider kept the default order.",
            Self::PreferredConnector => "Routed to the preferred connector.",
            Self::CostOptimized => "Chose a lower-cost connector within the auth-rate tolerance.",
            Self::VolumeCommitment => "Moved to a connector behind on its volume commitment.",
            Self::DebitNetworkOptimized => "Routed by debit network cost.",
            Self::NoRuleMatched => {
                "No rule matched; using the eligible connectors in request order."
            }
            Self::NoActiveRule => {
                "No active routing algorithm; using the eligible connectors in request order."
            }
            Self::RuleOutputNotEligible => {
                "None of the connectors the rule chose are eligible; using the eligible connectors in request order."
            }
            Self::RuleInputInvalid => {
                "The routing rule could not be evaluated; using the eligible connectors in request order."
            }
            Self::SuccessRateUnavailable => {
                "Success-rate ranking failed; using the order unranked."
            }
            Self::NoEligibleConnectors => "No eligible connector is left to try.",
            Self::NoActiveRuleNoCandidates => {
                "No active routing algorithm and no eligible connector to try."
            }
        }
    }
}

impl Outcome {
    fn new(kind: OutcomeType, strategy: Strategy, reason: Reason) -> Self {
        Self {
            kind,
            strategy,
            reason,
            message: reason.message().to_string(),
        }
    }

    /// The outcome when the rule layer set the order. `None` is a rule layer that failed.
    fn of_rule(selection: Option<RuleSelection>) -> Self {
        use OutcomeType::{Fallback, Routed};
        match selection {
            Some(RuleSelection::Matched) => Self::new(Routed, Strategy::Rule, Reason::RuleMatched),
            Some(RuleSelection::DefaultSelection) => {
                Self::new(Routed, Strategy::Rule, Reason::RuleDefaultSelection)
            }
            Some(RuleSelection::OutputPartiallyEligible) => {
                Self::new(Routed, Strategy::Rule, Reason::RuleOutputPartiallyEligible)
            }
            Some(RuleSelection::NoRuleMatched) => {
                Self::new(Fallback, Strategy::CandidateOrder, Reason::NoRuleMatched)
            }
            Some(RuleSelection::OutputNotEligible) => Self::new(
                Fallback,
                Strategy::CandidateOrder,
                Reason::RuleOutputNotEligible,
            ),
            Some(RuleSelection::NoActiveAlgorithm) => {
                Self::new(Fallback, Strategy::CandidateOrder, Reason::NoActiveRule)
            }
            None => Self::new(Fallback, Strategy::CandidateOrder, Reason::RuleInputInvalid),
        }
    }

    /// The outcome when the success-rate decider set the order.
    fn of_approach(approach: &GatewayDeciderApproach) -> Self {
        use GatewayDeciderApproach as A;
        let (strategy, reason) = match approach {
            A::SrSelection | A::SrSelectionV2Routing | A::SrSelectionV3Routing => {
                (Strategy::SuccessRate, Reason::SuccessRateRanked)
            }
            A::SrV2AllDowntimeRouting
            | A::SrV2DowntimeRouting
            | A::SrV2GlobalDowntimeRouting
            | A::SrV3AllDowntimeRouting
            | A::SrV3DowntimeRouting
            | A::SrV3GlobalDowntimeRouting => (Strategy::SuccessRate, Reason::SuccessRateDowntime),
            A::SrV2Hedging
            | A::SrV2AllDowntimeHedging
            | A::SrV2DowntimeHedging
            | A::SrV2GlobalDowntimeHedging
            | A::SrV3Hedging
            | A::SrV3AllDowntimeHedging
            | A::SrV3DowntimeHedging
            | A::SrV3GlobalDowntimeHedging => {
                (Strategy::SuccessRate, Reason::SuccessRateExploration)
            }
            A::PreferredConnectorRouting
            | A::PreferredConnectorAllDowntimeRouting
            | A::PreferredConnectorDowntimeRouting
            | A::PreferredConnectorGlobalDowntimeRouting => {
                (Strategy::SuccessRate, Reason::PreferredConnector)
            }
            A::SrSelectionMultiObjective => (Strategy::Cost, Reason::CostOptimized),
            A::SrSelectionVolumeCommitment => {
                (Strategy::VolumeCommitment, Reason::VolumeCommitment)
            }
            A::NtwBasedRouting => (Strategy::DebitNetwork, Reason::DebitNetworkOptimized),
            A::PriorityLogic => (Strategy::Rule, Reason::PriorityLogic),
            A::PlAllDowntimeRouting | A::PlDowntimeRouting | A::PlGlobalDowntimeRouting => {
                (Strategy::Rule, Reason::PriorityLogicDowntime)
            }
            A::MerchantPreference => (Strategy::Rule, Reason::MerchantPreference),
            A::AbTestStaticAlgorithm => (Strategy::Rule, Reason::ExperimentRule),
            A::Default | A::None => (Strategy::CandidateOrder, Reason::DeciderDefault),
        };
        Self::new(OutcomeType::Routed, strategy, reason)
    }
}

#[derive(Debug, Serialize)]
pub struct Warning {
    pub code: &'static str,
    pub param: Option<&'static str>,
    pub message: String,
}

impl Warning {
    fn for_selection(selection: RuleSelection) -> Option<Self> {
        let (code, message) = match selection {
            RuleSelection::OutputNotEligible => (
                "rule_output_not_eligible",
                "The rule chose connectors that are not in eligible_connectors.",
            ),
            RuleSelection::OutputPartiallyEligible => (
                "rule_output_partially_eligible",
                "The rule chose some connectors that are not in eligible_connectors; they were dropped.",
            ),
            RuleSelection::Matched
            | RuleSelection::DefaultSelection
            | RuleSelection::NoRuleMatched
            | RuleSelection::NoActiveAlgorithm => return None,
        };
        Some(Self {
            code,
            param: Some("routing_config.eligible_connectors"),
            message: message.to_string(),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleStatus {
    Matched,
    DefaultSelection,
    NoActiveRule,
}

impl From<RuleSelection> for RuleStatus {
    fn from(selection: RuleSelection) -> Self {
        match selection {
            RuleSelection::Matched => Self::Matched,
            RuleSelection::DefaultSelection
            | RuleSelection::NoRuleMatched
            | RuleSelection::OutputNotEligible
            | RuleSelection::OutputPartiallyEligible => Self::DefaultSelection,
            RuleSelection::NoActiveAlgorithm => Self::NoActiveRule,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct RuleSection {
    pub status: RuleStatus,
    /// The algorithm's output, by `type`: `straight_through`, `priority`, `volume_split` or
    /// `volume_split_priority`.
    pub output: serde_json::Value,
    /// The connectors the rule layer chose, narrowed to the eligible ones.
    pub selected: Vec<DecideConnector>,
    /// The connectors the payment method can use.
    pub eligible: Vec<DecideConnector>,
}

impl From<RoutingEvaluateResponse> for RuleSection {
    fn from(response: RoutingEvaluateResponse) -> Self {
        let RoutingEvaluateResponse {
            // Always the request's own `payment_id` on `/decide`.
            payment_id: _,
            // The same fact as `selection`, which names each case.
            status: _,
            output,
            evaluated_output,
            eligible_connectors,
            selection,
        } = response;
        Self {
            status: selection.into(),
            output: rename_connector_ids(output),
            selected: evaluated_output.into_iter().map(Into::into).collect(),
            eligible: eligible_connectors.into_iter().map(Into::into).collect(),
        }
    }
}

/// Rewrites the rule output's connector objects from `gateway_id` to
/// `payment_merchant_connector_id`, whatever the output type nests them under.
fn rename_connector_ids(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let is_connector = map.contains_key("gateway_name");
            serde_json::Value::Object(
                map.into_iter()
                    .map(|(key, value)| {
                        let key = if is_connector && key == "gateway_id" {
                            "payment_merchant_connector_id".to_string()
                        } else {
                            key
                        };
                        (key, rename_connector_ids(value))
                    })
                    .collect(),
            )
        }
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.into_iter().map(rename_connector_ids).collect())
        }
        other => other,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SuccessRateStatus {
    Applied,
    Failed,
}

#[derive(Debug, Serialize)]
pub struct SuccessRateSection {
    pub status: SuccessRateStatus,
    pub approach: Option<GatewayDeciderApproach>,
    /// Every scored connector, best first.
    pub scores: Vec<ConnectorScore>,
    pub scheduled_outage: Option<bool>,
    pub diagnostics: Option<SuccessRateDiagnostics>,
}

impl SuccessRateSection {
    fn failed() -> Self {
        Self {
            status: SuccessRateStatus::Failed,
            approach: None,
            scores: Vec::new(),
            scheduled_outage: None,
            diagnostics: None,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ConnectorScore {
    #[serde(flatten)]
    pub connector: DecideConnector,
    pub score: f64,
}

/// `gateway_priority_map` as a list, best score first, ties in identifier order.
fn scores(priority_map: Option<&serde_json::Value>) -> Vec<ConnectorScore> {
    let mut scores: Vec<(&String, f64)> = priority_map
        .and_then(serde_json::Value::as_object)
        .into_iter()
        .flatten()
        .filter_map(|(identifier, score)| score.as_f64().map(|score| (identifier, score)))
        .collect();
    scores.sort_by(|(left_id, left), (right_id, right)| {
        right.total_cmp(left).then_with(|| left_id.cmp(right_id))
    });
    scores
        .into_iter()
        .map(|(identifier, score)| ConnectorScore {
            connector: DecideConnector::from_identifier(identifier),
            score,
        })
        .collect()
}

/// The decider's internals for one decision, for support and the simulator.
#[derive(Debug, Serialize)]
pub struct SuccessRateDiagnostics {
    /// The connector at the head before downtime was checked.
    pub head_before_downtime_check: Option<DecideConnector>,
    pub reset_approach: ResetApproach,
    pub dimension: SuccessRateDimension,
    pub filter_wise_gateways: Option<serde_json::Value>,
    pub priority_logic: PriorityLogicSection,
    pub dynamic_mga_enabled: bool,
    pub gateway_mga_id_map: Option<serde_json::Value>,
    pub rust_based_decider: bool,
    /// The decider's CPU time.
    pub latency_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct SuccessRateDimension {
    pub value: Option<String>,
    pub level: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PriorityLogicSection {
    pub tag: Option<String>,
    pub output: Option<PriorityLogicOutput>,
}

#[derive(Debug, Serialize)]
pub struct PriorityLogicOutput {
    pub is_enforcement: bool,
    pub gws: Vec<String>,
    pub priority_logic_tag: Option<String>,
    pub gateway_reference_ids: HashMap<String, String>,
    pub primary_logic: Option<PriorityLogicData>,
    pub fallback_logic: Option<PriorityLogicData>,
}

impl From<GatewayPriorityLogicOutput> for PriorityLogicOutput {
    fn from(output: GatewayPriorityLogicOutput) -> Self {
        let GatewayPriorityLogicOutput {
            is_enforcement,
            gws,
            priority_logic_tag,
            gateway_reference_ids,
            primary_logic,
            fallback_logic,
        } = output;
        Self {
            is_enforcement,
            gws,
            priority_logic_tag,
            gateway_reference_ids: gateway_reference_ids.into_iter().collect(),
            primary_logic,
            fallback_logic,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct CostSection {
    pub outcome: MultiObjectiveOutcome,
    pub reason: String,
    pub cost_saved_bps: Option<f64>,
    pub qualified_count: usize,
    pub margin: f64,
    pub ev_gap_top2: Option<f64>,
    /// Every connector ranked on expected value, best first.
    pub ranked: Vec<RankedConnector>,
}

impl From<MultiObjectiveInfo> for CostSection {
    fn from(info: MultiObjectiveInfo) -> Self {
        let MultiObjectiveInfo {
            outcome,
            reason,
            cost_saved_bps,
            qualified_count,
            margin,
            ev_gap_top2,
            ranked,
        } = info;
        Self {
            outcome,
            reason,
            cost_saved_bps,
            qualified_count,
            margin,
            ev_gap_top2,
            ranked: ranked.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct RankedConnector {
    #[serde(flatten)]
    pub connector: DecideConnector,
    pub auth_rate: f64,
    pub cost_bps: Option<f64>,
    pub cost_source: Option<CostSource>,
    pub cost_model: Option<CostModelSection>,
    pub ev: f64,
    pub is_sr_head: bool,
    pub is_chosen: bool,
}

impl From<RankedPsp> for RankedConnector {
    fn from(ranked: RankedPsp) -> Self {
        let RankedPsp {
            summary:
                PspSummary {
                    psp,
                    auth_rate,
                    cost_bps,
                    cost_source,
                    cost_model,
                },
            ev,
            is_sr_head,
            is_chosen,
        } = ranked;
        Self {
            connector: DecideConnector::from_identifier(&psp),
            auth_rate,
            cost_bps,
            cost_source,
            cost_model: cost_model.map(Into::into),
            ev,
            is_sr_head,
            is_chosen,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct CostModelSection {
    pub brand: Option<String>,
    pub variant: Option<String>,
    pub issuer: Option<String>,
    pub ccy: Option<String>,
    pub ic_category: Option<String>,
    pub pct_bps: Option<f64>,
    pub fixed_fee: Option<f64>,
}

impl From<CostModel> for CostModelSection {
    fn from(model: CostModel) -> Self {
        let CostModel {
            brand,
            variant,
            issuer,
            ccy,
            ic_category,
            pct_bps,
            fixed_fee,
        } = model;
        Self {
            brand,
            variant,
            issuer,
            ccy,
            ic_category,
            pct_bps,
            fixed_fee,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct VolumeSteeringSection {
    pub outcome: VolumeSteerOutcome,
    pub reason: String,
    pub sr_head: Option<DecideConnector>,
    pub chosen: Option<DecideConnector>,
    pub sr_gap_conceded: Option<f64>,
    pub steer_rate: Option<f64>,
    pub steering_count: usize,
    pub run_id: Option<String>,
    pub blocked: Vec<BlockedConnector>,
}

#[derive(Debug, Serialize)]
pub struct BlockedConnector {
    pub connector: DecideConnector,
    pub gate: SteerBlock,
}

impl From<VolumeSteerInfo> for VolumeSteeringSection {
    fn from(info: VolumeSteerInfo) -> Self {
        let VolumeSteerInfo {
            outcome,
            reason,
            sr_head,
            chosen,
            sr_gap_conceded,
            steer_rate,
            steering_count,
            run_id,
            blocked,
        } = info;
        Self {
            outcome,
            reason,
            sr_head: sr_head.as_deref().map(DecideConnector::from_identifier),
            chosen: chosen.as_deref().map(DecideConnector::from_identifier),
            sr_gap_conceded,
            steer_rate,
            steering_count,
            run_id,
            blocked: blocked
                .into_iter()
                .map(|BlockedCommitment { connector, gate }| BlockedConnector {
                    connector: DecideConnector::from_identifier(&connector),
                    gate,
                })
                .collect(),
        }
    }
}

/// The parts of a decision the routing layers fill in.
struct Layers {
    connectors: Vec<DecideConnector>,
    outcome: Outcome,
    success_rate: Option<SuccessRateSection>,
    cost: Option<CostSection>,
    volume_steering: Option<VolumeSteeringSection>,
    debit_routing: Option<DebitRoutingOutput>,
}

impl From<DecidedGateway> for Layers {
    fn from(decision: DecidedGateway) -> Self {
        let DecidedGateway {
            decided_gateway,
            fallback_gateways,
            gateway_priority_map,
            filter_wise_gateways,
            priority_logic_tag,
            routing_approach,
            gateway_before_evaluation,
            priority_logic_output,
            debit_routing_output,
            reset_approach,
            routing_dimension,
            routing_dimension_level,
            is_scheduled_outage,
            is_dynamic_mga_enabled,
            gateway_mga_id_map,
            is_rust_based_decider,
            latency,
            multi_objective_info,
            volume_steer_info,
        } = decision;
        let cost: Option<CostSection> = multi_objective_info.map(Into::into);
        let volume_steering: Option<VolumeSteeringSection> = volume_steer_info.map(Into::into);
        let mut outcome = Outcome::of_approach(&routing_approach);
        // Cost and volume steering explain their own pick.
        let explanation = match outcome.reason {
            Reason::CostOptimized => cost.as_ref().map(|cost| &cost.reason),
            Reason::VolumeCommitment => volume_steering.as_ref().map(|steering| &steering.reason),
            _ => None,
        };
        if let Some(explanation) = explanation {
            outcome.message.clone_from(explanation);
        }
        Self {
            connectors: std::iter::once(&decided_gateway)
                .chain(&fallback_gateways)
                .map(|identifier| DecideConnector::from_identifier(identifier))
                .collect(),
            outcome,
            success_rate: Some(SuccessRateSection {
                status: SuccessRateStatus::Applied,
                approach: Some(routing_approach),
                scores: scores(gateway_priority_map.as_ref()),
                scheduled_outage: Some(is_scheduled_outage),
                diagnostics: Some(SuccessRateDiagnostics {
                    head_before_downtime_check: gateway_before_evaluation
                        .as_deref()
                        .map(DecideConnector::from_identifier),
                    reset_approach,
                    dimension: SuccessRateDimension {
                        value: routing_dimension,
                        level: routing_dimension_level,
                    },
                    filter_wise_gateways,
                    priority_logic: PriorityLogicSection {
                        tag: priority_logic_tag,
                        output: priority_logic_output.map(Into::into),
                    },
                    dynamic_mga_enabled: is_dynamic_mga_enabled,
                    gateway_mga_id_map,
                    rust_based_decider: is_rust_based_decider,
                    latency_ms: latency,
                }),
            }),
            cost,
            volume_steering,
            debit_routing: debit_routing_output,
        }
    }
}

impl DecideResponse {
    pub fn from_evaluation(
        evaluation: RoutingEvaluation,
        payment_id: String,
        request_id: String,
    ) -> Self {
        let RoutingEvaluation {
            static_routing,
            static_error,
            success_rate,
            evaluated_connectors,
        } = evaluation;
        let selection = static_routing.as_ref().map(|response| response.selection);
        let rule_outcome = Outcome::of_rule(selection);

        let mut warnings: Vec<Warning> = static_error
            .map(|message| Warning {
                code: "rule_evaluation_failed",
                param: None,
                message,
            })
            .into_iter()
            .chain(selection.and_then(Warning::for_selection))
            .collect();

        let mut layers = match success_rate {
            SuccessRateOutcome::Applied(decision) => Layers::from(*decision),
            SuccessRateOutcome::Failed {
                error_message,
                fallback_connectors,
            } => {
                warnings.push(Warning {
                    code: "success_rate_failed",
                    param: None,
                    message: error_message,
                });
                let strategy = match (rule_outcome.kind, rule_outcome.strategy) {
                    (OutcomeType::Routed, Strategy::Rule) => Strategy::Rule,
                    _ => Strategy::CandidateOrder,
                };
                Layers {
                    connectors: fallback_connectors.into_iter().map(Into::into).collect(),
                    outcome: Outcome::new(
                        OutcomeType::Fallback,
                        strategy,
                        Reason::SuccessRateUnavailable,
                    ),
                    success_rate: Some(SuccessRateSection::failed()),
                    cost: None,
                    volume_steering: None,
                    debit_routing: None,
                }
            }
            SuccessRateOutcome::Skipped => Layers {
                connectors: evaluated_connectors
                    .into_iter()
                    .flatten()
                    .map(Into::into)
                    .collect(),
                outcome: rule_outcome,
                success_rate: None,
                cost: None,
                volume_steering: None,
                debit_routing: None,
            },
        };
        if layers.connectors.is_empty() {
            let reason = if selection == Some(RuleSelection::NoActiveAlgorithm) {
                Reason::NoActiveRuleNoCandidates
            } else {
                Reason::NoEligibleConnectors
            };
            layers.outcome = Outcome::new(OutcomeType::NoConnector, Strategy::NotRouted, reason);
        }

        Self {
            id: format!("dec_{}", uuid::Uuid::now_v7().simple()),
            object: "routing_decision",
            payment_id,
            request_id,
            created_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            connectors: layers.connectors,
            outcome: layers.outcome,
            rule: static_routing.map(Into::into),
            success_rate: layers.success_rate,
            cost: layers.cost,
            volume_steering: layers.volume_steering,
            debit_routing: layers.debit_routing,
            warnings,
        }
    }
}

#[cfg(test)]
#[allow(clippy::panic_in_result_fn)]
mod tests {
    use super::*;
    use crate::euclid::ast::ConnectorInfo;
    use serde_json::{json, Value};

    fn connector(name: &str, id: Option<&str>) -> ConnectorInfo {
        ConnectorInfo {
            gateway_name: name.to_string(),
            gateway_id: id.map(str::to_string),
        }
    }

    fn rule(selection: RuleSelection, connectors: Vec<ConnectorInfo>) -> RoutingEvaluateResponse {
        RoutingEvaluateResponse {
            payment_id: Some("pay_1".to_string()),
            status: "success".to_string(),
            output: json!({"type": "priority", "connectors": connectors}),
            evaluated_output: connectors.clone(),
            eligible_connectors: connectors,
            selection,
        }
    }

    fn decide(evaluation: RoutingEvaluation) -> Result<Value, serde_json::Error> {
        serde_json::to_value(DecideResponse::from_evaluation(
            evaluation,
            "pay_1".to_string(),
            "req_1".to_string(),
        ))
    }

    fn stripe_adyen() -> Vec<ConnectorInfo> {
        vec![
            connector("stripe", Some("mca_s")),
            connector("adyen", Some("mca_a")),
        ]
    }

    #[test]
    fn rule_only_routes_by_the_rule_with_one_connector_shape() -> Result<(), serde_json::Error> {
        let actual = decide(RoutingEvaluation {
            static_routing: Some(rule(RuleSelection::Matched, stripe_adyen())),
            static_error: None,
            success_rate: SuccessRateOutcome::Skipped,
            evaluated_connectors: Some(stripe_adyen()),
        })?;
        let stripe = json!({"gateway_name": "stripe", "payment_merchant_connector_id": "mca_s"});
        let adyen = json!({"gateway_name": "adyen", "payment_merchant_connector_id": "mca_a"});
        assert_eq!(actual["object"], "routing_decision");
        assert!(actual["id"]
            .as_str()
            .is_some_and(|id| id.starts_with("dec_")));
        assert_eq!(actual["request_id"], "req_1");
        assert_eq!(actual["connectors"], json!([stripe, adyen]));
        assert_eq!(
            actual["outcome"],
            json!({"type": "routed", "strategy": "rule", "reason": "rule_matched",
                   "message": "Routed by the active rule."})
        );
        assert_eq!(
            actual["rule"],
            json!({"status": "matched",
                   "output": {"type": "priority", "connectors": [stripe, adyen]},
                   "selected": [stripe, adyen], "eligible": [stripe, adyen]})
        );
        for section in ["success_rate", "cost", "volume_steering", "debit_routing"] {
            assert_eq!(actual[section], Value::Null, "{section}");
        }
        assert_eq!(actual["warnings"], json!([]));
        Ok(())
    }

    #[test]
    fn a_cost_win_lists_the_full_order_and_every_decider_fact(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let decision: DecidedGateway = serde_json::from_value(json!({
            "decided_gateway": "adyen:mca_a",
            "fallback_gateways": ["stripe:mca_s"],
            "gateway_priority_map": {"adyen:mca_a": 0.97, "stripe:mca_s": 0.98},
            "filter_wise_gateways": null,
            "priority_logic_tag": "tag_1",
            "routing_approach": "SR_SELECTION_MULTI_OBJECTIVE",
            "gateway_before_evaluation": "stripe:mca_s",
            "priority_logic_output": {"isEnforcement": false, "gws": ["stripe:mca_s"],
                "priorityLogicTag": "tag_1", "gatewayReferenceIds": {}, "primaryLogic": null,
                "fallbackLogic": null},
            "debit_routing_output": null,
            "reset_approach": "NO_RESET",
            "routing_dimension": "ORDER_PAYMENT, CREDIT, CARD",
            "routing_dimension_level": "PM_LEVEL",
            "is_scheduled_outage": false,
            "is_dynamic_mga_enabled": false,
            "gateway_mga_id_map": null,
            "is_rust_based_decider": true,
            "latency": 4,
            "multi_objective_info": {"outcome": "COST_WON", "reason": "adyen beats stripe on EV",
                "costSavedBps": 1480.0, "qualifiedCount": 2, "margin": 1.0, "evGapTop2": 0.1351,
                "ranked": [
                    {"psp": "adyen:mca_a", "authRate": 0.97, "costBps": 20.0, "costSource": "SEED",
                     "ev": 0.9681, "isSrHead": false, "isChosen": true},
                    {"psp": "stripe:mca_s", "authRate": 0.98, "costBps": 1500.0, "costSource": "SEED",
                     "costModel": {"brand": "VISA", "pctBps": 150.0},
                     "ev": 0.833, "isSrHead": true, "isChosen": false}]},
            "volume_steer_info": null
        }))?;
        let actual = decide(RoutingEvaluation {
            static_routing: Some(rule(RuleSelection::Matched, stripe_adyen())),
            static_error: None,
            success_rate: SuccessRateOutcome::Applied(Box::new(decision)),
            evaluated_connectors: Some(vec![connector("adyen", Some("mca_a"))]),
        })?;
        let stripe = json!({"gateway_name": "stripe", "payment_merchant_connector_id": "mca_s"});
        let adyen = json!({"gateway_name": "adyen", "payment_merchant_connector_id": "mca_a"});
        assert_eq!(actual["connectors"], json!([adyen, stripe]));
        assert_eq!(
            actual["outcome"],
            json!({"type": "routed", "strategy": "cost", "reason": "cost_optimized",
                   "message": "adyen beats stripe on EV"})
        );
        assert_eq!(
            actual["success_rate"],
            json!({
                "status": "applied",
                "approach": "SR_SELECTION_MULTI_OBJECTIVE",
                "scores": [
                    {"gateway_name": "stripe", "payment_merchant_connector_id": "mca_s", "score": 0.98},
                    {"gateway_name": "adyen", "payment_merchant_connector_id": "mca_a", "score": 0.97}
                ],
                "scheduled_outage": false,
                "diagnostics": {
                    "head_before_downtime_check": stripe,
                    "reset_approach": "NO_RESET",
                    "dimension": {"value": "ORDER_PAYMENT, CREDIT, CARD", "level": "PM_LEVEL"},
                    "filter_wise_gateways": null,
                    "priority_logic": {"tag": "tag_1", "output": {"is_enforcement": false,
                        "gws": ["stripe:mca_s"], "priority_logic_tag": "tag_1",
                        "gateway_reference_ids": {}, "primary_logic": null, "fallback_logic": null}},
                    "dynamic_mga_enabled": false,
                    "gateway_mga_id_map": null,
                    "rust_based_decider": true,
                    "latency_ms": 4
                }
            })
        );
        assert_eq!(
            actual["cost"],
            json!({
                "outcome": "COST_WON", "reason": "adyen beats stripe on EV",
                "cost_saved_bps": 1480.0, "qualified_count": 2, "margin": 1.0, "ev_gap_top2": 0.1351,
                "ranked": [
                    {"gateway_name": "adyen", "payment_merchant_connector_id": "mca_a",
                     "auth_rate": 0.97, "cost_bps": 20.0, "cost_source": "SEED", "cost_model": null,
                     "ev": 0.9681, "is_sr_head": false, "is_chosen": true},
                    {"gateway_name": "stripe", "payment_merchant_connector_id": "mca_s",
                     "auth_rate": 0.98, "cost_bps": 1500.0, "cost_source": "SEED",
                     "cost_model": {"brand": "VISA", "variant": null, "issuer": null, "ccy": null,
                                    "ic_category": null, "pct_bps": 150.0, "fixed_fee": null},
                     "ev": 0.833, "is_sr_head": true, "is_chosen": false}
                ]
            })
        );
        assert_eq!(actual["volume_steering"], Value::Null);
        Ok(())
    }

    #[test]
    fn a_failed_success_rate_call_falls_back_to_the_rule_and_says_why(
    ) -> Result<(), serde_json::Error> {
        let actual = decide(RoutingEvaluation {
            static_routing: Some(rule(RuleSelection::Matched, stripe_adyen())),
            static_error: None,
            success_rate: SuccessRateOutcome::Failed {
                error_message: "decider unavailable".to_string(),
                fallback_connectors: stripe_adyen(),
            },
            evaluated_connectors: Some(stripe_adyen()),
        })?;
        assert_eq!(actual["outcome"]["type"], "fallback");
        assert_eq!(actual["outcome"]["strategy"], "rule");
        assert_eq!(actual["outcome"]["reason"], "success_rate_unavailable");
        assert_eq!(actual["success_rate"]["status"], "failed");
        assert_eq!(
            actual["warnings"],
            json!([{"code": "success_rate_failed", "param": null, "message": "decider unavailable"}])
        );
        assert_eq!(actual["connectors"][0]["gateway_name"], "stripe");
        Ok(())
    }

    #[test]
    fn a_failed_rule_layer_is_reported_instead_of_dropped() -> Result<(), serde_json::Error> {
        let actual = decide(RoutingEvaluation {
            static_routing: None,
            static_error: Some("Invalid value for card_network".to_string()),
            success_rate: SuccessRateOutcome::Skipped,
            evaluated_connectors: Some(stripe_adyen()),
        })?;
        assert_eq!(actual["outcome"]["type"], "fallback");
        assert_eq!(actual["outcome"]["strategy"], "candidate_order");
        assert_eq!(actual["outcome"]["reason"], "rule_input_invalid");
        assert_eq!(actual["rule"], Value::Null);
        assert_eq!(actual["warnings"][0]["code"], "rule_evaluation_failed");
        assert_eq!(
            actual["warnings"][0]["message"],
            "Invalid value for card_network"
        );
        Ok(())
    }

    #[test]
    fn each_rule_selection_reports_its_own_outcome() -> Result<(), serde_json::Error> {
        for (selection, kind, reason, status, warning) in [
            (
                RuleSelection::DefaultSelection,
                "routed",
                "rule_default_selection",
                "default_selection",
                None,
            ),
            (
                RuleSelection::NoRuleMatched,
                "fallback",
                "no_rule_matched",
                "default_selection",
                None,
            ),
            (
                RuleSelection::OutputNotEligible,
                "fallback",
                "rule_output_not_eligible",
                "default_selection",
                Some("rule_output_not_eligible"),
            ),
            (
                RuleSelection::OutputPartiallyEligible,
                "routed",
                "rule_output_partially_eligible",
                "default_selection",
                Some("rule_output_partially_eligible"),
            ),
            (
                RuleSelection::NoActiveAlgorithm,
                "fallback",
                "no_active_rule",
                "no_active_rule",
                None,
            ),
        ] {
            let actual = decide(RoutingEvaluation {
                static_routing: Some(rule(selection, stripe_adyen())),
                static_error: None,
                success_rate: SuccessRateOutcome::Skipped,
                evaluated_connectors: Some(stripe_adyen()),
            })?;
            assert_eq!(actual["outcome"]["type"], kind, "{selection:?}");
            assert_eq!(actual["outcome"]["reason"], reason, "{selection:?}");
            assert_eq!(actual["rule"]["status"], status, "{selection:?}");
            assert_eq!(
                actual["warnings"][0]["code"].as_str(),
                warning,
                "{selection:?}"
            );
        }
        Ok(())
    }

    #[test]
    fn nothing_left_to_try_is_no_connector() -> Result<(), serde_json::Error> {
        for (selection, reason) in [
            (
                RuleSelection::NoActiveAlgorithm,
                "no_active_rule_no_candidates",
            ),
            (RuleSelection::Matched, "no_eligible_connectors"),
        ] {
            let actual = decide(RoutingEvaluation {
                static_routing: Some(rule(selection, Vec::new())),
                static_error: None,
                success_rate: SuccessRateOutcome::Skipped,
                evaluated_connectors: Some(Vec::new()),
            })?;
            assert_eq!(actual["connectors"], json!([]));
            assert_eq!(actual["outcome"]["type"], "no_connector");
            assert_eq!(actual["outcome"]["strategy"], "none");
            assert_eq!(actual["outcome"]["reason"], reason);
        }
        Ok(())
    }

    #[test]
    fn connectors_named_without_an_account_read_back_with_a_null_id(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let decision: DecidedGateway = serde_json::from_value(json!({
            "decided_gateway": "stripe", "fallback_gateways": ["adyen"],
            "gateway_priority_map": {"stripe": 1.0, "adyen": 1.0},
            "routing_approach": "SR_SELECTION_V3_ROUTING", "reset_approach": "NO_RESET",
            "is_scheduled_outage": false, "is_dynamic_mga_enabled": false,
            "is_rust_based_decider": true
        }))?;
        let actual = decide(RoutingEvaluation {
            static_routing: Some(rule(RuleSelection::NoActiveAlgorithm, Vec::new())),
            static_error: None,
            success_rate: SuccessRateOutcome::Applied(Box::new(decision)),
            evaluated_connectors: Some(vec![connector("stripe", None)]),
        })?;
        let stripe = json!({"gateway_name": "stripe", "payment_merchant_connector_id": null});
        let adyen = json!({"gateway_name": "adyen", "payment_merchant_connector_id": null});
        assert_eq!(actual["connectors"], json!([stripe, adyen]));
        assert_eq!(actual["outcome"]["reason"], "success_rate_ranked");
        // Equal scores sort by identifier.
        assert_eq!(actual["success_rate"]["scores"][0]["gateway_name"], "adyen");
        Ok(())
    }
}
