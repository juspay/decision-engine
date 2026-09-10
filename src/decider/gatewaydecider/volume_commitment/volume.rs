//! What each PSP was actually *delivered*, read from the traffic — deliberately separate from
//! what the contract *promised*.

use std::collections::HashMap;

use async_trait::async_trait;
use clickhouse::{Client, Row};
use masking::PeekInterface;
use serde::Deserialize;

use super::inputs::{Commitment, MeasuredVolume};
use super::math;
use crate::analytics::clickhouse::common::{fetch_all, payment_amount_expr, DOMAIN_TABLE};
use crate::analytics::clickhouse::query::{BoundQueryBuilder, FilterClause};
use crate::analytics::flow::FlowType;
use crate::config::ClickHouseAnalyticsConfig;
use crate::decider::gatewaydecider::types::GatewayDeciderApproach;
use crate::logger;

/// Approach stamped on nudged decisions; steered volume counts toward the goal, not toward what
/// routing provides unaided.
static STEERED_APPROACH: once_cell::sync::Lazy<String> =
    once_cell::sync::Lazy::new(|| GatewayDeciderApproach::SrSelectionVolumeCommitment.to_string());

/// SQL predicates for steered / unaided decide events.
static STEERED_PRED: once_cell::sync::Lazy<String> =
    once_cell::sync::Lazy::new(|| format!("routing_approach = '{}'", *STEERED_APPROACH));
static UNAIDED_PRED: once_cell::sync::Lazy<String> = once_cell::sync::Lazy::new(|| {
    format!(
        "(routing_approach IS NULL OR routing_approach != '{}')",
        *STEERED_APPROACH
    )
});

/// The filters every query here starts from: one merchant, one flow type.
fn base_filters(builder: &mut BoundQueryBuilder, merchant_id: &str, flow: FlowType) {
    builder.add_filter(FilterClause::eq("merchant_id", merchant_id.to_string()));
    builder.add_filter(FilterClause::raw(format!(
        "flow_type = '{}'",
        flow.as_str()
    )));
}

/// Why delivered volume could not be measured. Distinct from "nothing was delivered": a plan
/// built on an empty measurement would read every PSP as owing its whole goal.
#[derive(Debug, thiserror::Error)]
pub enum VolumeError {
    #[error("no volume source is configured (clickhouse analytics is disabled)")]
    Unavailable,
    #[error("could not read routed volume from clickhouse: {0}")]
    Read(String),
}

/// Where observed traffic is read from.
///
/// Only what the forecast needs: everything the dashboard reads goes through the analytics read
/// store, so this stays the decisioning dependency it is (and stays easy to fake in tests).
#[async_trait]
pub trait VolumeSource: Send + Sync {
    /// What each PSP has been sent this cycle, and how fast, over `pace_window_days`. Amounts
    /// come back on the contract's own scale, so `amount_scale` is applied to every measurement.
    async fn measure(
        &self,
        merchant_id: &str,
        commitments: &[Commitment],
        pace_window_days: u32,
        amount_scale: f64,
    ) -> Result<MeasuredVolume, VolumeError>;
}

/// Reads routed volume out of the analytics events in ClickHouse.
pub struct ClickHouseVolumeSource {
    client: Client,
}

impl ClickHouseVolumeSource {
    /// Build a client against the analytics ClickHouse; no probe — an unreachable ClickHouse
    /// logs and measures nothing rather than failing startup.
    pub fn new(config: &ClickHouseAnalyticsConfig) -> Self {
        let mut client = Client::default()
            .with_url(config.url.clone())
            .with_database(config.database.clone())
            .with_user(config.user.clone());
        if let Some(password) = &config.password {
            client = client.with_password(password.peek().clone());
        }
        Self { client }
    }

