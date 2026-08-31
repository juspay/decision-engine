//! Flattens the stored volume-contract document (a routing-rule row activated in the
//! `volume_commitment` slot) into `CommitmentInputs`; nothing downstream knows the DSL exists.

use async_trait::async_trait;
use chrono::{DateTime, Datelike, Months, NaiveDate, Utc};
use chrono_tz::Tz;
use diesel::associations::HasTable;
use diesel::{BoolExpressionMethods, ExpressionMethods};

use super::inputs::{Commitment, CommitmentInputs, InputSource};
use super::math::{MIN_TEST_CYCLE_MINUTES, SECS_PER_DAY, TEST_DAY_SECS};
use super::FEATURE_FLAG;
use crate::app::get_tenant_app_state;
use crate::euclid::types::StaticRoutingAlgorithm;
use crate::euclid::types::{AlgorithmType, RoutingAlgorithm, RoutingAlgorithmMapper};
use crate::euclid::volume_contract::{
    Amount, BillingCycle, BillingCycleType, CommitmentMetric, ContractStatus, ContractTerms,
    Reward, RoutingMode, TierRate, VolumeContract, VolumeContractConfig,
};
use crate::feedback::constants::kvRedis;
use crate::logger;
use crate::redis::feature::is_feature_enabled;
#[cfg(feature = "mysql")]
use crate::storage::schema::routing_algorithm::dsl as algo_dsl;
#[cfg(feature = "mysql")]
use crate::storage::schema::routing_algorithm_mapper::dsl as mapper_dsl;
#[cfg(feature = "postgres")]
use crate::storage::schema_pg::routing_algorithm::dsl as algo_dsl;
#[cfg(feature = "postgres")]
use crate::storage::schema_pg::routing_algorithm_mapper::dsl as mapper_dsl;

/// One basis point is one ten-thousandth. Rewards and tolerance both arrive in bps.
const BPS: f64 = 10_000.0;

/// Reads commitments from the merchant's active volume-contract document.
pub struct DslInputSource;

#[async_trait]
impl InputSource for DslInputSource {
    async fn load(&self, merchant_id: &str) -> Option<CommitmentInputs> {
        if !feature_on(merchant_id).await {
            return None;
        }
        let (config, anchor_ms, rule_id) = active_config(merchant_id).await?;
        to_commitment_inputs(merchant_id, &config, anchor_ms, rule_id)
    }

    /// Every merchant with a contract activated; `load` is where the feature flag is applied.
    async fn list_active(&self) -> Vec<String> {
        active_merchant_ids().await
    }
}

/// Per-merchant feature flag; without it no payment is steered, so nothing is forecast either.
async fn feature_on(merchant_id: &str) -> bool {
    is_feature_enabled(FEATURE_FLAG.to_string(), merchant_id.to_string(), kvRedis()).await
}

/// Active contract document, its `modified_at` (the anchor for `test_minutes` cycles, stamped on
/// activation and on edit — see `routing_rules::stamp_contract_activation`), and its rule id.
async fn active_config(merchant_id: &str) -> Option<(VolumeContractConfig, i64, String)> {
    let state = get_tenant_app_state().await;

    // A merchant holds at most one mapper row per slot; the volume_commitment slot is ours.
    let mapper = crate::generics::generic_find_one_optional::<
        <RoutingAlgorithmMapper as HasTable>::Table,
        _,
        RoutingAlgorithmMapper,
    >(
        &state.db,
        mapper_dsl::created_by
            .eq(merchant_id.to_string())
            .and(mapper_dsl::algorithm_for.eq(AlgorithmType::VolumeCommitment.to_string())),
    )
    .await
    .ok()
    .flatten()?;

    let algorithm = crate::generics::generic_find_one_optional::<
        <RoutingAlgorithm as HasTable>::Table,
        _,
        RoutingAlgorithm,
    >(
        &state.db,
        algo_dsl::id.eq(mapper.routing_algorithm_id.clone()),
    )
    .await
    .ok()
    .flatten()?;

    let anchor_ms = algorithm.modified_at.assume_utc().unix_timestamp() * 1000;

    match serde_json::from_str::<StaticRoutingAlgorithm>(&algorithm.algorithm_data) {
        Ok(StaticRoutingAlgorithm::VolumeContract(config)) => {
            Some((*config, anchor_ms, algorithm.id.clone()))
        }
        Ok(_) => {
            // The write path enforces that this slot only ever holds a contract document, so this
            // means the row was written by something that bypassed it.
            logger::error!(
                tag = "volume_commitment",
                merchant_id = merchant_id,
                "the volume_commitment slot holds a non-contract algorithm; ignoring it"
            );
            None
        }
        Err(error) => {
            logger::error!(
                tag = "volume_commitment",
                merchant_id = merchant_id,
                "could not parse the active volume contract: {error}"
            );
            None
        }
    }
}

