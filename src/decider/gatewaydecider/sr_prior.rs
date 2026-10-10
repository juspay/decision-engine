//! Cold-start prior for SR v3 scores.
//!
//! An SR v3 window is created full of synthetic successes (`createKeysIfNotExist`) and an absent
//! window reads as 1.0, so a new or sparse segment ranks every gateway at ~100% until roughly a
//! bucket's worth of real outcomes has pushed the seed out. Nothing the engine already knows about
//! the gateway (its merchant-level success rate) is used.
//!
//! This module holds the pure pieces of the fix:
//!
//! * [`WindowStats`] separates synthetic seed entries from real outcomes. Synthetic entries are
//!   written as [`SYNTHETIC_SUCCESS`] / [`SYNTHETIC_FAILURE`] (`"1.0"` / `"0.0"`), which every
//!   reader parses to the same `f64` as the legacy `"1"` / `"0"`, so the legacy score is
//!   unchanged. Entries written before this change cannot be told apart and count as real, which
//!   reproduces the legacy behaviour for those windows.
//! * [`SrPrior`] shrinks the real outcomes toward the parent success rate:
//!   `posterior = (m·p + s) / (m + n)`, with the matching Beta posterior for sampling.
//! * [`fit_prior_strength`] learns `m` by empirical Bayes (Beta-Binomial method of moments) from
//!   per-segment outcomes, with guards for thin or degenerate data.
//! * [`SrPriorSnapshot`] is what the autopilot job writes (`SR_V3_PRIOR_<merchant_id>`) and
//!   [`resolve_prior`] is how the decider reads it, including flag handling and fallbacks.
//!
//! Nothing here touches Redis, the database or the clock, so the evaluation harness
//! (`examples/sr_v3_cold_start_eval.rs`) drives exactly these functions.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Queue entry written for a synthetic (seed / reset) success. Parses to `1.0` like `"1"`.
pub const SYNTHETIC_SUCCESS: &str = "1.0";
/// Queue entry written for a synthetic (seed / reset) failure. Parses to `0.0` like `"0"`.
pub const SYNTHETIC_FAILURE: &str = "0.0";

/// Prior strength used when neither a learned nor a configured value is available. Chosen on the
/// tuning seeds of the evaluation harness (see the module docs of the harness).
pub const DEFAULT_PRIOR_STRENGTH: f64 = 20.0;
/// Lower clamp for any prior strength (learned or configured).
pub const MIN_PRIOR_STRENGTH: f64 = 1.0;
/// Upper clamp for any prior strength (learned or configured).
pub const MAX_PRIOR_STRENGTH: f64 = 1000.0;

/// Parent rates are kept strictly inside (0, 1) so the Beta posterior is always proper.
const RATE_EPSILON: f64 = 1e-4;
/// Floor for Beta parameters handed to the sampler.
const MIN_BETA_PARAM: f64 = 1e-3;

/// Version of the [`SrPriorSnapshot`] layout. Snapshots with another version are ignored.
pub const PRIOR_SNAPSHOT_VERSION: u32 = 1;

/// Hard ceiling on a snapshot's age, enforced by the decider regardless of the `validUntilMs` the
/// job wrote, so a misconfigured validity can never keep stale priors in force indefinitely.
pub const MAX_PRIOR_SNAPSHOT_AGE_MS: i64 = 7 * 24 * 3_600 * 1_000;
/// Tolerated clock skew between the replica that wrote a snapshot and the one reading it.
const SNAPSHOT_CLOCK_SKEW_MS: i64 = 5 * 60 * 1_000;

/// Service-config name of a merchant's prior snapshot. Scoped by merchant so priors can never be
/// read across merchants; the snapshot also carries the merchant id, checked on read.
pub fn prior_snapshot_config_name(merchant_id: &str) -> String {
    format!("SR_V3_PRIOR_{merchant_id}")
}

// ── Window parsing ───────────────────────────────────────────────────────────

/// Real and synthetic counts of one SR v3 window.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct WindowStats {
    pub real_successes: f64,
    pub real_observations: u32,
    pub synthetic_successes: f64,
    pub synthetic_observations: u32,
}

impl WindowStats {
    /// Classifies queue entries. Synthetic markers are matched exactly; every other entry that
    /// parses to a finite number is a real outcome, weighted by its value clamped to `[0, 1]`.
    /// Malformed and non-finite entries are skipped.
    pub fn from_entries<S: AsRef<str>>(entries: &[S]) -> Self {
        let mut stats = Self::default();
        for entry in entries {
            let entry = entry.as_ref();
            if entry == SYNTHETIC_SUCCESS {
                stats.synthetic_successes += 1.0;
                stats.synthetic_observations += 1;
            } else if entry == SYNTHETIC_FAILURE {
                stats.synthetic_observations += 1;
            } else if let Some(value) = entry.parse::<f64>().ok().filter(|v| v.is_finite()) {
                stats.real_successes += value.clamp(0.0, 1.0);
                stats.real_observations += 1;
            }
        }
        stats
    }

    pub fn real_failures(&self) -> f64 {
        (f64::from(self.real_observations) - self.real_successes).max(0.0)
    }

    /// The score the legacy reader derives from the same entries (mean over every entry), or
    /// `None` for an empty window.
    pub fn legacy_score(&self) -> Option<f64> {
        let total = self.real_observations + self.synthetic_observations;
        (total > 0).then(|| {
            ((self.real_successes + self.synthetic_successes) / f64::from(total)).clamp(0.0, 1.0)
        })
    }
}

/// The legacy SR v3 window score: the mean of every entry that parses as `f64`, clamped to
/// `[0, 1]`; `None` when nothing parses. This is exactly what `get_score_from_redis` has always
/// computed, extracted so the evaluation harness scores the existing routing with the same code.
pub fn legacy_window_score<S: AsRef<str>>(entries: &[S]) -> Option<f64> {
    let parsed: Vec<f64> = entries
        .iter()
        .filter_map(|value| value.as_ref().parse::<f64>().ok())
        .collect();
    if parsed.is_empty() {
        return None;
    }
    let success_count: f64 = parsed.iter().sum();
    Some((success_count / parsed.len() as f64).clamp(0.0, 1.0))
}

/// Score of a gateway under the cold-start mode: the posterior mean when a prior is available,
/// otherwise the legacy score (an empty window reads 1.0, as an absent Redis key does today).
pub fn cold_start_score(prior: Option<&SrPrior>, window: &WindowStats) -> f64 {
    match prior {
        Some(prior) => prior.posterior_mean(window),
        None => window.legacy_score().unwrap_or(1.0),
    }
}

// ── Posterior ────────────────────────────────────────────────────────────────

/// Where a prior's strength came from. Logged with the decision for debuggability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PriorStrengthSource {
    /// Learned by empirical Bayes in the autopilot job.
    Learned,
    /// `defaultPriorStrength` from the merchant's (or the default) SR v3 config.
    Configured,
    /// [`DEFAULT_PRIOR_STRENGTH`].
    Default,
}

/// Which parent a prior shrinks toward.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PriorScope {
    /// The gateway's own merchant-wide success rate.
    Gateway,
    /// The merchant-wide rate of the payment method across all gateways, for a gateway without
    /// enough recent outcomes of its own. Keeps every gateway of a decision on the same footing
    /// instead of mixing shrunk scores with optimistic legacy ones.
    Pooled,
}

