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
        day_secs: u64,
        pace_window_days: u32,
        amount_scale: f64,
        into: &mut MeasuredVolume,
    ) -> Result<(), VolumeError> {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let day_ms = math::day_ms(day_secs);
        let pace_days = i64::from(pace_window_days.max(1));
        let Windows {
            today_start_ms,
            traffic_start_ms,
            recent_start_ms,
            pace_start_ms,
        } = windows(cycle_start_ms, now_ms, day_secs, pace_days);
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
        let pace_days_covered = elapsed_window_days(now_ms, pace_start_ms, pace_days, day_ms);
        let traffic_days_covered =
            elapsed_window_days(now_ms, traffic_start_ms, pace_days, day_ms);
        let recent_days_covered =
            elapsed_window_days(now_ms, recent_start_ms, RECENT_TRAFFIC_DAYS, day_ms);
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
            into.pace
                .insert(gateway.clone(), nan_to_zero(row.pace_total) / pace_days_covered);
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
        if group_daily > 0.0 {
            into.total_daily = Some(match into.total_daily {
                Some(existing) => existing.max(group_daily),
                None => group_daily,
            });
        }
        // Only once the short window is a window. Before a whole contract day has passed it is a
        // slice of one divided by a full day, which understates the rate — the very failure the
        // wide window exists to avoid, and not one to reintroduce here.
        if group_recent_daily > 0.0 && now_ms.saturating_sub(recent_start_ms) >= day_ms {
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

        let mut by_cycle: HashMap<(i64, u64), Vec<String>> = HashMap::new();
        for commitment in commitments {
            by_cycle
                .entry((commitment.period_start_ms, commitment.day_secs))
                .or_default()
                .push(commitment.connector.clone());
        }

        for ((cycle_start_ms, day_secs), connectors) in by_cycle {
            self.measure_group(
                merchant_id,
                &connectors,
                cycle_start_ms,
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

fn windows(cycle_start_ms: i64, now_ms: i64, day_secs: u64, pace_days: i64) -> Windows {
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
        recent_start_ms: now_ms.saturating_sub(RECENT_TRAFFIC_DAYS * day_ms),
        pace_start_ms: traffic_start_ms.max(cycle_start_ms),
    }
}

/// Contract days the short recent-rate window spans. Two, so a single quiet day cannot halve the
/// estimate, and short enough to follow a rate that is still moving.
const RECENT_TRAFFIC_DAYS: i64 = 2;

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
        let w = windows(cycle_start, now, math::SECS_PER_DAY, 7);

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
        let w = windows(cycle_start, now, math::SECS_PER_DAY, 7);

        assert_eq!(w.recent_start_ms, now - RECENT_TRAFFIC_DAYS * DAY_MS);
        // Two contract days, so a single quiet one cannot halve the estimate.
        assert!(
            (elapsed_window_days(now, w.recent_start_ms, RECENT_TRAFFIC_DAYS, DAY_MS) - 2.0).abs()
                < 1e-9
        );
        // Much shorter than the window the merchant's overall size is read from.
        assert!(w.recent_start_ms > w.traffic_start_ms);
    }

    /// Once a cycle is older than the pace window there is nothing to reach back for, and the two
    /// windows are the same trailing week.
    #[test]
    fn a_mature_cycle_reads_both_over_the_same_week() {
        let cycle_start = 100 * DAY_MS;
        let now = cycle_start + 20 * DAY_MS + DAY_MS / 2;
        let w = windows(cycle_start, now, math::SECS_PER_DAY, 7);

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
