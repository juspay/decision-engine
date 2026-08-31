//! Builds and stores the steering plan on demand (HTTP-triggered); never touches a live payment.

use chrono::Utc;
use futures::FutureExt;
use serde::{Deserialize, Serialize};

use super::inputs::CommitmentInputs;
use super::math::{self, PACE_WINDOW_DAYS};
use super::plan::{self, PspPlan, SteeringPlan};
use super::volume::VolumeError;
use super::Deps;
use crate::config::VolumeCommitmentConfig;
use crate::logger;

/// This merchant's forecast cadence: its own override, else the config default.
pub fn interval_secs(inputs: &CommitmentInputs, config: &VolumeCommitmentConfig) -> u64 {
    inputs
        .forecast_interval_secs
        .unwrap_or(config.default_forecast_interval_secs)
        .max(1)
}

/// The cadence to fall back on when no merchant ran.
pub fn default_interval_secs(config: &VolumeCommitmentConfig) -> u64 {
    config.default_forecast_interval_secs.max(1)
}

/// A plan may steer for this many forecast intervals before going stale: one missed run is
/// tolerated, three means the scheduler is gone and steering must stop.
const PLAN_FRESH_FOR_INTERVALS: u64 = 3;

/// What one merchant's run produced.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MerchantRun {
    pub merchant_id: String,
    /// Commitments still being chased.
    pub psps_tracked: usize,
    /// Of those, the ones normal routing is not feeding enough.
    pub psps_steering: usize,
    /// Commitments given up on.
    pub psps_dropped: usize,
    /// When this merchant wants forecasting run again.
    pub next_run_in_secs: u64,
}

/// What a whole run produced — handed back so the scheduler learns when to come back without
/// holding cadence configuration of its own.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RunReport {
    pub merchants_processed: usize,
    /// Merchants with no commitments we could use. Not an error.
    pub merchants_skipped: usize,
    /// Merchants whose pass panicked. The rest of the run still completed.
    pub merchants_failed: usize,
    /// The soonest any merchant wants a run again, so one sweep can serve them all.
    pub next_run_in_secs: u64,
    pub merchants: Vec<MerchantRun>,
}

/// Forecast every active merchant; a panicking merchant is counted and the sweep continues.
pub async fn run_all(deps: &Deps) -> RunReport {
    let mut report = RunReport {
        merchants_processed: 0,
        merchants_skipped: 0,
        merchants_failed: 0,
        next_run_in_secs: default_interval_secs(&deps.config),
        merchants: Vec::new(),
    };

    for merchant_id in deps.inputs.list_active().await {
        match std::panic::AssertUnwindSafe(run_for_merchant(deps, &merchant_id))
            .catch_unwind()
            .await
        {
            Ok(Ok(Some(run))) => {
                report.merchants_processed += 1;
                report.merchants.push(run);
            }
            Ok(Ok(None)) => report.merchants_skipped += 1,
            // Already logged where it happened; the previous plan stands.
            Ok(Err(_)) => report.merchants_failed += 1,
            Err(_) => {
                report.merchants_failed += 1;
                logger::error!(
                    tag = "volume_commitment",
                    merchant_id = merchant_id.as_str(),
                    "panic in volume commitment pass; the rest of the run continues"
                );
            }
        }
    }

    // Serve the most impatient merchant; the rest are simply run early.
    if let Some(soonest) = report.merchants.iter().map(|m| m.next_run_in_secs).min() {
        report.next_run_in_secs = soonest;
    }
    report
}

/// Run a forecast for one merchant. `Ok(None)` means it has no commitments we can use; `Err`
/// means delivery could not be measured, so no plan was written.
pub async fn run_for_merchant(
    deps: &Deps,
    merchant_id: &str,
) -> Result<Option<MerchantRun>, VolumeError> {
    let Some(inputs) = deps.inputs.load(merchant_id).await else {
        return Ok(None);
    };
    let plan = build_plan(deps, &inputs).await?;

    Ok(Some(MerchantRun {
        psps_tracked: plan.psps.len(),
        psps_steering: plan.needing_steering().count(),
        psps_dropped: plan.dropped.len(),
        next_run_in_secs: interval_secs(&inputs, &deps.config),
        merchant_id: inputs.merchant_id,
    }))
}

