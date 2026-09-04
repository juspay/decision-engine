//! Read-only dashboard/projection/impact/samples views plus the scheduler-called run endpoint.

use std::collections::HashMap;

use axum::extract::{Path, Query};
use axum::http::{HeaderMap, StatusCode};
use axum::{Extension, Json};
use futures::FutureExt;
use masking::PeekInterface;
use serde::{Deserialize, Serialize};

use crate::analytics::models::{
    CommitmentAnalytics, CommitmentAnalyticsQuery, CommitmentAuditEvent, CommitmentAuditKind,
    CommitmentDayVolume, CommitmentWindow,
};
use crate::auth::AuthContext;
use crate::decider::gatewaydecider::volume_commitment;
use volume_commitment::controller::{self, RunReport};
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

/// True when the request carries the deployment's admin secret — what the scheduler presents.
/// Checked here as well as in the middleware because the middleware attaches no session to such
/// a caller, and in api-key compat mode a request with no credentials at all reaches the handler.
fn presents_admin_secret(headers: &HeaderMap) -> bool {
    let Some(state) = crate::app::APP_STATE.get() else {
        return false;
    };
    let expected = state.global_config.admin_secret.secret.peek();
    let provided = headers
        .get("x-admin-secret")
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    !expected.is_empty() && provided == expected
}

/// Who may run what: the admin secret may run anything, a session or api key only its own
/// merchant, and the sweep of every merchant (1 + 2N database reads and N ClickHouse queries,
/// serial) is the scheduler's alone.
fn authorize_run(
    admin: bool,
    session: Option<&AuthContext>,
    merchant_id: Option<&str>,
) -> Result<(), (StatusCode, String)> {
    if admin {
        return Ok(());
    }
    match (session, merchant_id) {
        (_, None) => Err((
            StatusCode::FORBIDDEN,
            "running a forecast for every merchant needs the admin secret".to_string(),
        )),
        (Some(context), Some(merchant_id)) if context.merchant_id == merchant_id => Ok(()),
        (Some(_), Some(_)) => Err((
            StatusCode::FORBIDDEN,
            "this session may only run a forecast for its own merchant".to_string(),
        )),
        (None, Some(_)) => Err((
            StatusCode::UNAUTHORIZED,
            "running a forecast needs a session, an api key, or the admin secret".to_string(),
        )),
    }
}

/// `POST /volume-commitment/run-forecast` — re-measure, re-decide what to chase, re-mark who is
/// behind. A merchant with no usable commitments counts as skipped; one whose delivery could not
/// be measured, or whose pass panicked, as failed — its previous plan stands.
pub async fn run_forecast(
    headers: HeaderMap,
    session: Option<Extension<AuthContext>>,
    Query(query): Query<RunQuery>,
) -> Result<Json<RunReport>, (StatusCode, String)> {
    authorize_run(
        presents_admin_secret(&headers),
        session.as_ref().map(|Extension(context)| context),
        query.merchant_id.as_deref(),
    )?;

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
        Ok(Ok(Some(run))) => (1, 0, 0, vec![run]),
        Ok(Ok(None)) => (0, 1, 0, Vec::new()),
        // Logged where it happened.
        Ok(Err(_)) => (0, 0, 1, Vec::new()),
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

/// Where every commitment stands, as the dashboard and the projection both report it.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeCommitmentView {
    pub merchant_id: String,
    /// `true` whenever a contract document is live for the merchant *and* the feature is on —
    /// even between the previous cycle's plan expiring and the next forecast, when `psps` is
    /// empty and nothing is steered.
    pub active: bool,
    /// `true` when a contract document is activated, whether or not the feature is switched on.
    /// With `feature_enabled` false this is the "configured but inert" state: the contract is
    /// live, no payment is steered against it.
    pub contract_configured: bool,
    /// The merchant's `volume_commitment_routing_enabled` flag. Off means nothing is steered
    /// however the contract is configured, so the dashboard can say so instead of showing
    /// nothing.
    pub feature_enabled: bool,
    /// Epoch seconds of the last controller tick.
    pub computed_at_epoch_secs: Option<i64>,
    /// How far a nudge may stray from the best-approving PSP.
    pub tolerance: Option<f64>,
    /// Total volume the merchant expects per day, from the contract document. A declaration, not
    /// a measurement — the simulator drives traffic at this rate, but the plan no longer trusts it.
    pub expected_daily_traffic: Option<f64>,
    /// Total volume per day actually flowing across every PSP. This is what feasibility and the
    /// steer rates are judged against; a wide gap from `expected_daily_traffic` means the
    /// contract's declared traffic does not describe this merchant. `None` before any is measured.
    pub measured_daily_traffic: Option<f64>,
    /// Contract-day length in seconds (`SECS_PER_DAY`, or 60 on a test cycle).
    pub day_secs: Option<u64>,
    /// The routing rule holding the active contract, so the dashboard can act on it.
    pub rule_id: Option<String>,
    /// Reward still reachable across surviving commitments.
    pub reward_at_stake: f64,
    /// When the billing cycle these commitments race opened and closes, RFC 3339. `None` where the
    /// document's commitments do not share one cycle — each then has its own window, and there is
    /// no single one to report. Per-commitment windows come from the `/series` endpoint.
    pub cycle_start: Option<String>,
    pub cycle_end: Option<String>,
    /// Contract days in the shared cycle — minutes on a test cycle, where a "day" lasts `day_secs`.
    pub days_total: Option<u32>,
    /// ISO-4217 code every amount here is denominated in, so a reader can render major units.
    pub currency: Option<String>,
    /// False when a measurement query failed: the positions below are floors, not readings, and
    /// must not be shown as a merchant's real standing.
    pub measurement_available: bool,
    pub psps: Vec<PspPacing>,
    pub eliminated: Vec<EliminatedPspView>,
}

