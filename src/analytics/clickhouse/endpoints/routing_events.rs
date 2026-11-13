use std::collections::{BTreeMap, HashMap, HashSet};

use crate::analytics::models::{
    AnalyticsRange, RoutingEvent, RoutingEventType, RoutingEventsQuery, RoutingEventsResponse,
    ROUTING_EVENTS_SECOND_BUCKET_MAX_WINDOW_MS, ROUTING_EVENTS_SECOND_BUCKET_MS,
    ROUTING_EVENTS_STALENESS_BUCKETS, ROUTING_EVENTS_STALENESS_FLOOR_MS,
};
use crate::analytics::service::now_ms;
use crate::error::ApiError;

use super::super::metrics;
use super::super::metrics::calibration_events::CalibrationEventRow;
use super::super::metrics::score_bucket_series::ScoreBucketPoint;

pub async fn load(
    client: &clickhouse::Client,
    query: &RoutingEventsQuery,
) -> Result<RoutingEventsResponse, ApiError> {
    let (start_ms, end_ms) = effective_window_bounds(query);
    // Fetch one staleness horizon before the window so the leader baseline is
    // known and the first in-window bucket does not fire spurious events.
    let lookback_start_ms = start_ms.saturating_sub(staleness_horizon_ms(query.bucket_ms));
    let points =
        metrics::score_bucket_series::load(client, query, lookback_start_ms, end_ms).await?;
    let mut events = detect_routing_events(&points, query, start_ms);

    // Autopilot calibration retunes are real emitted domain events (not derivable from the
    // score series), so replay them from their own store and merge into the same feed.
    let calibration_rows =
        metrics::calibration_events::load(client, query, start_ms, end_ms).await?;
    events.extend(calibration_events_to_routing_events(
        &query.merchant_id,
        calibration_rows,
    ));
    // Re-establish the newest-first, deterministic order across both sources and re-cap:
    // `detect_routing_events` already sorted/truncated its own output, so the calibration
    // rows we appended need folding back in.
    events.sort_by(|a, b| b.bucket_ms.cmp(&a.bucket_ms).then_with(|| a.id.cmp(&b.id)));
    events.truncate(query.limit);

    Ok(RoutingEventsResponse {
        merchant_id: query.merchant_id.clone(),
        range: format_range(query),
        events,
        generated_at_ms: now_ms(),
    })
}

fn effective_window_bounds(query: &RoutingEventsQuery) -> (i64, i64) {
    let now = now_ms();
    let end_ms = query.end_ms.unwrap_or(now).min(now);
    // Snapshot rows are per-transaction; cap the window to bound scan volume —
    // one week normally, one hour in second-granularity mode.
    let max_window_ms = if query.bucket_ms == ROUTING_EVENTS_SECOND_BUCKET_MS {
        ROUTING_EVENTS_SECOND_BUCKET_MAX_WINDOW_MS
    } else {
        AnalyticsRange::W1.window_ms()
    };
    let min_start_ms = end_ms.saturating_sub(max_window_ms);
    let start_ms = query
        .start_ms
        .filter(|start_ms| *start_ms >= 0 && *start_ms < end_ms)
        .unwrap_or_else(|| end_ms.saturating_sub(query.range.window_ms()))
        .max(min_start_ms);
    (start_ms, end_ms)
}

fn staleness_horizon_ms(bucket_ms: i64) -> i64 {
    (ROUTING_EVENTS_STALENESS_BUCKETS * bucket_ms).max(ROUTING_EVENTS_STALENESS_FLOOR_MS)
}

fn format_range(query: &RoutingEventsQuery) -> String {
    if query.start_ms.is_some() && query.end_ms.is_some() {
        return "custom".to_string();
    }

    match query.range {
        AnalyticsRange::M15 => "15m".to_string(),
        AnalyticsRange::H1 => "1h".to_string(),
        AnalyticsRange::H12 => "12h".to_string(),
        AnalyticsRange::D1 => "1d".to_string(),
        AnalyticsRange::W1 => "1w".to_string(),
    }
}

#[derive(Debug, Clone)]
struct GatewayState {
    score: f64,
    transaction_count: Option<i64>,
    last_seen_bucket_ms: i64,
}

type DimensionKey = (Option<String>, Option<String>);

fn dimension_part(value: &Option<String>) -> &str {
    value.as_deref().unwrap_or("-")
}

fn event_id(
    event_type: RoutingEventType,
    merchant_id: &str,
    dimension: &DimensionKey,
    bucket_ms: i64,
    previous_gateway: Option<&str>,
    gateway: &str,
) -> String {
    let transition = match previous_gateway {
        Some(previous) => format!("{previous}>{gateway}"),
        None => gateway.to_string(),
    };
    format!(
        "{}:{}:{}:{}:{}:{}",
        event_type.as_str(),
        merchant_id,
        dimension_part(&dimension.0),
        dimension_part(&dimension.1),
        bucket_ms,
        transition
    )
}

/// Deterministic ID for a calibration event, stable across polls so the client dedupes on
/// it. Keyed on the full cluster grain (finer than the pmt/pm the derived events use) plus
/// the emit timestamp, so every distinct retune surfaces exactly once.
fn calibration_event_id(merchant_id: &str, row: &CalibrationEventRow) -> String {
    format!(
        "{}:{}:{}:{}:{}:{}:{}:{}:{}",
        RoutingEventType::CalibrationApplied.as_str(),
        merchant_id,
        row.payment_method_type.as_deref().unwrap_or("-"),
        row.payment_method.as_deref().unwrap_or("-"),
        row.card_network.as_deref().unwrap_or("-"),
        row.currency.as_deref().unwrap_or("-"),
        row.country.as_deref().unwrap_or("-"),
        row.auth_type.as_deref().unwrap_or("-"),
        row.created_at_ms,
    )
}