    /// One aggregate per (cycle start, timezone) group — usually a single round trip, since PSP
    /// contracts tend to share a cycle. Window boundaries are midnights in the contract's zone.
    async fn measure_group(
        &self,
        merchant_id: &str,
        connectors: &[String],
        cycle_start_ms: i64,
        cycle_days: i64,
        day_secs: u64,
        pace_window_days: u32,
        amount_scale: f64,
        into: &mut MeasuredVolume,
    ) -> Result<(), VolumeError> {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let day_ms = math::day_ms(day_secs);
        let pace_days = i64::from(pace_window_days.max(1));
        let recent_days = recent_traffic_days(cycle_days, pace_days);
        let Windows {
            today_start_ms,
            traffic_start_ms,
            recent_start_ms,
            pace_start_ms,
        } = windows(cycle_start_ms, now_ms, day_secs, pace_days, recent_days);
        // One query covers both windows, so it opens at whichever starts first: the cycle, which
        // bounds what a commitment has delivered, or the traffic average, which may predate it.
        let from_ms = cycle_start_ms.min(traffic_start_ms).min(recent_start_ms);
        let unaided = &*UNAIDED_PRED;
        let steered = &*STEERED_PRED;
        // Traffic carries major currency units; the goals this is compared against are in minor.
        let amount = payment_amount_expr(amount_scale);

        let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);
        builder.extend_selects([
            "gateway".to_string(),
            format!("sumIf({amount}, created_at_ms >= {cycle_start_ms}) AS achieved"),
            format!("sumIf({amount}, created_at_ms >= {pace_start_ms}) AS pace_total"),
            format!(
                "sumIf({amount}, created_at_ms >= {pace_start_ms} AND {unaided}) \
                 AS unaided_total"
            ),
            // Today's running steered total, measured from the start of the contract day.
            format!(
                "sumIf({amount}, created_at_ms >= {today_start_ms} AND {steered}) \
                 AS steered_today"
            ),
            // Every PSP the merchant routed to, over a window of its own — see `windows`.
            format!("sumIf({amount}, created_at_ms >= {traffic_start_ms}) AS traffic_total"),
            // The same flow over a short recent window — what feasibility extrapolates from.
            format!("sumIf({amount}, created_at_ms >= {recent_start_ms}) AS recent_total"),
            // When traffic starts inside each window, so the rate is divided by the span it was
            // actually flowing for rather than by the window's nominal length — see
            // `covered_from`. Zero where the window caught nothing, so both are read through
            // `first_seen`.
            format!(
                "minIf(created_at_ms, created_at_ms >= {traffic_start_ms}) AS traffic_first_ms"
            ),
            format!("minIf(created_at_ms, created_at_ms >= {recent_start_ms}) AS recent_first_ms"),
        ]);
        base_filters(&mut builder, merchant_id, FlowType::DecideGatewayDecision);
        builder.add_filter(FilterClause::raw(format!("created_at_ms >= {from_ms}")));
        // Deliberately unfiltered by connector: the per-PSP maps below still only take the
        // contract's own connectors, but the total traffic steering can draw on is every PSP the
        // merchant routed to, not just the ones under contract.
        builder.extend_group_bys(["gateway"]);

        let rows = match fetch_all::<VolumeRow>(builder.build(&self.client)).await {
            Ok(rows) => rows,
            Err(error) => {
                // Zeroes read as "behind", but nudges stay within tolerance, so the cost is
                // bounded. Loud, because a persistent failure makes the plan meaningless.
                logger::error!(
                    tag = "volume_commitment",
                    merchant_id = merchant_id,
                    "could not read routed volume from clickhouse: {error:?}"
                );
                into.measurement_failed = true;
                return Err(VolumeError::Read(format!("{error:?}")));
            }
        };

