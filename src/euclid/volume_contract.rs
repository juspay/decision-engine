//! Volume-commitment contract configuration DSL.
//!
//! Expresses a merchant's PSP volume-commitment contracts as a typed document that rides the
//! euclid rule machinery: the document is the `volume_contract` variant of
//! [`StaticRoutingAlgorithm`](super::types::StaticRoutingAlgorithm), persisted in
//! `routing_algorithm.algorithm_data` and activated through `routing_algorithm_mapper` under the
//! dedicated `algorithm_for = "volume_commitment"` slot, so it can never collide with the
//! merchant's live payment/payout/3DS algorithm.
//!
//! v1 scope is expression + storage only: no forecasting, pacing, feasibility or steering logic
//! reads this document yet. Archetypes `lumpsum` (A) and `tiered` (C) are writable;
//! `min_commitment` (B) parses — its wire format is frozen for forward compatibility — but is
//! rejected by validation until the terms are finalized.
//!
//! # Canonical form
//! The document declares one `metric` (gmv or transaction count) and one `currency` — the
//! consuming engine has no concept of currencies or mixed units, so `expected_daily_traffic`
//! and every per-PSP goal must be comparable numbers. Amounts are accepted as JSON integers or
//! decimal strings, in the units declared by `currency.amount_units` (`major` or `minor`,
//! default `minor`). [`canonicalize`] converts every amount to an integer in *minor* currency
//! units (or a plain count for `metric: volume`) and rewrites `amount_units` to `minor` before
//! the document is stored, so every consumer reads exactly one representation. Tolerance is
//! likewise accepted as `"5pp"` / `"550bps"` / plain basis points and stored as integer basis
//! points. The DSL stores contract facts only — deriving goals/rewards from tiers, fractions
//! from bps, or period-end dates from billing cycles is deliberately left to the consumer.
//!
//! # Schema evolution
//! Documents carry `schema_version`; writes accept only `SCHEMA_VERSION_MIN..=SCHEMA_VERSION_MAX`.
//! - Adding an optional field with a serde default: no bump.
//! - Adding a required field, removing a field, or changing the meaning of one: bump
//!   `SCHEMA_VERSION_MAX` and keep parse support for older versions.
//! - Enabling archetype B, or the reserved `scope` conditions, is a validation change only.

use serde::{Deserialize, Serialize};

use super::ast::Comparison;
use super::errors::ValidationErrorDetails;
use crate::types::currency::Currency;

pub const SCHEMA_VERSION_MIN: u32 = 1;
pub const SCHEMA_VERSION_MAX: u32 = 1;

fn default_schema_version() -> u32 {
    SCHEMA_VERSION_MAX
}

/// Hard caps on document size and rates, enforced by [`validate_volume_contract_config`].
const MAX_CONTRACTS: usize = 50;
const MAX_TIERS: usize = 20;
const MAX_TOLERANCE_BPS: u16 = 2000;
const MAX_RATE_BPS: u32 = 10_000;
const MAX_REBATE_LAG_DAYS: u16 = 365;
/// Low enough that a `test_minutes` cycle can still forecast and release in chunks; production
/// contracts simply set sane values.
const MIN_INTERVAL_SECS: u32 = 5;
const MAX_INTERVAL_SECS: u32 = 604_800; // one week

// ── Document root ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VolumeContractConfig {
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    pub routing_mode: RoutingMode,
    /// SR-gap tolerance between the best PSP and a steered PSP. Accepts `500`, `"5pp"` or
    /// `"500bps"` on input; stored as basis points.
    #[serde(alias = "tolerance")]
    pub tolerance_bps: ToleranceBps,
    /// What every amount in this document counts — money processed or transactions. One metric
    /// per merchant: the consumer compares `expected_daily_traffic` against per-PSP goals, so
    /// they must all be in the same unit.
    #[serde(default)]
    pub metric: CommitmentMetric,
    /// The single currency every amount in this document is denominated in (informational for
    /// `metric: volume`, where amounts are plain counts).
    pub currency: CurrencySpec,
    /// Expected total volume per day across all PSPs, in the document's metric/currency unit.
    /// Seeds the consumer's day-0 pace forecast.
    pub expected_daily_traffic: Amount,
    /// Per-merchant override of the forecast cadence; omit to use the engine's global default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forecast_interval_secs: Option<u32>,
    pub volume_contracts: Vec<VolumeContract>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::Display)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum RoutingMode {
    /// Mode 1: optimize auth rate by default, steer only when a commitment drifts short.
    PaceGuarded,
    /// Mode 2: fulfill the commitments first.
    VolumeCommitment,
}

// ── Per-PSP contract ──────────────────────────────────────────────────────────

/// One volume-commitment contract with one PSP.
///
/// `Deserialize` is hand-written (delegating to a derived mirror struct) because
/// `#[serde(flatten)]` on `terms` disables `deny_unknown_fields`; the manual impl restores
/// unknown-key rejection so a typo'd field is a 400, not silently dropped.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct VolumeContract {
    /// Unique within the document. `[A-Za-z0-9_-]{1,64}`.
    pub id: String,
    /// PSP connector value, e.g. `"adyen"` — a value, never an enum, matching
    /// `ConnectorInfo.gateway_name` and `cost_ingestion.connector`.
    pub connector: String,
    #[serde(default)]
    pub status: ContractStatus,
    pub billing_cycle: BillingCycle,
    /// Archetype discriminator plus the matching terms block, adjacently tagged and flattened —
    /// the archetype↔block pairing is enforced by the type system, not by validation.
    #[serde(flatten)]
    pub terms: ContractTerms,
    /// RESERVED: future payment-cluster scoping (card_scheme / funding_type / country …)
    /// expressed as euclid conditions. Must be absent in v1 (validated).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<Vec<Comparison>>,
}

const VOLUME_CONTRACT_KEYS: &[&str] = &[
    "id",
    "connector",
    "status",
    "billing_cycle",
    "archetype",
    "terms",
    "scope",
];

