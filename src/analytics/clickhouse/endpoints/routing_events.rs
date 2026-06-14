use std::collections::{BTreeMap, HashMap, HashSet};

use crate::analytics::models::{
    AnalyticsRange, RoutingEvent, RoutingEventType, RoutingEventsQuery, RoutingEventsResponse,
    ROUTING_EVENTS_SECOND_BUCKET_MAX_WINDOW_MS, ROUTING_EVENTS_SECOND_BUCKET_MS,
    ROUTING_EVENTS_STALENESS_BUCKETS, ROUTING_EVENTS_STALENESS_FLOOR_MS,
};
use crate::analytics::service::now_ms;
use crate::error::ApiError;

use super::super::metrics;
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
    let events = detect_routing_events(&points, query, start_ms);

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
        let mut seen_gateways: HashSet<String> = HashSet::new();
        let mut previous_leader: Option<String> = None;
        let mut is_first_bucket = true;

        for (&bucket_ms, rows) in buckets {
            for row in rows {
                let gateway = row.gateway.clone().unwrap_or_default();
                let score = row.score_value.unwrap_or_default();
                let passes_txn_gate =
                    row.transaction_count.unwrap_or(0) >= query.min_transaction_count;

                if !seen_gateways.contains(&gateway) {
                    seen_gateways.insert(gateway.clone());
                    if !is_first_bucket && passes_txn_gate {
                        events.push(RoutingEvent {
                            id: event_id(
                                RoutingEventType::GatewayEntered,
                                &query.merchant_id,
                                dimension,
                                bucket_ms,
                                None,
                                &gateway,
                            ),
                            event_type: RoutingEventType::GatewayEntered,
                            merchant_id: query.merchant_id.clone(),
                            payment_method_type: dimension.0.clone(),
                            payment_method: dimension.1.clone(),
                            bucket_ms,
                            gateway: gateway.clone(),
                            previous_gateway: None,
                            score: Some(score),
                            previous_score: None,
                            transaction_count: row.transaction_count,
                        });
                    }
                } else if let Some(previous) = state.get(&gateway) {
                    let delta = score - previous.score;
                    if delta.abs() >= query.swing_threshold && passes_txn_gate {
                        events.push(RoutingEvent {
                            id: event_id(
                                RoutingEventType::ScoreSwing,
                                &query.merchant_id,
                                dimension,
                                bucket_ms,
                                None,
                                &gateway,
                            ),
                            event_type: RoutingEventType::ScoreSwing,
                            merchant_id: query.merchant_id.clone(),
                            payment_method_type: dimension.0.clone(),
                            payment_method: dimension.1.clone(),
                            bucket_ms,
                            gateway: gateway.clone(),
                            previous_gateway: None,
                            score: Some(score),
                            previous_score: Some(previous.score),
                            transaction_count: row.transaction_count,
                        });
                    }
                }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytics::models::{RoutingEventsQuery, ROUTING_EVENTS_BUCKET_MS};

    const BUCKET: i64 = ROUTING_EVENTS_BUCKET_MS;

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
            swing_threshold: 10.0,
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

    #[test]
    fn gateway_entering_mid_window_is_reported_but_window_start_storm_is_not() {
        let points = vec![
            point(0, "adyen", 89.0, 100),
            point(0, "stripe", 87.0, 100),
            point(BUCKET, "newpay", 50.0, 20),
        ];
        let events = detect_routing_events(&points, &query(), 0);
        let entries: Vec<_> = events
            .iter()
            .filter(|event| event.event_type == RoutingEventType::GatewayEntered)
            .collect();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].gateway, "newpay");
    }

    #[test]
    fn score_swing_beyond_threshold_is_reported() {
        let points = vec![
            point(0, "adyen", 89.0, 100),
            point(BUCKET, "adyen", 70.0, 100),
        ];
        let events = detect_routing_events(&points, &query(), 0);
        let swings: Vec<_> = events
            .iter()
            .filter(|event| event.event_type == RoutingEventType::ScoreSwing)
            .collect();
        assert_eq!(swings.len(), 1);
        assert_eq!(swings[0].score, Some(70.0));
        assert_eq!(swings[0].previous_score, Some(89.0));
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
            point(2 * BUCKET, "adyen", 91.0, 100),
        ];
        let mut limited_query = query();
        limited_query.limit = 1;
        let events = detect_routing_events(&points, &limited_query, 0);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].bucket_ms, 2 * BUCKET);
        assert_eq!(events[0].gateway, "adyen");
    }
}