/// A validated Beta prior: `Beta(m·p, m·(1−p))` with `p` the parent success rate and `m` the
/// prior strength (pseudo-observations).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SrPrior {
    parent_success_rate: f64,
    strength: f64,
    strength_source: PriorStrengthSource,
    scope: PriorScope,
}

impl SrPrior {
    /// Returns `None` for a non-finite rate or a non-finite / non-positive strength. A rate in
    /// `[0, 1]` is pulled strictly inside the interval; the strength is clamped to
    /// `[MIN_PRIOR_STRENGTH, MAX_PRIOR_STRENGTH]`.
    pub fn new(
        parent_success_rate: f64,
        strength: f64,
        strength_source: PriorStrengthSource,
    ) -> Option<Self> {
        if !valid_rate(parent_success_rate) || !valid_strength(strength) {
            return None;
        }
        Some(Self {
            parent_success_rate: parent_success_rate.clamp(RATE_EPSILON, 1.0 - RATE_EPSILON),
            strength: strength.clamp(MIN_PRIOR_STRENGTH, MAX_PRIOR_STRENGTH),
            strength_source,
            scope: PriorScope::Gateway,
        })
    }

    /// The same prior, marked as shrinking toward the pooled payment-method rate.
    pub fn pooled(self) -> Self {
        Self {
            scope: PriorScope::Pooled,
            ..self
        }
    }

    pub fn scope(&self) -> PriorScope {
        self.scope
    }

    pub fn parent_success_rate(&self) -> f64 {
        self.parent_success_rate
    }

    pub fn strength(&self) -> f64 {
        self.strength
    }

    pub fn strength_source(&self) -> PriorStrengthSource {
        self.strength_source
    }

    /// `(m·p + s) / (m + n)` over the window's *real* outcomes; synthetic entries are ignored.
    /// Always in `[0, 1]`; equals `p` for a window without real outcomes.
    pub fn posterior_mean(&self, window: &WindowStats) -> f64 {
        let (alpha, beta) = self.posterior_counts(window);
        (alpha / (alpha + beta)).clamp(0.0, 1.0)
    }

    /// Beta posterior parameters `(m·p + s, m·(1−p) + f)`, floored so `Beta::new` accepts them.
    pub fn posterior_beta_params(&self, window: &WindowStats) -> (f64, f64) {
        let (alpha, beta) = self.posterior_counts(window);
        (alpha.max(MIN_BETA_PARAM), beta.max(MIN_BETA_PARAM))
    }

    /// `m + n`: the number of observations the posterior is worth. Replaces the bucket size in
    /// variance terms (the SR v3 sigma bonus) when the prior is in use.
    pub fn effective_observations(&self, window: &WindowStats) -> f64 {
        self.strength + f64::from(window.real_observations)
    }

    fn posterior_counts(&self, window: &WindowStats) -> (f64, f64) {
        let successes = window
            .real_successes
            .clamp(0.0, f64::from(window.real_observations));
        let alpha = self.strength * self.parent_success_rate + successes;
        let beta = self.strength * (1.0 - self.parent_success_rate) + window.real_failures();
        (alpha, beta)
    }
}

fn valid_rate(rate: f64) -> bool {
    rate.is_finite() && (0.0..=1.0).contains(&rate)
}

fn valid_strength(strength: f64) -> bool {
    strength.is_finite() && strength > 0.0
}

/// True for a usable prior strength (finite and positive; it is clamped on use).
pub fn is_valid_prior_strength(strength: f64) -> bool {
    valid_strength(strength)
}

// ── Empirical-Bayes fit ──────────────────────────────────────────────────────

/// Outcomes of one segment (one SR key) for one gateway.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SegmentOutcome {
    pub successes: u64,
    pub observations: u64,
}

/// Guards for [`fit_prior_strength`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PriorFitConfig {
    /// Minimum number of qualifying segments before `m` is considered identifiable.
    pub min_segments: usize,
    /// Segments with fewer outcomes are left out of the fit. Under adaptive routing a gateway that
    /// starts badly in a segment stops receiving traffic there, so tiny segments are biased toward
    /// extreme rates and inflate the apparent heterogeneity (driving `m` down). Requiring a few
    /// outcomes per segment removes most of that selection bias (see the harness report).
    pub min_segment_observations: u64,
}

impl Default for PriorFitConfig {
    fn default() -> Self {
        Self {
            min_segments: 10,
            min_segment_observations: 5,
        }
    }
}

/// Why a prior strength could not be learned. The caller falls back to the configured strength.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriorFitError {
    /// Fewer than `min_segments` segments had `min_segment_observations` outcomes.
    InsufficientSegments { qualifying: usize, required: usize },
    /// The pooled success rate is 0 or 1, so there is no spread to measure.
    DegenerateRate,
    /// Segment sizes leave no degrees of freedom for the between-segment variance.
    Unidentifiable,
}

impl std::fmt::Display for PriorFitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InsufficientSegments {
                qualifying,
                required,
            } => write!(
                f,
                "insufficient segments ({qualifying} qualifying, {required} required)"
            ),
            Self::DegenerateRate => write!(f, "degenerate pooled success rate"),
            Self::Unidentifiable => write!(f, "between-segment variance not identifiable"),
        }
    }
}

/// Learns the prior strength `m` (the Beta concentration `α + β`) from per-segment outcomes with
/// the Beta-Binomial method-of-moments estimator for unequal segment sizes (Kleinman, 1973):
///
/// ```text
/// p̂ = Σsᵢ / Σnᵢ,   S = Σ nᵢ (sᵢ/nᵢ − p̂)²,   N = Σnᵢ,   k = #segments
/// ρ̂ = (S − p̂(1−p̂)(k−1)) / (p̂(1−p̂)(N − Σnᵢ²/N − (k−1))),   m̂ = (1 − ρ̂) / ρ̂
/// ```
///
/// `ρ` is the intra-segment correlation; `ρ̂ ≤ 0` means no detectable heterogeneity and maps to
/// `MAX_PRIOR_STRENGTH`. The result is clamped to `[MIN_PRIOR_STRENGTH, MAX_PRIOR_STRENGTH]`.
/// Malformed rows (`successes > observations`) are clamped.
pub fn fit_prior_strength(
    segments: &[SegmentOutcome],
    config: &PriorFitConfig,
) -> Result<f64, PriorFitError> {
    let min_obs = config.min_segment_observations.max(1);
    let qualifying: Vec<(f64, f64)> = segments
        .iter()
        .filter(|seg| seg.observations >= min_obs)
        .map(|seg| {
            let n = seg.observations as f64;
            (seg.successes.min(seg.observations) as f64, n)
        })
        .collect();
    let k = qualifying.len();
    let required = config.min_segments.max(2);
    if k < required {
        return Err(PriorFitError::InsufficientSegments {
            qualifying: k,
            required,
        });
    }
    let total: f64 = qualifying.iter().map(|(_, n)| n).sum();
    let pooled = qualifying.iter().map(|(s, _)| s).sum::<f64>() / total;
    if pooled <= 0.0 || pooled >= 1.0 {
        return Err(PriorFitError::DegenerateRate);
    }
    let spread: f64 = qualifying
        .iter()
        .map(|(s, n)| n * (s / n - pooled).powi(2))
        .sum();
    let k_minus_one = (k - 1) as f64;
    let size_term =
        total - qualifying.iter().map(|(_, n)| n * n).sum::<f64>() / total - k_minus_one;
    if size_term <= 0.0 || !size_term.is_finite() {
        return Err(PriorFitError::Unidentifiable);
    }
    let bernoulli_var = pooled * (1.0 - pooled);
    let rho = (spread - bernoulli_var * k_minus_one) / (bernoulli_var * size_term);
    if !rho.is_finite() {
        return Err(PriorFitError::Unidentifiable);
    }
    if rho <= 1.0 / (1.0 + MAX_PRIOR_STRENGTH) {
        return Ok(MAX_PRIOR_STRENGTH);
    }
    Ok(((1.0 - rho) / rho).clamp(MIN_PRIOR_STRENGTH, MAX_PRIOR_STRENGTH))
}