/// Map raw calibration domain-event rows into `calibration_applied` routing events. The
/// score/leader fields are unused for this type; the retune knobs ride the dedicated
/// `bucket_size`/`hedging_percent` fields, and `bucket_ms` carries the emit time so the
/// event slots into the shared newest-first timeline.
fn calibration_events_to_routing_events(
    merchant_id: &str,
    rows: Vec<CalibrationEventRow>,
) -> Vec<RoutingEvent> {
    rows.into_iter()
        .map(|row| RoutingEvent {
            id: calibration_event_id(merchant_id, &row),
            event_type: RoutingEventType::CalibrationApplied,
            merchant_id: merchant_id.to_string(),
            payment_method_type: row.payment_method_type.clone(),
            payment_method: row.payment_method.clone(),
            bucket_ms: row.created_at_ms,
            gateway: String::new(),
            previous_gateway: None,
            score: None,
            previous_score: None,
            transaction_count: None,
            bucket_size: row.new_bucket.map(|b| b as i32),
            previous_bucket_size: row.previous_bucket.map(|b| b.round() as i32),
            hedging_percent: row.new_hedge,
            previous_hedging_percent: row.previous_hedge,
            card_network: row.card_network,
            currency: row.currency,
            country: row.country,
            auth_type: row.auth_type,
        })
        .collect()
}

/// Walk bucketed score points per dimension, carrying the last known score per
/// gateway forward across sparse buckets, and emit routing events. Pure and
/// deterministic so event IDs are stable across polls.
fn detect_routing_events(
    points: &[ScoreBucketPoint],
    query: &RoutingEventsQuery,
    window_start_ms: i64,
) -> Vec<RoutingEvent> {
    let staleness_ms = staleness_horizon_ms(query.bucket_ms);

    // dimension -> bucket -> rows, ordered for deterministic iteration.
    let mut dimensions: BTreeMap<DimensionKey, BTreeMap<i64, Vec<&ScoreBucketPoint>>> =
        BTreeMap::new();
    for point in points {
        if point.gateway.is_none() || point.score_value.is_none() {
            continue;
        }
        dimensions
            .entry((
                point.payment_method_type.clone(),
                point.payment_method.clone(),
            ))
            .or_default()
            .entry(point.bucket_ms)
            .or_default()
            .push(point);
    }

    let mut events = Vec::new();

    for (dimension, buckets) in &dimensions {
        let mut state: HashMap<String, GatewayState> = HashMap::new();
        // Gateways currently inside the leader's auth band; membership is
        // edge-triggered so an entry event fires only on a fresh crossing.
        let mut in_auth_band: HashSet<String> = HashSet::new();
        let mut previous_leader: Option<String> = None;
        // The raw best gateway of the immediately preceding bucket, used to seed a
        // demoted leader into the band silently when it stays within tolerance.
        let mut prior_bucket_leader: Option<String> = None;
        let mut is_first_bucket = true;

        for (&bucket_ms, rows) in buckets {
            for row in rows {
                let gateway = row.gateway.clone().unwrap_or_default();
                let score = row.score_value.unwrap_or_default();
                state.insert(
                    gateway,
                    GatewayState {
                        score,
                        transaction_count: row.transaction_count,
                        last_seen_bucket_ms: bucket_ms,
                    },
                );
            }

            // Leader = best eligible score; ties break to the lexicographically
            // smaller gateway so the outcome is deterministic.
            let leader = state
                .iter()
                .filter(|(_, gateway_state)| {
                    gateway_state.transaction_count.unwrap_or(0) >= query.min_transaction_count
                        && bucket_ms - gateway_state.last_seen_bucket_ms <= staleness_ms
                })
                .max_by(|(name_a, state_a), (name_b, state_b)| {
                    state_a
                        .score
                        .partial_cmp(&state_b.score)
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then_with(|| name_b.cmp(name_a))
                })
                .map(|(name, gateway_state)| (name.clone(), gateway_state.score));

            if let Some((leader_name, leader_score)) = leader {
                // Snapshot the prior bucket's leader before advancing it; the band
                // detector uses it to recognise a leader that just got overtaken.
                let former_leader = prior_bucket_leader.clone();
                prior_bucket_leader = Some(leader_name.clone());

                // Auth-band detection runs only when multi-objective routing is on;
                // otherwise the band is meaningless and we emit leader changes alone.
                if query.auth_band.is_on() {
                    emit_auth_band_events(
                        &mut events,
                        &mut in_auth_band,
                        &state,
                        &leader_name,
                        leader_score,
                        bucket_ms,
                        dimension,
                        query,
                        staleness_ms,
                        is_first_bucket,
                        former_leader.as_deref(),
                    );
                }

                match &previous_leader {
                    Some(previous) if *previous != leader_name => {
                        let previous_state = state.get(previous);
                        let previous_still_eligible = previous_state.is_some_and(|gateway_state| {
                            gateway_state.transaction_count.unwrap_or(0)
                                >= query.min_transaction_count
                                && bucket_ms - gateway_state.last_seen_bucket_ms <= staleness_ms
                        });
                        let previous_score =
                            previous_state.map(|gateway_state| gateway_state.score);
                        // A still-eligible incumbent must be beaten by a real
                        // margin; an aged-out incumbent loses the crown outright.
                        let is_decisive_flip = !previous_still_eligible
                            || previous_score
                                .is_none_or(|score| leader_score > score + query.min_score_delta);

                        if is_decisive_flip {
                            events.push(RoutingEvent {
                                id: event_id(
                                    RoutingEventType::LeaderChanged,
                                    &query.merchant_id,
                                    dimension,
                                    bucket_ms,
                                    Some(previous),
                                    &leader_name,
                                ),
                                event_type: RoutingEventType::LeaderChanged,
                                merchant_id: query.merchant_id.clone(),
                                payment_method_type: dimension.0.clone(),
                                payment_method: dimension.1.clone(),
                                bucket_ms,
                                gateway: leader_name.clone(),
                                previous_gateway: Some(previous.clone()),
                                score: Some(leader_score),
                                previous_score,
                                transaction_count: state
                                    .get(&leader_name)
                                    .and_then(|gateway_state| gateway_state.transaction_count),
                                bucket_size: None,
                                previous_bucket_size: None,
                                hedging_percent: None,
                                previous_hedging_percent: None,
                                card_network: None,
                                currency: None,
                                country: None,
                                auth_type: None,
                            });
                            previous_leader = Some(leader_name);
                        }
                    }
                    Some(_) => {}
                    None => {
                        previous_leader = Some(leader_name);
                    }
                }
            }

            is_first_bucket = false;
        }
    }

    // Drop events from the lookback prefix; newest first; deterministic order.
    events.retain(|event| event.bucket_ms >= window_start_ms);
    events.sort_by(|a, b| b.bucket_ms.cmp(&a.bucket_ms).then_with(|| a.id.cmp(&b.id)));
    events.truncate(query.limit);
    events
}

