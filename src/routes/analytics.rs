use std::collections::HashMap;
use std::sync::Arc;

use axum::{extract::Query, Json};
use serde::{Deserialize, Serialize};

use crate::tenant::GlobalAppState;

// ---------------------------------------------------------------------------
// Shared types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct TimeRangeParams {
    /// Time range: 15m, 1h, 6h, 24h, 7d
    pub range: Option<String>,
    /// Bucket granularity: 10s, 1m, 5m, 1h
    pub granularity: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GatewayScoreParams {
    pub merchant: Option<String>,
    pub pmt: Option<String>,
    pub gateway: Option<String>,
    #[serde(flatten)]
    pub time: TimeRangeParams,
}

#[derive(Debug, Deserialize)]
pub struct DecisionParams {
    pub group_by: Option<String>,
    #[serde(flatten)]
    pub time: TimeRangeParams,
}

#[derive(Debug, Deserialize)]
pub struct RoutingStatsParams {
    pub range: Option<String>,
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct GatewayScoreEntry {
    pub endpoint: String,
    pub total_requests: u64,
    pub success_count: u64,
    pub failure_count: u64,
    pub success_rate: f64,
}

#[derive(Debug, Serialize)]
pub struct GatewayScoresResponse {
    pub current: Vec<GatewayScoreEntry>,
}

#[derive(Debug, Serialize)]
pub struct DecisionBucket {
    pub endpoint: String,
    pub total_count: u64,
    pub success_count: u64,
    pub failure_count: u64,
}

#[derive(Debug, Serialize)]
pub struct DecisionsResponse {
    pub buckets: Vec<DecisionBucket>,
}

#[derive(Debug, Serialize)]
pub struct FeedbackEntry {
    pub endpoint: String,
    pub total_count: u64,
    pub success_count: u64,
    pub failure_count: u64,
}

#[derive(Debug, Serialize)]
pub struct FeedbacksResponse {
    pub entries: Vec<FeedbackEntry>,
}

#[derive(Debug, Serialize)]
pub struct RoutingStatEntry {
    pub endpoint: String,
    pub total_requests: u64,
    pub success_count: u64,
    pub failure_count: u64,
    pub error_rate: f64,
}

#[derive(Debug, Serialize)]
pub struct RoutingStatsResponse {
    pub stats: Vec<RoutingStatEntry>,
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn serve() -> axum::Router<Arc<GlobalAppState>> {
    axum::Router::new()
        .route("/gateway-scores", axum::routing::get(gateway_scores))
        .route("/decisions", axum::routing::get(decisions))
        .route("/feedbacks", axum::routing::get(feedbacks))
        .route("/routing-stats", axum::routing::get(routing_stats))
}

// ---------------------------------------------------------------------------
// Helpers – read current Prometheus counters
// ---------------------------------------------------------------------------

/// Collect per-endpoint totals from `API_REQUEST_TOTAL_COUNTER`.
fn collect_total_counts() -> HashMap<String, u64> {
    let mut totals: HashMap<String, u64> = HashMap::new();
    let metric_families = prometheus::gather();
    for mf in &metric_families {
        if mf.get_name() == "api_requests_total" {
            for m in mf.get_metric() {
                let mut endpoint = String::new();
                for lp in m.get_label() {
                    if lp.get_name() == "endpoint" {
                        endpoint = lp.get_value().to_string();
                    }
                }
                if !endpoint.is_empty() {
                    let val = m.get_counter().get_value() as u64;
                    *totals.entry(endpoint).or_default() += val;
                }
            }
        }
    }
    totals
}

/// Collect per-endpoint per-status counts from `API_REQUEST_COUNTER`.
fn collect_status_counts() -> HashMap<String, HashMap<String, u64>> {
    let mut counts: HashMap<String, HashMap<String, u64>> = HashMap::new();
    let metric_families = prometheus::gather();
    for mf in &metric_families {
        if mf.get_name() == "api_requests_by_status" {
            for m in mf.get_metric() {
                let mut endpoint = String::new();
                let mut status = String::new();
                for lp in m.get_label() {
                    match lp.get_name() {
                        "endpoint" => endpoint = lp.get_value().to_string(),
                        "status" => status = lp.get_value().to_string(),
                        _ => {}
                    }
                }
                if !endpoint.is_empty() {
                    let val = m.get_counter().get_value() as u64;
                    *counts
                        .entry(endpoint)
                        .or_default()
                        .entry(status)
                        .or_default() += val;
                }
            }
        }
    }
    counts
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// `GET /analytics/gateway-scores`
///
/// Returns the current scoring snapshot derived from Prometheus counters.
/// Filters optionally by `gateway` query param (matches endpoint name substring).
pub async fn gateway_scores(
    Query(params): Query<GatewayScoreParams>,
) -> Json<GatewayScoresResponse> {
    let totals = collect_total_counts();
    let status_counts = collect_status_counts();

    let gateway_filter = params.gateway.as_deref().unwrap_or("");

    let mut current: Vec<GatewayScoreEntry> = Vec::new();

    for (endpoint, total) in &totals {
        if !gateway_filter.is_empty() && !endpoint.contains(gateway_filter) {
            continue;
        }

        let statuses = status_counts.get(endpoint);
        let success = statuses
            .and_then(|s| s.get("success"))
            .copied()
            .unwrap_or(0);
        let failure = statuses
            .and_then(|s| s.get("failure"))
            .copied()
            .unwrap_or(0);

        let sr = if *total > 0 {
            (success as f64 / *total as f64) * 100.0
        } else {
            0.0
        };

        current.push(GatewayScoreEntry {
            endpoint: endpoint.clone(),
            total_requests: *total,
            success_count: success,
            failure_count: failure,
            success_rate: (sr * 100.0).round() / 100.0,
        });
    }

    current.sort_by(|a, b| b.total_requests.cmp(&a.total_requests));

    Json(GatewayScoresResponse { current })
}

/// `GET /analytics/decisions`
///
/// Returns decision counts from Prometheus, optionally grouped by endpoint.
pub async fn decisions(Query(params): Query<DecisionParams>) -> Json<DecisionsResponse> {
    let totals = collect_total_counts();
    let status_counts = collect_status_counts();

    let decision_endpoints: Vec<&str> = match params.group_by.as_deref() {
        Some("gateway") => vec!["decide_gateway", "decision_gateway"],
        Some("approach") => vec!["decide_gateway"],
        _ => totals.keys().map(|k| k.as_str()).collect(),
    };

    let mut buckets: Vec<DecisionBucket> = Vec::new();

    for endpoint in decision_endpoints {
        let total = totals.get(endpoint).copied().unwrap_or(0);
        let statuses = status_counts.get(endpoint);
        let success = statuses
            .and_then(|s| s.get("success"))
            .copied()
            .unwrap_or(0);
        let failure = statuses
            .and_then(|s| s.get("failure"))
            .copied()
            .unwrap_or(0);

        buckets.push(DecisionBucket {
            endpoint: endpoint.to_string(),
            total_count: total,
            success_count: success,
            failure_count: failure,
        });
    }

    buckets.sort_by(|a, b| b.total_count.cmp(&a.total_count));

    Json(DecisionsResponse { buckets })
}

/// `GET /analytics/feedbacks`
///
/// Returns feedback ingestion stats from Prometheus counters.
pub async fn feedbacks(Query(_params): Query<TimeRangeParams>) -> Json<FeedbacksResponse> {
    let totals = collect_total_counts();
    let status_counts = collect_status_counts();

    let feedback_endpoints = ["update_score", "update_gateway_score"];

    let mut entries: Vec<FeedbackEntry> = Vec::new();

    for endpoint in &feedback_endpoints {
        let total = totals.get(*endpoint).copied().unwrap_or(0);
        let statuses = status_counts.get(*endpoint);
        let success = statuses
            .and_then(|s| s.get("success"))
            .copied()
            .unwrap_or(0);
        let failure = statuses
            .and_then(|s| s.get("failure"))
            .copied()
            .unwrap_or(0);

        entries.push(FeedbackEntry {
            endpoint: endpoint.to_string(),
            total_count: total,
            success_count: success,
            failure_count: failure,
        });
    }

    Json(FeedbacksResponse { entries })
}

/// `GET /analytics/routing-stats`
///
/// Returns per-endpoint routing statistics including error rate.
pub async fn routing_stats(
    Query(_params): Query<RoutingStatsParams>,
) -> Json<RoutingStatsResponse> {
    let totals = collect_total_counts();
    let status_counts = collect_status_counts();

    let mut stats: Vec<RoutingStatEntry> = Vec::new();

    for (endpoint, total) in &totals {
        let statuses = status_counts.get(endpoint);
        let success = statuses
            .and_then(|s| s.get("success"))
            .copied()
            .unwrap_or(0);
        let failure = statuses
            .and_then(|s| s.get("failure"))
            .copied()
            .unwrap_or(0);

        let error_rate = if *total > 0 {
            (failure as f64 / *total as f64) * 100.0
        } else {
            0.0
        };

        stats.push(RoutingStatEntry {
            endpoint: endpoint.clone(),
            total_requests: *total,
            success_count: success,
            failure_count: failure,
            error_rate: (error_rate * 100.0).round() / 100.0,
        });
    }

    stats.sort_by(|a, b| b.total_requests.cmp(&a.total_requests));

    Json(RoutingStatsResponse { stats })
}