        // Divide by the days actually queried — a fixed 7 would understate the rate early in a
        // cycle and mid-day, and an understated routing rate reads as a phantom shortfall.
        //
        // For the two merchant-wide windows "actually queried" runs from the first payment they
        // caught. Both may open before the merchant's first traffic — the recent one always
        // reaches two contract days back, whatever is there — and a merchant sends nothing over
        // the stretch in front of that payment, so it is no part of the flow being measured.
        let traffic_from_ms =
            covered_from(traffic_start_ms, first_seen(&rows, |r| r.traffic_first_ms));
        let recent_from_ms =
            covered_from(recent_start_ms, first_seen(&rows, |r| r.recent_first_ms));
        let pace_days_covered = elapsed_window_days(now_ms, pace_start_ms, pace_days, day_ms);
        let traffic_days_covered = elapsed_window_days(now_ms, traffic_from_ms, pace_days, day_ms);
        let recent_days_covered = elapsed_window_days(now_ms, recent_from_ms, recent_days, day_ms);
        let mut group_daily = 0.0;
        let mut group_recent_daily = 0.0;
        for row in rows {
            let Some(gateway) = row.gateway else { continue };
            // Every gateway counts toward the traffic total, contracted or not.
            group_daily += nan_to_zero(row.traffic_total) / traffic_days_covered;
            group_recent_daily += nan_to_zero(row.recent_total) / recent_days_covered;
            if !connectors.iter().any(|c| c == &gateway) {
                continue;
            }
            into.achieved
                .insert(gateway.clone(), nan_to_zero(row.achieved));
            into.pace.insert(
                gateway.clone(),
                nan_to_zero(row.pace_total) / pace_days_covered,
            );
            into.routing_gives_daily.insert(
                gateway.clone(),
                nan_to_zero(row.unaided_total) / pace_days_covered,
            );
            // Not divided: this is today's running total, not a rate.
            into.steered_today
                .insert(gateway, nan_to_zero(row.steered_today));
        }

        // Commitments sharing a cycle are measured in one group; a document mixing cycles runs
        // this more than once over near-identical windows, each estimating the same merchant-wide
        // flow. Keep the largest rather than adding them, which would count the traffic twice.
        //
        // A rate is published only once a contract day of traffic stands behind it. Below that,
        // `elapsed_window_days` floors the divisor at a whole day, so the figure would be a slice
        // of a day presented as all of one — too low, and this is the rate feasibility writes
        // commitments off against. Left unset it says as much, and the contract's declared
        // expectation governs until the traffic can answer for itself. The test is on the traffic
        // measured, not on the window: the recent window is two contract days wide by
        // construction, whether or not a merchant was sending across them.
        let traffic_measured_ms = now_ms.saturating_sub(traffic_from_ms);
        let recent_measured_ms = now_ms.saturating_sub(recent_from_ms);
        if group_daily > 0.0 && traffic_measured_ms >= day_ms {
            into.total_daily = Some(match into.total_daily {
                Some(existing) => existing.max(group_daily),
                None => group_daily,
            });
        }
        if group_recent_daily > 0.0 && recent_measured_ms >= day_ms {
            into.recent_daily = Some(match into.recent_daily {
                Some(existing) => existing.max(group_recent_daily),
                None => group_recent_daily,
            });
        }
        Ok(())
    }
}

#[async_trait]
impl VolumeSource for ClickHouseVolumeSource {
    async fn measure(
        &self,
        merchant_id: &str,
        commitments: &[Commitment],
        pace_window_days: u32,
        amount_scale: f64,
    ) -> Result<MeasuredVolume, VolumeError> {
        let mut measured = MeasuredVolume::default();

        // Grouped by the whole cycle, not just its opening: how long a cycle runs sets how far
        // back the recent-rate window reads, so two cycles of different lengths are two groups.
        let mut by_cycle: HashMap<(i64, i64, u64), Vec<String>> = HashMap::new();
        for commitment in commitments {
            by_cycle
                .entry((
                    commitment.period_start_ms,
                    commitment.period_end_ms,
                    commitment.day_secs,
                ))
                .or_default()
                .push(commitment.connector.clone());
        }

        for ((cycle_start_ms, cycle_end_ms, day_secs), connectors) in by_cycle {
            let cycle_days =
                math::cycle_days(cycle_start_ms, cycle_end_ms, day_secs).round() as i64;
            self.measure_group(
                merchant_id,
                &connectors,
                cycle_start_ms,
                cycle_days,
                day_secs,
                pace_window_days,
                amount_scale,
                &mut measured,
            )
            .await?;
        }
        Ok(measured)
    }
}

