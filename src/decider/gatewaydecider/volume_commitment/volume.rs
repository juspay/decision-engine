//! What each PSP was actually *delivered*, read from the traffic — deliberately separate from
//! what the contract *promised*.

use std::collections::HashMap;

use async_trait::async_trait;
use clickhouse::{Client, Row};
use masking::PeekInterface;
use serde::Deserialize;

use super::inputs::{Commitment, MeasuredVolume};
use super::math;
use crate::analytics::clickhouse::common::{fetch_all, DOMAIN_TABLE};
use crate::analytics::clickhouse::query::{BoundQueryBuilder, FilterClause, OrderClause};
use crate::analytics::flow::FlowType;
use crate::config::ClickHouseAnalyticsConfig;
use crate::logger;

/// Payment amount on the decision event, inside `details` JSON. Keep identical to the cost
/// savings metrics' expression — if the request shape moves, both move together.
const AMOUNT_EXPR: &str =
    "JSONExtractFloat(assumeNotNull(details), 'request', 'paymentInfo', 'amount')";

/// Approach stamped on nudged decisions. Steered volume counts toward the goal but not toward
/// what *routing* provides, or the controller would read its own nudges as routing catching up.
const STEERED_APPROACH: &str = "SR_SELECTION_VOLUME_COMMITMENT";

/// One PSP's volume on one day of its cycle, for the pacing chart.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DayVolume {
    pub connector: String,
    /// Days since the PSP's cycle started (0 = the first day).
    pub day_index: u32,
    /// Everything delivered that day.
    pub total: f64,
    /// Of that, what the nudge moved.
    pub steered: f64,
}

/// What kind of audit entry an event is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditKind {
    Forecast,
    Steered,
    Eliminated,
}

/// One entry in the audit trail, reconstructed from the analytics events in ClickHouse — so it
/// covers real payments and survives restarts.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEvent {
    pub at_epoch_ms: i64,
    pub kind: AuditKind,
    /// The contract execution this entry belongs to. `None` on events written before runs were
    /// named, which group under an "earlier activity" bucket rather than being dropped.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connector: Option<String>,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<f64>,
}

/// Where observed traffic is read from.
#[async_trait]
pub trait VolumeSource: Send + Sync {
    /// What each PSP in `commitments` has been sent. Connectors with no traffic are simply absent,
    /// which the controller reads as zero.
    async fn measure(
        &self,
        merchant_id: &str,
        commitments: &[Commitment],
        pace_window_days: u32,
    ) -> MeasuredVolume;

    /// Per-day volume for each PSP since its cycle started. Empty when nothing can be measured.
    async fn daily_series(&self, merchant_id: &str, commitments: &[Commitment]) -> Vec<DayVolume>;

    /// The audit trail, newest first: forecasts and eliminations from the controller's events,
    /// steer chunks from the decide events themselves.
    async fn audit_events(&self, merchant_id: &str, limit: u64) -> Vec<AuditEvent>;
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
        into: &mut MeasuredVolume,
    ) {
        let now_ms = chrono::Utc::now().timestamp_millis();
        let day_ms = math::day_ms(day_secs);
        let pace_days = i64::from(pace_window_days.max(1));
        // The pace window is the recent slice of the cycle, never wider than the cycle itself —
        // averaging over days before the cycle began would understate a young commitment's rate.
        let start_of_today_ms =
            cycle_start_ms + math::day_index(cycle_start_ms, now_ms, day_secs) * day_ms;
        let pace_start_ms = start_of_today_ms
            .saturating_sub(pace_days.saturating_sub(1) * day_ms)
            .max(cycle_start_ms);

        let unaided =
            format!("(routing_approach IS NULL OR routing_approach != '{STEERED_APPROACH}')");
        // Start of the contract day now in progress — the window the steering rate is set against.
        let today_start_ms =
            cycle_start_ms + math::day_index(cycle_start_ms, now_ms, day_secs) * day_ms;

        let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);
        builder.extend_selects([
            "gateway".to_string(),
            format!("sum({AMOUNT_EXPR}) AS achieved"),
            format!("sumIf({AMOUNT_EXPR}, created_at_ms >= {pace_start_ms}) AS pace_total"),
            format!(
                "sumIf({AMOUNT_EXPR}, created_at_ms >= {pace_start_ms} AND {unaided}) \
                 AS unaided_total"
            ),
            format!(
                "sumIf({AMOUNT_EXPR}, created_at_ms >= {today_start_ms} \
                 AND routing_approach = '{STEERED_APPROACH}') AS steered_today"
            ),
        ]);
        builder.add_filter(FilterClause::eq("merchant_id", merchant_id.to_string()));
        builder.add_filter(FilterClause::raw(format!(
            "flow_type = '{}'",
            FlowType::DecideGatewayDecision.as_str()
        )));
        builder.add_filter(FilterClause::raw(format!(
            "created_at_ms >= {cycle_start_ms}"
        )));
        builder.add_filter(FilterClause::raw(format!(
            "gateway IN ({})",
            connectors
                .iter()
                .map(|c| format!("'{}'", c.replace('\'', "\\'")))
                .collect::<Vec<_>>()
                .join(", ")
        )));
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
                return;
            }
        };

        // Divide by the days actually queried — a fixed 7 would understate the rate early in a
        // cycle and mid-day, and an understated routing rate reads as a phantom shortfall.
        let window_days = elapsed_window_days(now_ms, pace_start_ms, pace_days, day_ms);
        for row in rows {
            let Some(gateway) = row.gateway else { continue };
            into.achieved.insert(gateway.clone(), row.achieved);
            into.pace
                .insert(gateway.clone(), row.pace_total / window_days);
            into.routing_gives_daily
                .insert(gateway.clone(), row.unaided_total / window_days);
            // Not divided: this is today's running total, not a rate.
            into.steered_today.insert(gateway, row.steered_today);
        }
    }
}