/// Measure, position, choose what to chase, mark who is behind — from scratch every run — then store.
///
/// An unmeasurable merchant gets no plan at all: the previous one keeps steering until it goes
/// stale. Building on "nothing delivered" would steer every PSP at its maximum rate and, late in
/// a cycle, eliminate commitments that are in fact on pace.
pub async fn build_plan(
    deps: &Deps,
    inputs: &CommitmentInputs,
) -> Result<SteeringPlan, VolumeError> {
    let now = Utc::now();
    let measured = deps
        .volume
        .measure(&inputs.merchant_id, &inputs.commitments, PACE_WINDOW_DAYS)
        .await
        .map_err(|error| {
            logger::error!(
                tag = "volume_commitment",
                merchant_id = inputs.merchant_id.as_str(),
                "forecast skipped, the previous plan stands: {error}"
            );
            error
        })?;

    // Where each PSP stands, and the longest horizon any commitment still runs for.
    let starting_pace =
        math::starting_pace(inputs.expected_daily_traffic, inputs.commitments.len());
    let mut psps = Vec::with_capacity(inputs.commitments.len());
    let mut longest_period = 0.0_f64;
    for commitment in &inputs.commitments {
        let (psp, days_left) = position(commitment, &measured, starting_pace, now, inputs);
        longest_period = longest_period.max(days_left);
        psps.push(psp);
    }

    // One run per cycle. Commitments in a document normally share a cycle; where they differ, the
    // earliest opening is the run this plan belongs to.
    let cycle_start_ms = inputs
        .commitments
        .iter()
        .map(|c| c.period_start_ms)
        .min()
        .unwrap_or(0);
    let day_secs = inputs.day_secs();

    // First contract day: drop only the unreachable; afterwards the reward-ranked budget pass
    // (see `plan::drop_unreachable`).
    let traffic_left = math::traffic_left(inputs.expected_daily_traffic, longest_period);
    let first_day = math::day_index(cycle_start_ms, now.timestamp_millis(), day_secs) < 1;
    let (mut kept, dropped) = if first_day {
        plan::drop_unreachable(psps, inputs.expected_daily_traffic)
    } else {
        plan::choose_commitments_to_keep(psps, traffic_left, inputs.expected_daily_traffic)
    };

    plan::mark_who_needs_steering(&mut kept);
    // Then set each behind-pace PSP's share of the eligible flow, which is what the payment path
    // samples instead of counting.
    for psp in kept.iter_mut().filter(|p| p.needs_steering) {
        psp.steer_rate = rate_for(psp, &measured, inputs, now);
    }

    let computed_at = now.timestamp();
    let fresh_for = PLAN_FRESH_FOR_INTERVALS.saturating_mul(interval_secs(inputs, &deps.config));
    // Cap staleness at the soonest cycle end (over all commitments, kept or dropped) so no plan
    // outlives its period.
    let soonest_cycle_end = inputs
        .commitments
        .iter()
        .map(|c| c.period_end_ms / 1000)
        .min()
        .unwrap_or(i64::MAX);

    let plan = SteeringPlan {
        merchant_id: inputs.merchant_id.clone(),
        run_id: math::run_id(cycle_start_ms),
        contract_anchor_ms: inputs.contract_anchor_ms,
        computed_at_epoch_secs: computed_at,
        stale_after_epoch_secs: computed_at
            .saturating_add(i64::try_from(fresh_for).unwrap_or(i64::MAX))
            .min(soonest_cycle_end),
        tolerance: inputs.tolerance,
        psps: kept,
        dropped,
    };

    log_plan(&plan, &measured);
    deps.state.store_plan(&inputs.merchant_id, &plan).await;
    audit_plan(&plan);

    Ok(plan)
}