/// Every merchant with a contract document activated. Read on each pass of the schedule.
async fn active_merchant_ids() -> Vec<String> {
    let state = get_tenant_app_state().await;
    let rows: Vec<RoutingAlgorithmMapper> = crate::generics::generic_find_all::<
        <RoutingAlgorithmMapper as HasTable>::Table,
        _,
        RoutingAlgorithmMapper,
    >(
        &state.db,
        mapper_dsl::algorithm_for.eq(AlgorithmType::VolumeCommitment.to_string()),
    )
    .await
    .unwrap_or_default();

    rows.into_iter().map(|row| row.created_by).collect()
}

/// Flatten a document into controller inputs; `None` when nothing is paceable (empty, inactive,
/// or an unimplemented routing mode).
fn to_commitment_inputs(
    merchant_id: &str,
    config: &VolumeContractConfig,
    anchor_ms: i64,
    rule_id: String,
) -> Option<CommitmentInputs> {
    // Mode 2 puts commitments ahead of approval rate; this engine only implements Mode 1, where
    // routing stays in charge and steering is a nudge within tolerance.
    if config.routing_mode != RoutingMode::PaceGuarded {
        logger::warn!(
            tag = "volume_commitment",
            merchant_id = merchant_id,
            "routing_mode {:?} is not implemented; this merchant is not steered",
            config.routing_mode
        );
        return None;
    }

    let expected_daily_traffic = canonical(&config.expected_daily_traffic)?;

    let commitments: Vec<Commitment> = config
        .volume_contracts
        .iter()
        .filter(|contract| contract.status == ContractStatus::Active)
        .filter_map(|contract| to_commitment(merchant_id, contract, anchor_ms))
        .collect();

    if commitments.is_empty() {
        return None;
    }

    Some(CommitmentInputs {
        merchant_id: merchant_id.to_string(),
        contract_anchor_ms: anchor_ms,
        contract_rule_id: rule_id,
        tolerance: f64::from(config.tolerance_bps.0) / BPS,
        expected_daily_traffic,
        forecast_interval_secs: config.forecast_interval_secs.map(u64::from),
        // Counts are not money: no currency, so the dashboard shows plain numbers.
        currency: matches!(config.metric, CommitmentMetric::Gmv)
            .then(|| config.currency.denomination.to_string()),
        commitments,
    })
}

/// One contract into one commitment. `None` for anything this engine cannot price.
fn to_commitment(
    merchant_id: &str,
    contract: &VolumeContract,
    anchor_ms: i64,
) -> Option<Commitment> {
    let skip = |why: &str| {
        logger::warn!(
            tag = "volume_commitment",
            merchant_id = merchant_id,
            connector = contract.connector.as_str(),
            "contract {} is not being paced: {why}",
            contract.id
        );
        None::<Commitment>
    };

    let (goal, reward, reward_note) = match &contract.terms {
        ContractTerms::Lumpsum(terms) => {
            let goal = canonical(&terms.target)?;
            (
                goal,
                reward_amount(&terms.reward, goal)?,
                reward_note(&terms.reward),
            )
        }
        ContractTerms::Tiered(terms) => {
            // Validation guarantees exactly one targeted tier, and that it is retroactive — a
            // marginal tier pays nothing at its own threshold, so it could not name a reward.
            let tier = terms.tiers.iter().find(|tier| tier.targeted)?;
            let goal = canonical(&tier.threshold)?;
            match &tier.rate {
                TierRate::Retroactive(rate) => (
                    goal,
                    goal * f64::from(rate.rebate_bps) / BPS,
                    format!("{} rebate, tier", pct_label(f64::from(rate.rebate_bps))),
                ),
                TierRate::Marginal(_) => return skip("its targeted tier is marginal"),
            }
        }
        // Parses so the wire format stays frozen, but v1 validation rejects it, so a stored
        // document should never contain one.
        ContractTerms::MinCommitment(_) => return skip("min_commitment terms are not supported"),
    };

    let window = match cycle_window(&contract.billing_cycle, Utc::now(), anchor_ms) {
        Some(window) => window,
        None => return skip("its billing cycle could not be resolved"),
    };

    Some(Commitment {
        connector: contract.connector.clone(),
        goal,
        reward,
        reward_note,
        period_start_ms: window.start_ms,
        period_end_ms: window.end_ms,
        day_secs: window.day_secs,
        timezone: contract.billing_cycle.timezone.clone(),
    })
}