#[async_trait]
impl VolumeSource for ClickHouseVolumeSource {
    async fn measure(
        &self,
        merchant_id: &str,
        commitments: &[Commitment],
        pace_window_days: u32,
    ) -> MeasuredVolume {
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
                &mut measured,
            )
            .await;
        }
        measured
    }

    async fn daily_series(&self, merchant_id: &str, commitments: &[Commitment]) -> Vec<DayVolume> {
        let mut series = Vec::new();

        let mut by_cycle: HashMap<(i64, u64), Vec<String>> = HashMap::new();
        for commitment in commitments {
            by_cycle
                .entry((commitment.period_start_ms, commitment.day_secs))
                .or_default()
                .push(commitment.connector.clone());
        }

        for ((cycle_start_ms, day_secs), connectors) in by_cycle {
            let day_ms = math::day_ms(day_secs);
            let steered = format!("routing_approach = '{STEERED_APPROACH}'");

            let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);
            builder.extend_selects([
                "gateway".to_string(),
                // Whole local days since the cycle opened, so each bucket is a contract-day.
                format!(
                    "toInt64(intDiv(created_at_ms - {cycle_start_ms}, {day_ms})) AS day_index"
                ),
                format!("sum({AMOUNT_EXPR}) AS total"),
                format!("sumIf({AMOUNT_EXPR}, {steered}) AS steered"),
            ]);
            builder.add_filter(FilterClause::eq("merchant_id", merchant_id.to_string()));
            builder.add_filter(FilterClause::raw(format!(
                "flow_type = '{}'",
                FlowType::DecideGatewayDecision.as_str()
            )));
            builder.add_filter(FilterClause::raw(format!(
                "created_at_ms >= {cycle_start_ms}"
            )));
            builder.add_filter(FilterClause::raw(format!(
                "gateway IN ({})",
                connectors
                    .iter()
                    .map(|c| format!("'{}'", c.replace('\'', "\\'")))
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
            builder.extend_group_bys(["gateway", "day_index"]);

            match fetch_all::<DayVolumeRow>(builder.build(&self.client)).await {
                Ok(rows) => {
                    for row in rows {
                        let Some(gateway) = row.gateway else { continue };
                        let Ok(day_index) = u32::try_from(row.day_index) else {
                            continue;
                        };
                        series.push(DayVolume {
                            connector: gateway,
                            day_index,
                            total: row.total,
                            steered: row.steered,
                        });
                    }
                }
                Err(error) => {
                    logger::error!(
                        tag = "volume_commitment",
                        merchant_id = merchant_id,
                        "could not read the daily volume series from clickhouse: {error:?}"
                    );
                }
            }
        }
        series
    }

    async fn audit_events(&self, merchant_id: &str, limit: u64) -> Vec<AuditEvent> {
        let mut events = Vec::new();

        // Forecast runs (which carry the eliminations) — one event per controller run.
        let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);
        builder.extend_selects(["created_at_ms".to_string(), "details".to_string()]);
        builder.add_filter(FilterClause::eq("merchant_id", merchant_id.to_string()));
        builder.add_filter(FilterClause::raw(format!(
            "flow_type = '{}'",
            FlowType::VolumeCommitmentForecast.as_str()
        )));
        builder.add_order_by(OrderClause::desc("created_at_ms"));
        builder.set_limit(Some(limit));
        match fetch_all::<ForecastEventRow>(builder.build(&self.client)).await {
            Ok(rows) => {
                for row in rows {
                    events.extend(forecast_row_to_events(&row));
                }
            }
            Err(error) => logger::error!(
                tag = "volume_commitment",
                merchant_id = merchant_id,
                "could not read forecast audit events from clickhouse: {error:?}"
            ),
        }

        // Steer chunks — the decide events the nudge diverted, with the reason it recorded.
        let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);
        builder.extend_selects([
            "created_at_ms".to_string(),
            "gateway".to_string(),
            format!("{AMOUNT_EXPR} AS amount"),
            "JSONExtractString(assumeNotNull(details), 'response', 'volume_steer_info', 'reason') \
             AS reason"
                .to_string(),
            "JSONExtractString(assumeNotNull(details), 'response', 'volume_steer_info', 'runId') \
             AS run_id"
                .to_string(),
        ]);
        builder.add_filter(FilterClause::eq("merchant_id", merchant_id.to_string()));
        builder.add_filter(FilterClause::raw(format!(
            "flow_type = '{}'",
            FlowType::DecideGatewayDecision.as_str()
        )));
        builder.add_filter(FilterClause::raw(format!(
            "routing_approach = '{STEERED_APPROACH}'"
        )));
        builder.add_order_by(OrderClause::desc("created_at_ms"));
        builder.set_limit(Some(limit));
        match fetch_all::<SteerEventRow>(builder.build(&self.client)).await {
            Ok(rows) => {
                for row in rows {
                    events.push(AuditEvent {
                        at_epoch_ms: row.created_at_ms,
                        kind: AuditKind::Steered,
                        run_id: (!row.run_id.is_empty()).then(|| row.run_id.clone()),
                        message: if row.reason.is_empty() {
                            format!(
                                "Steered a payment of {:.0} to {}.",
                                row.amount,
                                row.gateway.as_deref().unwrap_or("?")
                            )
                        } else {
                            row.reason
                        },
                        connector: row.gateway,
                        amount: Some(row.amount),
                    });
                }
            }
            Err(error) => logger::error!(
                tag = "volume_commitment",
                merchant_id = merchant_id,
                "could not read steer audit events from clickhouse: {error:?}"
            ),
        }

        events.sort_by_key(|e| std::cmp::Reverse(e.at_epoch_ms));
        events.truncate(limit as usize);
        events
    }
}