/// Laplace-smoothed parent success rate `(s + 1) / (n + 2)`, or `None` below
/// `min_observations` outcomes.
pub fn parent_success_rate(
    successes: u64,
    observations: u64,
    min_observations: u64,
) -> Option<f64> {
    if observations == 0 || observations < min_observations {
        return None;
    }
    let successes = successes.min(observations) as f64;
    Some((successes + 1.0) / (observations as f64 + 2.0))
}

// ── Snapshot written by the autopilot job ───────────────────────────────────

/// Prior for one `(payment method type, payment method, gateway)`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SrPriorEntry {
    pub payment_method_type: String,
    pub payment_method: String,
    pub gateway: String,
    /// Laplace-smoothed success rate of the gateway across all of the merchant's segments, over
    /// the (short) parent window.
    pub parent_success_rate: f64,
    /// Outcomes behind `parent_success_rate`.
    pub observations: u64,
    /// Empirical-Bayes prior strength; absent when learning is off or the fit was not possible.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub learned_prior_strength: Option<f64>,
    /// Segments that qualified for the fit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fit_segments: Option<u64>,
    /// Why learning was skipped, for operators.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fit_error: Option<String>,
}

/// Merchant-wide prior of a `(payment method type, payment method)` across all gateways.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SrPooledPriorEntry {
    pub payment_method_type: String,
    pub payment_method: String,
    pub parent_success_rate: f64,
    pub observations: u64,
}

/// The per-merchant prior store (`SR_V3_PRIOR_<merchant_id>`), owned by the autopilot job.
/// Manual configuration never lives here, so the job can rewrite it freely.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SrPriorSnapshot {
    pub version: u32,
    pub merchant_id: String,
    pub computed_at_ms: i64,
    /// The decider ignores the snapshot from this instant on (e.g. the job stopped running).
    pub valid_until_ms: i64,
    /// Window the parent success rates were computed over.
    pub lookback_secs: u64,
    /// Window the prior strengths were fitted over (absent when nothing was learned).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fit_lookback_secs: Option<u64>,
    pub entries: Vec<SrPriorEntry>,
    /// Fallback priors for gateways without an entry of their own.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pooled_entries: Vec<SrPooledPriorEntry>,
}

impl SrPriorSnapshot {
    /// True when the snapshot has the expected version, belongs to `merchant_id` and is fresh:
    /// before its `valid_until_ms`, at most [`MAX_PRIOR_SNAPSHOT_AGE_MS`] old, and not written in
    /// the future beyond the tolerated clock skew.
    pub fn usable_for(&self, merchant_id: &str, now_ms: i64) -> bool {
        self.version == PRIOR_SNAPSHOT_VERSION
            && self.merchant_id == merchant_id
            && now_ms < self.valid_until_ms
            && now_ms.saturating_sub(self.computed_at_ms) <= MAX_PRIOR_SNAPSHOT_AGE_MS
            && self.computed_at_ms <= now_ms.saturating_add(SNAPSHOT_CLOCK_SKEW_MS)
    }

    /// Entry for a gateway; payment method and gateway names compare case-insensitively.
    pub fn entry(
        &self,
        payment_method_type: &str,
        payment_method: &str,
        gateway: &str,
    ) -> Option<&SrPriorEntry> {
        self.entries.iter().find(|e| {
            e.payment_method_type
                .eq_ignore_ascii_case(payment_method_type)
                && e.payment_method.eq_ignore_ascii_case(payment_method)
                && e.gateway.eq_ignore_ascii_case(gateway)
        })
    }

    /// Pooled entry of a payment method (case-insensitive).
    pub fn pooled_entry(
        &self,
        payment_method_type: &str,
        payment_method: &str,
    ) -> Option<&SrPooledPriorEntry> {
        self.pooled_entries.iter().find(|e| {
            e.payment_method_type
                .eq_ignore_ascii_case(payment_method_type)
                && e.payment_method.eq_ignore_ascii_case(payment_method)
        })
    }
}

/// The two per-merchant feature flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ColdStartFlags {
    /// `ENABLE_SR_V3_COLD_START_PRIOR`: shrink scores toward the parent rate.
    pub prior_enabled: bool,
    /// `ENABLE_SR_V3_LEARNED_PRIOR_STRENGTH`: use the empirical-Bayes strength when available.
    /// Has no effect on routing unless `prior_enabled` is also set.
    pub learned_strength_enabled: bool,
}

/// The prior the decider uses for one gateway, or `None` to keep the legacy score.
///
/// `None` when the cold-start flag is off, the snapshot is missing / stale / for another merchant
/// / of another version, or neither a gateway entry nor a pooled entry of the payment method is
/// usable. A gateway without its own entry shrinks toward the pooled entry. The strength is the
/// learned one (gateway entries only) when that flag is on and the value is valid, else
/// `configured_strength` when valid, else [`DEFAULT_PRIOR_STRENGTH`].
pub fn resolve_prior(
    flags: ColdStartFlags,
    snapshot: Option<&SrPriorSnapshot>,
    merchant_id: &str,
    now_ms: i64,
    payment_method_type: &str,
    payment_method: &str,
    gateway: &str,
    configured_strength: Option<f64>,
) -> Option<SrPrior> {
    if !flags.prior_enabled {
        return None;
    }
    let snapshot = snapshot.filter(|s| s.usable_for(merchant_id, now_ms))?;
    let configured = configured_strength.filter(|m| valid_strength(*m));
    let fallback = || match configured {
        Some(m) => (m, PriorStrengthSource::Configured),
        None => (DEFAULT_PRIOR_STRENGTH, PriorStrengthSource::Default),
    };
    let gateway_prior = snapshot
        .entry(payment_method_type, payment_method, gateway)
        .and_then(|entry| {
            let learned = entry
                .learned_prior_strength
                .filter(|m| flags.learned_strength_enabled && valid_strength(*m));
            let (strength, source) = learned
                .map(|m| (m, PriorStrengthSource::Learned))
                .unwrap_or_else(fallback);
            SrPrior::new(entry.parent_success_rate, strength, source)
        });
    gateway_prior.or_else(|| {
        let pooled = snapshot.pooled_entry(payment_method_type, payment_method)?;
        let (strength, source) = fallback();
        SrPrior::new(pooled.parent_success_rate, strength, source).map(SrPrior::pooled)
    })
}

/// One row of the autopilot query: a gateway's outcomes in one segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewaySegmentOutcome {
    pub payment_method_type: String,
    pub payment_method: String,
    pub gateway: String,
    pub outcome: SegmentOutcome,
}