/// Float aggregates are non-nullable and can come back NaN/inf; every consumer wants zero.
fn nan_to_zero(value: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}

/// `sum`/`sumIf` over `JSONExtractFloat` are non-nullable Float64 (NaN when nothing matched), so
/// these must be `f64` — `Option<f64>` fails to decode and takes the whole query down.
#[derive(Debug, Deserialize, Row)]
struct VolumeRow {
    gateway: Option<String>,
    achieved: f64,
    pace_total: f64,
    unaided_total: f64,
    steered_today: f64,
    traffic_total: f64,
    recent_total: f64,
    traffic_first_ms: i64,
    recent_first_ms: i64,
}

/// The lower bounds one measurement query reads from.
///
/// Two averages, two windows, because they answer different questions.
///
/// A commitment's own progress belongs to its cycle, so `pace_start_ms` never opens before the
/// cycle does: the days before it hold none of this commitment's volume, and averaging them in
/// would report a young commitment as running slower than it is.
///
/// The merchant's total flow is not a property of the cycle. It is the traffic steering has to
/// divert from, and it does not reset at a billing boundary, so `traffic_start_ms` is a plain
/// trailing window free to predate the cycle. Clamping it to the cycle as well meant a contract
/// read on its first day saw only the hours since the cycle opened, called that a whole day's
/// flow, and wrote off every commitment needing more than those few hours would supply —
/// including ones that needed a couple of percent of what the merchant really does.
///
/// That wide window answers "how much does this merchant do?", which is stable and wants history.
/// It is the wrong answer to "how much will arrive between now and the cycle close?", which is
/// what feasibility extrapolates and which follows the *current* rate. On a merchant whose volume
/// is climbing, a window spanning the whole cycle so far reports about half the rate now flowing,
/// and a commitment needing more than that half is written off while it is comfortably reachable.
/// `recent_start_ms` is the short window that question gets.
struct Windows {
    /// Start of the contract day `now` falls in.
    today_start_ms: i64,
    /// Where the merchant-wide traffic average opens.
    traffic_start_ms: i64,
    /// Where the short recent-rate window opens — what feasibility extrapolates from.
    recent_start_ms: i64,
    /// Where a commitment's own pace average opens.
    pace_start_ms: i64,
}

fn windows(
    cycle_start_ms: i64,
    now_ms: i64,
    day_secs: u64,
    pace_days: i64,
    recent_days: i64,
) -> Windows {
    let day_ms = math::day_ms(day_secs);
    let today_start_ms =
        cycle_start_ms + math::day_index(cycle_start_ms, now_ms, day_secs) * day_ms;
    let traffic_start_ms =
        today_start_ms.saturating_sub(pace_days.saturating_sub(1).max(0) * day_ms);
    Windows {
        today_start_ms,
        traffic_start_ms,
        // Ends at now, not at the start of the contract day: a rate meant to track a change has
        // to include the change.
        recent_start_ms: now_ms.saturating_sub(recent_days * day_ms),
        pace_start_ms: traffic_start_ms.max(cycle_start_ms),
    }
}

/// Fewest contract days the short recent-rate window spans, so a single quiet day cannot halve
/// the estimate.
const MIN_RECENT_TRAFFIC_DAYS: i64 = 2;

