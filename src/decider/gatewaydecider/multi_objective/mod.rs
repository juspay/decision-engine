pub mod algorithm;
pub mod cluster_key;
pub mod hypersense_client;

use serde::{Deserialize, Serialize};

/// Cost-aggressiveness λ: how far past the statistical noise floor a cost saving is
/// allowed to widen the auth band. 1.0 = break-even (trade auth for cost only up to the
/// point the saving pays for the expected auth loss). Fully automatic in v1.
pub const LAMBDA_COST: f64 = 1.0;
/// Noise-floor confidence z: multiplier on the SR estimate's standard error. 1.0 ≈ 68%.
pub const Z_NOISE: f64 = 1.0;
/// Circuit breaker: the most a cost saving may widen the band *beyond* the noise floor,
/// as a fraction of auth-rate (0.02 = 2pp). Bounds the blast radius if the cost feed is
/// wrong — without it, a bogus "huge saving" could admit a measurably-worse PSP. The
/// noise floor is always honored on top of this (it is statistically free).
pub const MAX_ECON_BAND: f64 = 0.02;
/// Default merchant margin (fraction of ticket) when none is configured. Drives the
/// economic band `Δcost/(100·margin)`.
pub const DEFAULT_MARGIN: f64 = 0.20;

/// How the post-step picks among the PSPs the auth band admits. The band (gate) is
/// identical either way — only the final pick differs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CostPickStrategy {
    /// Production default: route to the highest expected-value PSP in the band, where
    /// `EV = auth·(margin − cost)`. Safe and optimal — never trades a visible auth point
    /// for a cheaper PSP unless the saving outweighs it.
    #[default]
    MaxEv,
    /// Comparison-only: route to the **cheapest** PSP in the band regardless of EV — the
    /// "band alone" behavior the EV pick replaced. Surfaces more/larger cost savings but
    /// also more auth value risked (see `scratch/deriving-routing-config.md` §5.4). Wired
    /// to the simulator's toggle so the two strategies can be compared side by side.
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
    /// Effective auth band the decider applied for this txn, in percentage points.
    /// Derived per-txn as `max(noise_floor, λ·Δcost/(100·margin))` — no longer a static
    /// config. For AuthWon this is the head's noise floor; for CostWon it is the gate the
    /// promoted PSP cleared. (Serializes as `tolerancePp` for analytics/UI continuity.)
    pub tolerance_pp: f64,
    /// The PSP the SR scorer would have picked (head of score map before reorder).
    pub sr_head: Option<PspSummary>,
    /// The PSP the post-step actually chose. Equals `sr_head` when auth won.
    pub chosen: Option<PspSummary>,
    /// Cost saved in bps when outcome == CostWon (== sr_head.cost_bps - chosen.cost_bps).
    pub cost_saved_bps: Option<f64>,
    /// Number of PSPs that survived the auth-rate band gate.
    pub qualified_count: usize,
    /// Merchant margin (fraction of ticket) the decider applied for this txn. Lets
    /// callers value the auth-rate tradeoff a cost override accepted —
    /// `(sr_head.auth − chosen.auth) × ticket × margin` — and net it against the fee
    /// saved, rather than reading the fee saving in isolation.
    pub margin: f64,
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