/// Settings for [`build_prior_entries`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PriorBuildConfig {
    pub fit: PriorFitConfig,
    /// Gateways (and payment methods, for the pooled entry) with fewer outcomes in the parent
    /// window get no entry.
    pub min_parent_observations: u64,
    /// Learn the prior strength (the merchant has `ENABLE_SR_V3_LEARNED_PRIOR_STRENGTH`).
    pub learn_strength: bool,
}

impl Default for PriorBuildConfig {
    fn default() -> Self {
        Self {
            fit: PriorFitConfig::default(),
            min_parent_observations: 30,
            learn_strength: false,
        }
    }
}

/// Entries of a prior snapshot.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PriorEntries {
    pub entries: Vec<SrPriorEntry>,
    pub pooled_entries: Vec<SrPooledPriorEntry>,
}

fn sum_outcomes<'a>(outcomes: impl Iterator<Item = &'a SegmentOutcome>) -> (u64, u64) {
    outcomes.fold((0u64, 0u64), |(s, n), o| {
        (
            s.saturating_add(o.successes.min(o.observations)),
            n.saturating_add(o.observations),
        )
    })
}

type GatewayKey = (String, String, String);

fn group_by_gateway(rows: &[GatewaySegmentOutcome]) -> BTreeMap<GatewayKey, Vec<SegmentOutcome>> {
    let mut grouped: BTreeMap<GatewayKey, Vec<SegmentOutcome>> = BTreeMap::new();
    for row in rows {
        grouped
            .entry((
                row.payment_method_type.clone(),
                row.payment_method.clone(),
                row.gateway.clone(),
            ))
            .or_default()
            .push(row.outcome);
    }
    grouped
}

/// Builds the snapshot entries from two windows of per-segment rows:
///
/// * `parent_rows` (a short, recent window) give each `(pmt, pm, gateway)` its parent rate and
///   each `(pmt, pm)` a pooled rate across gateways. Gateway health changes quickly, and a stale
///   parent holds sparse segments on a degraded gateway.
/// * `fit_rows` (a long window) give the learned prior strength, when enabled. Between-segment
///   heterogeneity changes slowly and needs many segments to estimate.
///
/// Gateways without enough parent outcomes get no entry (the decider then uses the pooled
/// entry). Output is sorted for stable snapshots.
pub fn build_prior_entries(
    parent_rows: &[GatewaySegmentOutcome],
    fit_rows: &[GatewaySegmentOutcome],
    config: &PriorBuildConfig,
) -> PriorEntries {
    let parents = group_by_gateway(parent_rows);
    let fits = if config.learn_strength {
        group_by_gateway(fit_rows)
    } else {
        BTreeMap::new()
    };

    let mut pooled: BTreeMap<(String, String), (u64, u64)> = BTreeMap::new();
    let mut entries = Vec::new();
    for ((pmt, pm, gateway), segments) in &parents {
        let (successes, observations) = sum_outcomes(segments.iter());
        let acc = pooled.entry((pmt.clone(), pm.clone())).or_default();
        acc.0 = acc.0.saturating_add(successes);
        acc.1 = acc.1.saturating_add(observations);
        let Some(parent) =
            parent_success_rate(successes, observations, config.min_parent_observations)
        else {
            continue;
        };
        let (learned, fit_segments, fit_error) = if config.learn_strength {
            let segments = fits
                .get(&(pmt.clone(), pm.clone(), gateway.clone()))
                .map(Vec::as_slice)
                .unwrap_or_default();
            let qualifying = segments
                .iter()
                .filter(|s| s.observations >= config.fit.min_segment_observations.max(1))
                .count() as u64;
            match fit_prior_strength(segments, &config.fit) {
                Ok(m) => (Some(m), Some(qualifying), None),
                Err(e) => (None, Some(qualifying), Some(e.to_string())),
            }
        } else {
            (None, None, None)
        };
        entries.push(SrPriorEntry {
            payment_method_type: pmt.clone(),
            payment_method: pm.clone(),
            gateway: gateway.clone(),
            parent_success_rate: parent,
            observations,
            learned_prior_strength: learned,
            fit_segments,
            fit_error,
        });
    }
    let pooled_entries = pooled
        .into_iter()
        .filter_map(|((pmt, pm), (successes, observations))| {
            Some(SrPooledPriorEntry {
                payment_method_type: pmt,
                payment_method: pm,
                parent_success_rate: parent_success_rate(
                    successes,
                    observations,
                    config.min_parent_observations,
                )?,
                observations,
            })
        })
        .collect();
    PriorEntries {
        entries,
        pooled_entries,
    }
}

#[cfg(test)]
mod tests {
    use rand::rngs::StdRng;
    use rand::{Rng, SeedableRng};
    use rand_distr::{Beta, Distribution};

    use super::*;

    fn window(real: &[u8], synthetic_ones: usize) -> Vec<String> {
        let mut entries: Vec<String> = real.iter().map(|v| v.to_string()).collect();
        entries.extend(std::iter::repeat_n(
            SYNTHETIC_SUCCESS.to_string(),
            synthetic_ones,
        ));
        entries
    }

    fn prior(p: f64, m: f64) -> SrPrior {
        SrPrior::new(p, m, PriorStrengthSource::Configured).expect("valid prior")
    }

    fn snapshot(merchant: &str, entries: Vec<SrPriorEntry>) -> SrPriorSnapshot {
        SrPriorSnapshot {
            version: PRIOR_SNAPSHOT_VERSION,
            merchant_id: merchant.to_string(),
            computed_at_ms: 1_000,
            valid_until_ms: 10_000,
            lookback_secs: 3_600,
            fit_lookback_secs: None,
            entries,
            pooled_entries: Vec::new(),
        }
    }

    fn entry(gw: &str, p: f64, learned: Option<f64>) -> SrPriorEntry {
        SrPriorEntry {
            payment_method_type: "CARD".into(),
            payment_method: "VISA".into(),
            gateway: gw.into(),
            parent_success_rate: p,
            observations: 500,
            learned_prior_strength: learned,
            fit_segments: None,
            fit_error: None,
        }
    }

    // ── window parsing & compatibility ──

    #[test]
    fn synthetic_markers_read_like_legacy_entries_under_the_legacy_reader() {
        // A seed written with the new markers must score exactly like the legacy `"1"` seed, so
        // routing is unchanged for merchants without the flag and for old binaries mid-deploy.
        let legacy = vec!["1".to_string(); 125];
        let marked = vec![SYNTHETIC_SUCCESS.to_string(); 125];
        assert_eq!(legacy_window_score(&legacy), legacy_window_score(&marked));
        let mixed_legacy = ["0", "1", "1", "0", "1"];
        let mixed_marked = [SYNTHETIC_FAILURE, "1", SYNTHETIC_SUCCESS, "0", "1"];
        assert_eq!(
            legacy_window_score(&mixed_legacy),
            legacy_window_score(&mixed_marked)
        );
    }

    #[test]
    fn legacy_window_score_matches_the_original_formula() {
        assert_eq!(legacy_window_score::<&str>(&[]), None);
        assert_eq!(legacy_window_score(&["x", ""]), None);
        assert_eq!(legacy_window_score(&["1", "0", "1", "1"]), Some(0.75));
        assert_eq!(legacy_window_score(&["1", "junk", "0"]), Some(0.5));
        assert_eq!(legacy_window_score(&["5"]), Some(1.0));
    }