/// The reward's terms in words, for the contract card: "0.25% rebate" or "lump sum".
fn reward_note(reward: &Reward) -> String {
    match reward {
        Reward::Flat(_) => "lump sum".to_string(),
        Reward::Percentage(pct) => format!("{} rebate", pct_label(f64::from(pct.rebate_bps))),
    }
}

/// Basis points as a short percentage — 25 → "0.25%", 150 → "1.5%", 200 → "2%".
fn pct_label(bps: f64) -> String {
    let pct = bps / 100.0;
    let text = format!("{pct:.2}");
    let text = text.trim_end_matches('0').trim_end_matches('.');
    format!("{text}%")
}

/// What the merchant earns for landing `goal`.
fn reward_amount(reward: &Reward, goal: f64) -> Option<f64> {
    match reward {
        Reward::Flat(flat) => canonical(&flat.flat_amount),
        Reward::Percentage(pct) => Some(goal * f64::from(pct.rebate_bps) / BPS),
    }
}

/// Amounts are canonicalized to integer minor units before storage, so a decimal here means the
/// document was written by something that skipped canonicalization.
fn canonical(amount: &Amount) -> Option<f64> {
    amount.as_canonical().map(|value| value as f64)
}

/// One resolved billing cycle: when it opened, when it closes, and how long a contract "day"
/// lasts inside it.
pub struct CycleWindow {
    pub start_ms: i64,
    pub end_ms: i64,
    pub day_secs: u64,
}

/// Current billing window in the contract's timezone; a `test_minutes` cycle repeats from
/// `anchor_ms` with one contract day per minute.
fn cycle_window(cycle: &BillingCycle, now: DateTime<Utc>, anchor_ms: i64) -> Option<CycleWindow> {
    if cycle.cycle_type == BillingCycleType::TestMinutes {
        let minutes = u32::from(cycle.anchor).max(MIN_TEST_CYCLE_MINUTES);
        let span_ms = i64::from(minutes) * i64::try_from(TEST_DAY_SECS).unwrap_or(60) * 1000;
        // Anchored to activation, not the epoch, so a fresh contract always gets a whole first cycle.
        let elapsed = now.timestamp_millis().saturating_sub(anchor_ms).max(0);
        let start_ms = anchor_ms + (elapsed / span_ms) * span_ms;
        return Some(CycleWindow {
            start_ms,
            end_ms: start_ms + span_ms,
            day_secs: TEST_DAY_SECS,
        });
    }

    let tz: Tz = cycle.timezone.parse().ok()?;
    let today = now.with_timezone(&tz).date_naive();
    let anchor = u32::from(cycle.anchor);

    let (start, span) = match cycle.cycle_type {
        BillingCycleType::CalendarMonth => {
            // `anchor` is a day of the month, 1..=30.
            let this_month = clamped(today.year(), today.month(), anchor)?;
            let start = if today >= this_month {
                this_month
            } else {
                let previous = first_of_month(today)?.checked_sub_months(Months::new(1))?;
                clamped(previous.year(), previous.month(), anchor)?
            };
            (start, Months::new(1))
        }
        BillingCycleType::CalendarQuarter => {
            // `anchor` is which month of the quarter the cycle opens on, 1..=3. Boundaries fall
            // every three months from there.
            let month_index = i64::from(today.year()) * 12 + i64::from(today.month()) - 1;
            let offset = (month_index - (i64::from(anchor) - 1)).rem_euclid(3);
            (from_month_index(month_index - offset)?, Months::new(3))
        }
        BillingCycleType::CalendarYear => {
            // `anchor` is the opening month, 1..=12.
            let year = if today.month() >= anchor {
                today.year()
            } else {
                today.year() - 1
            };
            (NaiveDate::from_ymd_opt(year, anchor, 1)?, Months::new(12))
        }
        // Returned above; `None` here rather than a panic, so a future variant degrades safely.
        BillingCycleType::TestMinutes => return None,
    };

    let end = start.checked_add_months(span)?;
    Some(CycleWindow {
        start_ms: start_of_day_ms(start, &tz),
        end_ms: start_of_day_ms(end, &tz),
        day_secs: SECS_PER_DAY,
    })
}

/// Epoch ms at local midnight on `date` in `tz`; a DST gap resolves to the earliest valid instant.
fn start_of_day_ms(date: NaiveDate, tz: &Tz) -> i64 {
    use chrono::TimeZone;
    let Some(midnight) = date.and_hms_opt(0, 0, 0) else {
        return 0;
    };
    tz.from_local_datetime(&midnight)
        .earliest()
        .map(|dt| dt.timestamp_millis())
        .unwrap_or_else(|| midnight.and_utc().timestamp_millis())
}

