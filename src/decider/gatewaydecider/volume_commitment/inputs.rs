//! The flat shape the controller works in. [`super::dsl`] flattens the contract DSL into it, so
//! nothing downstream depends on how contracts are stored.

use std::collections::HashMap;

use async_trait::async_trait;

/// Everything we need to know about one merchant for one billing cycle.
#[derive(Debug, Clone)]
pub struct CommitmentInputs {
    pub merchant_id: String,
    /// When the active contract document was written — its identity for our purposes. A plan built
    /// from a different document is describing a contract that is no longer in force.
    pub contract_anchor_ms: i64,
    /// The routing-rule id holding the document, so a caller can act on the contract itself.
    pub contract_rule_id: String,
    /// How much approval rate we may give up to win volume, as a fraction (5pp = 0.05).
    pub tolerance: f64,
    /// Total payment volume we expect per day across all PSPs.
    pub expected_daily_traffic: f64,
    /// Forecast cadence override; None means the config default.
    pub forecast_interval_secs: Option<u64>,
    /// ISO-4217 code every amount in the document is denominated in, for display.
    pub currency: Option<String>,
    /// Multiplier that puts a measured payment amount on the scale the goals below are held at.
    /// Traffic reaches `/decide-gateway` in major currency units — that is how the simulator, the
    /// cost tiles and the analytics pages all read it — while the contract DSL canonicalizes every
    /// goal, reward and daily-traffic figure to minor units. `1.0` for a contract that counts
    /// transactions, where there is no currency and so no conversion to make.
    pub amount_scale: f64,
    pub commitments: Vec<Commitment>,
}

impl CommitmentInputs {
    /// Contract-day length shared by the document's commitments (a calendar day when empty).
    pub fn day_secs(&self) -> u64 {
        self.commitments
            .first()
            .map(|c| c.day_secs)
            .unwrap_or(super::math::SECS_PER_DAY)
    }
}

/// One promise the merchant made to one PSP.
#[derive(Debug, Clone)]
pub struct Commitment {
    pub connector: String,
    /// Volume promised for the period.
    pub goal: f64,
    /// What the merchant earns if the goal is met.
    pub reward: f64,
    /// How the reward is earned, in words — "0.25% rebate", "lump sum" — for the contract card.
    pub reward_note: String,
    /// Instant the current cycle opened — the lower bound of the window we measure.
    pub period_start_ms: i64,
    /// Instant it closes (the next cycle's start).
    pub period_end_ms: i64,
    /// How long one contract "day" lasts. A calendar cycle counts real days; a `test_minutes`
    /// cycle counts minutes, so the same pacing maths plays out in minutes instead of a month.
    pub day_secs: u64,
    /// IANA zone the billing cycle runs in, kept for display.
    pub timezone: String,
}

/// Volume actually measured, per PSP.
#[derive(Debug, Clone, Default)]
pub struct MeasuredVolume {
    /// Total sent to each PSP so far this cycle.
    pub achieved: HashMap<String, f64>,
    /// Recent average volume per day.
    pub pace: HashMap<String, f64>,
    /// Volume normal routing sends each PSP per day, without any help.
    pub routing_gives_daily: HashMap<String, f64>,
    /// Volume the nudge has already steered here this contract day; subtracted from the shortfall to close the loop.
    pub steered_today: HashMap<String, f64>,
    /// Total volume per day across *every* PSP over the pace window — the flow steering actually
    /// has to divert from, as opposed to the flow the contract declares. `None` before any traffic
    /// has been measured, when only the declaration is available.
    pub total_daily: Option<f64>,
    /// The same flow over a short recent window. `total_daily` answers "how much does this
    /// merchant do", which wants history; this answers "how much will arrive before the cycle
    /// closes", which follows the current rate. Feasibility uses this where it exists — `None`
    /// until the short window spans a whole contract day, before which it is not yet a rate.
    pub recent_daily: Option<f64>,
    /// Set when a measurement query failed, leaving the maps below what was really delivered.
    /// The forecast loop ignores it — unmeasured reads as behind, and nudges stay inside
    /// tolerance — but anything that *reports* a position to a merchant must not present
    /// an unread cycle as an empty one.
    pub measurement_failed: bool,
}

impl MeasuredVolume {
    /// A connector's value in one of the maps, zero when it had no traffic.
    fn of(map: &HashMap<String, f64>, connector: &str) -> f64 {
        map.get(connector).copied().unwrap_or(0.0)
    }

    pub fn achieved_for(&self, connector: &str) -> f64 {
        Self::of(&self.achieved, connector)
    }

    /// `None` when unmeasured, so the caller can fall back to a starting guess.
    pub fn pace_for(&self, connector: &str) -> Option<f64> {
        self.pace.get(connector).copied()
    }

    pub fn routing_gives_daily_for(&self, connector: &str) -> f64 {
        Self::of(&self.routing_gives_daily, connector)
    }

    pub fn steered_today_for(&self, connector: &str) -> f64 {
        Self::of(&self.steered_today, connector)
    }
}

/// Where commitments come from — the contract DSL in production, a stub in tests.
#[async_trait]
pub trait InputSource: Send + Sync {
    /// Whether volume-contract routing is switched on for this merchant.
    async fn feature_enabled(&self, merchant_id: &str) -> bool;
    /// One merchant's commitments as configured, whether or not the feature is on. Only
    /// dashboard surfaces that must tell "no contract" apart from "contract live but feature
    /// off" call this; anything that acts on a commitment calls `load`.
    async fn load_configured(&self, merchant_id: &str) -> Option<CommitmentInputs>;
    /// What routing acts on: one merchant's commitments while the feature is on, else None.
    async fn load(&self, merchant_id: &str) -> Option<CommitmentInputs> {
        if !self.feature_enabled(merchant_id).await {
            return None;
        }
        self.load_configured(merchant_id).await
    }
    /// Every merchant with commitments, checked on each pass of the background loop.
    async fn list_active(&self) -> Vec<String>;
}