/// Render a plan as the dashboard view of it. Shared by the live view, which reads the stored
/// plan, and the projection, which computes one it never stores — so the two can never describe
/// the same plan differently.

fn view_from_plan(
    merchant_id: String,
    inputs: &CommitmentInputs,
    plan: &SteeringPlan,
    measured: &volume_commitment::MeasuredVolume,
) -> VolumeCommitmentView {
    let goals: HashMap<&str, f64> = inputs
        .commitments
        .iter()
        .map(|c| (c.connector.as_str(), c.goal))
        .collect();

    VolumeCommitmentView {
        active: true,
        contract_configured: true,
        computed_at_epoch_secs: Some(plan.computed_at_epoch_secs),
        tolerance: Some(plan.tolerance),
        // Folded from `0.0` rather than summed: `f64`'s additive identity is `-0.0`, so an empty
        // sum serializes as `-0.0` — a signed zero reward reads as a defect on the wire.
        reward_at_stake: plan
            .psps
            .iter()
            .map(|p| p.reward)
            .fold(0.0, |total, r| total + r),
        measured_daily_traffic: measured.total_daily,
        measurement_available: !measured.measurement_failed,
        psps: plan
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
            .collect(),
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
        ..pending(merchant_id, inputs)
    }
}

/// Where every commitment stands right now, from the stored plan. Read by the composed dashboard
/// and, through `get_projection`, by the not-yet-enabled preview.
async fn pacing_view(merchant_id: String) -> VolumeCommitmentView {
    let Some(deps) = volume_commitment::deps() else {
        // Startup wiring has not run — report inactive rather than failing the card.
        return inactive(merchant_id);
    };

    // Reported separately from the contract so the dashboard can tell an unconfigured merchant
    // apart from one whose contract is live but whose feature flag is off.
    let feature_enabled = deps.inputs.feature_enabled(&merchant_id).await;

    // Load inputs once for goals and measurement; `None` = no usable contract.
    let Some(inputs) = deps.inputs.load_configured(&merchant_id).await else {
        return VolumeCommitmentView {
            feature_enabled,
            ..inactive(merchant_id)
        };
    };

    // Contract activated, routing switched off: nothing is steered against it, so there is no
    // plan to report — only the fact that the contract is inert.
    if !feature_enabled {
        return VolumeCommitmentView {
            contract_configured: true,
            expected_daily_traffic: Some(inputs.expected_daily_traffic),
            day_secs: Some(inputs.day_secs()),
            rule_id: Some(inputs.contract_rule_id.clone()),
            ..inactive(merchant_id)
        };
    }

    // No plan for *this* contract yet (first forecast pending, cycle just rolled, or the stored
    // plan belongs to a replaced document): the contract is live, nothing is paced.
    let Some(plan) = current_plan(deps, &inputs).await else {
        return pending(merchant_id, &inputs);
    };

    let measured = deps
        .volume
        .measure(
            &merchant_id,
            &inputs.commitments,
            math::PACE_WINDOW_DAYS,
            inputs.amount_scale,
        )
        .await
        .unwrap_or_else(|error| {
            crate::logger::warn!(
                tag = "volume_commitment",
                merchant_id = merchant_id.as_str(),
                "pacing card rendered without delivered volume: {error}"
            );
            volume_commitment::MeasuredVolume {
                measurement_failed: true,
                ..Default::default()
            }
        });

    view_from_plan(merchant_id, &inputs, &plan, &measured)
}

