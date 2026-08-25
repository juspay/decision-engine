//! The main server's volume-commitment surface: the read-only pacing view, and the run endpoint
//! the scheduler calls. Runs live here — not on the scheduler's port — so they share the exact
//! plan and counters the routing path reads.

use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use futures::FutureExt;

use crate::decider::gatewaydecider::volume_commitment;
use volume_commitment::controller::{self, RunReport};

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
    pub gap: f64,
    pub reward: f64,
    pub reason: String,
}

/// `GET /merchant-account/:merchant_id/volume-commitment`
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeCommitmentView {
    pub merchant_id: String,
    /// `false` before the controller's first tick — nothing is steered.
    pub active: bool,
    /// Epoch seconds of the last controller tick.
    pub computed_at_epoch_secs: Option<i64>,
    /// How far a nudge may stray from the best-approving PSP.
    pub tolerance: Option<f64>,
    /// Total volume the merchant expects per day, from the contract document.
    pub expected_daily_traffic: Option<f64>,
    /// How long one contract day lasts, in seconds — 86400 for calendar cycles, 60 on a
    /// `test_minutes` cycle. Lets a client size simulated traffic to the contract.
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

    // Goals live on the contract source, not the plan — and measuring delivered volume needs the
    // same commitments (their connectors and cycle starts), so load them once for both. `None`
    // means the merchant has no usable contract *or* the feature is switched off for it; either
    // way nothing is being paced, whatever plan a previous run may have left in memory.
    let inputs = deps.inputs.load(&merchant_id).await;
    if inputs.is_none() {
        return Ok(Json(inactive(merchant_id)));
    }

    let Some(plan) = deps.state.load_plan(&merchant_id).await else {
        return Ok(Json(inactive(merchant_id)));
    };

    // The stored plan belongs to a contract that has since been replaced — activating a new
    // document leaves the previous plan in the store until the next forecast lands. Reporting it
    // would show the old contract's verdicts against the new contract's name, which is how a
    // freshly activated contract appeared to arrive with everything already eliminated.
    let anchor_matches = inputs
        .as_ref()
        .is_some_and(|i| i.contract_anchor_ms == plan.contract_anchor_ms);
    if !anchor_matches {
        let mut pending = inactive(merchant_id);
        pending.active = true; // a contract *is* live; its first plan is simply not built yet
        return Ok(Json(pending));
    }
    let goals: std::collections::HashMap<String, f64> = inputs
        .as_ref()
        .map(|i| {
            i.commitments
                .iter()
                .map(|c| (c.connector.clone(), c.goal))
                .collect()
        })
        .unwrap_or_default();
    let measured = match inputs.as_ref() {
        Some(i) => {
            deps.volume
                .measure(
                    &merchant_id,
                    &i.commitments,
                    volume_commitment::math::PACE_WINDOW_DAYS,
                )
                .await
        }
        None => Default::default(),
    };

    let mut psps = Vec::with_capacity(plan.psps.len());
    for entry in &plan.psps {
        psps.push(PspPacing {
            goal: goals.get(&entry.connector).copied().unwrap_or(0.0),
            achieved: measured
                .achieved
                .get(&entry.connector)
                .copied()
                .unwrap_or(0.0),
            gap: entry.remaining,
            pace: measured.pace.get(&entry.connector).copied().unwrap_or(0.0),
            sr_volume: entry.routing_gives_daily,
            floor_per_day: entry.needed_daily,
            steer_rate: entry.steer_rate,
            steered_today: measured
                .steered_today
                .get(&entry.connector)
                .copied()
                .unwrap_or(0.0),
            reward: entry.reward,
            steering: entry.needs_steering,
            connector: entry.connector.clone(),
        });
    }

    Ok(Json(VolumeCommitmentView {
        merchant_id,
        active: true,
        computed_at_epoch_secs: Some(plan.computed_at_epoch_secs),
        tolerance: Some(plan.tolerance),
        expected_daily_traffic: inputs.as_ref().map(|i| i.expected_daily_traffic),
        day_secs: inputs.as_ref().and_then(|i| i.commitments.first().map(|c| c.day_secs)),
        rule_id: inputs.as_ref().map(|i| i.contract_rule_id.clone()),
        reward_at_stake: plan.psps.iter().map(|p| p.reward).sum(),
        psps,
        eliminated: plan
            .dropped
            .iter()
            .map(|p| EliminatedPspView {
                connector: p.connector.clone(),
                gap: p.remaining,
                reward: p.reward,
                reason: p.reason.clone(),
            })
            .collect(),
    }))
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
    /// First day of the cycle, `YYYY-MM-DD` in the contract's zone.
    pub cycle_start: String,
    /// When the cycle closes — the next one's start. Lets a client show a countdown and size a
    /// simulated run to the time actually left rather than to a whole cycle.
    pub cycle_end: String,
    /// Length of the cycle in days — the x-axis span the promise line runs to.
    pub days_total: u32,
    /// True when the current plan has given this commitment up.
    pub eliminated: bool,
    pub points: Vec<volume_commitment::volume::DayVolume>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesResponse {
    pub merchant_id: String,
    pub connectors: Vec<ConnectorSeries>,
}

/// `?run_id=` renders a past execution instead of the one in flight.
#[derive(Debug, Deserialize)]
pub struct SeriesQuery {
    #[serde(default)]
    pub run_id: Option<String>,
}

/// The cycle window a run covers, taken from its id.
///
/// A run is named for the instant its cycle opened, so the id alone fixes the start; the length
/// comes from the contract's current cycle, which is exact for a fixed-length cycle and within a
/// day or two for calendar months of differing length.
fn shift_to_run(commitments: &mut [volume_commitment::Commitment], run_id: &str) {
    let Some(start_ms) = run_id
        .strip_prefix("vcr_")
        .and_then(|ms| ms.parse::<i64>().ok())
    else {
        return;
    };
    for commitment in commitments {
        let length_ms = commitment.period_end_ms - commitment.period_start_ms;
        commitment.period_start_ms = start_ms;
        commitment.period_end_ms = start_ms + length_ms;
    }
}

/// `GET /merchant-account/:merchant_id/volume-commitment/series` — per-day delivered volume per
/// PSP over a cycle, plus the promise each line is racing. Defaults to the cycle in flight;
/// `?run_id=` renders a past one, so a finished run can be read with its own chart rather than
/// against whatever is live now.
pub async fn get_series(
    Path(merchant_id): Path<String>,
    Query(query): Query<SeriesQuery>,
) -> Result<Json<SeriesResponse>, (StatusCode, String)> {
    let empty = || SeriesResponse {
        merchant_id: merchant_id.clone(),
        connectors: Vec::new(),
    };
    let Some(deps) = volume_commitment::deps() else {
        return Ok(Json(empty()));
    };
    let Some(inputs) = deps.inputs.load(&merchant_id).await else {
        return Ok(Json(empty()));
    };

    let eliminated: Vec<String> = deps
        .state
        .load_plan(&merchant_id)
        .await
        .map(|plan| plan.dropped.iter().map(|d| d.connector.clone()).collect())
        .unwrap_or_default();

    // Re-aim the window at the requested run before measuring; everything downstream then reads
    // that cycle, not the live one.
    let mut commitments = inputs.commitments.clone();
    if let Some(run_id) = &query.run_id {
        shift_to_run(&mut commitments, run_id);
    }

    let mut points = deps.volume.daily_series(&merchant_id, &commitments).await;
    points.sort_by_key(|p| p.day_index);

    let connectors = commitments
        .iter()
        .map(|commitment| ConnectorSeries {
            connector: commitment.connector.clone(),
            goal: commitment.goal,
            reward: commitment.reward,
            cycle_start: chrono::DateTime::from_timestamp_millis(commitment.period_start_ms)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_default(),
            cycle_end: chrono::DateTime::from_timestamp_millis(commitment.period_end_ms)
                .map(|dt| dt.to_rfc3339())
                .unwrap_or_default(),
            // The cycle's full span in contract days — a whole number by construction, since it
            // is measured end-to-start rather than from now.
            days_total: volume_commitment::math::days_left(
                commitment.period_end_ms,
                commitment.period_start_ms,
                commitment.day_secs,
            )
            .round()
            .max(1.0) as u32,
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
    pub events: Vec<volume_commitment::volume::AuditEvent>,
}

/// `?run_id=` narrows the audit to one execution of the contract.
#[derive(Debug, Deserialize)]
pub struct AuditQuery {
    #[serde(default)]
    pub run_id: Option<String>,
}

/// `GET /merchant-account/:merchant_id/volume-commitment/audit` — everything the feature did for
/// this merchant and why, reconstructed from the analytics events: forecasts, steer chunks,
/// eliminations. Persistent, because the events are.
pub async fn get_audit(
    Path(merchant_id): Path<String>,
    Query(query): Query<AuditQuery>,
) -> Result<Json<AuditResponse>, (StatusCode, String)> {
    let Some(deps) = volume_commitment::deps() else {
        return Ok(Json(AuditResponse {
            merchant_id,
            runs: Vec::new(),
            events: Vec::new(),
        }));
    };

    // Read the whole window once, then group: the run list and the filtered events are two views
    // of the same events, and a merchant has few enough runs in flight to summarise in memory.
    let all = deps.volume.audit_events(&merchant_id, 500).await;
    let current_run = deps
        .state
        .load_plan(&merchant_id)
        .await
        .map(|plan| plan.run_id);

    let mut order: Vec<String> = Vec::new();
    let mut summaries: std::collections::HashMap<String, RunSummary> =
        std::collections::HashMap::new();
    for event in &all {
        // Events written before runs were named still deserve a home rather than vanishing.
        let run_id = event.run_id.clone().unwrap_or_else(|| "earlier".to_string());
        let entry = summaries.entry(run_id.clone()).or_insert_with(|| {
            order.push(run_id.clone());
            RunSummary {
                started_at_epoch_ms: run_id
                    .strip_prefix("vcr_")
                    .and_then(|ms| ms.parse().ok())
                    .unwrap_or(event.at_epoch_ms),
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
            volume_commitment::volume::AuditKind::Forecast => entry.forecasts += 1,
            volume_commitment::volume::AuditKind::Steered => entry.steers += 1,
            volume_commitment::volume::AuditKind::Eliminated => entry.eliminations += 1,
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
            .filter(|e| e.run_id.as_deref().unwrap_or("earlier") == wanted)
            .collect(),
        None => all,
    };

    Ok(Json(AuditResponse {
        merchant_id,
        runs,
        events,
    }))
}
