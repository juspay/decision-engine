pub mod algorithm;
pub mod cluster_key;
pub mod hypersense_client;
pub mod seed_costs;

use serde::{Deserialize, Serialize};

/// Default merchant margin (fraction of ticket) when none is configured. Drives the
/// expected-value calculation `EV = auth·(margin − cost)`.
pub const DEFAULT_MARGIN: f64 = 0.20;

/// How the post-step picks among all PSPs with cost data. Ranking is pure — there is no
/// auth band or admission gate; only the final pick differs between strategies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CostPickStrategy {
    /// Production default: route to the highest expected-value PSP, where
    /// `EV = auth·(margin − cost)`. Balances auth and cost in a single number.
    #[default]
    MaxEv,
    /// Comparison-only: route to the **cheapest** PSP regardless of EV, ignoring the auth
    /// tradeoff entirely. Surfaces more/larger cost savings but risks more auth value.
    /// Wired to the simulator's toggle so the two strategies can be compared side by side.
    CheapestInBand,
}

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
    /// The PSP the SR scorer would have picked (head of score map before reorder).
    pub sr_head: Option<PspSummary>,
    /// The PSP the post-step actually chose. Equals `sr_head` when auth won.
    pub chosen: Option<PspSummary>,
    /// Cost saved in bps when outcome == CostWon (== sr_head.cost_bps - chosen.cost_bps).
    pub cost_saved_bps: Option<f64>,
    /// Number of PSPs ranked on expected value (i.e. those that had cost data).
    pub qualified_count: usize,
    /// Merchant margin (fraction of ticket) the decider applied for this txn. Lets
    /// callers value the auth-rate tradeoff a cost override accepted —
    /// `(sr_head.auth − chosen.auth) × ticket × margin` — and net it against the fee
    /// saved, rather than reading the fee saving in isolation.
    pub margin: f64,
    /// Expected-value gap between the top-two EV-ranked PSPs (every PSP that had cost
    /// data is ranked), as a fraction of
    /// ticket: `EV(#1) − EV(#2)` where `EV = auth·(margin − cost_bps/10_000)`. This is
    /// the margin of victory of the winning pick — small values mean the decision was
    /// close. `None` when fewer than two PSPs had the cost data needed to rank on EV.
    /// (Serializes as `evGapTop2`.)
    #[serde(default)]
    pub ev_gap_top2: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MultiObjectiveOutcome {
    /// A PSP with higher expected value was promoted above the SR head.
    CostWon,
    /// Multi-objective ran but kept the SR head. Possible sub-cases (see `reason`):
    /// - only 1 PSP available to rank
    /// - the SR head was already the highest expected-value PSP
    /// - no PSP (or not the head) had the cost data needed to rank on EV
    AuthWon,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PspSummary {
    pub psp: String,
    pub auth_rate: f64,
    pub cost_bps: Option<f64>,
}