/// Contract days the short recent-rate window spans, for a cycle of `cycle_days`.
///
/// This window answers "how much will arrive before the cycle closes", and its answer decides
/// which commitments are written off and what share of the flow every steer takes. It has to be
/// short enough to follow a rate that is still moving, and long enough to be a sample of the
/// thing it extrapolates.
///
/// A fixed two days met the first requirement and not the second. Two contract days is a minute
/// on a test cycle — a fine sample of a ten-minute race — but two *calendar* days on a monthly
/// contract, and a merchant reads 30-40% under its own rate if those two land on a weekend. A
/// commitment needing more than that reads unreachable and is dropped for the rest of the month,
/// on the strength of one quiet weekend.
///
/// A quarter of the cycle keeps the demo's minute and gives a monthly contract a whole week,
/// which is the shortest window that holds every day of the weekly cycle traffic actually has.
/// Capped at the pace window, past which it stops being the *recent* rate at all.
fn recent_traffic_days(cycle_days: i64, pace_days: i64) -> i64 {
    (cycle_days / 4).clamp(
        MIN_RECENT_TRAFFIC_DAYS,
        pace_days.max(MIN_RECENT_TRAFFIC_DAYS),
    )
}

/// The earliest payment any gateway reported for one window, or None where it caught none —
/// `minIf` over no rows is zero, which is not an instant.
fn first_seen(rows: &[VolumeRow], pick: fn(&VolumeRow) -> i64) -> Option<i64> {
    rows.iter().map(pick).filter(|ms| *ms > 0).min()
}

/// Where a window's measured span really begins: its own opening, or the first payment inside it
/// when the merchant was not yet sending any. Traffic that started before the window opened
/// leaves the window's own start standing — the flow was there for all of it.
fn covered_from(window_start_ms: i64, first_seen_ms: Option<i64>) -> i64 {
    match first_seen_ms {
        Some(first) if first > window_start_ms => first,
        _ => window_start_ms,
    }
}

/// Contract days the query covered, floored at one (sub-day rates are noise) and capped at the pace window.
fn elapsed_window_days(now_ms: i64, window_start_ms: i64, pace_days: i64, day_ms: i64) -> f64 {
    ((now_ms.saturating_sub(window_start_ms)) as f64 / day_ms.max(1) as f64)
        .clamp(1.0, pace_days as f64)
}

/// Serves volume handed to it up front; stands in for ClickHouse when analytics is off. A
/// merchant it was given nothing for is *unmeasurable*, not at zero — no plan is built from it.
pub struct FixtureVolumeSource {
    merchants: HashMap<String, MeasuredVolume>,
}

impl FixtureVolumeSource {
    pub fn new(merchants: HashMap<String, MeasuredVolume>) -> Self {
        Self { merchants }
    }
}