/// Emit auth-band crossing events for every eligible non-leader gateway.
/// `GatewayEnteredAuthBand` fires when a gateway's score first rises into the
/// leader's band (`>= query.auth_band.band_floor(leader, candidate)`);
/// `GatewayExitedAuthBand` fires when it later drops below that floor or ages out.
/// The floor is **per candidate** — under `NoiseFloor` it widens with each gateway's
/// own SR variance, matching the live decider's gate. Membership is edge-triggered via
/// `in_auth_band`, so each crossing fires exactly once. The leader is the band
/// reference and never a member — a member promoted to leader exits silently.
/// A leader that gets overtaken but stays within its band never actually left it
/// (only the reference moved), so it is re-seeded as a member silently via
/// `former_leader` and does not fire a spurious entry.
#[allow(clippy::too_many_arguments)]
fn emit_auth_band_events(
    events: &mut Vec<RoutingEvent>,
    in_auth_band: &mut HashSet<String>,
    state: &HashMap<String, GatewayState>,
    leader_name: &str,
    leader_score: f64,
    bucket_ms: i64,
    dimension: &DimensionKey,
    query: &RoutingEventsQuery,
    staleness_ms: i64,
    is_first_bucket: bool,
    former_leader: Option<&str>,
) {
    // The leader is the reference, never a member.
    in_auth_band.remove(leader_name);

    // When leadership just changed, the outgoing leader moves from "the reference"
    // to an ordinary candidate. If it is still eligible and within tolerance of the
    // new leader it was inside the band all along — the reference simply moved — so
    // seed it as a member silently. Without this, the next bucket would report it as
    // freshly entering a band it never left (e.g. two PSPs trading the lead while
    // both stay within tolerance would each re-fire an entry on every flip-back).
    // Hysteresis half-width is sized per gateway to the noise of *its* gap with
    // the leader, so the enter/exit deadband self-tunes to each score's variance.
    if let Some(former) = former_leader.filter(|former| *former != leader_name) {
        if let Some(former_state) = state.get(former) {
            let eligible = former_state.transaction_count.unwrap_or(0)
                >= query.min_transaction_count
                && bucket_ms - former_state.last_seen_bucket_ms <= staleness_ms;
            // A demoted leader was already inside the band, so seed it as a member
            // if it still holds at/above its own band floor (same threshold a member
            // is held to before exiting).
            if eligible
                && former_state.score
                    >= query.auth_band.band_floor(leader_score, former_state.score)
            {
                in_auth_band.insert(former.to_string());
            }
        }
    }

    for (gateway, gateway_state) in state {
        if gateway == leader_name {
            continue;
        }
        let eligible = gateway_state.transaction_count.unwrap_or(0) >= query.min_transaction_count
            && bucket_ms - gateway_state.last_seen_bucket_ms <= staleness_ms;
        let was_in_band = in_auth_band.contains(gateway);
        // Instantaneous, single-threshold membership: a gateway is in the cost band
        // exactly when its score is within tolerance of the leader (>= band_floor),
        // so both "entered" and "exited" fire the instant the score crosses the edge
        // — no hysteresis/deadband. Flap protection instead lives downstream (the UI
        // collapses any rapid re-crossings into one "contesting ×N" row), and the
        // deterministic outcome scheduler keeps scores from jittering at the edge.
        let band_floor = query
            .auth_band
            .band_floor(leader_score, gateway_state.score);
        let in_band_now = eligible && gateway_state.score >= band_floor;

        if in_band_now && !was_in_band {
            in_auth_band.insert(gateway.clone());
            // Seed membership silently at the window start so an already-tight
            // field does not fire a storm of entries on the first bucket.
            if !is_first_bucket {
                events.push(auth_band_event(
                    RoutingEventType::GatewayEnteredAuthBand,
                    query,
                    dimension,
                    bucket_ms,
                    leader_name,
                    leader_score,
                    gateway,
                    gateway_state,
                ));
            }
        } else if !in_band_now && was_in_band {
            in_auth_band.remove(gateway);
            events.push(auth_band_event(
                RoutingEventType::GatewayExitedAuthBand,
                query,
                dimension,
                bucket_ms,
                leader_name,
                leader_score,
                gateway,
                gateway_state,
            ));
        }
    }
}