/// `GET /merchant-account/:merchant_id/volume-commitment/projection` — what contract routing would
/// do for this cycle if it were switched on right now.
///
/// The same view as the live endpoint, from a plan computed on the spot and never stored, so a
/// merchant can see which commitments are still winnable *before* enabling the feature rather than
/// inferring it from a chart that does not move. Read-only in every sense: no plan is published, no
/// forecast is filed, and the feature flag is left exactly as it was.
pub async fn get_projection(
    Path(merchant_id): Path<String>,
) -> Result<Json<VolumeCommitmentView>, (StatusCode, String)> {
    let Some(deps) = volume_commitment::deps() else {
        return Ok(Json(inactive(merchant_id)));
    };

    let feature_enabled = deps.inputs.feature_enabled(&merchant_id).await;

    // Deliberately `load_configured`: the merchant this exists for has the feature off, which is
    // exactly when `load` reports nothing.
    let Some(inputs) = deps.inputs.load_configured(&merchant_id).await else {
        return Ok(Json(VolumeCommitmentView {
            feature_enabled,
            ..inactive(merchant_id)
        }));
    };

    let (plan, measured) = controller::compute_plan(deps, &inputs)
        .await
        .map_err(|error| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                format!("delivered volume cannot be measured right now: {error}"),
            )
        })?;

    Ok(Json(VolumeCommitmentView {
        // A projection describes what *would* happen; only the flag says whether it is happening.
        active: feature_enabled,
        feature_enabled,
        ..view_from_plan(merchant_id, &inputs, &plan, &measured)
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

/// Whether the stored plan describes the run being viewed. No `run_id` means the live cycle,
/// and a string that is not a run id falls back to the live cycle exactly as
/// `commitments_for_run` does — only a real, different run id disqualifies the plan.
fn plan_covers_run(plan_run_id: &str, run_id: Option<&str>) -> bool {
    match run_id.filter(|r| math::run_start_ms(r).is_some()) {
        Some(wanted) => plan_run_id == wanted,
        None => true,
    }
}

/// The stored plan, only when it belongs to the run being viewed: a past run must not borrow the
/// live plan's eliminated/steering verdicts — they describe a different cycle.
async fn plan_for_run(
    deps: &Deps,
    inputs: &CommitmentInputs,
    run_id: Option<&str>,
) -> Option<SteeringPlan> {
    current_plan(deps, inputs)
        .await
        .filter(|plan| plan_covers_run(&plan.run_id, run_id))
}

/// A live contract with no plan for it yet: active, nothing paced. Also the base `view_from_plan`
/// fills in from, so the contract-level facts are described in one place.
fn pending(merchant_id: String, inputs: &CommitmentInputs) -> VolumeCommitmentView {
    // Commitments in a document normally share a cycle, but nothing forces them to: two contracts
    // can name different anchors or timezones. Report a cycle only when there is genuinely one, so
    // a mixed document cannot be described by a window that belongs to none of its commitments.
    let mut windows = inputs
        .commitments
        .iter()
        .map(|c| (c.period_start_ms, c.period_end_ms));
    let first = windows.next();
    let shared = first.filter(|first| windows.all(|w| w == *first));
    let (start_ms, end_ms) = (shared.map(|w| w.0), shared.map(|w| w.1));
    VolumeCommitmentView {
        active: true,
        contract_configured: true,
        feature_enabled: true,
        expected_daily_traffic: Some(inputs.expected_daily_traffic),
        day_secs: Some(inputs.day_secs()),
        rule_id: Some(inputs.contract_rule_id.clone()),
        cycle_start: start_ms.map(rfc3339),
        cycle_end: end_ms.map(rfc3339),
        days_total: start_ms
            .zip(end_ms)
            .map(|(start, end)| math::days_total(start, end, inputs.day_secs())),
        currency: inputs.currency.clone(),
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
        contract_configured: false,
        feature_enabled: false,
        computed_at_epoch_secs: None,
        tolerance: None,
        expected_daily_traffic: None,
        measured_daily_traffic: None,
        day_secs: None,
        rule_id: None,
        reward_at_stake: 0.0,
        cycle_start: None,
        cycle_end: None,
        days_total: None,
        currency: None,
        measurement_available: true,
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
    /// Instant the cycle opened, RFC 3339 in UTC (the contract's zone shapes the boundary,
    /// not the rendering).
    pub cycle_start: String,
    /// Cycle close (the next cycle's start), for countdowns and sizing a simulated run.
    pub cycle_end: String,
    /// Length of the cycle in days — the x-axis span the promise line runs to.
    pub days_total: u32,
    /// True when the current plan has given this commitment up.
    pub eliminated: bool,
    pub points: Vec<CommitmentDayVolume>,
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

/// The analytics read store, or `None` when analytics is not wired — every commitment read then
/// degrades to "nothing measured" rather than failing the dashboard.
fn read_store() -> Option<std::sync::Arc<dyn crate::analytics::store::AnalyticsReadStore>> {
    crate::app::APP_STATE
        .get()
        .map(|state| state.analytics_runtime.read_store())
}

/// The windows analytics has to read for these commitments. Analytics knows nothing about
/// contracts, so the cycle bounds and the steered-approach vocabulary are handed to it here.
fn analytics_query(
    inputs: &CommitmentInputs,
    commitments: &[Commitment],
    per_day: u32,
) -> CommitmentAnalyticsQuery {
    CommitmentAnalyticsQuery {
        merchant_id: inputs.merchant_id.clone(),
        windows: commitments
            .iter()
            .map(|c| CommitmentWindow {
                connector: c.connector.clone(),
                cycle_start_ms: c.period_start_ms,
                cycle_end_ms: c.period_end_ms,
                day_secs: c.day_secs,
            })
            .collect(),
        per_day,
        audit_limit: AUDIT_EVENT_WINDOW,
        steered_approach: volume_commitment::steered_approach(),
        amount_scale: inputs.amount_scale,
    }
}

/// Per-connector series rows around the points analytics returned.
fn series_response(
    merchant_id: String,
    inputs: &CommitmentInputs,
    commitments: &[Commitment],
    eliminated: &[String],
    points: &[CommitmentDayVolume],
) -> SeriesResponse {
    SeriesResponse {
        merchant_id,
        currency: inputs.currency.clone(),
        day_secs: commitments.first().map(|c| c.day_secs),
        connectors: commitments
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
            .collect(),
    }
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
    pub events: Vec<CommitmentAuditEvent>,
}

/// `?run_id=` narrows a view to one execution of the contract instead of the one in flight.
#[derive(Debug, Deserialize)]
pub struct RunScopedQuery {
    #[serde(default)]
    pub run_id: Option<String>,
}

/// Pacing, series and audit together — one request in place of the four the dashboard used to poll.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DashboardResponse {
    pub pacing: VolumeCommitmentView,
    pub series: SeriesResponse,
    pub audit: AuditResponse,
}

/// The run list and the (optionally run-filtered) events, both from one set of audit events.
fn audit_response(
    merchant_id: String,
    all: Vec<CommitmentAuditEvent>,
    current_run: Option<String>,
    wanted_run: Option<&str>,
) -> AuditResponse {
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
            CommitmentAuditKind::Forecast => entry.forecasts += 1,
            CommitmentAuditKind::Steered => entry.steers += 1,
            CommitmentAuditKind::Eliminated => entry.eliminations += 1,
        }
    }

    let mut runs: Vec<RunSummary> = order
        .into_iter()
        .filter_map(|id| summaries.remove(&id))
        .collect();
    runs.sort_by_key(|run| std::cmp::Reverse(run.started_at_epoch_ms));

    let events = match wanted_run {
        Some(wanted) => all
            .into_iter()
            .filter(|e| e.run_id.as_deref().unwrap_or(UNNAMED_RUN) == wanted)
            .collect(),
        None => all,
    };

    AuditResponse {
        merchant_id,
        runs,
        events,
    }
}

/// The contract state and the one analytics read every dashboard view is built from. `None` when
/// there is no usable contract.
async fn dashboard_parts(
    deps: &Deps,
    merchant_id: &str,
    run_id: Option<&str>,
    per_day: u32,
) -> Option<(
    CommitmentInputs,
    Vec<Commitment>,
    Vec<String>,
    CommitmentAnalytics,
)> {
    let inputs = deps.inputs.load(merchant_id).await?;
    let eliminated: Vec<String> = current_plan(deps, &inputs)
        .await
        .map(|plan| plan.dropped.iter().map(|d| d.connector.clone()).collect())
        .unwrap_or_default();
    let commitments = commitments_for_run(&inputs.commitments, run_id);
    let analytics = match read_store() {
        Some(store) => store
            .volume_commitment(&analytics_query(&inputs, &commitments, per_day))
            .await
            .unwrap_or_default(),
        None => CommitmentAnalytics::default(),
    };
    Some((inputs, commitments, eliminated, analytics))
}

/// `GET /merchant-account/:merchant_id/volume-commitment/dashboard` — pacing, series and audit in
/// one response.
///
/// Pacing is control-plane state (the stored plan); series and audit come from one concurrent
/// analytics read. Composing them here is what lets the browser poll once at one cadence instead
/// of four times at two.
pub async fn get_dashboard(
    Path(merchant_id): Path<String>,
    Query(query): Query<SeriesQuery>,
) -> Result<Json<DashboardResponse>, (StatusCode, String)> {
    let empty_series = || SeriesResponse {
        merchant_id: merchant_id.clone(),
        currency: None,
        day_secs: None,
        connectors: Vec::new(),
    };
    let empty_audit = || AuditResponse {
        merchant_id: merchant_id.clone(),
        runs: Vec::new(),
        events: Vec::new(),
    };

    let pacing = pacing_view(merchant_id.clone()).await;
    let Some(deps) = volume_commitment::deps() else {
        return Ok(Json(DashboardResponse {
            pacing,
            series: empty_series(),
            audit: empty_audit(),
        }));
    };
    let per_day = query.per_day.unwrap_or(1).clamp(1, MAX_BUCKETS_PER_DAY);
    let Some((inputs, commitments, eliminated, analytics)) =
        dashboard_parts(deps, &merchant_id, query.run_id.as_deref(), per_day).await
    else {
        return Ok(Json(DashboardResponse {
            pacing,
            series: empty_series(),
            audit: empty_audit(),
        }));
    };
    let current_run = deps
        .state
        .load_plan(&merchant_id)
        .await
        .map(|plan| plan.run_id);

    Ok(Json(DashboardResponse {
        pacing,
        series: series_response(
            merchant_id.clone(),
            &inputs,
            &commitments,
            &eliminated,
            &analytics.series,
        ),
        audit: audit_response(
            merchant_id,
            analytics.audit,
            current_run,
            query.run_id.as_deref(),
        ),
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
    /// A cycle's width immediately before `cycle`, describing the period `before` and
    /// `baseline_days` cover. Each commitment is measured against its own preceding cycle, so
    /// where a document mixes cycles this is the span containing them rather than a single window.
    pub baseline: ImpactWindow,
    pub connectors: Vec<ConnectorImpact>,
    /// Day-by-day delivery per PSP across the previous cycle, `day_index` counted from its start.
    pub baseline_days: Vec<CommitmentDayVolume>,
    /// The same across the cycle, `day_index` counted from the cycle's start.
    pub cycle_days: Vec<CommitmentDayVolume>,
}

/// `GET /merchant-account/:merchant_id/volume-commitment/impact` — each PSP's traffic in the cycle
/// before its own vs the one in flight, split by who sent it.
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

    let (eliminated, steering): (Vec<String>, Vec<String>) =
        plan_for_run(deps, &inputs, query.run_id.as_deref())
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
    // PSP contracts share a cycle in practice; where they do not, the reported window is the
    // widest span so every connector's traffic is inside it. The figures below are still measured
    // over each commitment's own cycle — this pair only says what period the view covers.
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
    let day_secs = first.day_secs;
    // Day buckets are counted from each commitment's own cycle start, so the axis has to be as
    // long as the longest cycle — not as long as the span between the earliest start and the
    // latest end, which is wider than any single cycle once the document mixes them.
    let days_total = commitments
        .iter()
        .map(|c| math::days_total(c.period_start_ms, c.period_end_ms, c.day_secs))
        .max()
        .unwrap_or(1);
    let cycle_len_ms = commitments
        .iter()
        .map(|c| c.period_end_ms - c.period_start_ms)
        .max()
        .unwrap_or(1)
        .max(1);

    let baseline_end_ms = cycle_start_ms;
    let baseline_start_ms = baseline_end_ms.saturating_sub(cycle_len_ms);

    // Four reads — totals and day-by-day for both halves — issued together by the analytics layer,
    // which steps each commitment back by its own cycle length to find that PSP's baseline.
    let impact = match read_store() {
        Some(store) => store
            .volume_commitment_impact(&analytics_query(&inputs, &commitments, 1))
            .await
            .unwrap_or_default(),
        None => crate::analytics::models::CommitmentImpactData::default(),
    };
    let (before, during) = (impact.before, impact.during);
    let (baseline_days, cycle_days) = (impact.baseline_days, impact.cycle_days);

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::AuthKind;

    fn session(merchant_id: &str) -> AuthContext {
        AuthContext {
            merchant_id: merchant_id.to_string(),
            auth_kind: AuthKind::Jwt,
            user_id: None,
            email: None,
            role: None,
            permissions: None,
        }
    }

    #[test]
    fn the_admin_secret_runs_anything() {
        assert!(authorize_run(true, None, None).is_ok());
        assert!(authorize_run(true, None, Some("m1")).is_ok());
        assert!(authorize_run(true, Some(&session("m2")), Some("m1")).is_ok());
    }

    /// The sweep is the scheduler's: a dashboard user with a write permission must not be able
    /// to put every merchant's forecast on the main server at once.
    #[test]
    fn the_sweep_needs_the_admin_secret() {
        let refused = authorize_run(false, Some(&session("m1")), None).unwrap_err();
        assert_eq!(refused.0, StatusCode::FORBIDDEN);
        let refused = authorize_run(false, None, None).unwrap_err();
        assert_eq!(refused.0, StatusCode::FORBIDDEN);
    }

    #[test]
    fn a_session_runs_only_its_own_merchant() {
        assert!(authorize_run(false, Some(&session("m1")), Some("m1")).is_ok());
        let refused = authorize_run(false, Some(&session("m1")), Some("m2")).unwrap_err();
        assert_eq!(refused.0, StatusCode::FORBIDDEN);
    }

    /// Api-key compat mode lets an unauthenticated request through the middleware; here it
    /// still needs to say who it is.
    #[test]
    fn no_credentials_at_all_is_refused() {
        let refused = authorize_run(false, None, Some("m1")).unwrap_err();
        assert_eq!(refused.0, StatusCode::UNAUTHORIZED);
    }

    /// A past run must not borrow the live plan's verdicts; the live view and a request for the
    /// live run itself keep them; garbage run ids fall back to the live view like everywhere else.
    #[test]
    fn the_live_plan_speaks_only_for_its_own_run() {
        assert!(plan_covers_run("vcr_1788189990000", None));
        assert!(plan_covers_run(
            "vcr_1788189990000",
            Some("vcr_1788189990000")
        ));
        assert!(!plan_covers_run(
            "vcr_1788189990000",
            Some("vcr_1788189810000")
        ));
        assert!(plan_covers_run("vcr_1788189990000", Some("abc")));
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SamplesResponse {
    pub merchant_id: String,
    pub samples: Vec<crate::config::SampleScenario>,
}

/// `GET /merchant-account/:merchant_id/volume-commitment/samples` — the deployment's demo
/// contracts, ready to load into the contract builder.
///
/// A sample is a template: the merchant picks one, edits it if they want to, and activates it.
/// From that point it is an ordinary contract and the ordinary engine paces it.
pub async fn get_samples(
    Path(merchant_id): Path<String>,
) -> Result<Json<SamplesResponse>, (StatusCode, String)> {
    let Some(deps) = volume_commitment::deps() else {
        return Ok(Json(SamplesResponse {
            merchant_id,
            samples: Vec::new(),
        }));
    };

    // A sample this deployment would refuse to store is worse than no sample: it reads as an
    // offer, and activating it returns a validation error the merchant did nothing to cause.
    let samples = deps
        .config
        .samples
        .iter()
        .filter(|sample| {
            crate::euclid::volume_contract::validate_volume_contract_config(
                &sample.contract,
                deps.config.allow_test_cycles,
            )
            .is_empty()
        })
        .cloned()
        .collect();

    Ok(Json(SamplesResponse {
        merchant_id,
        samples,
    }))
}
