//! Read-only pacing/series/audit/impact views plus the scheduler-called run endpoint.

use std::collections::HashMap;

use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::Json;
use futures::FutureExt;
use serde::{Deserialize, Serialize};

use crate::decider::gatewaydecider::volume_commitment;
use volume_commitment::controller::{self, RunReport};
use volume_commitment::volume::{AuditEvent, AuditKind, DayVolume};
use volume_commitment::{math, Commitment, CommitmentInputs, Deps, SteeringPlan};

/// Newest audit events read per request; runs and their counters are summarised from them.
const AUDIT_EVENT_WINDOW: u64 = 500;
/// Finest series resolution: one bucket per minute of a contract day.
const MAX_BUCKETS_PER_DAY: u32 = 1440;
/// Pseudo run id for events written before runs were named.
const UNNAMED_RUN: &str = "earlier";

/// `?merchant_id=` narrows a run to one merchant; omitted, the run sweeps everyone.
#[derive(Debug, Deserialize)]
pub struct RunQuery {
    #[serde(default)]
    pub merchant_id: Option<String>,
}

/// `POST /volume-commitment/run-forecast` — re-measure, re-decide what to chase, re-mark who is
/// behind. A merchant with no usable commitments counts as skipped, a panicking one as failed.
pub async fn run_forecast(
    Query(query): Query<RunQuery>,
) -> Result<Json<RunReport>, (StatusCode, String)> {
    let Some(deps) = volume_commitment::deps() else {
        // Startup wiring has not run. Worth a real error here: unlike the read view, a caller
        // asking for a run needs to know it did not happen.
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "volume commitment is not configured in this process".to_string(),
        ));
    };

    let Some(merchant_id) = query.merchant_id else {
        return Ok(Json(controller::run_all(deps).await));
    };

    let outcome = std::panic::AssertUnwindSafe(controller::run_for_merchant(deps, &merchant_id))
        .catch_unwind()
        .await;

    let (processed, skipped, failed, merchants) = match outcome {
        Ok(Some(run)) => (1, 0, 0, vec![run]),
        Ok(None) => (0, 1, 0, Vec::new()),
        Err(_) => {
            crate::logger::error!(
                tag = "volume_commitment",
                merchant_id = merchant_id.as_str(),
                "panic in volume commitment pass"
            );
            (0, 0, 1, Vec::new())
        }
    };

    Ok(Json(RunReport {
        merchants_processed: processed,
        merchants_skipped: skipped,
        merchants_failed: failed,
        next_run_in_secs: merchants
            .first()
            .map(|run| run.next_run_in_secs)
            .unwrap_or_else(|| controller::default_interval_secs(&deps.config)),
        merchants,
    }))
}

/// One PSP's pacing state.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PspPacing {
    pub connector: String,
    /// Target volume/GMV for the period.
    pub goal: f64,
    /// Volume sent so far this cycle.
    pub achieved: f64,
    /// Still outstanding (`goal - achieved`).
    pub gap: f64,
    /// Recent average daily volume.
    pub pace: f64,
    /// What regular SR routing delivers per day, unaided.
    pub sr_volume: f64,
    /// Volume per day needed from here on to still land the commitment.
    pub floor_per_day: f64,
    /// Volume the nudge has moved here so far today.
    pub steered_today: f64,
    /// Share of eligible payments currently being diverted here, 0..=1.
    pub steer_rate: f64,
    /// Reward captured if the commitment lands.
    pub reward: f64,
    /// `true` when SR alone is not delivering the floor.
    pub steering: bool,
}

/// A commitment the controller stopped chasing.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EliminatedPspView {
    pub connector: String,
    /// Volume sent so far this cycle — it keeps counting after the drop, since normal routing
    /// still sends the PSP whatever it would have anyway.
    pub achieved: f64,
    pub gap: f64,
    pub reward: f64,
    pub reason: String,
}