/// `day` in the given month, pulled back to the last day when the month is shorter — a cycle
/// anchored to the 30th still turns over in February.
fn clamped(year: i32, month: u32, day: u32) -> Option<NaiveDate> {
    let first = NaiveDate::from_ymd_opt(year, month, 1)?;
    let last_day = first.checked_add_months(Months::new(1))?.pred_opt()?.day();
    NaiveDate::from_ymd_opt(year, month, day.min(last_day))
}

fn first_of_month(date: NaiveDate) -> Option<NaiveDate> {
    NaiveDate::from_ymd_opt(date.year(), date.month(), 1)
}

/// Turn a `year * 12 + (month - 1)` index back into the first of that month.
fn from_month_index(index: i64) -> Option<NaiveDate> {
    let year = i32::try_from(index.div_euclid(12)).ok()?;
    let month = u32::try_from(index.rem_euclid(12)).ok()? + 1;
    NaiveDate::from_ymd_opt(year, month, 1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::euclid::volume_contract::Proration;

    fn cycle(cycle_type: BillingCycleType, anchor: u8, timezone: &str) -> BillingCycle {
        BillingCycle {
            cycle_type,
            anchor,
            timezone: timezone.to_string(),
            proration: Proration::FullPeriod,
        }
    }

    fn at(text: &str) -> DateTime<Utc> {
        text.parse().expect("valid RFC 3339 instant")
    }

    fn ymd(year: i32, month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(year, month, day).expect("valid date")
    }

    /// The window's boundaries read back as dates in the contract's zone, for readable assertions.
    fn window_dates(c: &BillingCycle, now: DateTime<Utc>) -> Option<(NaiveDate, NaiveDate)> {
        let tz: Tz = c.timezone.parse().ok()?;
        let w = cycle_window(c, now, 0)?;
        let as_date = |ms: i64| {
            DateTime::from_timestamp_millis(ms).map(|dt| dt.with_timezone(&tz).date_naive())
        };
        Some((as_date(w.start_ms)?, as_date(w.end_ms)?))
    }

    #[test]
    fn monthly_cycle_runs_anchor_to_anchor() {
        let c = cycle(BillingCycleType::CalendarMonth, 1, "UTC");
        assert_eq!(
            window_dates(&c, at("2026-08-20T12:00:00Z")),
            Some((ymd(2026, 8, 1), ymd(2026, 9, 1)))
        );
    }

    /// Before the anchor day, the merchant is still inside the cycle that opened last month.
    #[test]
    fn monthly_cycle_before_the_anchor_belongs_to_the_previous_month() {
        let c = cycle(BillingCycleType::CalendarMonth, 15, "UTC");
        assert_eq!(
            window_dates(&c, at("2026-08-03T12:00:00Z")),
            Some((ymd(2026, 7, 15), ymd(2026, 8, 15)))
        );
    }

    /// An anchor later than February's last day pulls back rather than failing.
    #[test]
    fn monthly_anchor_clamps_in_a_short_month() {
        let c = cycle(BillingCycleType::CalendarMonth, 30, "UTC");
        let (start, _) = window_dates(&c, at("2026-02-28T12:00:00Z")).expect("resolves");
        assert_eq!(start, ymd(2026, 2, 28));
    }

    /// The cycle turns over at midnight in the contract's zone, not in UTC. 03:00 UTC on the 1st
    /// is still 23:00 on the previous day in New York, so the previous cycle is still open.
    #[test]
    fn cycle_turns_over_in_the_contracts_timezone() {
        let c = cycle(BillingCycleType::CalendarMonth, 1, "America/New_York");
        assert_eq!(
            window_dates(&c, at("2026-08-01T03:00:00Z")),
            Some((ymd(2026, 7, 1), ymd(2026, 8, 1)))
        );
        // Same instant read in UTC has already rolled into August.
        let utc = cycle(BillingCycleType::CalendarMonth, 1, "UTC");
        assert_eq!(
            window_dates(&utc, at("2026-08-01T03:00:00Z")),
            Some((ymd(2026, 8, 1), ymd(2026, 9, 1)))
        );
    }

    /// `anchor` is the month within the quarter: 1 gives Jan/Apr/Jul/Oct.
    #[test]
    fn quarterly_cycle_opens_on_the_anchor_month_of_the_quarter() {
        let c = cycle(BillingCycleType::CalendarQuarter, 1, "UTC");
        assert_eq!(
            window_dates(&c, at("2026-08-20T12:00:00Z")),
            Some((ymd(2026, 7, 1), ymd(2026, 10, 1)))
        );
    }

    /// anchor 2 shifts the ladder to Feb/May/Aug/Nov.
    #[test]
    fn quarterly_anchor_shifts_the_whole_ladder() {
        let c = cycle(BillingCycleType::CalendarQuarter, 2, "UTC");
        assert_eq!(
            window_dates(&c, at("2026-08-20T12:00:00Z")),
            Some((ymd(2026, 8, 1), ymd(2026, 11, 1)))
        );
        // January falls in the quarter that opened the previous November.
        assert_eq!(
            window_dates(&c, at("2026-01-10T12:00:00Z")),
            Some((ymd(2025, 11, 1), ymd(2026, 2, 1)))
        );
    }

    #[test]
    fn yearly_cycle_opens_on_the_anchor_month() {
        let c = cycle(BillingCycleType::CalendarYear, 4, "UTC");
        assert_eq!(
            window_dates(&c, at("2026-08-20T12:00:00Z")),
            Some((ymd(2026, 4, 1), ymd(2027, 4, 1)))
        );
        // Before April, the open cycle is the one that started the previous April.
        assert_eq!(
            window_dates(&c, at("2026-02-20T12:00:00Z")),
            Some((ymd(2025, 4, 1), ymd(2026, 4, 1)))
        );
    }

    #[test]
    fn an_unknown_timezone_resolves_to_nothing_rather_than_guessing() {
        let c = cycle(BillingCycleType::CalendarMonth, 1, "Mars/Olympus_Mons");
        assert!(cycle_window(&c, at("2026-08-20T12:00:00Z"), 0).is_none());
    }
}

#[cfg(test)]
mod test_cycle_tests {
    use super::*;
    use crate::euclid::volume_contract::Proration;

    fn at(text: &str) -> DateTime<Utc> {
        text.parse().expect("valid instant")
    }

    /// A 30-minute test cycle: thirty one-minute contract days, anchored to activation.
    #[test]
    fn a_test_cycle_lasts_its_minutes_with_one_day_per_minute() {
        let cycle = BillingCycle {
            cycle_type: BillingCycleType::TestMinutes,
            anchor: 30,
            timezone: "UTC".to_string(),
            proration: Proration::FullPeriod,
        };
        // Contract written at 10:05; cycles run from there, not from a wall-clock grid.
        let anchor = at("2026-08-24T10:05:00Z").timestamp_millis();
        let now = at("2026-08-24T10:12:00Z");
        let w = cycle_window(&cycle, now, anchor).expect("resolves");

        assert_eq!(w.day_secs, 60);
        assert_eq!(w.end_ms - w.start_ms, 30 * 60_000);
        // Seven minutes after it was written, still inside the first cycle.
        assert_eq!(w.start_ms, anchor);
        // Twenty-three contract days remain.
        assert_eq!(
            super::super::math::days_left(w.end_ms, now.timestamp_millis(), w.day_secs),
            23.0
        );
    }
}

#[cfg(test)]
mod anchor_tests {
    use super::*;
    use crate::euclid::volume_contract::Proration;

    fn at(text: &str) -> DateTime<Utc> {
        text.parse().expect("valid instant")
    }

    fn two_minute_cycle() -> BillingCycle {
        BillingCycle {
            cycle_type: BillingCycleType::TestMinutes,
            anchor: 2,
            timezone: "UTC".to_string(),
            proration: Proration::FullPeriod,
        }
    }

    /// A contract activated seconds before a grid boundary must still get a whole first cycle.
    #[test]
    fn a_fresh_contract_gets_a_whole_first_cycle() {
        let written = at("2026-08-24T10:01:58Z");
        let w = cycle_window(&two_minute_cycle(), written, written.timestamp_millis())
            .expect("resolves");

        assert_eq!(w.start_ms, written.timestamp_millis());
        assert_eq!(
            super::super::math::days_left(w.end_ms, written.timestamp_millis(), w.day_secs),
            2.0
        );
    }

    /// It still repeats: five minutes into a two-minute contract is the third cycle.
    #[test]
    fn cycles_repeat_from_the_anchor() {
        let anchor = at("2026-08-24T10:00:00Z").timestamp_millis();
        let now = at("2026-08-24T10:05:00Z");
        let w = cycle_window(&two_minute_cycle(), now, anchor).expect("resolves");

        assert_eq!(w.start_ms, at("2026-08-24T10:04:00Z").timestamp_millis());
        assert_eq!(w.end_ms, at("2026-08-24T10:06:00Z").timestamp_millis());
    }
}