#[derive(Deserialize)]
struct VolumeContractDe {
    id: String,
    connector: String,
    #[serde(default)]
    status: ContractStatus,
    billing_cycle: BillingCycle,
    #[serde(flatten)]
    terms: ContractTerms,
    #[serde(default)]
    scope: Option<Vec<Comparison>>,
}

impl<'de> Deserialize<'de> for VolumeContract {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error as _;
        let map = serde_json::Map::deserialize(deserializer)?;
        reject_unknown_keys::<D>(&map, VOLUME_CONTRACT_KEYS, "volume contract")?;
        let de: VolumeContractDe =
            serde_json::from_value(serde_json::Value::Object(map)).map_err(D::Error::custom)?;
        Ok(Self {
            id: de.id,
            connector: de.connector,
            status: de.status,
            billing_cycle: de.billing_cycle,
            terms: de.terms,
            scope: de.scope,
        })
    }
}

fn reject_unknown_keys<'de, D>(
    map: &serde_json::Map<String, serde_json::Value>,
    allowed: &[&str],
    what: &str,
) -> Result<(), D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error as _;
    let unknown: Vec<String> = map
        .keys()
        .filter(|k| !allowed.contains(&k.as_str()))
        .cloned()
        .collect();
    if unknown.is_empty() {
        Ok(())
    } else {
        Err(D::Error::custom(format!(
            "unknown field(s) in {what}: {} (expected one of: {})",
            unknown.join(", "),
            allowed.join(", ")
        )))
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, strum::Display)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum ContractStatus {
    #[default]
    Active,
    Inactive,
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize, strum::Display,
)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum CommitmentMetric {
    /// Goal counted in money processed; amounts are currency amounts.
    #[default]
    Gmv,
    /// Goal counted in transactions; amounts are plain counts.
    Volume,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CurrencySpec {
    /// ISO-4217 currency every amount in this contract is denominated in.
    pub denomination: Currency,
    /// How amounts were expressed on input. Always rewritten to `minor` by [`canonicalize`]
    /// before storage.
    #[serde(default)]
    pub amount_units: AmountUnits,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AmountUnits {
    /// `6_000_000` = $6M. Converted to minor units at write time.
    Major,
    /// `600_000_000` = 6M USD cents. The canonical stored form.
    #[default]
    Minor,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BillingCycle {
    #[serde(rename = "type")]
    pub cycle_type: BillingCycleType,
    /// `calendar_month`: day-of-month 1–30; `calendar_quarter`: month-in-quarter 1–3;
    /// `calendar_year`: start month 1–12; `test_minutes`: cycle length in minutes 2–240.
    /// Range-validated per cycle type on write.
    pub anchor: u8,
    /// IANA zone name, e.g. `"America/New_York"`. Validated against the tz database on write.
    pub timezone: String,
    #[serde(default)]
    pub proration: Proration,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, strum::Display)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum BillingCycleType {
    CalendarMonth,
    CalendarQuarter,
    CalendarYear,
    /// TESTING: the cycle lasts `anchor` minutes and repeats from the Unix epoch, so a whole
    /// period plays out while you watch. Each minute counts as one contract "day", so pacing,
    /// elimination and steering behave exactly as on a calendar cycle — only faster.
    TestMinutes,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Proration {
    #[default]
    FullPeriod,
}

// ── Archetypes ────────────────────────────────────────────────────────────────

/// Contract archetype and its terms, adjacently tagged — the same `tag`/`content` idiom as
/// `StaticRoutingAlgorithm` and `ValueType`, so an archetype can never carry the wrong block.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, strum::Display)]
#[serde(tag = "archetype", content = "terms", rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum ContractTerms {
    /// Archetype A: hit `target`, earn `reward`.
    Lumpsum(FlatTerms),
    /// Archetype B: committed floor with a flat fee. Parses (wire format frozen) but is
    /// rejected by v1 validation until the business finalizes the terms.
    MinCommitment(CommitmentTerms),
    /// Archetype C: staggered rebates across volume/GMV tiers.
    Tiered(TieredTerms),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlatTerms {
    /// GMV (or transaction count, per `metric`) that unlocks the reward.
    pub target: Amount,
    pub reward: Reward,
}

/// The PRD's "`flat_amount` iff `kind == flat`, `rebate_bps` iff `kind == percentage`" rule as a
/// tagged enum — the mutual exclusion cannot be violated on the wire. The payloads are
/// standalone structs (not struct variants) because `deny_unknown_fields` only works on
/// structs, and a `flat` reward smuggling a `rebate_bps` must be a parse error.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum Reward {
    Flat(FlatReward),
    Percentage(PercentageReward),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FlatReward {
    pub flat_amount: Amount,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PercentageReward {
    pub rebate_bps: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CommitmentTerms {
    /// Committed volume floor the flat fee pays for.
    pub floor: Amount,
    pub reward: Reward,
    /// Rate charged above the floor, in basis points.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overage_rate_bps: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TieredTerms {
    /// Non-empty, thresholds strictly increasing (validated).
    pub tiers: Vec<Tier>,
}

/// One rebate tier. `Deserialize` is hand-written for the same flatten/unknown-key reason as
/// [`VolumeContract`].
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Tier {
    /// Tier kind and the rate field that goes with it — `retroactive` pays `rebate_bps` on the
    /// whole period once crossed, `marginal` pays `rate_bps` on volume above the threshold.
    #[serde(flatten)]
    pub rate: TierRate,
    pub threshold: Amount,
    /// The tier the merchant is steering for. Exactly one tier per contract must carry this
    /// (validated); the consumer reads that tier's threshold as the PSP's goal and its rebate as
    /// the reward, without interpreting the rest of the ladder.
    #[serde(default)]
    pub targeted: bool,
    /// Days after cycle close until the reward lands.
    #[serde(default)]
    pub rebate_lag_days: u16,
    #[serde(default)]
    pub rebate_settlement: RebateSettlement,
}

const TIER_KEYS: &[&str] = &[
    "kind",
    "rate",
    "threshold",
    "targeted",
    "rebate_lag_days",
    "rebate_settlement",
];

#[derive(Deserialize)]
struct TierDe {
    #[serde(flatten)]
    rate: TierRate,
    threshold: Amount,
    #[serde(default)]
    targeted: bool,
    #[serde(default)]
    rebate_lag_days: u16,
    #[serde(default)]
    rebate_settlement: RebateSettlement,
}

impl<'de> Deserialize<'de> for Tier {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error as _;
        let map = serde_json::Map::deserialize(deserializer)?;
        reject_unknown_keys::<D>(&map, TIER_KEYS, "tier")?;
        let de: TierDe =
            serde_json::from_value(serde_json::Value::Object(map)).map_err(D::Error::custom)?;
        Ok(Self {
            rate: de.rate,
            threshold: de.threshold,
            targeted: de.targeted,
            rebate_lag_days: de.rebate_lag_days,
            rebate_settlement: de.rebate_settlement,
        })
    }
}

/// Same standalone-struct pattern as [`Reward`], for the same unknown-field reason.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "rate", rename_all = "snake_case")]
pub enum TierRate {
    /// Rebate on the whole period's volume once the threshold is crossed.
    Retroactive(RetroactiveRate),
    /// Rate applied only to volume above the threshold.
    Marginal(MarginalRate),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetroactiveRate {
    pub rebate_bps: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MarginalRate {
    pub rate_bps: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RebateSettlement {
    CreditNote,
    #[default]
    Cash,
}

// ── Amounts and tolerance ─────────────────────────────────────────────────────

/// A monetary quantity (or transaction count for `metric: volume`). Input accepts a JSON integer
/// or a decimal string like `"6000000.50"`; [`canonicalize`] converts every amount to
/// `Amount::Int` in minor units before storage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Amount {
    Int(u64),
    Decimal(String),
}

impl Amount {
    /// The canonical integer value. `None` until [`canonicalize`] has run.
    pub fn as_canonical(&self) -> Option<u64> {
        match self {
            Self::Int(n) => Some(*n),
            Self::Decimal(_) => None,
        }
    }
}

/// Basis points, stored as a plain number. Input accepts `500`, `"5pp"`, `"500bps"` or `"500"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct ToleranceBps(pub u16);

impl<'de> Deserialize<'de> for ToleranceBps {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error as _;
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Int(u64),
            Str(String),
        }
        let bps = match Raw::deserialize(deserializer)? {
            Raw::Int(n) => n,
            Raw::Str(s) => parse_tolerance(&s).map_err(D::Error::custom)?,
        };
        u16::try_from(bps)
            .map(ToleranceBps)
            .map_err(|_| D::Error::custom(format!("tolerance {bps} bps out of range")))
    }
}

fn parse_tolerance(s: &str) -> Result<u64, String> {
    let t = s.trim().to_ascii_lowercase();
    let invalid = || format!("invalid tolerance '{s}': expected basis points, '<n>pp' or '<n>bps'");
    if let Some(pp) = t.strip_suffix("pp") {
        pp.trim()
            .parse::<u64>()
            .map(|n| n.saturating_mul(100))
            .map_err(|_| invalid())
    } else if let Some(bps) = t.strip_suffix("bps") {
        bps.trim().parse::<u64>().map_err(|_| invalid())
    } else {
        t.parse::<u64>().map_err(|_| invalid())
    }
}

/// ISO-4217 minor-unit exponent. Matched on the code (not the enum variant) so the mapping does
/// not have to track the `Currency` enum's contents.
pub fn minor_unit_exponent(currency: &Currency) -> u32 {
    match currency.to_string().as_str() {
        // Zero-decimal currencies.
        "BIF" | "CLP" | "DJF" | "GNF" | "ISK" | "JPY" | "KMF" | "KRW" | "PYG" | "RWF" | "UGX"
        | "VND" | "VUV" | "XAF" | "XOF" | "XPF" => 0,
        // Three-decimal currencies.
        "BHD" | "IQD" | "JOD" | "KWD" | "LYD" | "OMR" | "TND" => 3,
        _ => 2,
    }
}

// ── Canonicalization ──────────────────────────────────────────────────────────

/// Rewrites every amount in the document to its canonical integer form — minor currency units
/// for `metric: gmv`, a plain count for `metric: volume` — and sets `amount_units` to `minor`.
/// The document declares one metric and one currency, so a single unit applies everywhere,
/// `expected_daily_traffic` included. Runs before validation on both create and update; a
/// document is never stored un-canonicalized.
pub fn canonicalize(config: &mut VolumeContractConfig) -> Result<(), Vec<ValidationErrorDetails>> {
    let mut errors = Vec::new();

    // For transaction counts the currency exponent is meaningless: treat every amount as an
    // already-minor integer and reject fractions.
    let exponent = match config.metric {
        CommitmentMetric::Gmv => minor_unit_exponent(&config.currency.denomination),
        CommitmentMetric::Volume => 0,
    };
    let units = match config.metric {
        CommitmentMetric::Gmv => config.currency.amount_units,
        CommitmentMetric::Volume => AmountUnits::Minor,
    };

    canonicalize_amount(
        &mut config.expected_daily_traffic,
        units,
        exponent,
        "expected_daily_traffic",
        &mut errors,
    );

    for (idx, contract) in config.volume_contracts.iter_mut().enumerate() {
        let path = |field: &str| format!("volume_contracts[{idx}].{field}");

        match &mut contract.terms {
            ContractTerms::Lumpsum(flat) => {
                canonicalize_amount(
                    &mut flat.target,
                    units,
                    exponent,
                    &path("terms.target"),
                    &mut errors,
                );
                canonicalize_reward(
                    &mut flat.reward,
                    units,
                    exponent,
                    &path("terms.reward"),
                    &mut errors,
                );
            }
            ContractTerms::MinCommitment(commitment) => {
                canonicalize_amount(
                    &mut commitment.floor,
                    units,
                    exponent,
                    &path("terms.floor"),
                    &mut errors,
                );
                canonicalize_reward(
                    &mut commitment.reward,
                    units,
                    exponent,
                    &path("terms.reward"),
                    &mut errors,
                );
            }
            ContractTerms::Tiered(tiered) => {
                for (tier_idx, tier) in tiered.tiers.iter_mut().enumerate() {
                    canonicalize_amount(
                        &mut tier.threshold,
                        units,
                        exponent,
                        &path(&format!("terms.tiers[{tier_idx}].threshold")),
                        &mut errors,
                    );
                }
            }
        }
    }
    config.currency.amount_units = AmountUnits::Minor;
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn canonicalize_reward(
    reward: &mut Reward,
    units: AmountUnits,
    exponent: u32,
    field: &str,
    errors: &mut Vec<ValidationErrorDetails>,
) {
    if let Reward::Flat(flat) = reward {
        canonicalize_amount(
            &mut flat.flat_amount,
            units,
            exponent,
            &format!("{field}.flat_amount"),
            errors,
        );
    }
}

fn canonicalize_amount(
    amount: &mut Amount,
    units: AmountUnits,
    exponent: u32,
    field: &str,
    errors: &mut Vec<ValidationErrorDetails>,
) {
    match to_minor(amount, units, exponent) {
        Ok(minor) => *amount = Amount::Int(minor),
        Err(message) => errors.push(ValidationErrorDetails::new(
            field,
            "invalid_amount",
            format!("{field}: {message}"),
        )),
    }
}

fn to_minor(amount: &Amount, units: AmountUnits, exponent: u32) -> Result<u64, String> {
    let scale = match units {
        AmountUnits::Major => 10u128.pow(exponent),
        AmountUnits::Minor => 1,
    };
    let minor: u128 = match amount {
        Amount::Int(n) => (*n as u128) * scale,
        Amount::Decimal(s) => {
            let (int_part, frac_part) = parse_decimal(s)?;
            let frac_scale = match units {
                AmountUnits::Minor => {
                    if frac_part.chars().any(|c| c != '0') {
                        return Err(format!(
                            "fractional amount '{s}' is not allowed with minor units"
                        ));
                    }
                    String::new()
                }
                AmountUnits::Major => {
                    let trimmed = frac_part.trim_end_matches('0');
                    if trimmed.len() > exponent as usize {
                        return Err(format!(
                            "'{s}' has more than {exponent} decimal place(s) for this currency"
                        ));
                    }
                    // Pad to exactly `exponent` digits so it is already in minor units.
                    format!("{trimmed:0<width$}", width = exponent as usize)
                }
            };
            let int_value: u128 = int_part
                .parse()
                .map_err(|_| format!("'{s}' is not a valid amount"))?;
            let frac_value: u128 = if frac_scale.is_empty() {
                0
            } else {
                frac_scale
                    .parse()
                    .map_err(|_| format!("'{s}' is not a valid amount"))?
            };
            int_value
                .checked_mul(scale)
                .and_then(|v| v.checked_add(frac_value))
                .ok_or_else(|| format!("'{s}' overflows the supported amount range"))?
        }
    };
    u64::try_from(minor).map_err(|_| "amount overflows the supported range".to_string())
}

/// Splits a non-negative decimal literal into integer and fraction digits. Rejects signs,
/// exponents, separators and empty parts.
fn parse_decimal(s: &str) -> Result<(String, String), String> {
    let t = s.trim();
    let (int_part, frac_part) = match t.split_once('.') {
        Some((i, f)) => (i, f),
        None => (t, ""),
    };
    if int_part.is_empty()
        || !int_part.chars().all(|c| c.is_ascii_digit())
        || !frac_part.chars().all(|c| c.is_ascii_digit())
        || int_part.len() > 27
    {
        return Err(format!("'{s}' is not a valid non-negative decimal amount"));
    }
    Ok((int_part.to_string(), frac_part.to_string()))
}

// ── Validation ────────────────────────────────────────────────────────────────

/// Semantic write-time checks, run after [`canonicalize`] from `validate_routing_rule`. Parse-level
/// rules (unknown enum values, archetype↔terms mismatch, reward mutual exclusion, unknown fields)
/// are already guaranteed by the type definitions above.
pub fn validate_volume_contract_config(
    config: &VolumeContractConfig,
) -> Vec<ValidationErrorDetails> {
    let mut errors = Vec::new();

    if !(SCHEMA_VERSION_MIN..=SCHEMA_VERSION_MAX).contains(&config.schema_version) {
        errors.push(ValidationErrorDetails::new(
            "schema_version",
            "unsupported_schema_version",
            format!(
                "schema_version {} is not writable; supported: {SCHEMA_VERSION_MIN}..={SCHEMA_VERSION_MAX}",
                config.schema_version
            ),
        ));
    }

    if config.tolerance_bps.0 > MAX_TOLERANCE_BPS {
        errors.push(ValidationErrorDetails::new(
            "tolerance_bps",
            "out_of_range",
            format!(
                "tolerance {} bps exceeds the maximum of {MAX_TOLERANCE_BPS} bps ({}pp)",
                config.tolerance_bps.0,
                MAX_TOLERANCE_BPS / 100
            ),
        ));
    }

    validate_positive_amount(
        &config.expected_daily_traffic,
        "expected_daily_traffic",
        &mut errors,
    );

    for (field, interval) in [("forecast_interval_secs", config.forecast_interval_secs)] {
        if let Some(secs) = interval {
            if !(MIN_INTERVAL_SECS..=MAX_INTERVAL_SECS).contains(&secs) {
                errors.push(ValidationErrorDetails::new(
                    field,
                    "out_of_range",
                    format!(
                        "{field} must be within {MIN_INTERVAL_SECS}..={MAX_INTERVAL_SECS} seconds"
                    ),
                ));
            }
        }
    }

    if config.volume_contracts.is_empty() {
        errors.push(ValidationErrorDetails::new(
            "volume_contracts",
            "empty",
            "at least one volume contract is required",
        ));
    } else if config.volume_contracts.len() > MAX_CONTRACTS {
        errors.push(ValidationErrorDetails::new(
            "volume_contracts",
            "too_many",
            format!("at most {MAX_CONTRACTS} contracts are supported per document"),
        ));
    }

    let mut seen_ids = std::collections::HashSet::new();
    let mut seen_active_connectors = std::collections::HashSet::new();

    for (idx, contract) in config.volume_contracts.iter().enumerate() {
        let path = |field: &str| format!("volume_contracts[{idx}].{field}");

        if !is_valid_contract_id(&contract.id) {
            errors.push(ValidationErrorDetails::new(
                path("id"),
                "invalid_value",
                format!(
                    "contract id '{}' must match [A-Za-z0-9_-]{{1,64}}",
                    contract.id
                ),
            ));
        }
        if !seen_ids.insert(contract.id.clone()) {
            errors.push(ValidationErrorDetails::new(
                path("id"),
                "duplicate",
                format!("duplicate contract id '{}'", contract.id),
            ));
        }

        if !is_valid_connector(&contract.connector) {
            errors.push(ValidationErrorDetails::new(
                path("connector"),
                "invalid_value",
                format!(
                    "connector '{}' must match [a-z][a-z0-9_]{{0,63}}",
                    contract.connector
                ),
            ));
        }

        // Two active contracts on the same connector would compete for the same traffic — the
        // document has one metric, so there is one commitment per PSP.
        if contract.status == ContractStatus::Active
            && !seen_active_connectors.insert(contract.connector.clone())
        {
            errors.push(ValidationErrorDetails::new(
                path("connector"),
                "duplicate",
                format!(
                    "more than one active contract for connector '{}'",
                    contract.connector
                ),
            ));
        }

        validate_billing_cycle(&contract.billing_cycle, &path("billing_cycle"), &mut errors);
        validate_terms(&contract.terms, &path("terms"), &mut errors);

        if contract.scope.is_some() {
            errors.push(ValidationErrorDetails::new(
                path("scope"),
                "not_enabled",
                "'scope' is reserved for future payment-cluster scoping and must be absent",
            ));
        }
    }

    errors
}

fn validate_billing_cycle(
    cycle: &BillingCycle,
    path: &str,
    errors: &mut Vec<ValidationErrorDetails>,
) {
    let anchor_range = match cycle.cycle_type {
        // 1–30 per the PRD; February clamping is the evaluation layer's concern.
        BillingCycleType::CalendarMonth => 1..=30u8,
        BillingCycleType::CalendarQuarter => 1..=3u8,
        BillingCycleType::CalendarYear => 1..=12u8,
        // At least two minutes, so the cycle spans more than a single contract day to pace across.
        BillingCycleType::TestMinutes => 2..=240u8,
    };
    if !anchor_range.contains(&cycle.anchor) {
        errors.push(ValidationErrorDetails::new(
            format!("{path}.anchor"),
            "out_of_range",
            format!(
                "anchor {} is out of range {}..={} for cycle type '{}'",
                cycle.anchor,
                anchor_range.start(),
                anchor_range.end(),
                cycle.cycle_type
            ),
        ));
    }
    if cycle.timezone.parse::<chrono_tz::Tz>().is_err() {
        errors.push(ValidationErrorDetails::new(
            format!("{path}.timezone"),
            "invalid_value",
            format!(
                "'{}' is not a known IANA timezone (e.g. 'America/New_York', 'UTC')",
                cycle.timezone
            ),
        ));
    }
}

fn validate_terms(terms: &ContractTerms, path: &str, errors: &mut Vec<ValidationErrorDetails>) {
    match terms {
        ContractTerms::Lumpsum(flat) => {
            validate_positive_amount(&flat.target, &format!("{path}.target"), errors);
            validate_reward(&flat.reward, &format!("{path}.reward"), errors);
        }
        ContractTerms::MinCommitment(_) => {
            errors.push(ValidationErrorDetails::new(
                path,
                "not_enabled",
                "archetype 'min_commitment' is not enabled yet",
            ));
        }
        ContractTerms::Tiered(tiered) => {
            if tiered.tiers.is_empty() {
                errors.push(ValidationErrorDetails::new(
                    format!("{path}.tiers"),
                    "empty",
                    "a tiered contract needs at least one tier",
                ));
            } else if tiered.tiers.len() > MAX_TIERS {
                errors.push(ValidationErrorDetails::new(
                    format!("{path}.tiers"),
                    "too_many",
                    format!("at most {MAX_TIERS} tiers are supported"),
                ));
            }
            // The consumer takes one (goal, reward) per PSP: exactly one tier is the target.
            // A marginal tier cannot be the target — its rate applies only above the
            // threshold, so "reward at the goal" would be zero.
            let targeted = tiered.tiers.iter().filter(|t| t.targeted).count();
            if targeted != 1 {
                errors.push(ValidationErrorDetails::new(
                    format!("{path}.tiers"),
                    "invalid_target",
                    format!("exactly one tier must set targeted: true (found {targeted})"),
                ));
            }
            let mut prev_threshold: Option<u64> = None;
            for (idx, tier) in tiered.tiers.iter().enumerate() {
                let tier_path = format!("{path}.tiers[{idx}]");
                validate_positive_amount(
                    &tier.threshold,
                    &format!("{tier_path}.threshold"),
                    errors,
                );
                if let (Some(prev), Some(current)) = (prev_threshold, tier.threshold.as_canonical())
                {
                    if current <= prev {
                        errors.push(ValidationErrorDetails::new(
                            format!("{tier_path}.threshold"),
                            "not_increasing",
                            "tier thresholds must be strictly increasing",
                        ));
                    }
                }
                prev_threshold = tier.threshold.as_canonical().or(prev_threshold);
                if tier.targeted && matches!(tier.rate, TierRate::Marginal(_)) {
                    errors.push(ValidationErrorDetails::new(
                        format!("{tier_path}.targeted"),
                        "invalid_target",
                        "the targeted tier must be 'retroactive'",
                    ));
                }
                let rate_bps = match &tier.rate {
                    TierRate::Retroactive(rate) => rate.rebate_bps,
                    TierRate::Marginal(rate) => rate.rate_bps,
                };
                validate_rate_bps(rate_bps, &format!("{tier_path}.rate"), errors);
                if tier.rebate_lag_days > MAX_REBATE_LAG_DAYS {
                    errors.push(ValidationErrorDetails::new(
                        format!("{tier_path}.rebate_lag_days"),
                        "out_of_range",
                        format!("rebate_lag_days must be at most {MAX_REBATE_LAG_DAYS}"),
                    ));
                }
            }
        }
    }
}

fn validate_reward(reward: &Reward, path: &str, errors: &mut Vec<ValidationErrorDetails>) {
    match reward {
        Reward::Flat(flat) => {
            validate_positive_amount(&flat.flat_amount, &format!("{path}.flat_amount"), errors);
        }
        Reward::Percentage(percentage) => {
            validate_rate_bps(percentage.rebate_bps, &format!("{path}.rebate_bps"), errors);
        }
    }
}

fn validate_positive_amount(amount: &Amount, path: &str, errors: &mut Vec<ValidationErrorDetails>) {
    if amount.as_canonical() == Some(0) {
        errors.push(ValidationErrorDetails::new(
            path,
            "out_of_range",
            format!("{path} must be greater than zero"),
        ));
    }
}

fn validate_rate_bps(bps: u32, path: &str, errors: &mut Vec<ValidationErrorDetails>) {
    if !(1..=MAX_RATE_BPS).contains(&bps) {
        errors.push(ValidationErrorDetails::new(
            path,
            "out_of_range",
            format!("{path} must be within 1..={MAX_RATE_BPS} basis points"),
        ));
    }
}

fn is_valid_contract_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

fn is_valid_connector(connector: &str) -> bool {
    let mut chars = connector.chars();
    match chars.next() {
        Some(first) if first.is_ascii_lowercase() => {}
        _ => return false,
    }
    connector.len() <= 64 && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lumpsum_doc() -> serde_json::Value {
        serde_json::json!({
            "routing_mode": "pace_guarded",
            "tolerance_bps": 500,
            "currency": { "denomination": "USD" },
            "expected_daily_traffic": 80000000u64,
            "volume_contracts": [{
                "id": "adyen_2026_lumpsum",
                "connector": "adyen",
                "billing_cycle": { "type": "calendar_month", "anchor": 1, "timezone": "America/New_York" },
                "archetype": "lumpsum",
                "terms": { "target": 600000000u64, "reward": { "kind": "flat", "value": { "flat_amount": 1500000u64 } } }
            }]
        })
    }

    fn parse(value: serde_json::Value) -> Result<VolumeContractConfig, serde_json::Error> {
        serde_json::from_value(value)
    }

    fn parse_ok(value: serde_json::Value) -> VolumeContractConfig {
        parse(value).expect("document should parse")
    }

    #[test]
    fn defaults_are_applied_and_round_trip() {
        let config = parse_ok(lumpsum_doc());
        assert_eq!(config.schema_version, SCHEMA_VERSION_MAX);
        assert_eq!(config.metric, CommitmentMetric::Gmv);
        assert_eq!(config.currency.amount_units, AmountUnits::Minor);
        assert_eq!(config.forecast_interval_secs, None);
        let contract = &config.volume_contracts[0];
        assert_eq!(contract.status, ContractStatus::Active);
        assert_eq!(contract.billing_cycle.proration, Proration::FullPeriod);
        assert!(contract.scope.is_none());

        let round_tripped = parse_ok(serde_json::to_value(&config).unwrap());
        assert_eq!(config, round_tripped);
    }

    #[test]
    fn tiered_round_trip() {
        let mut doc = lumpsum_doc();
        doc["volume_contracts"][0]["archetype"] = "tiered".into();
        doc["volume_contracts"][0]["terms"] = serde_json::json!({
            "tiers": [
                { "kind": "retroactive", "rate": { "rebate_bps": 20 }, "threshold": 800000000u64, "targeted": true },
                { "kind": "marginal", "rate": { "rate_bps": 60 }, "threshold": 1000000000u64,
                  "rebate_lag_days": 30, "rebate_settlement": "credit_note" }
            ]
        });
        let config = parse_ok(doc);
        let ContractTerms::Tiered(tiered) = &config.volume_contracts[0].terms else {
            panic!("expected tiered terms");
        };
        assert_eq!(tiered.tiers.len(), 2);
        assert!(tiered.tiers[0].targeted);
        assert!(!tiered.tiers[1].targeted);
        assert_eq!(tiered.tiers[0].rebate_lag_days, 0);
        assert_eq!(tiered.tiers[0].rebate_settlement, RebateSettlement::Cash);
        assert_eq!(
            tiered.tiers[1].rate,
            TierRate::Marginal(MarginalRate { rate_bps: 60 })
        );
        let round_tripped = parse_ok(serde_json::to_value(&config).unwrap());
        assert_eq!(config, round_tripped);
    }

    #[test]
    fn archetype_terms_mismatch_is_rejected() {
        let mut doc = lumpsum_doc();
        // lumpsum archetype with a tiered block
        doc["volume_contracts"][0]["terms"] = serde_json::json!({ "tiers": [] });
        assert!(parse(doc).is_err());
    }

    #[test]
    fn reward_mutual_exclusion_is_a_parse_error() {
        let mut doc = lumpsum_doc();
        doc["volume_contracts"][0]["terms"]["reward"] = serde_json::json!({
            "kind": "flat",
            "value": { "flat_amount": 1, "rebate_bps": 25 }
        });
        assert!(parse(doc).is_err());
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let mut doc = lumpsum_doc();
        doc["volume_contracts"][0]["rebate_lagdays"] = 30.into();
        let err = parse(doc).unwrap_err().to_string();
        assert!(err.contains("rebate_lagdays"), "error was: {err}");

        let mut doc = lumpsum_doc();
        doc["volume_contracts"][0]["archetype"] = "tiered".into();
        doc["volume_contracts"][0]["terms"] = serde_json::json!({
            "tiers": [{ "kind": "retroactive", "rate": { "rebate_bps": 20 }, "threshold": 1u64, "lag": 3 }]
        });
        let err = parse(doc).unwrap_err().to_string();
        assert!(err.contains("lag"), "error was: {err}");

        let mut doc = lumpsum_doc();
        doc["surprise"] = 1.into();
        assert!(parse(doc).is_err());
    }

    #[test]
    fn min_commitment_parses_but_is_gated() {
        let mut doc = lumpsum_doc();
        doc["volume_contracts"][0]["archetype"] = "min_commitment".into();
        doc["volume_contracts"][0]["terms"] = serde_json::json!({
            "floor": 600000000u64,
            "reward": { "kind": "flat", "value": { "flat_amount": 12000000u64 } },
            "overage_rate_bps": 65
        });
        let config = parse_ok(doc);
        let errors = validate_volume_contract_config(&config);
        assert!(
            errors
                .iter()
                .any(|e| e.error_type == "not_enabled" && e.message.contains("min_commitment")),
            "errors were: {errors:?}"
        );
    }

    #[test]
    fn tolerance_accepts_pp_bps_and_alias() {
        for (value, expected) in [
            (serde_json::json!(500), 500u16),
            (serde_json::json!("5pp"), 500),
            (serde_json::json!("550bps"), 550),
            (serde_json::json!("275"), 275),
        ] {
            let mut doc = lumpsum_doc();
            doc["tolerance_bps"] = value;
            assert_eq!(parse_ok(doc).tolerance_bps.0, expected);
        }

        let mut doc = lumpsum_doc();
        doc.as_object_mut().unwrap().remove("tolerance_bps");
        doc["tolerance"] = "5pp".into();
        assert_eq!(parse_ok(doc).tolerance_bps.0, 500);

        let mut doc = lumpsum_doc();
        doc["tolerance_bps"] = "five".into();
        assert!(parse(doc).is_err());
    }

    #[test]
    fn canonicalize_converts_major_and_decimal_amounts() {
        let mut doc = lumpsum_doc();
        doc["currency"]["amount_units"] = "major".into();
        doc["expected_daily_traffic"] = 800000u64.into();
        doc["volume_contracts"][0]["terms"]["target"] = 6000000u64.into();
        doc["volume_contracts"][0]["terms"]["reward"]["value"]["flat_amount"] = "15000.50".into();
        let mut config = parse_ok(doc);
        canonicalize(&mut config).expect("canonicalization should succeed");
        assert_eq!(config.currency.amount_units, AmountUnits::Minor);
        assert_eq!(config.expected_daily_traffic, Amount::Int(80000000));
        let ContractTerms::Lumpsum(flat) = &config.volume_contracts[0].terms else {
            panic!()
        };
        assert_eq!(flat.target, Amount::Int(600000000));
        let Reward::Flat(flat_reward) = &flat.reward else {
            panic!()
        };
        assert_eq!(flat_reward.flat_amount, Amount::Int(1500050));
    }

    #[test]
    fn canonicalize_respects_currency_exponent() {
        // JPY: zero-decimal — major and minor coincide.
        let mut doc = lumpsum_doc();
        doc["currency"] = serde_json::json!({ "denomination": "JPY", "amount_units": "major" });
        doc["volume_contracts"][0]["terms"]["target"] = 1000u64.into();
        let mut config = parse_ok(doc);
        canonicalize(&mut config).unwrap();
        let ContractTerms::Lumpsum(flat) = &config.volume_contracts[0].terms else {
            panic!()
        };
        assert_eq!(flat.target, Amount::Int(1000));

        // BHD: three-decimal.
        let mut doc = lumpsum_doc();
        doc["currency"] = serde_json::json!({ "denomination": "BHD", "amount_units": "major" });
        doc["volume_contracts"][0]["terms"]["target"] = "1.234".into();
        let mut config = parse_ok(doc);
        canonicalize(&mut config).unwrap();
        let ContractTerms::Lumpsum(flat) = &config.volume_contracts[0].terms else {
            panic!()
        };
        assert_eq!(flat.target, Amount::Int(1234));
    }

    #[test]
    fn canonicalize_rejects_bad_amounts() {
        // Fraction with minor units.
        let mut doc = lumpsum_doc();
        doc["volume_contracts"][0]["terms"]["target"] = "100.5".into();
        let mut config = parse_ok(doc);
        assert!(canonicalize(&mut config).is_err());

        // Too many decimal places for the currency.
        let mut doc = lumpsum_doc();
        doc["currency"]["amount_units"] = "major".into();
        doc["volume_contracts"][0]["terms"]["target"] = "1.001".into();
        let mut config = parse_ok(doc);
        assert!(canonicalize(&mut config).is_err());

        // Garbage string.
        let mut doc = lumpsum_doc();
        doc["volume_contracts"][0]["terms"]["target"] = "6M".into();
        let mut config = parse_ok(doc);
        assert!(canonicalize(&mut config).is_err());

        // Fractional transaction count.
        let mut doc = lumpsum_doc();
        doc["metric"] = "volume".into();
        doc["volume_contracts"][0]["terms"]["target"] = "10.5".into();
        let mut config = parse_ok(doc);
        assert!(canonicalize(&mut config).is_err());
    }

    fn assert_single_error(config: &VolumeContractConfig, error_type: &str, field_part: &str) {
        let errors = validate_volume_contract_config(config);
        assert!(
            errors
                .iter()
                .any(|e| e.error_type == error_type && e.field.contains(field_part)),
            "expected {error_type} on {field_part}, got: {errors:?}"
        );
    }

    #[test]
    fn validation_catalog() {
        // A canonical, valid document passes.
        let mut config = parse_ok(lumpsum_doc());
        canonicalize(&mut config).unwrap();
        assert!(validate_volume_contract_config(&config).is_empty());

        // Unsupported schema version.
        let mut config = parse_ok(lumpsum_doc());
        config.schema_version = SCHEMA_VERSION_MAX + 1;
        assert_single_error(&config, "unsupported_schema_version", "schema_version");

        // Tolerance cap.
        let mut config = parse_ok(lumpsum_doc());
        config.tolerance_bps = ToleranceBps(2001);
        assert_single_error(&config, "out_of_range", "tolerance_bps");

        // Empty contract list.
        let mut config = parse_ok(lumpsum_doc());
        config.volume_contracts.clear();
        assert_single_error(&config, "empty", "volume_contracts");

        // Duplicate ids.
        let mut config = parse_ok(lumpsum_doc());
        let mut dup = config.volume_contracts[0].clone();
        dup.connector = "stripe".into();
        config.volume_contracts.push(dup);
        assert_single_error(&config, "duplicate", "id");

        // Two active contracts on the same connector.
        let mut config = parse_ok(lumpsum_doc());
        let mut dup = config.volume_contracts[0].clone();
        dup.id = "another".into();
        config.volume_contracts.push(dup);
        assert_single_error(&config, "duplicate", "connector");

        // Zero expected traffic.
        let mut config = parse_ok(lumpsum_doc());
        config.expected_daily_traffic = Amount::Int(0);
        assert_single_error(&config, "out_of_range", "expected_daily_traffic");

        // Interval overrides must be sane.
        let mut config = parse_ok(lumpsum_doc());
        config.forecast_interval_secs = Some(1);
        assert_single_error(&config, "out_of_range", "forecast_interval_secs");

        // Bad id / connector.
        let mut config = parse_ok(lumpsum_doc());
        config.volume_contracts[0].id = "spaced out".into();
        assert_single_error(&config, "invalid_value", "id");
        let mut config = parse_ok(lumpsum_doc());
        config.volume_contracts[0].connector = "Adyen".into();
        assert_single_error(&config, "invalid_value", "connector");

        // Anchor out of range per cycle type.
        let mut config = parse_ok(lumpsum_doc());
        config.volume_contracts[0].billing_cycle.anchor = 31;
        assert_single_error(&config, "out_of_range", "anchor");
        let mut config = parse_ok(lumpsum_doc());
        config.volume_contracts[0].billing_cycle.cycle_type = BillingCycleType::CalendarQuarter;
        config.volume_contracts[0].billing_cycle.anchor = 4;
        assert_single_error(&config, "out_of_range", "anchor");

        // Unknown timezone (well-formed but nonexistent).
        let mut config = parse_ok(lumpsum_doc());
        config.volume_contracts[0].billing_cycle.timezone = "Europe/Amsterdaam".into();
        assert_single_error(&config, "invalid_value", "timezone");

        // Zero target.
        let mut config = parse_ok(lumpsum_doc());
        if let ContractTerms::Lumpsum(flat) = &mut config.volume_contracts[0].terms {
            flat.target = Amount::Int(0);
        }
        assert_single_error(&config, "out_of_range", "target");

        // Reserved scope.
        let mut config = parse_ok(lumpsum_doc());
        config.volume_contracts[0].scope = Some(Vec::new());
        assert_single_error(&config, "not_enabled", "scope");
    }

    #[test]
    fn tier_thresholds_must_strictly_increase() {
        let mut doc = lumpsum_doc();
        doc["volume_contracts"][0]["archetype"] = "tiered".into();
        doc["volume_contracts"][0]["terms"] = serde_json::json!({
            "tiers": [
                { "kind": "retroactive", "rate": { "rebate_bps": 20 }, "threshold": 1000u64, "targeted": true },
                { "kind": "retroactive", "rate": { "rebate_bps": 25 }, "threshold": 1000u64 }
            ]
        });
        let mut config = parse_ok(doc);
        canonicalize(&mut config).unwrap();
        assert_single_error(&config, "not_increasing", "threshold");
    }

    #[test]
    fn exactly_one_retroactive_tier_must_be_targeted() {
        let tiered_doc = |tiers: serde_json::Value| {
            let mut doc = lumpsum_doc();
            doc["volume_contracts"][0]["archetype"] = "tiered".into();
            doc["volume_contracts"][0]["terms"] = serde_json::json!({ "tiers": tiers });
            let mut config = parse_ok(doc);
            canonicalize(&mut config).unwrap();
            config
        };

        // No targeted tier.
        let config = tiered_doc(serde_json::json!([
            { "kind": "retroactive", "rate": { "rebate_bps": 20 }, "threshold": 1000u64 }
        ]));
        assert_single_error(&config, "invalid_target", "tiers");

        // More than one targeted tier.
        let config = tiered_doc(serde_json::json!([
            { "kind": "retroactive", "rate": { "rebate_bps": 20 }, "threshold": 1000u64, "targeted": true },
            { "kind": "retroactive", "rate": { "rebate_bps": 25 }, "threshold": 2000u64, "targeted": true }
        ]));
        assert_single_error(&config, "invalid_target", "tiers");

        // A marginal tier cannot be the target.
        let config = tiered_doc(serde_json::json!([
            { "kind": "marginal", "rate": { "rate_bps": 60 }, "threshold": 1000u64, "targeted": true }
        ]));
        assert_single_error(&config, "invalid_target", "targeted");

        // One retroactive target passes.
        let config = tiered_doc(serde_json::json!([
            { "kind": "retroactive", "rate": { "rebate_bps": 20 }, "threshold": 1000u64, "targeted": true },
            { "kind": "marginal", "rate": { "rate_bps": 60 }, "threshold": 2000u64 }
        ]));
        assert!(
            validate_volume_contract_config(&config).is_empty(),
            "errors: {:?}",
            validate_volume_contract_config(&config)
        );
    }

    #[test]
    fn rate_bounds_are_enforced() {
        let mut doc = lumpsum_doc();
        doc["volume_contracts"][0]["terms"]["reward"] =
            serde_json::json!({ "kind": "percentage", "value": { "rebate_bps": 10001 } });
        let config = parse_ok(doc);
        assert_single_error(&config, "out_of_range", "rebate_bps");
    }
}