/// `GET /merchant-account/:merchant_id/volume-commitment`
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeCommitmentView {
    pub merchant_id: String,
    /// `true` whenever a contract document is live for the merchant — even between the previous
    /// cycle's plan expiring and the next forecast, when `psps` is empty and nothing is steered.
    pub active: bool,
    /// Epoch seconds of the last controller tick.
    pub computed_at_epoch_secs: Option<i64>,
    /// How far a nudge may stray from the best-approving PSP.
    pub tolerance: Option<f64>,
    /// Total volume the merchant expects per day, from the contract document.
    pub expected_daily_traffic: Option<f64>,
    /// Contract-day length in seconds (`SECS_PER_DAY`, or 60 on a test cycle).
    pub day_secs: Option<u64>,
    /// The routing rule holding the active contract, so the dashboard can act on it.
    pub rule_id: Option<String>,
    /// Reward still reachable across surviving commitments.
    pub reward_at_stake: f64,
    pub psps: Vec<PspPacing>,
    pub eliminated: Vec<EliminatedPspView>,
}

pub async fn get_volume_commitment(
    Path(merchant_id): Path<String>,
) -> Result<Json<VolumeCommitmentView>, (StatusCode, String)> {
    let Some(deps) = volume_commitment::deps() else {
        // Startup wiring has not run — report inactive rather than failing the card.
        return Ok(Json(inactive(merchant_id)));
    };

    // Load inputs once for goals and measurement; `None` = no usable contract or feature off.
    let Some(inputs) = deps.inputs.load(&merchant_id).await else {
        return Ok(Json(inactive(merchant_id)));
    };

    // No plan for *this* contract yet (first forecast pending, cycle just rolled, or the stored
    // plan belongs to a replaced document): the contract is live, nothing is paced.
    let Some(plan) = current_plan(deps, &inputs).await else {
        return Ok(Json(pending(merchant_id, &inputs)));
    };

    let goals: HashMap<&str, f64> = inputs
        .commitments
        .iter()
        .map(|c| (c.connector.as_str(), c.goal))
        .collect();
    let measured = deps
        .volume
        .measure(&merchant_id, &inputs.commitments, math::PACE_WINDOW_DAYS)
        .await;

    let psps = plan
        .psps
        .iter()
        .map(|entry| PspPacing {
            goal: goals.get(entry.connector.as_str()).copied().unwrap_or(0.0),
            achieved: measured.achieved_for(&entry.connector),
            gap: entry.remaining,
            pace: measured.pace_for(&entry.connector).unwrap_or(0.0),
            sr_volume: entry.routing_gives_daily,
            floor_per_day: entry.needed_daily,
            steer_rate: entry.steer_rate,
            steered_today: measured.steered_today_for(&entry.connector),
            reward: entry.reward,
            steering: entry.needs_steering,
            connector: entry.connector.clone(),
        })
        .collect();

    Ok(Json(VolumeCommitmentView {
        merchant_id,
        active: true,
        computed_at_epoch_secs: Some(plan.computed_at_epoch_secs),
        tolerance: Some(plan.tolerance),
        expected_daily_traffic: Some(inputs.expected_daily_traffic),
        day_secs: Some(inputs.day_secs()),
        rule_id: Some(inputs.contract_rule_id.clone()),
        reward_at_stake: plan.psps.iter().map(|p| p.reward).sum(),
        psps,
        eliminated: plan
            .dropped
            .iter()
            .map(|p| EliminatedPspView {
                connector: p.connector.clone(),
                achieved: measured.achieved_for(&p.connector),
                gap: p.remaining,
                reward: p.reward,
                reason: p.reason.clone(),
            })
            .collect(),
    }))
}

/// The stored plan, only if it was built from the contract now active — a plan left behind by a
/// replaced document must not lend its verdicts to the new one.
async fn current_plan(deps: &Deps, inputs: &CommitmentInputs) -> Option<SteeringPlan> {
    deps.state
        .load_plan(&inputs.merchant_id)
        .await
        .filter(|plan| plan.contract_anchor_ms == inputs.contract_anchor_ms)
}