#[async_trait]
impl VolumeSource for FixtureVolumeSource {
    async fn measure(
        &self,
        merchant_id: &str,
        _commitments: &[Commitment],
        _pace_window_days: u32,
        _amount_scale: f64,
    ) -> Result<MeasuredVolume, VolumeError> {
        self.merchants
            .get(merchant_id)
            .cloned()
            .ok_or(VolumeError::Unavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY_MS: i64 = math::SECS_PER_DAY as i64 * 1000;

    /// Mid-cycle at noon: six full days plus half of today.
    #[test]
    fn a_mature_window_divides_by_the_days_actually_covered() {
        let now = 6 * DAY_MS + DAY_MS / 2;
        assert!((elapsed_window_days(now, 0, 7, DAY_MS) - 6.5).abs() < 1e-9);
    }

    /// A cycle two days old must not divide by seven — that understated the rate ~3.5x.
    #[test]
    fn a_young_cycle_divides_by_its_own_age() {
        let now = 2 * DAY_MS;
        assert!((elapsed_window_days(now, 0, 7, DAY_MS) - 2.0).abs() < 1e-9);
    }

    #[test]
    fn the_window_never_shrinks_below_a_day_or_grows_past_the_pace_window() {
        assert_eq!(elapsed_window_days(DAY_MS / 4, 0, 7, DAY_MS), 1.0);
        assert_eq!(elapsed_window_days(30 * DAY_MS, 0, 7, DAY_MS), 7.0);
    }

    /// The case that wrote off a reachable commitment: a contract read hours into its first day.
    /// The cycle holds only those hours, but the merchant's flow has a week of history behind it,
    /// and it is the flow that decides what a commitment can still be given.
    #[test]
    fn the_traffic_window_reaches_back_past_a_young_cycle() {
        let cycle_start = 100 * DAY_MS;
        let now = cycle_start + DAY_MS / 4;
        let w = windows(cycle_start, now, math::SECS_PER_DAY, 7, 2);

        assert_eq!(w.today_start_ms, cycle_start);
        // A commitment has delivered nothing before its cycle, so its own pace starts there.
        assert_eq!(w.pace_start_ms, cycle_start);
        // The merchant was routing payments all week, so the flow is read over the whole week.
        assert_eq!(w.traffic_start_ms, cycle_start - 6 * DAY_MS);
        assert!((elapsed_window_days(now, w.traffic_start_ms, 7, DAY_MS) - 6.25).abs() < 1e-9);
    }

    /// The recent-rate window ends at `now`, not at the start of the contract day: a rate meant
    /// to follow a change has to include the change. It is also independent of the cycle — the
    /// question it answers is about traffic, not about a billing period.
    #[test]
    fn the_recent_window_is_a_short_trailing_slice_ending_now() {
        let cycle_start = 100 * DAY_MS;
        let now = cycle_start + 6 * DAY_MS + DAY_MS / 3;
        let w = windows(cycle_start, now, math::SECS_PER_DAY, 7, 2);

        assert_eq!(w.recent_start_ms, now - 2 * DAY_MS);
        // Two contract days, so a single quiet one cannot halve the estimate.
        assert!((elapsed_window_days(now, w.recent_start_ms, 2, DAY_MS) - 2.0).abs() < 1e-9);
        // Much shorter than the window the merchant's overall size is read from.
        assert!(w.recent_start_ms > w.traffic_start_ms);
    }

    /// A test cycle on a merchant with no traffic before it opens — the shape every fresh demo
    /// has. The recent window reaches two contract days back regardless, so all but a
    /// second of it predates the merchant's first payment, and the flow is measured from that
    /// payment. Read across the full window instead, the rate halves, and both commitments are
    /// written off as unreachable before reward ranking ever chooses between them.
    #[test]
    fn a_window_opening_before_the_first_payment_is_measured_from_that_payment() {
        let day_ms = 60_000; // one-minute contract days
        let cycle_start = 100 * day_ms;
        let first_payment = cycle_start + 5_000;
        let now = cycle_start + 68_000; // day 1.13, where the demo lost both commitments
        let w = windows(cycle_start, now, 60, 7, 2);

        // The window opens a contract day before the merchant's first payment ever landed.
        assert!(w.recent_start_ms < first_payment);
        let from = covered_from(w.recent_start_ms, Some(first_payment));
        assert_eq!(from, first_payment);

        // 118M of traffic over the 63s it has been flowing is ~112M a day; the window's nominal
        // two days would call the same traffic 59M.
        let covered = elapsed_window_days(now, from, 2, day_ms);
        assert!((118_000_000.0 / covered - 112_380_952.0).abs() < 1.0);
        assert!(118_000_000.0 / covered > 73_308_546.0); // stripe's daily need: reachable again
    }

    /// Traffic that predates the window is not evidence the window is short: it was flowing for
    /// every second of it, so the window's own opening stands.
    #[test]
    fn a_window_inside_a_running_flow_keeps_its_own_opening() {
        let cycle_start = 100 * DAY_MS;
        let now = cycle_start + 6 * DAY_MS;
        let w = windows(cycle_start, now, math::SECS_PER_DAY, 7, 2);

        assert_eq!(
            covered_from(w.traffic_start_ms, Some(cycle_start - 30 * DAY_MS)),
            w.traffic_start_ms
        );
        // A window that caught nothing has no first payment to measure from.
        assert_eq!(covered_from(w.traffic_start_ms, None), w.traffic_start_ms);
    }

    /// `minIf` over no matching rows is zero, which is not an instant — it must not be read as
    /// one, or a window would measure from the epoch.
    #[test]
    fn an_empty_window_reports_no_first_payment() {
        fn row(traffic_first_ms: i64) -> VolumeRow {
            VolumeRow {
                gateway: Some("adyen".to_string()),
                achieved: 0.0,
                pace_total: 0.0,
                unaided_total: 0.0,
                steered_today: 0.0,
                traffic_total: 0.0,
                recent_total: 0.0,
                traffic_first_ms,
                recent_first_ms: 0,
            }
        }
        assert_eq!(first_seen(&[row(0), row(0)], |r| r.traffic_first_ms), None);
        // The earliest across gateways, ignoring the ones that caught nothing.
        assert_eq!(
            first_seen(&[row(0), row(900), row(400)], |r| r.traffic_first_ms),
            Some(400)
        );
    }

    /// The window feasibility extrapolates from is a share of the cycle, so a ten-minute demo and
    /// a month-long contract each get one that means something for their own length.
    #[test]
    fn the_recent_window_is_a_share_of_the_cycle() {
        // A monthly contract: a whole week, which is the shortest window holding every day of the
        // weekly shape real traffic has. Two days here is a weekend, and a weekend reads 30-40%
        // under the merchant's own rate — enough to write off a commitment for the month.
        assert_eq!(recent_traffic_days(30, 7), 7);
        // A quarter, capped: past the pace window it stops being the recent rate at all.
        assert_eq!(recent_traffic_days(90, 7), 7);
        // The demo's ten contract days keep the two they had.
        assert_eq!(recent_traffic_days(10, 7), 2);
        // Never below two, however short the cycle: one quiet day must not halve the estimate.
        assert_eq!(recent_traffic_days(4, 7), 2);
        assert_eq!(recent_traffic_days(1, 7), 2);
    }

    /// Once a cycle is older than the pace window there is nothing to reach back for, and the two
    /// windows are the same trailing week.
    #[test]
    fn a_mature_cycle_reads_both_over_the_same_week() {
        let cycle_start = 100 * DAY_MS;
        let now = cycle_start + 20 * DAY_MS + DAY_MS / 2;
        let w = windows(cycle_start, now, math::SECS_PER_DAY, 7, 2);

        assert_eq!(w.today_start_ms, cycle_start + 20 * DAY_MS);
        assert_eq!(w.traffic_start_ms, cycle_start + 14 * DAY_MS);
        assert_eq!(w.pace_start_ms, w.traffic_start_ms);
    }

    /// Compressed days divide by the compressed day length, not the calendar one.
    #[test]
    fn a_simulated_window_divides_by_virtual_days() {
        let day_ms = 120_000; // 120s contract days
        assert!((elapsed_window_days(3 * day_ms, 0, 7, day_ms) - 3.0).abs() < 1e-9);
    }

    /// With analytics off the fixture stands in for ClickHouse; it must refuse rather than
    /// report zero delivery, or every PSP would be steered at its maximum rate.
    #[tokio::test]
    async fn the_fixture_refuses_a_merchant_it_has_no_volume_for() {
        let mut known = MeasuredVolume::default();
        known.achieved.insert("stripe".to_string(), 42.0);
        let source = FixtureVolumeSource::new(HashMap::from([("m1".to_string(), known)]));

        let measured = source
            .measure("m1", &[], 7, 1.0)
            .await
            .expect("fixture merchant");
        assert_eq!(measured.achieved_for("stripe"), 42.0);
        assert!(matches!(
            source.measure("m2", &[], 7, 1.0).await,
            Err(VolumeError::Unavailable)
        ));
    }
}