    #[test]
    fn window_stats_separate_real_from_synthetic() {
        let stats = WindowStats::from_entries(&window(&[1, 0, 0, 1, 1], 120));
        assert_eq!(stats.real_observations, 5);
        assert_eq!(stats.real_successes, 3.0);
        assert_eq!(stats.real_failures(), 2.0);
        assert_eq!(stats.synthetic_observations, 120);
        assert_eq!(stats.synthetic_successes, 120.0);
        assert_eq!(stats.legacy_score(), Some(123.0 / 125.0));
    }

    #[test]
    fn window_stats_skip_malformed_and_non_finite_entries() {
        let stats = WindowStats::from_entries(&["abc", "", "NaN", "inf", "-inf", "1", " 1", "0.0"]);
        assert_eq!(stats.real_observations, 1);
        assert_eq!(stats.synthetic_observations, 1);
        assert_eq!(stats.synthetic_successes, 0.0);
    }

    #[test]
    fn out_of_range_real_values_are_clamped() {
        let stats = WindowStats::from_entries(&["7", "-3"]);
        assert_eq!(stats.real_observations, 2);
        assert_eq!(stats.real_successes, 1.0);
    }

    #[test]
    fn empty_window_has_no_legacy_score_and_reads_one_without_a_prior() {
        let stats = WindowStats::from_entries::<&str>(&[]);
        assert_eq!(stats, WindowStats::default());
        assert_eq!(stats.legacy_score(), None);
        assert_eq!(cold_start_score(None, &stats), 1.0);
    }

    #[test]
    fn legacy_windows_count_as_real_and_reproduce_the_legacy_ranking_signal() {
        // A pre-change window (seed written as "1") cannot be told apart; its entries count as
        // real, so the posterior is dominated by the window, as the legacy score was.
        let legacy: Vec<String> = std::iter::repeat_n("1".to_string(), 120)
            .chain(["0"; 5].iter().map(|s| s.to_string()))
            .collect();
        let stats = WindowStats::from_entries(&legacy);
        assert_eq!(stats.real_observations, 125);
        assert_eq!(stats.synthetic_observations, 0);
        let posterior = prior(0.7, 20.0).posterior_mean(&stats);
        assert!((posterior - (14.0 + 120.0) / 145.0).abs() < 1e-12);
    }

    // ── posterior ──

    #[test]
    fn posterior_with_no_real_outcomes_is_the_parent_rate() {
        let p = prior(0.82, 20.0);
        let seeded = WindowStats::from_entries(&window(&[], 125));
        assert!((p.posterior_mean(&seeded) - 0.82).abs() < 1e-12);
        assert!((p.posterior_mean(&WindowStats::default()) - 0.82).abs() < 1e-12);
    }

    #[test]
    fn synthetic_seed_does_not_move_the_posterior() {
        let p = prior(0.6, 20.0);
        let with_seed = WindowStats::from_entries(&window(&[1, 0, 0], 122));
        let without = WindowStats::from_entries(&window(&[1, 0, 0], 0));
        assert_eq!(p.posterior_mean(&with_seed), p.posterior_mean(&without));
        assert!((p.posterior_mean(&without) - (12.0 + 1.0) / 23.0).abs() < 1e-12);
    }

    #[test]
    fn posterior_lies_between_the_prior_and_the_data_property() {
        let mut rng = StdRng::seed_from_u64(7);
        for _ in 0..5_000 {
            let p = rng.gen_range(0.0..=1.0);
            let m = rng.gen_range(0.5..2_000.0);
            let n: u32 = rng.gen_range(0..400);
            let s = if n == 0 { 0 } else { rng.gen_range(0..=n) };
            let pr = SrPrior::new(p, m, PriorStrengthSource::Learned).expect("valid");
            let stats = WindowStats {
                real_successes: f64::from(s),
                real_observations: n,
                ..WindowStats::default()
            };
            let post = pr.posterior_mean(&stats);
            assert!((0.0..=1.0).contains(&post));
            let data = if n == 0 {
                pr.parent_success_rate()
            } else {
                f64::from(s) / f64::from(n)
            };
            let lo = pr.parent_success_rate().min(data) - 1e-9;
            let hi = pr.parent_success_rate().max(data) + 1e-9;
            assert!(
                post >= lo && post <= hi,
                "p={p} m={m} n={n} s={s} post={post}"
            );
            let (a, b) = pr.posterior_beta_params(&stats);
            assert!(a > 0.0 && b > 0.0);
            if a > MIN_BETA_PARAM && b > MIN_BETA_PARAM {
                assert!((a / (a + b) - post).abs() < 1e-9);
            }
        }
    }

    #[test]
    fn posterior_is_monotone_in_real_successes() {
        let p = prior(0.5, 20.0);
        let mut last = -1.0;
        for s in 0..=50u32 {
            let stats = WindowStats {
                real_successes: f64::from(s),
                real_observations: 50,
                ..WindowStats::default()
            };
            let post = p.posterior_mean(&stats);
            assert!(post > last);
            last = post;
        }
    }

    #[test]
    fn prior_rejects_invalid_values_and_clamps_extremes() {
        let src = PriorStrengthSource::Configured;
        assert!(SrPrior::new(f64::NAN, 20.0, src).is_none());
        assert!(SrPrior::new(1.2, 20.0, src).is_none());
        assert!(SrPrior::new(-0.1, 20.0, src).is_none());
        assert!(SrPrior::new(0.5, 0.0, src).is_none());
        assert!(SrPrior::new(0.5, -5.0, src).is_none());
        assert!(SrPrior::new(0.5, f64::INFINITY, src).is_none());
        let zero = SrPrior::new(0.0, 20.0, src).expect("clamped");
        assert!(zero.parent_success_rate() > 0.0);
        let one = SrPrior::new(1.0, 1e9, src).expect("clamped");
        assert!(one.parent_success_rate() < 1.0);
        assert_eq!(one.strength(), MAX_PRIOR_STRENGTH);
        assert_eq!(
            SrPrior::new(0.5, 0.01, src).expect("clamped").strength(),
            MIN_PRIOR_STRENGTH
        );
        let (a, b) = one.posterior_beta_params(&WindowStats::default());
        assert!(a > 0.0 && b > 0.0);
    }

    #[test]
    fn effective_observations_add_real_outcomes_to_the_strength() {
        let p = prior(0.5, 20.0);
        let stats = WindowStats::from_entries(&window(&[1, 0, 1], 50));
        assert_eq!(p.effective_observations(&stats), 23.0);
    }

    #[test]
    fn degradation_pulls_the_posterior_down_and_recovery_restores_it() {
        let p = prior(0.9, 20.0);
        let healthy = WindowStats::from_entries(&window(&[1; 40], 85));
        let before = p.posterior_mean(&healthy);
        // 30 failures arrive (LPUSH puts the newest at the front; the window keeps 125).
        let mut entries = vec!["0".to_string(); 30];
        entries.extend(window(&[1; 40], 55));
        let degraded = p.posterior_mean(&WindowStats::from_entries(&entries));
        assert!(
            degraded < before - 0.3,
            "before={before} degraded={degraded}"
        );
        // Recovery: 80 successes push the failures out of the window.
        let mut entries = vec!["1".to_string(); 80];
        entries.extend(vec!["0".to_string(); 30]);
        entries.truncate(125);
        let mid = p.posterior_mean(&WindowStats::from_entries(&entries));
        let recovered = p.posterior_mean(&WindowStats::from_entries(&vec!["1".to_string(); 125]));
        assert!(mid > degraded && recovered > mid);
    }