/// A live contract with no plan for it yet: active, nothing paced.
fn pending(merchant_id: String, inputs: &CommitmentInputs) -> VolumeCommitmentView {
    VolumeCommitmentView {
        active: true,
        expected_daily_traffic: Some(inputs.expected_daily_traffic),
        day_secs: Some(inputs.day_secs()),
        rule_id: Some(inputs.contract_rule_id.clone()),
        ..inactive(merchant_id)
    }
}

/// Epoch ms as RFC 3339, or empty for an instant chrono cannot represent.
fn rfc3339(epoch_ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(epoch_ms)
        .map(|dt| dt.to_rfc3339())
        .unwrap_or_default()
}

fn inactive(merchant_id: String) -> VolumeCommitmentView {
    VolumeCommitmentView {
        merchant_id,
        active: false,
        computed_at_epoch_secs: None,
        tolerance: None,
        expected_daily_traffic: None,
        day_secs: None,
        rule_id: None,
        reward_at_stake: 0.0,
        psps: Vec::new(),
        eliminated: Vec::new(),
    }
}

/// One PSP's chart data: its promise and its per-day delivery.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorSeries {
    pub connector: String,
    pub goal: f64,
    pub reward: f64,
    /// How the reward is earned — "0.25% rebate", "lump sum".
    pub reward_note: String,
    /// First day of the cycle, `YYYY-MM-DD` in the contract's zone.
    pub cycle_start: String,
    /// Cycle close (the next cycle's start), for countdowns and sizing a simulated run.
    pub cycle_end: String,
    /// Length of the cycle in days — the x-axis span the promise line runs to.
    pub days_total: u32,
    /// True when the current plan has given this commitment up.
    pub eliminated: bool,
    pub points: Vec<DayVolume>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesResponse {
    pub merchant_id: String,
    /// ISO-4217 code the amounts are in, when the contract states one.
    pub currency: Option<String>,
    /// How long one contract day lasts, in seconds — the unit `points[].day` counts in.
    pub day_secs: Option<u64>,
    pub connectors: Vec<ConnectorSeries>,
}

/// `?run_id=` renders a past execution instead of the one in flight; `?per_day=` asks for that
/// many buckets per contract day (default 1) so a live chart can move within a day.
#[derive(Debug, Deserialize)]
pub struct SeriesQuery {
    #[serde(default)]
    pub run_id: Option<String>,
    #[serde(default)]
    pub per_day: Option<u32>,
}

/// Every commitment re-aimed at `[start_ms, end_ms)` — a past run, or the baseline before one.
fn rewindow(commitments: &[Commitment], start_ms: i64, end_ms: i64) -> Vec<Commitment> {
    commitments
        .iter()
        .map(|c| Commitment {
            period_start_ms: start_ms,
            period_end_ms: end_ms,
            ..c.clone()
        })
        .collect()
}

/// The commitments as they stood in the run `run_id` names: the current cycle's length, starting
/// where that run opened. The live cycle for anything that is not a run id.
fn commitments_for_run(commitments: &[Commitment], run_id: Option<&str>) -> Vec<Commitment> {
    match run_id.and_then(math::run_start_ms) {
        Some(start_ms) => commitments
            .iter()
            .map(|c| Commitment {
                period_start_ms: start_ms,
                period_end_ms: start_ms.saturating_add(c.period_end_ms - c.period_start_ms),
                ..c.clone()
            })
            .collect(),
        None => commitments.to_vec(),
    }
}