/// One stored forecast event into audit entries: the run itself, then one entry per elimination.
fn forecast_row_to_events(row: &ForecastEventRow) -> Vec<AuditEvent> {
    let details: serde_json::Value = row
        .details
        .as_deref()
        .and_then(|d| serde_json::from_str(d).ok())
        .unwrap_or_default();
    let tracked = details["tracked"].as_u64().unwrap_or(0);
    let steering: Vec<&str> = details["steering"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();

    let run_id = details["runId"].as_str().map(str::to_string);

    let mut events = vec![AuditEvent {
        at_epoch_ms: row.created_at_ms,
        kind: AuditKind::Forecast,
        run_id: run_id.clone(),
        connector: None,
        message: if steering.is_empty() {
            format!("Forecast: {tracked} commitment(s) tracked, all on pace — nothing to steer.")
        } else {
            format!(
                "Forecast: {tracked} commitment(s) tracked; {} behind pace and steering ({}).",
                steering.len(),
                steering.join(", ")
            )
        },
        amount: None,
    }];

    for dropped in details["dropped"].as_array().into_iter().flatten() {
        let connector = dropped["connector"].as_str().unwrap_or("?");
        events.push(AuditEvent {
            at_epoch_ms: row.created_at_ms,
            kind: AuditKind::Eliminated,
            run_id: run_id.clone(),
            connector: Some(connector.to_string()),
            message: format!(
                "{connector} eliminated: {}",
                dropped["reason"].as_str().unwrap_or("no reason recorded")
            ),
            amount: None,
        });
    }
    events
}

/// One row of the forecast audit query.
#[derive(Debug, Deserialize, Row)]
struct ForecastEventRow {
    created_at_ms: i64,
    details: Option<String>,
}

/// One row of the steer audit query.
#[derive(Debug, Deserialize, Row)]
struct SteerEventRow {
    created_at_ms: i64,
    gateway: Option<String>,
    amount: f64,
    reason: String,
    run_id: String,
}

/// One row of the daily series query.
#[derive(Debug, Deserialize, Row)]
struct DayVolumeRow {
    gateway: Option<String>,
    day_index: i64,
    total: f64,
    steered: f64,
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
}

/// Contract days of history the query actually covered, flooring at one: sub-day rates are noise,
/// and the floor errs toward steering slightly early (tolerance-bounded) over missing the goal.
fn elapsed_window_days(now_ms: i64, window_start_ms: i64, pace_days: i64, day_ms: i64) -> f64 {
    ((now_ms.saturating_sub(window_start_ms)) as f64 / day_ms.max(1) as f64)
        .clamp(1.0, pace_days as f64)
}

/// Serves volume handed to it up front; stands in for ClickHouse when analytics is off.
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
    ) -> MeasuredVolume {
        self.merchants.get(merchant_id).cloned().unwrap_or_default()
    }

    async fn daily_series(&self, _merchant_id: &str, _commitments: &[Commitment]) -> Vec<DayVolume> {
        Vec::new()
    }

    async fn audit_events(&self, _merchant_id: &str, _limit: u64) -> Vec<AuditEvent> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY_MS: i64 = 86_400_000;

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

    /// Compressed days divide by the compressed day length, not the calendar one.
    #[test]
    fn a_simulated_window_divides_by_virtual_days() {
        let day_ms = 120_000; // 120s contract days
        assert!((elapsed_window_days(3 * day_ms, 0, 7, day_ms) - 3.0).abs() < 1e-9);
    }

}