    // ── empirical Bayes ──

    fn simulate_segments(
        rng: &mut StdRng,
        kappa: f64,
        p: f64,
        k: usize,
        n: u64,
    ) -> Vec<SegmentOutcome> {
        let beta = Beta::new(p * kappa, (1.0 - p) * kappa).expect("valid beta");
        (0..k)
            .map(|_| {
                let rate: f64 = beta.sample(rng);
                let successes = (0..n).filter(|_| rng.gen::<f64>() < rate).count() as u64;
                SegmentOutcome {
                    successes,
                    observations: n,
                }
            })
            .collect()
    }

    #[test]
    fn fit_recovers_the_concentration_of_simulated_segments() {
        let mut rng = StdRng::seed_from_u64(11);
        for &kappa in &[3.0, 10.0, 40.0] {
            let mut estimates: Vec<f64> = (0..60)
                .map(|_| {
                    let segs = simulate_segments(&mut rng, kappa, 0.85, 200, 60);
                    fit_prior_strength(&segs, &PriorFitConfig::default()).expect("fit")
                })
                .collect();
            estimates.sort_by(|a, b| a.total_cmp(b));
            let median = estimates[estimates.len() / 2];
            assert!(
                median > kappa * 0.7 && median < kappa * 1.4,
                "kappa={kappa} median={median}"
            );
        }
    }

    #[test]
    fn homogeneous_segments_hit_the_upper_clamp() {
        let segs: Vec<SegmentOutcome> = (0..50)
            .map(|_| SegmentOutcome {
                successes: 80,
                observations: 100,
            })
            .collect();
        assert_eq!(
            fit_prior_strength(&segs, &PriorFitConfig::default()),
            Ok(MAX_PRIOR_STRENGTH)
        );
    }

    #[test]
    fn maximal_heterogeneity_hits_the_lower_clamp() {
        let segs: Vec<SegmentOutcome> = (0..50)
            .map(|i| SegmentOutcome {
                successes: if i % 2 == 0 { 100 } else { 0 },
                observations: 100,
            })
            .collect();
        assert_eq!(
            fit_prior_strength(&segs, &PriorFitConfig::default()),
            Ok(MIN_PRIOR_STRENGTH)
        );
    }

    #[test]
    fn fit_requires_enough_qualifying_segments() {
        let cfg = PriorFitConfig::default();
        let mut segs = vec![
            SegmentOutcome {
                successes: 3,
                observations: 4
            };
            100
        ];
        segs.extend(vec![
            SegmentOutcome {
                successes: 8,
                observations: 10
            };
            9
        ]);
        assert_eq!(
            fit_prior_strength(&segs, &cfg),
            Err(PriorFitError::InsufficientSegments {
                qualifying: 9,
                required: 10
            })
        );
        assert_eq!(
            fit_prior_strength(&[], &cfg),
            Err(PriorFitError::InsufficientSegments {
                qualifying: 0,
                required: 10
            })
        );
    }

    #[test]
    fn fit_rejects_degenerate_rates() {
        let all_success = vec![
            SegmentOutcome {
                successes: 10,
                observations: 10
            };
            20
        ];
        let all_failure = vec![
            SegmentOutcome {
                successes: 0,
                observations: 10
            };
            20
        ];
        let cfg = PriorFitConfig::default();
        assert_eq!(
            fit_prior_strength(&all_success, &cfg),
            Err(PriorFitError::DegenerateRate)
        );
        assert_eq!(
            fit_prior_strength(&all_failure, &cfg),
            Err(PriorFitError::DegenerateRate)
        );
    }

    #[test]
    fn fit_reports_unidentifiable_without_degrees_of_freedom() {
        // Two single-outcome segments: N − Σn²/N − (k−1) = 2 − 1 − 1 = 0.
        let segs = [
            SegmentOutcome {
                successes: 1,
                observations: 1,
            },
            SegmentOutcome {
                successes: 0,
                observations: 1,
            },
        ];
        let cfg = PriorFitConfig {
            min_segments: 2,
            min_segment_observations: 1,
        };
        assert_eq!(
            fit_prior_strength(&segs, &cfg),
            Err(PriorFitError::Unidentifiable)
        );
    }

    #[test]
    fn fit_survives_malformed_and_extreme_rows() {
        let mut segs = vec![
            SegmentOutcome {
                successes: 50,
                observations: 10
            };
            5
        ];
        segs.extend((0..20).map(|i| SegmentOutcome {
            successes: u64::MAX / 4 + i,
            observations: u64::MAX / 2,
        }));
        segs.push(SegmentOutcome {
            successes: 0,
            observations: 0,
        });
        let m = fit_prior_strength(&segs, &PriorFitConfig::default()).expect("fit");
        assert!((MIN_PRIOR_STRENGTH..=MAX_PRIOR_STRENGTH).contains(&m));
    }

    #[test]
    fn fit_output_is_always_finite_and_clamped_property() {
        let mut rng = StdRng::seed_from_u64(23);
        let cfg = PriorFitConfig {
            min_segments: 2,
            min_segment_observations: 1,
        };
        for _ in 0..2_000 {
            let k = rng.gen_range(0..40);
            let segs: Vec<SegmentOutcome> = (0..k)
                .map(|_| {
                    let n = rng.gen_range(0..300u64);
                    SegmentOutcome {
                        successes: rng.gen_range(0..=n + 5),
                        observations: n,
                    }
                })
                .collect();
            if let Ok(m) = fit_prior_strength(&segs, &cfg) {
                assert!(m.is_finite() && (MIN_PRIOR_STRENGTH..=MAX_PRIOR_STRENGTH).contains(&m));
            }
        }
    }

    #[test]
    fn parent_rate_is_laplace_smoothed_and_gated() {
        assert_eq!(parent_success_rate(0, 0, 0), None);
        assert_eq!(parent_success_rate(10, 20, 30), None);
        assert_eq!(parent_success_rate(28, 30, 30), Some(29.0 / 32.0));
        assert_eq!(parent_success_rate(40, 30, 30), Some(31.0 / 32.0));
    }

    // ── snapshot & resolution ──