/// `GET /merchant-account/:merchant_id/volume-commitment/series` — per-bucket delivered volume per
/// PSP and the promise each races; `?run_id=` renders a past cycle.
pub async fn get_series(
    Path(merchant_id): Path<String>,
    Query(query): Query<SeriesQuery>,
) -> Result<Json<SeriesResponse>, (StatusCode, String)> {
    let empty = || SeriesResponse {
        merchant_id: merchant_id.clone(),
        currency: None,
        day_secs: None,
        connectors: Vec::new(),
    };
    let Some(deps) = volume_commitment::deps() else {
        return Ok(Json(empty()));
    };
    let Some(inputs) = deps.inputs.load(&merchant_id).await else {
        return Ok(Json(empty()));
    };

    let eliminated: Vec<String> = current_plan(deps, &inputs)
        .await
        .map(|plan| plan.dropped.iter().map(|d| d.connector.clone()).collect())
        .unwrap_or_default();

    let commitments = commitments_for_run(&inputs.commitments, query.run_id.as_deref());
    let per_day = query.per_day.unwrap_or(1).clamp(1, MAX_BUCKETS_PER_DAY);
    let points = deps
        .volume
        .daily_series(&merchant_id, &commitments, per_day)
        .await;

    let connectors = commitments
        .iter()
        .map(|commitment| ConnectorSeries {
            connector: commitment.connector.clone(),
            goal: commitment.goal,
            reward: commitment.reward,
            reward_note: commitment.reward_note.clone(),
            cycle_start: rfc3339(commitment.period_start_ms),
            cycle_end: rfc3339(commitment.period_end_ms),
            days_total: math::days_total(
                commitment.period_start_ms,
                commitment.period_end_ms,
                commitment.day_secs,
            ),
            eliminated: eliminated.contains(&commitment.connector),
            points: points
                .iter()
                .filter(|p| p.connector == commitment.connector)
                .cloned()
                .collect(),
        })
        .collect();

    Ok(Json(SeriesResponse {
        merchant_id,
        currency: inputs.currency.clone(),
        day_secs: commitments.first().map(|c| c.day_secs),
        connectors,
    }))
}

/// One execution of the contract, summarised for a picker.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RunSummary {
    pub run_id: String,
    /// When the cycle this run covers opened, from the id itself.
    pub started_at_epoch_ms: i64,
    /// When the run was last heard from — its most recent forecast or steer.
    pub last_activity_epoch_ms: i64,
    pub forecasts: usize,
    pub steers: usize,
    pub eliminations: usize,
    /// True for the run whose cycle is still open.
    pub is_current: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditResponse {
    pub merchant_id: String,
    /// Every execution of this merchant's contract, newest first.
    pub runs: Vec<RunSummary>,
    /// Newest first. Narrowed to `?run_id=` when one is given.
    pub events: Vec<AuditEvent>,
}

/// `?run_id=` narrows a view to one execution of the contract instead of the one in flight.
#[derive(Debug, Deserialize)]
pub struct RunScopedQuery {
    #[serde(default)]
    pub run_id: Option<String>,
}

/// `GET /merchant-account/:merchant_id/volume-commitment/audit` — forecasts, steers and
/// eliminations reconstructed from analytics events, grouped by run.
pub async fn get_audit(
    Path(merchant_id): Path<String>,
    Query(query): Query<RunScopedQuery>,
) -> Result<Json<AuditResponse>, (StatusCode, String)> {
    let Some(deps) = volume_commitment::deps() else {
        return Ok(Json(AuditResponse {
            merchant_id,
            runs: Vec::new(),
            events: Vec::new(),
        }));
    };

    // One read, two views: the run list and the filtered events come from the same window.
    let all = deps
        .volume
        .audit_events(&merchant_id, AUDIT_EVENT_WINDOW)
        .await;
    let current_run = deps
        .state
        .load_plan(&merchant_id)
        .await
        .map(|plan| plan.run_id);

    let mut order: Vec<String> = Vec::new();
    let mut summaries: HashMap<String, RunSummary> = HashMap::new();
    for event in &all {
        // Events written before runs were named still deserve a home rather than vanishing.
        let run_id = event
            .run_id
            .clone()
            .unwrap_or_else(|| UNNAMED_RUN.to_string());
        let entry = summaries.entry(run_id.clone()).or_insert_with(|| {
            order.push(run_id.clone());
            RunSummary {
                started_at_epoch_ms: math::run_start_ms(&run_id).unwrap_or(event.at_epoch_ms),
                last_activity_epoch_ms: event.at_epoch_ms,
                is_current: current_run.as_deref() == Some(run_id.as_str()),
                run_id,
                forecasts: 0,
                steers: 0,
                eliminations: 0,
            }
        });
        entry.last_activity_epoch_ms = entry.last_activity_epoch_ms.max(event.at_epoch_ms);
        match event.kind {
            AuditKind::Forecast => entry.forecasts += 1,
            AuditKind::Steered => entry.steers += 1,
            AuditKind::Eliminated => entry.eliminations += 1,
        }
    }

    let mut runs: Vec<RunSummary> = order
        .into_iter()
        .filter_map(|id| summaries.remove(&id))
        .collect();
    runs.sort_by_key(|run| std::cmp::Reverse(run.started_at_epoch_ms));

    let events = match &query.run_id {
        Some(wanted) => all
            .into_iter()
            .filter(|e| e.run_id.as_deref().unwrap_or(UNNAMED_RUN) == wanted)
            .collect(),
        None => all,
    };

    Ok(Json(AuditResponse {
        merchant_id,
        runs,
        events,
    }))
}