/// Record the run as a domain analytics event so the audit trail survives this process.
fn audit_plan(plan: &SteeringPlan) {
    let steering: Vec<&str> = plan
        .needing_steering()
        .map(|p| p.connector.as_str())
        .collect();
    let details = serde_json::json!({
        "runId": plan.run_id,
        "tracked": plan.psps.len(),
        "steering": steering,
        "dropped": plan.dropped.iter().map(|d| serde_json::json!({
            "connector": d.connector,
            "reason": d.reason,
            "remaining": d.remaining,
            "reward": d.reward,
        })).collect::<Vec<_>>(),
    });

    crate::analytics::DomainAnalyticsEvent::record_operation(
        crate::analytics::AnalyticsFlowContext::new(
            crate::analytics::ApiFlow::DynamicRouting,
            crate::analytics::FlowType::VolumeCommitmentForecast,
        ),
        // Reusing an existing route, as the SR auto-calibrator does for its retune events.
        crate::analytics::AnalyticsRoute::UpdateGatewayScore,
        Some(plan.merchant_id.clone()),
        None,
        None,
        None,
        None,
        Some("success".to_string()),
        Some(details.to_string()),
        None,
    );
}

/// Steer rate = (today's shortfall − already steered today) / traffic expected for the rest of
/// the contract day.
fn rate_for(
    psp: &PspPlan,
    measured: &super::inputs::MeasuredVolume,
    inputs: &CommitmentInputs,
    now: chrono::DateTime<Utc>,
) -> f64 {
    let shortfall = math::daily_shortfall(psp.needed_daily, psp.routing_gives_daily);
    let already = measured.steered_today_for(&psp.connector);

    let day_ms = math::day_ms(psp.day_secs) as f64;
    let elapsed_ms = ((now.timestamp_millis() - psp.period_start_ms).max(0) as f64) % day_ms;
    let day_remaining = ((day_ms - elapsed_ms) / day_ms).clamp(0.0, 1.0);

    math::steer_rate(
        (shortfall - already).max(0.0),
        inputs.expected_daily_traffic * day_remaining,
    )
}

/// One commitment's position: what is owed, what each remaining day must bring, and what routing
/// delivers unaided. A "day" is the contract's own — a calendar day, or a minute on a test cycle.
fn position(
    commitment: &super::inputs::Commitment,
    measured: &super::inputs::MeasuredVolume,
    starting_pace: f64,
    now: chrono::DateTime<Utc>,
    inputs: &CommitmentInputs,
) -> (PspPlan, f64) {
    let days_left = math::days_left(
        commitment.period_end_ms,
        now.timestamp_millis(),
        commitment.day_secs,
    );

    let achieved = measured.achieved_for(&commitment.connector);
    let pace = measured
        .pace_for(&commitment.connector)
        .unwrap_or(starting_pace);
    let routing_gives_daily = measured.routing_gives_daily_for(&commitment.connector);
    let remaining = math::remaining(commitment.goal, achieved);

    logger::debug!(
        tag = "volume_commitment",
        merchant_id = inputs.merchant_id.as_str(),
        connector = commitment.connector.as_str(),
        "goal={:.0} achieved={:.0} pace={:.0} forecast={:.0} remaining={:.0} days_left={:.2}",
        commitment.goal,
        achieved,
        pace,
        math::forecast(achieved, pace, days_left),
        remaining,
        days_left,
    );

    (
        PspPlan {
            connector: commitment.connector.clone(),
            reward: commitment.reward,
            remaining,
            needed_daily: math::needed_daily(remaining, days_left),
            routing_gives_daily,
            needs_steering: false, // set by mark_who_needs_steering
            steer_rate: 0.0,       // set once we know who is behind
            period_start_ms: commitment.period_start_ms,
            period_end_ms: commitment.period_end_ms,
            day_secs: commitment.day_secs,
        },
        days_left,
    )
}