    #[test]
    fn snapshot_round_trips_and_tolerates_missing_optional_fields() {
        let snap = snapshot("m1", vec![entry("stripe", 0.8, Some(42.0))]);
        let json = serde_json::to_string(&snap).expect("serialize");
        assert!(json.contains("\"parentSuccessRate\":0.8"));
        let back: SrPriorSnapshot = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, snap);
        let minimal = r#"{"version":1,"merchantId":"m1","computedAtMs":1,"validUntilMs":5,
            "lookbackSecs":60,"entries":[{"paymentMethodType":"CARD","paymentMethod":"VISA",
            "gateway":"adyen","parentSuccessRate":0.7,"observations":40}]}"#;
        let parsed: SrPriorSnapshot = serde_json::from_str(minimal).expect("deserialize");
        assert_eq!(parsed.entries[0].learned_prior_strength, None);
    }

    #[test]
    fn snapshot_of_another_merchant_is_never_used() {
        let snap = snapshot("merchant_a", vec![entry("stripe", 0.8, None)]);
        let flags = ColdStartFlags {
            prior_enabled: true,
            learned_strength_enabled: true,
        };
        assert!(snap.usable_for("merchant_a", 5_000));
        assert!(!snap.usable_for("merchant_b", 5_000));
        assert!(!snap.usable_for("merchant_a_suffix", 5_000));
        assert_eq!(
            resolve_prior(
                flags,
                Some(&snap),
                "merchant_b",
                5_000,
                "CARD",
                "VISA",
                "stripe",
                None
            ),
            None
        );
    }

    #[test]
    fn stale_or_foreign_version_snapshots_are_ignored() {
        let flags = ColdStartFlags {
            prior_enabled: true,
            learned_strength_enabled: false,
        };
        let snap = snapshot("m1", vec![entry("stripe", 0.8, None)]);
        assert!(resolve_prior(
            flags,
            Some(&snap),
            "m1",
            9_999,
            "CARD",
            "VISA",
            "stripe",
            None
        )
        .is_some());
        assert!(resolve_prior(
            flags,
            Some(&snap),
            "m1",
            10_000,
            "CARD",
            "VISA",
            "stripe",
            None
        )
        .is_none());
        let mut v2 = snap.clone();
        v2.version = PRIOR_SNAPSHOT_VERSION + 1;
        assert!(resolve_prior(
            flags,
            Some(&v2),
            "m1",
            5_000,
            "CARD",
            "VISA",
            "stripe",
            None
        )
        .is_none());
        assert!(resolve_prior(flags, None, "m1", 5_000, "CARD", "VISA", "stripe", None).is_none());
    }

    #[test]
    fn job_interruption_falls_back_at_expiry_and_recovers_on_the_next_write() {
        let flags = ColdStartFlags {
            prior_enabled: true,
            learned_strength_enabled: true,
        };
        let hour = 3_600_000;
        let resolve = |snap: &SrPriorSnapshot, now: i64| {
            resolve_prior(flags, Some(snap), "m1", now, "CARD", "VISA", "stripe", None)
        };
        // Written at t0, valid for 6 h (the job's default `prior_max_age_secs`).
        let t0 = 1_700_000_000_000;
        let mut snap = snapshot("m1", vec![entry("stripe", 0.8, Some(40.0))]);
        snap.pooled_entries = vec![pooled("CARD", "VISA", 0.75)];
        snap.computed_at_ms = t0;
        snap.valid_until_ms = t0 + 6 * hour;
        // The job stops: the last snapshot keeps applying until it expires…
        assert!(resolve(&snap, t0 + 6 * hour - 1).is_some());
        // …and from expiry on every gateway is scored the legacy way (deterministically None,
        // gateway and pooled entries alike).
        assert!(resolve(&snap, t0 + 6 * hour).is_none());
        assert!(resolve_prior(
            flags,
            Some(&snap),
            "m1",
            t0 + 7 * hour,
            "CARD",
            "VISA",
            "x",
            None
        )
        .is_none());
        // The job resumes and writes a fresh snapshot: priors apply again immediately.
        let mut fresh = snap.clone();
        fresh.computed_at_ms = t0 + 9 * hour;
        fresh.valid_until_ms = t0 + 15 * hour;
        assert!(resolve(&fresh, t0 + 9 * hour + 1).is_some());
    }

    #[test]
    fn stale_snapshots_cannot_apply_indefinitely_even_with_a_huge_validity() {
        let flags = ColdStartFlags {
            prior_enabled: true,
            learned_strength_enabled: false,
        };
        let t0 = 1_700_000_000_000;
        let mut snap = snapshot("m1", vec![entry("stripe", 0.8, None)]);
        snap.computed_at_ms = t0;
        snap.valid_until_ms = i64::MAX;
        let at = |now| {
            resolve_prior(
                flags,
                Some(&snap),
                "m1",
                now,
                "CARD",
                "VISA",
                "stripe",
                None,
            )
        };
        assert!(at(t0 + MAX_PRIOR_SNAPSHOT_AGE_MS).is_some());
        assert!(at(t0 + MAX_PRIOR_SNAPSHOT_AGE_MS + 1).is_none());
        // A snapshot stamped in the future (beyond clock skew) is not trusted either.
        assert!(at(t0 - 10 * 60 * 1_000).is_none());
        assert!(at(t0 - 60 * 1_000).is_some());
    }

    #[test]
    fn entries_match_case_insensitively_and_missing_gateways_fall_back() {
        let flags = ColdStartFlags {
            prior_enabled: true,
            learned_strength_enabled: false,
        };
        let snap = snapshot("m1", vec![entry("Stripe", 0.8, None)]);
        assert!(resolve_prior(
            flags,
            Some(&snap),
            "m1",
            5_000,
            "card",
            "visa",
            "STRIPE",
            None
        )
        .is_some());
        assert!(resolve_prior(
            flags,
            Some(&snap),
            "m1",
            5_000,
            "CARD",
            "VISA",
            "adyen",
            None
        )
        .is_none());
        assert!(resolve_prior(
            flags,
            Some(&snap),
            "m1",
            5_000,
            "UPI",
            "VISA",
            "stripe",
            None
        )
        .is_none());
    }

    #[test]
    fn invalid_entry_rate_falls_back_to_legacy() {
        let flags = ColdStartFlags {
            prior_enabled: true,
            learned_strength_enabled: true,
        };
        let snap = snapshot(
            "m1",
            vec![
                entry("stripe", f64::NAN, Some(10.0)),
                entry("adyen", 1.7, None),
            ],
        );
        assert!(resolve_prior(
            flags,
            Some(&snap),
            "m1",
            5_000,
            "CARD",
            "VISA",
            "stripe",
            None
        )
        .is_none());
        assert!(resolve_prior(
            flags,
            Some(&snap),
            "m1",
            5_000,
            "CARD",
            "VISA",
            "adyen",
            None
        )
        .is_none());
    }

    #[test]
    fn feature_flag_combinations_select_the_documented_strength() {
        let snap = snapshot("m1", vec![entry("stripe", 0.8, Some(55.0))]);
        let resolve = |prior_enabled, learned_strength_enabled, configured| {
            resolve_prior(
                ColdStartFlags {
                    prior_enabled,
                    learned_strength_enabled,
                },
                Some(&snap),
                "m1",
                5_000,
                "CARD",
                "VISA",
                "stripe",
                configured,
            )
            .map(|p| (p.strength(), p.strength_source()))
        };
        // Both off and learned-only: legacy scoring.
        assert_eq!(resolve(false, false, Some(30.0)), None);
        assert_eq!(resolve(false, true, Some(30.0)), None);
        // Prior only: configured, else default; the learned value is ignored.
        assert_eq!(
            resolve(true, false, Some(30.0)),
            Some((30.0, PriorStrengthSource::Configured))
        );
        assert_eq!(
            resolve(true, false, None),
            Some((DEFAULT_PRIOR_STRENGTH, PriorStrengthSource::Default))
        );
        // Both on: learned wins.
        assert_eq!(
            resolve(true, true, Some(30.0)),
            Some((55.0, PriorStrengthSource::Learned))
        );
    }

    #[test]
    fn invalid_learned_or_configured_strengths_fall_through() {
        let flags = ColdStartFlags {
            prior_enabled: true,
            learned_strength_enabled: true,
        };
        for bad in [f64::NAN, -1.0, 0.0, f64::INFINITY] {
            let snap = snapshot("m1", vec![entry("stripe", 0.8, Some(bad))]);
            let resolved = resolve_prior(
                flags,
                Some(&snap),
                "m1",
                5_000,
                "CARD",
                "VISA",
                "stripe",
                Some(bad),
            )
            .expect("falls back to default");
            assert_eq!(resolved.strength_source(), PriorStrengthSource::Default);
            let resolved = resolve_prior(
                flags,
                Some(&snap),
                "m1",
                5_000,
                "CARD",
                "VISA",
                "stripe",
                Some(12.0),
            )
            .expect("falls back to configured");
            assert_eq!(resolved.strength(), 12.0);
        }
    }

    fn pooled(pmt: &str, pm: &str, p: f64) -> SrPooledPriorEntry {
        SrPooledPriorEntry {
            payment_method_type: pmt.into(),
            payment_method: pm.into(),
            parent_success_rate: p,
            observations: 900,
        }
    }

    #[test]
    fn gateways_without_an_entry_shrink_toward_the_pooled_rate() {
        let flags = ColdStartFlags {
            prior_enabled: true,
            learned_strength_enabled: true,
        };
        let mut snap = snapshot("m1", vec![entry("stripe", 0.9, Some(80.0))]);
        snap.pooled_entries = vec![pooled("CARD", "VISA", 0.75)];
        let own = resolve_prior(
            flags,
            Some(&snap),
            "m1",
            5_000,
            "CARD",
            "VISA",
            "stripe",
            Some(15.0),
        )
        .expect("own entry");
        assert_eq!(own.scope(), PriorScope::Gateway);
        assert_eq!(own.strength_source(), PriorStrengthSource::Learned);
        let fallback = resolve_prior(
            flags,
            Some(&snap),
            "m1",
            5_000,
            "card",
            "visa",
            "adyen",
            Some(15.0),
        )
        .expect("pooled entry");
        assert_eq!(fallback.scope(), PriorScope::Pooled);
        assert!((fallback.parent_success_rate() - 0.75).abs() < 1e-12);
        // A pooled prior never takes a learned strength: that is fitted per gateway.
        assert_eq!(fallback.strength(), 15.0);
        assert_eq!(fallback.strength_source(), PriorStrengthSource::Configured);
        // Pooled entries are per payment method.
        assert!(
            resolve_prior(flags, Some(&snap), "m1", 5_000, "UPI", "UPI", "adyen", None).is_none()
        );
    }

    #[test]
    fn an_invalid_gateway_entry_falls_back_to_the_pooled_rate() {
        let flags = ColdStartFlags {
            prior_enabled: true,
            learned_strength_enabled: false,
        };
        let mut snap = snapshot("m1", vec![entry("stripe", f64::NAN, None)]);
        snap.pooled_entries = vec![pooled("CARD", "VISA", 0.8)];
        let prior = resolve_prior(
            flags,
            Some(&snap),
            "m1",
            5_000,
            "CARD",
            "VISA",
            "stripe",
            None,
        )
        .expect("pooled");
        assert_eq!(prior.scope(), PriorScope::Pooled);
        assert_eq!(prior.strength_source(), PriorStrengthSource::Default);
    }

    #[test]
    fn pooled_entries_are_ignored_when_the_snapshot_is_unusable() {
        let flags = ColdStartFlags {
            prior_enabled: true,
            learned_strength_enabled: false,
        };
        let mut snap = snapshot("m1", Vec::new());
        snap.pooled_entries = vec![pooled("CARD", "VISA", 0.8)];
        assert!(
            resolve_prior(flags, Some(&snap), "m2", 5_000, "CARD", "VISA", "x", None).is_none()
        );
        assert!(
            resolve_prior(flags, Some(&snap), "m1", 10_000, "CARD", "VISA", "x", None).is_none()
        );
        let off = ColdStartFlags::default();
        assert!(resolve_prior(off, Some(&snap), "m1", 5_000, "CARD", "VISA", "x", None).is_none());
    }

    #[test]
    fn parent_rates_come_from_the_parent_window_and_strength_from_the_fit_window() {
        let mut rng = StdRng::seed_from_u64(5);
        let fit_rows: Vec<GatewaySegmentOutcome> = simulate_segments(&mut rng, 10.0, 0.9, 60, 30)
            .into_iter()
            .map(|o| row("a", o.successes, o.observations))
            .collect();
        // The recent window shows the gateway degraded to ~50%.
        let parent_rows = vec![row("a", 20, 40), row("a", 21, 40)];
        let cfg = PriorBuildConfig {
            learn_strength: true,
            ..PriorBuildConfig::default()
        };
        let built = build_prior_entries(&parent_rows, &fit_rows, &cfg);
        let a = &built.entries[0];
        assert!((a.parent_success_rate - 42.0 / 82.0).abs() < 1e-12);
        assert_eq!(a.observations, 80);
        assert!(a.learned_prior_strength.is_some());
        assert_eq!(a.fit_segments, Some(60));
    }

    #[test]
    fn snapshots_written_before_pooled_entries_existed_still_parse() {
        let old = r#"{"version":1,"merchantId":"m1","computedAtMs":1,"validUntilMs":5,
            "lookbackSecs":60,"entries":[]}"#;
        let parsed: SrPriorSnapshot = serde_json::from_str(old).expect("deserialize");
        assert!(parsed.pooled_entries.is_empty());
        assert_eq!(parsed.fit_lookback_secs, None);
    }

    // ── snapshot building ──

    fn row(gw: &str, s: u64, n: u64) -> GatewaySegmentOutcome {
        GatewaySegmentOutcome {
            payment_method_type: "CARD".into(),
            payment_method: "VISA".into(),
            gateway: gw.into(),
            outcome: SegmentOutcome {
                successes: s,
                observations: n,
            },
        }
    }

    #[test]
    fn build_aggregates_parents_and_skips_thin_gateways() {
        let rows = vec![row("a", 18, 20), row("a", 9, 20), row("b", 5, 10)];
        let built = build_prior_entries(&rows, &rows, &PriorBuildConfig::default());
        let entries = built.entries;
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].gateway, "a");
        assert_eq!(entries[0].observations, 40);
        assert!((entries[0].parent_success_rate - 28.0 / 42.0).abs() < 1e-12);
        assert_eq!(entries[0].learned_prior_strength, None);
        assert_eq!(entries[0].fit_error, None);
        // The pooled entry covers every gateway of the payment method, thin ones included.
        assert_eq!(built.pooled_entries.len(), 1);
        assert_eq!(built.pooled_entries[0].observations, 50);
        assert!((built.pooled_entries[0].parent_success_rate - 33.0 / 52.0).abs() < 1e-12);
    }

    #[test]
    fn build_learns_strength_only_when_enabled_and_reports_why_not() {
        let mut rng = StdRng::seed_from_u64(3);
        let rows: Vec<GatewaySegmentOutcome> = simulate_segments(&mut rng, 10.0, 0.8, 80, 30)
            .into_iter()
            .map(|o| row("a", o.successes, o.observations))
            .chain([row("b", 40, 50)])
            .collect();
        let cfg = PriorBuildConfig {
            learn_strength: true,
            ..PriorBuildConfig::default()
        };
        let entries = build_prior_entries(&rows, &rows, &cfg).entries;
        let a = entries.iter().find(|e| e.gateway == "a").expect("a");
        assert!(a.learned_prior_strength.is_some());
        assert_eq!(a.fit_segments, Some(80));
        let b = entries.iter().find(|e| e.gateway == "b").expect("b");
        assert_eq!(b.learned_prior_strength, None);
        assert!(b
            .fit_error
            .as_deref()
            .is_some_and(|e| e.contains("insufficient")));
    }
}