/// What one PSP received in a window — payments and volume.
#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ImpactSlice {
    pub payments: u64,
    pub volume: f64,
}

/// One PSP's before-and-after: what it got without the contract, and what it got with it —
/// split into what routing sent on its own, what was steered in, and what it gave up.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectorImpact {
    pub connector: String,
    pub goal: f64,
    pub reward: f64,
    /// True when the plan for this cycle has given the commitment up.
    pub eliminated: bool,
    /// True when the live plan is currently diverting extra payments here.
    pub steering: bool,
    /// Everything this PSP received in the previous cycle.
    pub before: ImpactSlice,
    /// Everything this PSP received in the cycle (`unaided + steered`).
    pub with_contract: ImpactSlice,
    /// The part normal routing sent here by itself.
    pub unaided: ImpactSlice,
    /// The part the nudge moved here to meet the commitment.
    pub steered: ImpactSlice,
    /// What routing would have sent here but the nudge moved to a PSP behind on its commitment.
    pub ceded: ImpactSlice,
}

/// A window of time the impact view compares.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImpactWindow {
    pub start_ms: i64,
    pub end_ms: i64,
}

/// `GET /merchant-account/:merchant_id/volume-commitment/impact`
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImpactResponse {
    pub merchant_id: String,
    /// When the active contract document went live.
    pub contract_since_ms: i64,
    /// The cycle being reported: the one in flight, or the run `?run_id=` asked for.
    pub cycle: ImpactWindow,
    /// Length of that cycle in contract days.
    pub days_total: u32,
    /// How long one contract day lasts, in seconds.
    pub day_secs: u64,
    /// The cycle immediately before `cycle` — what `before` and `baseline_days` are measured over.
    pub baseline: ImpactWindow,
    pub connectors: Vec<ConnectorImpact>,
    /// Day-by-day delivery per PSP across the previous cycle, `day_index` counted from its start.
    pub baseline_days: Vec<DayVolume>,
    /// The same across the cycle, `day_index` counted from the cycle's start.
    pub cycle_days: Vec<DayVolume>,
}