/// Build an auth-band entry/exit event. The gateway's own score and txn count
/// describe the crossing; `previous_gateway`/`previous_score` carry the leader it
/// is measured against, so the band reference travels with the event.
#[allow(clippy::too_many_arguments)]
fn auth_band_event(
    event_type: RoutingEventType,
    query: &RoutingEventsQuery,
    dimension: &DimensionKey,
    bucket_ms: i64,
    leader_name: &str,
    leader_score: f64,
    gateway: &str,
    gateway_state: &GatewayState,
) -> RoutingEvent {
    RoutingEvent {
        id: event_id(
            event_type,
            &query.merchant_id,
            dimension,
            bucket_ms,
            Some(leader_name),
            gateway,
        ),
        event_type,
        merchant_id: query.merchant_id.clone(),
        payment_method_type: dimension.0.clone(),
        payment_method: dimension.1.clone(),
        bucket_ms,
        gateway: gateway.to_string(),
        previous_gateway: Some(leader_name.to_string()),
        score: Some(gateway_state.score),
        previous_score: Some(leader_score),
        transaction_count: gateway_state.transaction_count,
        bucket_size: None,
        previous_bucket_size: None,
        hedging_percent: None,
        previous_hedging_percent: None,
        card_network: None,
        currency: None,
        country: None,
        auth_type: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytics::models::{AuthBandSpec, RoutingEventsQuery, ROUTING_EVENTS_BUCKET_MS};

    const BUCKET: i64 = ROUTING_EVENTS_BUCKET_MS;

    fn calibration_row(created_at_ms: i64) -> CalibrationEventRow {
        CalibrationEventRow {
            created_at_ms,
            payment_method_type: Some("card".to_string()),
            payment_method: Some("credit".to_string()),
            card_network: Some("VISA".to_string()),
            currency: None,
            country: None,
            auth_type: None,
            new_hedge: Some(8.5),
            previous_hedge: Some(5.0),
            new_bucket: Some(250),
            previous_bucket: Some(100.0),
        }
    }

    #[test]
    fn calibration_rows_map_to_calibration_events_with_stable_ids() {
        let rows = vec![calibration_row(1_700_000_000_000)];
        let events = calibration_events_to_routing_events("m_123", rows);
        assert_eq!(events.len(), 1);
        let event = &events[0];
        assert_eq!(event.event_type, RoutingEventType::CalibrationApplied);
        assert_eq!(event.bucket_ms, 1_700_000_000_000);
        assert_eq!(event.bucket_size, Some(250));
        assert_eq!(event.previous_bucket_size, Some(100));
        assert_eq!(event.hedging_percent, Some(8.5));
        assert_eq!(event.previous_hedging_percent, Some(5.0));
        assert_eq!(event.card_network.as_deref(), Some("VISA"));
        // The score/leader fields are unused for calibration events.
        assert_eq!(event.gateway, "");
        assert!(event.score.is_none());
        // ID embeds the full cluster grain + emit time; "-" fills unset dims.
        assert_eq!(
            event.id,
            "calibration_applied:m_123:card:credit:VISA:-:-:-:1700000000000"
        );
        // Deterministic across polls.
        let rerun =
            calibration_events_to_routing_events("m_123", vec![calibration_row(1_700_000_000_000)]);
        assert_eq!(rerun[0].id, event.id);
    }

    fn query() -> RoutingEventsQuery {
        RoutingEventsQuery {
            merchant_id: "m_123".to_string(),
            range: AnalyticsRange::H12,
            start_ms: None,
            end_ms: None,
            payment_method_type: None,
            payment_method: None,
            min_transaction_count: 10,
            min_score_delta: 0.5,
            // Test scores are on a 0..100 scale, so a fixed 5-point band is workable.
            // Fixed(..) = multi-objective on; Off disables auth-band detection.
            auth_band: AuthBandSpec::Fixed(5.0),
            limit: 50,
            bucket_ms: ROUTING_EVENTS_BUCKET_MS,
        }
    }

    fn point(
        bucket_ms: i64,
        gateway: &str,
        score: f64,
        transaction_count: i64,
    ) -> ScoreBucketPoint {
        ScoreBucketPoint {
            bucket_ms,
            merchant_id: Some("m_123".to_string()),
            payment_method_type: Some("card".to_string()),
            payment_method: Some("credit".to_string()),
            gateway: Some(gateway.to_string()),
            score_value: Some(score),
            transaction_count: Some(transaction_count),
        }
    }

    #[test]
    fn leader_flip_emits_event_with_stable_id() {
        let points = vec![
            point(0, "adyen", 89.0, 100),
            point(0, "stripe", 87.0, 100),
            point(BUCKET, "stripe", 90.0, 100),
        ];
        let events = detect_routing_events(&points, &query(), 0);
        let flips: Vec<_> = events
            .iter()
            .filter(|event| event.event_type == RoutingEventType::LeaderChanged)
            .collect();
        assert_eq!(flips.len(), 1);
        let flip = flips[0];
        assert_eq!(flip.gateway, "stripe");
        assert_eq!(flip.previous_gateway.as_deref(), Some("adyen"));
        assert_eq!(flip.score, Some(90.0));
        assert_eq!(flip.previous_score, Some(89.0));
        assert_eq!(
            flip.id,
            format!("leader_changed:m_123:card:credit:{BUCKET}:adyen>stripe")
        );

        // Identical input yields identical IDs across polls.
        let rerun = detect_routing_events(&points, &query(), 0);
        assert_eq!(
            rerun
                .iter()
                .map(|event| event.id.clone())
                .collect::<Vec<_>>(),
            events
                .iter()
                .map(|event| event.id.clone())
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn flip_below_min_score_delta_is_suppressed() {
        let points = vec![
            point(0, "adyen", 89.0, 100),
            point(0, "stripe", 87.0, 100),
            point(BUCKET, "stripe", 89.3, 100),
        ];
        let events = detect_routing_events(&points, &query(), 0);
        assert!(events
            .iter()
            .all(|event| event.event_type != RoutingEventType::LeaderChanged));
    }

    // 0..1 scores with a fixed 5pp auth band, used by the band-floor edge tests.
    fn unit_scale_band_query() -> RoutingEventsQuery {
        RoutingEventsQuery {
            min_score_delta: 0.001,
            auth_band: AuthBandSpec::Fixed(0.05),
            ..query()
        }
    }

    // 0..1 scores with the derived per-candidate noise-floor band (z = 1), the live
    // decider's cost-independent gate. The band width follows each candidate's SR
    // variance and the SRV3 bucket size.
    fn noise_floor_query(bucket_size: i32) -> RoutingEventsQuery {
        RoutingEventsQuery {
            min_score_delta: 0.001,
            auth_band: AuthBandSpec::NoiseFloor {
                bucket_size,
                z: 1.0,
            },
            ..query()
        }
    }

    #[test]
    fn tied_scores_do_not_flip_leader() {
        let points = vec![
            point(0, "adyen", 89.0, 100),
            point(0, "stripe", 87.0, 100),
            point(BUCKET, "stripe", 89.0, 100),
        ];
        let events = detect_routing_events(&points, &query(), 0);
        assert!(events
            .iter()
            .all(|event| event.event_type != RoutingEventType::LeaderChanged));
    }

    #[test]
    fn carry_forward_keeps_absent_leader_on_top() {
        // adyen has no snapshot in bucket 1 but its carried score still leads.
        let points = vec![
            point(0, "adyen", 89.0, 100),
            point(0, "stripe", 80.0, 100),
            point(BUCKET, "stripe", 85.0, 100),
        ];
        let events = detect_routing_events(&points, &query(), 0);
        assert!(events
            .iter()
            .all(|event| event.event_type != RoutingEventType::LeaderChanged));
    }

    #[test]
    fn stale_leader_loses_crown_without_delta_requirement() {
        let stale_bucket = (ROUTING_EVENTS_STALENESS_BUCKETS + 1) * BUCKET;
        let mut points = vec![point(0, "adyen", 95.0, 100), point(0, "stripe", 80.0, 100)];
        // stripe keeps reporting, adyen goes silent past the staleness horizon.
        for bucket_index in 1..=(ROUTING_EVENTS_STALENESS_BUCKETS + 1) {
            points.push(point(bucket_index * BUCKET, "stripe", 80.0, 100));
        }
        let events = detect_routing_events(&points, &query(), 0);
        let flips: Vec<_> = events
            .iter()
            .filter(|event| event.event_type == RoutingEventType::LeaderChanged)
            .collect();
        assert_eq!(flips.len(), 1);
        assert_eq!(flips[0].gateway, "stripe");
        assert_eq!(flips[0].previous_gateway.as_deref(), Some("adyen"));
        assert_eq!(flips[0].bucket_ms, stale_bucket);
    }

    #[test]
    fn low_transaction_count_gateways_are_ignored() {
        let points = vec![
            point(0, "adyen", 89.0, 100),
            point(0, "stripe", 87.0, 100),
            // Beats adyen on score but is below the txn gate: no flip, no entry.
            point(BUCKET, "newpay", 99.0, 3),
        ];
        let events = detect_routing_events(&points, &query(), 0);
        assert!(events.is_empty());
    }

    fn entered_band_events(events: &[RoutingEvent]) -> Vec<&RoutingEvent> {
        events
            .iter()
            .filter(|event| event.event_type == RoutingEventType::GatewayEnteredAuthBand)
            .collect()
    }

    fn exited_band_events(events: &[RoutingEvent]) -> Vec<&RoutingEvent> {
        events
            .iter()
            .filter(|event| event.event_type == RoutingEventType::GatewayExitedAuthBand)
            .collect()
    }

    #[test]
    fn gateway_crossing_into_auth_band_emits_once() {
        // adyen leads at 89, so the band floor is 84. stripe starts below it and
        // rises into the band, then holds — one entry, no re-fire.
        let points = vec![
            point(0, "adyen", 89.0, 100),
            point(0, "stripe", 80.0, 100),
            point(BUCKET, "adyen", 89.0, 100),
            point(BUCKET, "stripe", 86.0, 100),
            point(2 * BUCKET, "adyen", 89.0, 100),
            point(2 * BUCKET, "stripe", 87.0, 100),
        ];
        let events = detect_routing_events(&points, &query(), 0);
        let band = entered_band_events(&events);
        assert_eq!(band.len(), 1);
        assert_eq!(band[0].gateway, "stripe");
        assert_eq!(band[0].previous_gateway.as_deref(), Some("adyen"));
        assert_eq!(band[0].score, Some(86.0));
        assert_eq!(band[0].previous_score, Some(89.0));
        assert_eq!(band[0].bucket_ms, BUCKET);
        assert_eq!(
            band[0].id,
            format!("gateway_entered_auth_band:m_123:card:credit:{BUCKET}:adyen>stripe")
        );
        // stripe never falls back out, so no exit fires.
        assert!(exited_band_events(&events).is_empty());
    }

    #[test]
    fn gateway_leaving_and_reentering_band_emits_exit_then_entry() {
        let points = vec![
            // Seeds inside the band at the window start: silent.
            point(0, "adyen", 89.0, 100),
            point(0, "stripe", 86.0, 100),
            // Drops out of the band: exit event.
            point(BUCKET, "adyen", 89.0, 100),
            point(BUCKET, "stripe", 80.0, 100),
            // Crosses back in: fresh entry event.
            point(2 * BUCKET, "adyen", 89.0, 100),
            point(2 * BUCKET, "stripe", 86.0, 100),
        ];
        let events = detect_routing_events(&points, &query(), 0);

        let exited = exited_band_events(&events);
        assert_eq!(exited.len(), 1);
        assert_eq!(exited[0].gateway, "stripe");
        assert_eq!(exited[0].bucket_ms, BUCKET);

        let entered = entered_band_events(&events);
        assert_eq!(entered.len(), 1);
        assert_eq!(entered[0].gateway, "stripe");
        assert_eq!(entered[0].bucket_ms, 2 * BUCKET);
    }

    #[test]
    fn already_tight_field_does_not_storm_at_window_start() {
        // stripe is already inside adyen's band on the first bucket: seeded, not fired.
        let points = vec![point(0, "adyen", 89.0, 100), point(0, "stripe", 86.0, 100)];
        let events = detect_routing_events(&points, &query(), 0);
        assert!(entered_band_events(&events).is_empty());
        assert!(exited_band_events(&events).is_empty());
    }

    #[test]
    fn former_leader_within_tolerance_does_not_re_enter_band_on_flip() {
        // adyen leads at 95 (band floor 90), stripe is out at 80. stripe then
        // overtakes at 99 (floor 94); adyen's carried 95 still lands inside the new
        // band. adyen was the reference the prior bucket and never dropped below
        // tolerance, so the band's reference simply moved with the flip: it must not
        // report a fresh entry, and it was never a member so it does not exit.
        let points = vec![
            point(0, "adyen", 95.0, 100),
            point(0, "stripe", 80.0, 100),
            point(BUCKET, "stripe", 99.0, 100),
        ];
        let events = detect_routing_events(&points, &query(), 0);
        assert!(entered_band_events(&events).is_empty());
        assert!(exited_band_events(&events).is_empty());
        // The leadership flip itself still surfaces.
        assert!(events.iter().any(|event| {
            event.event_type == RoutingEventType::LeaderChanged
                && event.gateway == "stripe"
                && event.previous_gateway.as_deref() == Some("adyen")
        }));
    }

    #[test]
    fn leaders_trading_within_tolerance_do_not_re_enter_band() {
        // The reported bug: two PSPs swap the lead while both stay within tolerance.
        // Each flip-back used to re-fire an entry for the gateway dropping to #2,
        // even though it never left the band. stripe enters once on the way in; the
        // subsequent lead swaps emit leader changes only, no further band entries.
        let points = vec![
            point(0, "adyen", 90.0, 100),
            point(0, "stripe", 80.0, 100), // out of band (floor 85): seeded
            point(BUCKET, "adyen", 90.0, 100),
            point(BUCKET, "stripe", 88.0, 100), // crosses in → one entered
            point(2 * BUCKET, "stripe", 91.0, 100), // stripe takes the lead
            point(2 * BUCKET, "adyen", 90.0, 100), // adyen drops to #2, still in band
            point(3 * BUCKET, "adyen", 92.0, 100), // adyen retakes the lead
            point(3 * BUCKET, "stripe", 91.0, 100), // stripe drops to #2, still in band
        ];
        let events = detect_routing_events(&points, &query(), 0);

        let entered = entered_band_events(&events);
        assert_eq!(entered.len(), 1, "only stripe's initial crossing enters");
        assert_eq!(entered[0].gateway, "stripe");
        assert_eq!(entered[0].bucket_ms, BUCKET);
        assert!(exited_band_events(&events).is_empty());

        let flips = events
            .iter()
            .filter(|event| event.event_type == RoutingEventType::LeaderChanged)
            .count();
        assert_eq!(flips, 2, "both lead swaps still surface as leader changes");
    }

    #[test]
    fn out_of_band_gateway_never_enters() {
        // stripe trails far behind adyen's band the whole time.
        let points = vec![
            point(0, "adyen", 89.0, 100),
            point(0, "stripe", 60.0, 100),
            point(BUCKET, "adyen", 89.0, 100),
            point(BUCKET, "stripe", 70.0, 100),
        ];
        let events = detect_routing_events(&points, &query(), 0);
        assert!(entered_band_events(&events).is_empty());
        assert!(exited_band_events(&events).is_empty());
    }

    #[test]
    fn low_transaction_count_gateway_does_not_enter_band() {
        // newpay sits inside the band but stays below the txn gate: no entry.
        let points = vec![
            point(0, "adyen", 89.0, 100),
            point(0, "stripe", 80.0, 100),
            point(BUCKET, "adyen", 89.0, 100),
            point(BUCKET, "newpay", 88.0, 3),
        ];
        let events = detect_routing_events(&points, &query(), 0);
        assert!(entered_band_events(&events).is_empty());
    }

    #[test]
    fn gateway_dropping_out_of_band_emits_exit() {
        let points = vec![
            point(0, "adyen", 89.0, 100),
            // stripe seeded inside the band (floor 84).
            point(0, "stripe", 86.0, 100),
            point(BUCKET, "adyen", 89.0, 100),
            // stripe falls below the floor: a single exit event.
            point(BUCKET, "stripe", 80.0, 100),
        ];
        let events = detect_routing_events(&points, &query(), 0);
        let exited = exited_band_events(&events);
        assert_eq!(exited.len(), 1);
        assert_eq!(exited[0].gateway, "stripe");
        assert_eq!(exited[0].previous_gateway.as_deref(), Some("adyen"));
        assert_eq!(exited[0].score, Some(80.0));
        assert_eq!(exited[0].previous_score, Some(89.0));
        assert_eq!(exited[0].bucket_ms, BUCKET);
        assert_eq!(
            exited[0].id,
            format!("gateway_exited_auth_band:m_123:card:credit:{BUCKET}:adyen>stripe")
        );
    }

    #[test]
    fn band_member_promoted_to_leader_does_not_emit_exit() {
        let points = vec![
            point(0, "adyen", 89.0, 100),
            // stripe seeded inside the band, then overtakes adyen as leader.
            point(0, "stripe", 86.0, 100),
            point(BUCKET, "stripe", 95.0, 100),
        ];
        let events = detect_routing_events(&points, &query(), 0);
        assert!(exited_band_events(&events).is_empty());
    }

    #[test]
    fn member_holding_in_band_does_not_re_fire() {
        // Single instantaneous threshold at the band floor (no hysteresis). adyen
        // leads 0.95, so floor (tolerance 0.05) is 0.90. stripe crosses in once, then
        // wobbles while staying above the floor — membership is edge-triggered, so it
        // emits nothing until it genuinely drops below the floor, where it exits once.
        let points = vec![
            point(0, "adyen", 0.95, 100),
            point(0, "stripe", 0.88, 100), // below floor: out of band
            point(BUCKET, "adyen", 0.95, 100),
            point(BUCKET, "stripe", 0.94, 100), // crosses floor: single entry
            point(2 * BUCKET, "adyen", 0.95, 100),
            point(2 * BUCKET, "stripe", 0.92, 100), // stays above floor: hold
            point(3 * BUCKET, "adyen", 0.95, 100),
            point(3 * BUCKET, "stripe", 0.91, 100), // stays above floor: hold
            point(4 * BUCKET, "adyen", 0.95, 100),
            point(4 * BUCKET, "stripe", 0.93, 100), // stays above floor: hold
            point(5 * BUCKET, "adyen", 0.95, 100),
            point(5 * BUCKET, "stripe", 0.85, 100), // drops below floor 0.90: prompt exit
        ];
        let events = detect_routing_events(&points, &unit_scale_band_query(), 0);

        // Exactly one entry (the genuine crossing in) and one exit (the genuine drop
        // below the floor) — the in-band wobble produces nothing.
        let entered = entered_band_events(&events);
        assert_eq!(entered.len(), 1, "wobble must not re-fire entries");
        assert_eq!(entered[0].bucket_ms, BUCKET);

        let exited = exited_band_events(&events);
        assert_eq!(exited.len(), 1, "wobble must not re-fire exits");
        assert_eq!(exited[0].bucket_ms, 5 * BUCKET);
    }

    #[test]
    fn exit_fires_promptly_at_the_band_floor() {
        // Asymmetric hysteresis: a member that slips just below the floor exits on
        // that bucket, without waiting for an extra deadband-sized drop. adyen leads
        // 0.95 (floor 0.90); stripe is seeded in-band then dips to 0.895.
        let points = vec![
            point(0, "adyen", 0.95, 100),
            point(0, "stripe", 0.94, 100), // seeded in band (clears enter floor ~0.93)
            point(BUCKET, "adyen", 0.95, 100),
            point(BUCKET, "stripe", 0.895, 100), // just below floor → exits now
        ];
        let events = detect_routing_events(&points, &unit_scale_band_query(), 0);
        let exited = exited_band_events(&events);
        assert_eq!(exited.len(), 1);
        assert_eq!(exited[0].bucket_ms, BUCKET);
    }

    #[test]
    fn psp_enters_once_holds_silently_then_exits_once() {
        // The full contract: a PSP that enters the band fires one entered event,
        // stays silent for every bucket it remains in the band, then fires exactly
        // one exited event when it drops below the floor. adyen leads at 89 so the
        // band floor is 84 throughout.
        let points = vec![
            point(0, "adyen", 89.0, 100),
            point(0, "stripe", 80.0, 100), // below floor: out of band (seeded)
            point(BUCKET, "adyen", 89.0, 100),
            point(BUCKET, "stripe", 86.0, 100), // crosses in → one entered
            point(2 * BUCKET, "adyen", 89.0, 100),
            point(2 * BUCKET, "stripe", 87.0, 100), // still in band → silent
            point(3 * BUCKET, "adyen", 89.0, 100),
            point(3 * BUCKET, "stripe", 88.0, 100), // still in band → silent
            point(4 * BUCKET, "adyen", 89.0, 100),
            point(4 * BUCKET, "stripe", 70.0, 100), // drops out → one exited
        ];
        let events = detect_routing_events(&points, &query(), 0);

        let entered = entered_band_events(&events);
        assert_eq!(entered.len(), 1, "exactly one entered event for the stint");
        assert_eq!(entered[0].gateway, "stripe");
        assert_eq!(entered[0].bucket_ms, BUCKET);

        let exited = exited_band_events(&events);
        assert_eq!(exited.len(), 1, "exactly one exited event for the stint");
        assert_eq!(exited[0].gateway, "stripe");
        assert_eq!(exited[0].bucket_ms, 4 * BUCKET);
    }

    #[test]
    fn multi_objective_off_suppresses_band_events_but_keeps_leader_changes() {
        // AuthBandSpec::Off models a merchant with multi-objective routing off:
        // the band is meaningless, so only the leader flip surfaces.
        let mut mo_off = query();
        mo_off.auth_band = AuthBandSpec::Off;
        let points = vec![
            point(0, "adyen", 89.0, 100),
            point(0, "stripe", 80.0, 100),
            // stripe would enter the band here and overtake at the next bucket.
            point(BUCKET, "adyen", 89.0, 100),
            point(BUCKET, "stripe", 86.0, 100),
            point(2 * BUCKET, "stripe", 95.0, 100),
        ];
        let events = detect_routing_events(&points, &mo_off, 0);
        assert!(entered_band_events(&events).is_empty());
        assert!(exited_band_events(&events).is_empty());
        // The SR leader flip is independent of multi-objective and still fires.
        assert!(events
            .iter()
            .any(|event| event.event_type == RoutingEventType::LeaderChanged));
    }

    #[test]
    fn noise_floor_band_rejects_gateway_beyond_its_derived_floor() {
        // Leader 0.90, B=125 → stripe's per-candidate floor ≈ 4.17pp. stripe only
        // climbs to 0.85 (5pp back), so it never enters the band.
        let points = vec![
            point(0, "adyen", 0.90, 100),
            point(0, "stripe", 0.80, 100),
            point(BUCKET, "adyen", 0.90, 100),
            point(BUCKET, "stripe", 0.85, 100),
        ];
        let events = detect_routing_events(&points, &noise_floor_query(125), 0);
        assert!(
            entered_band_events(&events).is_empty(),
            "5pp gap exceeds the ~4.2pp noise floor at B=125; stripe stays out"
        );
    }

    #[test]
    fn larger_bucket_tightens_noise_floor_band() {
        // A 0.88 candidate 2pp behind a 0.90 leader is inside the band at B=125
        // (floor ≈ 3.96pp) but outside it at B=2000 (floor ≈ 1.0pp): the noise floor
        // scales as 1/√B, exactly like the live decider. This is the lever that
        // actually tightens the band — not margin (see scratch/deriving-routing-config).
        let points = vec![
            point(0, "adyen", 0.90, 100),
            point(0, "stripe", 0.80, 100),
            point(BUCKET, "adyen", 0.90, 100),
            point(BUCKET, "stripe", 0.88, 100),
        ];
        let small_bucket = detect_routing_events(&points, &noise_floor_query(125), 0);
        let entered = entered_band_events(&small_bucket);
        assert_eq!(entered.len(), 1, "enters the band at B=125");
        assert_eq!(entered[0].gateway, "stripe");

        let large_bucket = detect_routing_events(&points, &noise_floor_query(2000), 0);
        assert!(
            entered_band_events(&large_bucket).is_empty(),
            "the same 2pp gap is outside the tighter B=2000 floor"
        );
    }

    #[test]
    fn lookback_prefix_events_are_dropped() {
        let window_start = 2 * BUCKET;
        let points = vec![
            point(0, "adyen", 89.0, 100),
            point(0, "stripe", 87.0, 100),
            // Flip happens before the requested window: used as baseline only.
            point(BUCKET, "stripe", 95.0, 100),
            point(window_start, "stripe", 95.5, 100),
        ];
        let events = detect_routing_events(&points, &query(), window_start);
        assert!(events.is_empty());
    }

    #[test]
    fn dimensions_are_tracked_independently() {
        let mut upi = point(BUCKET, "stripe", 99.0, 100);
        upi.payment_method_type = Some("upi".to_string());
        upi.payment_method = Some("collect".to_string());
        let points = vec![
            point(0, "adyen", 89.0, 100),
            point(0, "stripe", 87.0, 100),
            // A high stripe score in another dimension must not flip card/credit.
            upi,
        ];
        let events = detect_routing_events(&points, &query(), 0);
        assert!(events
            .iter()
            .all(|event| event.event_type != RoutingEventType::LeaderChanged));
    }

    #[test]
    fn events_are_sorted_newest_first_and_limited() {
        let points = vec![
            point(0, "adyen", 89.0, 100),
            point(0, "stripe", 87.0, 100),
            point(BUCKET, "stripe", 90.0, 100),
            // adyen retakes the lead by a wide margin so stripe drops out of the
            // band — the newest bucket holds a single, unambiguous event.
            point(2 * BUCKET, "adyen", 100.0, 100),
        ];
        let mut limited_query = query();
        limited_query.limit = 1;
        let events = detect_routing_events(&points, &limited_query, 0);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].bucket_ms, 2 * BUCKET);
        assert_eq!(events[0].gateway, "adyen");
    }
}
