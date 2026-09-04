//! Builds and stores the steering plan on demand (HTTP-triggered); never touches a live payment.

use chrono::Utc;
use futures::FutureExt;
use serde::{Deserialize, Serialize};

use super::inputs::CommitmentInputs;
use std::collections::{HashMap, HashSet};

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

/// The scheduled forecast: work the plan out, then publish it to the routing path and the audit trail.
pub async fn build_plan(
    deps: &Deps,
    inputs: &CommitmentInputs,
) -> Result<SteeringPlan, VolumeError> {
    let (plan, measured) = compute_plan(deps, inputs).await?;
    log_plan(&plan, &measured);
    deps.state.store_plan(&inputs.merchant_id, &plan).await;
    audit_plan(&plan);
    Ok(plan)
}

/// The plan `build_plan` would publish, worked out and handed back without being published: no
/// stored plan for the routing path to pick up, no forecast on the audit trail. Callers that only
/// want to *report* a position use this; the scheduled forecast uses `build_plan`.
///
/// Returns what was measured alongside the plan, because a plan alone cannot say what each PSP has
/// delivered or how fast — `PspPlan` carries the decision, `MeasuredVolume` the evidence for it.
pub async fn compute_plan(
    deps: &Deps,
    inputs: &CommitmentInputs,
) -> Result<(SteeringPlan, super::inputs::MeasuredVolume), VolumeError> {
    let now = Utc::now();
    // Unmeasurable is an error, not zeros: a plan built on an empty measurement would read every
    // PSP as owing its whole goal, steering each at its maximum rate.
    let measured = deps
        .volume
        .measure(
            &inputs.merchant_id,
            &inputs.commitments,
            PACE_WINDOW_DAYS,
            inputs.amount_scale,
        )
        .await
        .map_err(|error| {
            logger::error!(
                tag = "volume_commitment",
                merchant_id = inputs.merchant_id.as_str(),
                "forecast skipped, the previous plan stands: {error}"
            );
            error
        })?;

    // Steering can only divert traffic that exists. `expected_daily_traffic` is a contract term —
    // what the merchant told us to expect — and a wrong one silently breaks both decisions that
    // depend on it: a commitment reads as reachable when it is not, and the steer rate is a share
    // of a flow that never arrives. Prefer what was actually measured; fall back to the
    // declaration only before there is any traffic to measure.
    let daily_traffic = measured
        .total_daily
        .unwrap_or(inputs.expected_daily_traffic);

    // Two questions, two rates. `daily_traffic` above answers "how much does this merchant do",
    // which is stable and wants history. Feasibility and the steer rate ask a different one — how
    // much will arrive between now and the cycle close — and that follows the *current* rate.
    // Answering it from the wide window writes off commitments on a merchant whose volume is
    // climbing: the average over the cycle so far can be half what is flowing now, and a
    // commitment needing more than that half reads as lost while it is comfortably reachable.
    let forward_daily_traffic = measured
        .recent_daily
        .or(measured.total_daily)
        .unwrap_or(inputs.expected_daily_traffic);

    // Where each PSP stands, and the longest horizon any commitment still runs for.
    let starting_pace = math::starting_pace(daily_traffic, inputs.commitments.len());
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
    let traffic_left = math::traffic_left(forward_daily_traffic, longest_period);
    let first_day = math::day_index(cycle_start_ms, now.timestamp_millis(), day_secs) < 1;
    let run_id = math::run_id(cycle_start_ms);

    // Every commitment, so one reprieved below can be put back exactly as it was.
    let by_connector: HashMap<String, plan::PspPlan> = psps
        .iter()
        .map(|psp| (psp.connector.clone(), psp.clone()))
        .collect();

    // What the last forecast of *this* run read as unreachable. A plan from another run or another
    // contract describes a different race, so its verdicts carry no weight here.
    let previously_flagged: HashSet<String> = deps
        .state
        .load_plan(&inputs.merchant_id)
        .await
        .filter(|prev| {
            prev.run_id == run_id && prev.contract_anchor_ms == inputs.contract_anchor_ms
        })
        .map(|prev| prev.flagged_unreachable.into_iter().collect())
        .unwrap_or_default();

    let (mut kept, verdict) = if first_day {
        plan::drop_unreachable(psps, forward_daily_traffic)
    } else {
        plan::choose_commitments_to_keep(psps, traffic_left, forward_daily_traffic)
    };

    // A drop is permanent in effect — a dropped commitment receives only natural traffic, so its
    // remaining never falls while the days left do, and its required rate climbs out of reach. One
    // forecast is a thin basis for that: the estimate behind it follows a rate, and rates move.
    // So the verdict has to repeat. A commitment reading unreachable for the first time is kept
    // for one more interval and merely flagged; if the next forecast agrees, it drops.
    let flagged_unreachable: Vec<String> = verdict.iter().map(|d| d.connector.clone()).collect();
    let (dropped, reprieved): (Vec<_>, Vec<_>) = verdict
        .into_iter()
        .partition(|d| previously_flagged.contains(&d.connector));
    for psp in reprieved {
        if let Some(restored) = by_connector.get(&psp.connector) {
            kept.push(restored.clone());
        }
    }

    plan::mark_who_needs_steering(&mut kept, now.timestamp_millis());
    // Then set each behind-pace PSP's share of the eligible flow, which is what the payment path
    // samples instead of counting.
    for psp in kept.iter_mut().filter(|p| p.needs_steering) {
        psp.steer_rate = rate_for(psp, &measured, forward_daily_traffic, now);
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
        run_id,
        contract_anchor_ms: inputs.contract_anchor_ms,
        computed_at_epoch_secs: computed_at,
        stale_after_epoch_secs: computed_at
            .saturating_add(i64::try_from(fresh_for).unwrap_or(i64::MAX))
            .min(soonest_cycle_end),
        tolerance: inputs.tolerance,
        psps: kept,
        dropped,
        flagged_unreachable,
    };

    Ok((plan, measured))
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
    daily_traffic: f64,
    now: chrono::DateTime<Utc>,
) -> f64 {
    let shortfall = math::daily_shortfall(psp.needed_daily, psp.routing_gives_daily);
    let already = measured.steered_today_for(&psp.connector);

    let day_ms = math::day_ms(psp.day_secs) as f64;
    let elapsed_ms = ((now.timestamp_millis() - psp.period_start_ms).max(0) as f64) % day_ms;
    let day_remaining = ((day_ms - elapsed_ms) / day_ms).clamp(0.0, 1.0);

    math::steer_rate(
        (shortfall - already).max(0.0),
        daily_traffic * day_remaining,
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
            goal: commitment.goal,
            achieved,
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
    use super::*;
    use crate::decider::gatewaydecider::volume_commitment::inputs::{
        Commitment, InputSource, MeasuredVolume,
    };
    use crate::decider::gatewaydecider::volume_commitment::state::StateStore;
    use crate::decider::gatewaydecider::volume_commitment::volume::FixtureVolumeSource;
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// Records writes instead of performing them, so a test can assert none happened.
    #[derive(Default)]
    struct CountingStateStore {
        stores: AtomicUsize,
        /// What a previous forecast left behind. A drop needs the verdict twice, so a test that
        /// wants one has to hand the second forecast the first forecast's plan.
        previous: std::sync::Mutex<Option<SteeringPlan>>,
    }

    #[async_trait]
    impl StateStore for CountingStateStore {
        async fn load_plan(&self, _merchant_id: &str) -> Option<SteeringPlan> {
            self.previous.lock().ok().and_then(|prev| prev.clone())
        }
        async fn store_plan(&self, _merchant_id: &str, _plan: &SteeringPlan) {
            self.stores.fetch_add(1, Ordering::SeqCst);
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

    /// `compute_plan` takes inputs directly, so this only has to satisfy the `Deps` field.
    struct NoInputs;

    #[async_trait]
    impl InputSource for NoInputs {
        async fn feature_enabled(&self, _merchant_id: &str) -> bool {
            false
        }
        async fn load_configured(&self, _merchant_id: &str) -> Option<CommitmentInputs> {
            None
        }
        async fn list_active(&self) -> Vec<String> {
            Vec::new()
        }
    }

    /// A cycle that opened `elapsed_days` ago and runs for `total_days`, with one commitment.
    fn inputs_at(elapsed_days: f64, total_days: i64, goal: f64) -> CommitmentInputs {
        let day_ms = math::day_ms(math::SECS_PER_DAY);
        let now_ms = Utc::now().timestamp_millis();
        let start_ms = now_ms - (elapsed_days * day_ms as f64) as i64;
        CommitmentInputs {
            merchant_id: "m1".to_string(),
            contract_anchor_ms: start_ms,
            contract_rule_id: "routing_test".to_string(),
            tolerance: 0.05,
            // 120M a day, the rate the day-31 incident ran at.
            expected_daily_traffic: 120_000_000.0,
            forecast_interval_secs: None,
            currency: Some("USD".to_string()),
            // USD: two decimal places between the traffic's units and the contract's.
            amount_scale: 100.0,
            commitments: vec![Commitment {
                connector: "adyen".to_string(),
                goal,
                reward: 120_000.0,
                reward_note: "2bps rebate".to_string(),
                period_start_ms: start_ms,
                period_end_ms: start_ms + total_days * day_ms,
                day_secs: math::SECS_PER_DAY,
                timezone: "UTC".to_string(),
            }],
        }
    }

    fn deps_with(measured: MeasuredVolume) -> (Deps, Arc<CountingStateStore>) {
        let state = Arc::new(CountingStateStore::default());
        let deps = Deps {
            config: VolumeCommitmentConfig::default(),
            inputs: Arc::new(NoInputs),
            state: state.clone(),
            volume: Arc::new(FixtureVolumeSource::new(HashMap::from([(
                "m1".to_string(),
                measured,
            )]))),
        };
        (deps, state)
    }

    /// Two forecasts, the second seeing the first — the shape a scheduled run has, and the only
    /// one in which a commitment is actually dropped.
    async fn settled_plan(
        deps: &Deps,
        state: &std::sync::Arc<CountingStateStore>,
        inputs: &CommitmentInputs,
    ) -> SteeringPlan {
        let (first, _) = compute_plan(deps, inputs).await.expect("fixture measures");
        *state.previous.lock().expect("not poisoned") = Some(first);
        let (second, _) = compute_plan(deps, inputs).await.expect("fixture measures");
        second
    }

    fn measured_with(achieved: f64) -> MeasuredVolume {
        MeasuredVolume {
            achieved: HashMap::from([("adyen".to_string(), achieved)]),
            ..Default::default()
        }
    }

    /// The declared traffic says the goal is comfortably reachable; the traffic actually flowing
    /// says it is nowhere near. The measurement wins — declaring a flow does not create it.
    #[tokio::test]
    async fn a_goal_is_judged_against_measured_traffic_not_the_declaration() {
        let measured = MeasuredVolume {
            achieved: HashMap::from([("adyen".to_string(), 0.0)]),
            // 120_000_000 a day is declared by `inputs_at`; this is what is really flowing.
            total_daily: Some(25_000.0),
            ..Default::default()
        };
        let (deps, state) = deps_with(measured);
        // Needs 200_000 a day: under the declared 120_000_000, far over the measured 25_000.
        let inputs = inputs_at(0.5, 30, 6_000_000.0);

        let plan = settled_plan(&deps, &state, &inputs).await;

        assert!(
            plan.psps.is_empty(),
            "unreachable on the traffic that exists"
        );
        assert_eq!(plan.dropped.len(), 1);
        assert!(
            plan.dropped[0].reason.contains("25000"),
            "the reason should quote the measured flow, got: {}",
            plan.dropped[0].reason
        );
    }

    /// The regression: a merchant whose volume is climbing. The wide window averages the whole
    /// cycle so far and reports half the rate now flowing; feasibility must read the recent rate,
    /// or a commitment needing more than that half is written off while comfortably reachable.
    #[tokio::test]
    async fn feasibility_reads_the_recent_rate_not_the_cycle_average() {
        let measured = MeasuredVolume {
            achieved: HashMap::from([("adyen".to_string(), 0.0)]),
            // Averaged over the cycle so far — half of what is arriving now.
            total_daily: Some(100_000.0),
            recent_daily: Some(250_000.0),
            ..Default::default()
        };
        let (deps, state) = deps_with(measured);
        // Needs 200_000 a day: over the cycle average, under the rate actually flowing.
        let inputs = inputs_at(0.5, 30, 6_000_000.0);

        let plan = settled_plan(&deps, &state, &inputs).await;

        assert_eq!(plan.psps.len(), 1, "reachable at the rate arriving now");
        assert!(plan.dropped.is_empty());
    }

    /// Without a recent rate — too early in the cycle for the short window to be one — the wide
    /// window still answers, rather than the question going unanswered.
    #[tokio::test]
    async fn the_cycle_average_is_the_fallback_before_a_recent_rate_exists() {
        let measured = MeasuredVolume {
            achieved: HashMap::from([("adyen".to_string(), 0.0)]),
            total_daily: Some(100_000.0),
            recent_daily: None,
            ..Default::default()
        };
        let (deps, state) = deps_with(measured);
        let inputs = inputs_at(0.5, 30, 6_000_000.0);

        let plan = settled_plan(&deps, &state, &inputs).await;

        assert!(
            plan.psps.is_empty(),
            "200_000 a day against 100_000 flowing"
        );
    }

    /// A drop is permanent in effect, so one forecast is not enough to order it. The first
    /// unreachable reading keeps the commitment and only flags it.
    #[tokio::test]
    async fn one_unreachable_forecast_only_flags_the_commitment() {
        let measured = MeasuredVolume {
            achieved: HashMap::from([("adyen".to_string(), 0.0)]),
            total_daily: Some(25_000.0),
            recent_daily: Some(25_000.0),
            ..Default::default()
        };
        let (deps, _state) = deps_with(measured);
        let inputs = inputs_at(0.5, 30, 6_000_000.0);

        let (plan, _measured) = compute_plan(&deps, &inputs)
            .await
            .expect("fixture measures");

        assert_eq!(plan.psps.len(), 1, "kept for one more forecast");
        assert!(plan.dropped.is_empty(), "not dropped on a single reading");
        assert_eq!(plan.flagged_unreachable, vec!["adyen".to_string()]);
    }

    /// A flag from another run describes a different race. Carrying it over would drop a
    /// commitment on its first reading of a cycle it has barely started.
    #[tokio::test]
    async fn a_flag_from_another_run_does_not_confirm_a_drop() {
        let measured = MeasuredVolume {
            achieved: HashMap::from([("adyen".to_string(), 0.0)]),
            total_daily: Some(25_000.0),
            recent_daily: Some(25_000.0),
            ..Default::default()
        };
        let (deps, state) = deps_with(measured);
        let inputs = inputs_at(0.5, 30, 6_000_000.0);

        let (mut stale, _) = compute_plan(&deps, &inputs)
            .await
            .expect("fixture measures");
        stale.run_id = "some-other-run".to_string();
        *state.previous.lock().expect("not poisoned") = Some(stale);

        let (plan, _measured) = compute_plan(&deps, &inputs)
            .await
            .expect("fixture measures");

        assert_eq!(
            plan.psps.len(),
            1,
            "the stale flag must not confirm anything"
        );
        assert!(plan.dropped.is_empty());
    }

    /// With nothing measured yet there is only the declaration to go on, so it is still used.
    #[tokio::test]
    async fn the_declaration_is_the_fallback_before_anything_is_measured() {
        let (deps, _state) = deps_with(MeasuredVolume::default());
        let inputs = inputs_at(0.5, 30, 6_000_000.0);

        let (plan, _measured) = compute_plan(&deps, &inputs)
            .await
            .expect("fixture measures");

        assert_eq!(plan.psps.len(), 1, "kept on the declared 120M a day");
        assert!(plan.dropped.is_empty());
    }

    /// The property the split exists for: working a plan out must not publish it, or a merchant
    /// with the feature off would have one waiting for the routing path the moment it flipped.
    #[tokio::test]
    async fn compute_plan_stores_nothing() {
        let (deps, state) = deps_with(measured_with(99_311.0));
        let inputs = inputs_at(30.49, 31, 600_000_000.0);

        let (_plan, _measured) = compute_plan(&deps, &inputs)
            .await
            .expect("fixture measures");

        assert_eq!(state.stores.load(Ordering::SeqCst), 0);
    }

    /// Day 31 of 31 with almost nothing delivered: the daily need dwarfs the day's whole traffic,
    /// so the commitment is dropped rather than chased. This is the position the projection has to
    /// be able to report before a merchant enables the feature.
    #[tokio::test]
    async fn late_in_cycle_an_untouched_commitment_is_unreachable() {
        let (deps, state) = deps_with(measured_with(99_311.0));
        let inputs = inputs_at(30.49, 31, 600_000_000.0);

        let plan = settled_plan(&deps, &state, &inputs).await;

        assert!(plan.psps.is_empty(), "nothing should still be chased");
        assert_eq!(plan.dropped.len(), 1);
        assert_eq!(plan.dropped[0].connector, "adyen");
    }

    /// The same contract at the start of a fresh cycle is comfortably winnable — the difference
    /// between the two verdicts is only *when* the merchant enabled.
    #[tokio::test]
    async fn early_in_cycle_the_same_commitment_is_kept() {
        let (deps, _state) = deps_with(measured_with(0.0));
        let inputs = inputs_at(0.5, 31, 600_000_000.0);

        let (plan, _measured) = compute_plan(&deps, &inputs)
            .await
            .expect("fixture measures");

        assert!(plan.dropped.is_empty(), "30 days left is ample");
        assert_eq!(plan.psps.len(), 1);
    }
}