/// `GET /merchant-account/:merchant_id/volume-commitment/impact` — each PSP's traffic in the
/// previous cycle (same length, ending where this one starts) vs this one, split by who sent it.
pub async fn get_impact(
    Path(merchant_id): Path<String>,
    Query(query): Query<RunScopedQuery>,
) -> Result<Json<ImpactResponse>, (StatusCode, String)> {
    let not_active = || {
        (
            StatusCode::NOT_FOUND,
            format!("no active volume contract for merchant {merchant_id}"),
        )
    };
    let Some(deps) = volume_commitment::deps() else {
        return Err(not_active());
    };
    let Some(inputs) = deps.inputs.load(&merchant_id).await else {
        return Err(not_active());
    };

    let (eliminated, steering): (Vec<String>, Vec<String>) = current_plan(deps, &inputs)
        .await
        .map(|plan| {
            (
                plan.dropped.iter().map(|d| d.connector.clone()).collect(),
                plan.needing_steering()
                    .map(|p| p.connector.clone())
                    .collect(),
            )
        })
        .unwrap_or_default();

    let commitments = commitments_for_run(&inputs.commitments, query.run_id.as_deref());
    let Some(first) = commitments.first() else {
        return Err(not_active());
    };
    // PSP contracts share a cycle in practice; the response reports the widest span so every
    // connector's traffic is inside it.
    let cycle_start_ms = commitments
        .iter()
        .map(|c| c.period_start_ms)
        .min()
        .unwrap_or(first.period_start_ms);
    let cycle_end_ms = commitments
        .iter()
        .map(|c| c.period_end_ms)
        .max()
        .unwrap_or(first.period_end_ms);
    let cycle_len_ms = (cycle_end_ms - cycle_start_ms).max(1);
    let day_secs = first.day_secs;
    let days_total = math::days_total(cycle_start_ms, cycle_end_ms, day_secs);

    let baseline_end_ms = cycle_start_ms;
    let baseline_start_ms = baseline_end_ms.saturating_sub(cycle_len_ms);

    let connectors: Vec<String> = commitments.iter().map(|c| c.connector.clone()).collect();
    let before = deps
        .volume
        .window_totals(
            &merchant_id,
            &connectors,
            baseline_start_ms,
            baseline_end_ms,
        )
        .await;
    let during = deps
        .volume
        .window_totals(&merchant_id, &connectors, cycle_start_ms, cycle_end_ms)
        .await;

    // Day-by-day for both windows: hand the series reader the baseline as if it were a cycle.
    let baseline_days = deps
        .volume
        .daily_series(
            &merchant_id,
            &rewindow(&commitments, baseline_start_ms, baseline_end_ms),
            1,
        )
        .await;
    let cycle_days = deps
        .volume
        .daily_series(
            &merchant_id,
            &rewindow(&commitments, cycle_start_ms, cycle_end_ms),
            1,
        )
        .await;

    let slice = |payments: u64, volume: f64| ImpactSlice { payments, volume };
    let connectors = commitments
        .iter()
        .map(|commitment| {
            let b = before.iter().find(|t| t.connector == commitment.connector);
            let d = during.iter().find(|t| t.connector == commitment.connector);
            ConnectorImpact {
                connector: commitment.connector.clone(),
                goal: commitment.goal,
                reward: commitment.reward,
                eliminated: eliminated.contains(&commitment.connector),
                steering: steering.contains(&commitment.connector),
                before: b.map(|t| slice(t.payments, t.volume)).unwrap_or_default(),
                with_contract: d.map(|t| slice(t.payments, t.volume)).unwrap_or_default(),
                unaided: d
                    .map(|t| {
                        slice(
                            t.payments.saturating_sub(t.steered_payments),
                            (t.volume - t.steered_volume).max(0.0),
                        )
                    })
                    .unwrap_or_default(),
                steered: d
                    .map(|t| slice(t.steered_payments, t.steered_volume))
                    .unwrap_or_default(),
                ceded: d
                    .map(|t| slice(t.ceded_payments, t.ceded_volume))
                    .unwrap_or_default(),
            }
        })
        .collect();

    Ok(Json(ImpactResponse {
        merchant_id,
        contract_since_ms: inputs.contract_anchor_ms,
        cycle: ImpactWindow {
            start_ms: cycle_start_ms,
            end_ms: cycle_end_ms,
        },
        days_total,
        day_secs,
        baseline: ImpactWindow {
            start_ms: baseline_start_ms,
            end_ms: baseline_end_ms,
        },
        connectors,
        baseline_days,
        cycle_days,
    }))
}