/// One log line per PSP, so the merchant can be shown why volume went where it did.
fn log_plan(plan: &SteeringPlan, measured: &super::inputs::MeasuredVolume) {
    for psp in &plan.psps {
        logger::info!(
            tag = "volume_commitment",
            merchant_id = plan.merchant_id.as_str(),
            connector = psp.connector.as_str(),
            "achieved={:.0} remaining={:.0} needed_daily={:.0} routing_gives_daily={:.0} \
             needs_steering={} reward={:.0}",
            measured.achieved_for(&psp.connector),
            psp.remaining,
            psp.needed_daily,
            psp.routing_gives_daily,
            psp.needs_steering,
            psp.reward,
        );
    }

    for psp in &plan.dropped {
        logger::info!(
            tag = "volume_commitment_dropped",
            merchant_id = plan.merchant_id.as_str(),
            connector = psp.connector.as_str(),
            "remaining={:.0} reward={:.0} reason={}",
            psp.remaining,
            psp.reward,
            psp.reason,
        );
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    use async_trait::async_trait;

    use super::*;
    use crate::decider::gatewaydecider::volume_commitment::inputs::Commitment;
    use crate::decider::gatewaydecider::volume_commitment::state::StateStore;
    use crate::decider::gatewaydecider::volume_commitment::volume::FixtureVolumeSource;
    use crate::decider::gatewaydecider::volume_commitment::DslInputSource;

    /// Records whether a plan was ever written; every other operation is inert.
    #[derive(Default)]
    struct RecordingStore {
        stored: AtomicBool,
    }

    #[async_trait]
    impl StateStore for RecordingStore {
        async fn load_plan(&self, _merchant_id: &str) -> Option<SteeringPlan> {
            None
        }
        async fn store_plan(&self, _merchant_id: &str, _plan: &SteeringPlan) {
            self.stored.store(true, Ordering::SeqCst);
        }
        async fn clear_plan(&self, _merchant_id: &str) {}
        async fn try_acquire_run_lease(&self, _merchant_id: &str, _ttl_secs: u64) -> bool {
            true
        }
        async fn release_run_lease(&self, _merchant_id: &str) {}
        async fn last_run_started_at(&self, _merchant_id: &str) -> Option<i64> {
            None
        }
    }

    fn inputs() -> CommitmentInputs {
        let now_ms = Utc::now().timestamp_millis();
        CommitmentInputs {
            merchant_id: "m1".to_string(),
            contract_anchor_ms: now_ms,
            contract_rule_id: "rule".to_string(),
            tolerance: 0.05,
            expected_daily_traffic: 1_000.0,
            forecast_interval_secs: None,
            currency: None,
            commitments: vec![Commitment {
                connector: "adyen".to_string(),
                goal: 10_000.0,
                reward: 100.0,
                reward_note: "lump sum".to_string(),
                period_start_ms: now_ms,
                period_end_ms: now_ms + 30 * math::SECS_PER_DAY as i64 * 1000,
                day_secs: math::SECS_PER_DAY,
                timezone: "UTC".to_string(),
            }],
        }
    }

    /// A ClickHouse outage (or no ClickHouse at all) must leave the previous plan in place, not
    /// replace it with one that reads every PSP as owing its whole goal.
    #[tokio::test]
    async fn an_unmeasurable_merchant_gets_no_plan() {
        let store = Arc::new(RecordingStore::default());
        let deps = Deps {
            config: VolumeCommitmentConfig::default(),
            inputs: Arc::new(DslInputSource),
            state: store.clone(),
            volume: Arc::new(FixtureVolumeSource::new(HashMap::new())),
        };

        let outcome = build_plan(&deps, &inputs()).await;

        assert!(matches!(outcome, Err(VolumeError::Unavailable)));
        assert!(
            !store.stored.load(Ordering::SeqCst),
            "no plan may be written"
        );
    }

    /// The same inputs with a measurement behind them do produce and store a plan.
    #[tokio::test]
    async fn a_measured_merchant_gets_a_plan() {
        let store = Arc::new(RecordingStore::default());
        let deps = Deps {
            config: VolumeCommitmentConfig::default(),
            inputs: Arc::new(DslInputSource),
            state: store.clone(),
            volume: Arc::new(FixtureVolumeSource::new(HashMap::from([(
                "m1".to_string(),
                super::super::inputs::MeasuredVolume::default(),
            )]))),
        };

        let plan = build_plan(&deps, &inputs()).await.expect("measured");

        assert_eq!(plan.psps.len() + plan.dropped.len(), 1);
        assert!(store.stored.load(Ordering::SeqCst));
    }
}
