pub mod algorithm;
pub mod cluster_key;
pub mod hypersense_client;

use serde::{Deserialize, Serialize};

pub const DEFAULT_TOLERANCE_BAND_PP: f64 = 0.5;

/// Surfaced on the `/decide-gateway` response when the multi-objective post-step
/// actually ran. Lets callers see why the gateway was picked (auth still won, or
/// cost won and saved N bps) without having to reconstruct the logic.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MultiObjectiveInfo {
    /// Terminal outcome of the post-step.
    pub outcome: MultiObjectiveOutcome,
    /// Human-readable explanation of the decision.
    pub reason: String,
    /// Tolerance band used for this txn (percentage points of auth-rate).
    pub tolerance_pp: f64,
    /// The PSP the SR scorer would have picked (head of score map before reorder).
    pub sr_head: Option<PspSummary>,
    /// The PSP the post-step actually chose. Equals `sr_head` when auth won.
    pub chosen: Option<PspSummary>,
    /// Cost saved in bps when outcome == CostWon (== sr_head.cost_bps - chosen.cost_bps).
    pub cost_saved_bps: Option<f64>,
    /// Number of PSPs that survived the auth-rate band gate.
    pub qualified_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MultiObjectiveOutcome {
    /// A cheaper PSP qualified under the band and was promoted above the SR head.
    CostWon,
    /// Multi-objective ran but kept the SR head. Possible sub-cases (see `reason`):
    /// - only 1 PSP qualified under the band
    /// - the SR head was already the cheapest qualifier
    /// - all qualified PSPs lacked cost data
    AuthWon,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PspSummary {
    pub psp: String,
    pub auth_rate: f64,
    pub cost_bps: Option<f64>,
}
