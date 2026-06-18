use serde::{Deserialize, Serialize};

use crate::analytics::flow::AnalyticsRoute;

pub const MAX_ANALYTICS_LOOKBACK_MS: i64 = 18 * 30 * 24 * 60 * 60 * 1000;
pub const MIN_ANALYTICS_PAGE: usize = 1;
pub const MIN_ANALYTICS_PAGE_SIZE: usize = 1;
pub const MAX_ANALYTICS_PAGE_SIZE: usize = 50;
pub const DEFAULT_ANALYTICS_PAGE_SIZE: usize = 10;
pub const DEFAULT_PAYMENT_AUDIT_PAGE_SIZE: usize = 12;

pub fn normalise_page(page: Option<u32>) -> usize {
    page.unwrap_or(MIN_ANALYTICS_PAGE as u32)
        .max(MIN_ANALYTICS_PAGE as u32) as usize
}

pub fn normalise_page_size(page_size: Option<u32>, default: usize) -> usize {
    page_size.unwrap_or(default as u32).clamp(
        MIN_ANALYTICS_PAGE_SIZE as u32,
        MAX_ANALYTICS_PAGE_SIZE as u32,
    ) as usize
}

fn normalise_gateways(raw: Option<String>) -> Vec<String> {
    raw.into_iter()
        .flat_map(|value| value.split(',').map(str::to_owned).collect::<Vec<_>>())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsQuery {
    pub merchant_id: String,
    pub range: AnalyticsRange,
    pub start_ms: Option<i64>,
    pub end_ms: Option<i64>,
    pub page: usize,
    pub page_size: usize,
    pub payment_method_type: Option<String>,
    pub payment_method: Option<String>,
    pub card_network: Option<String>,
    pub card_is_in: Option<String>,
    pub currency: Option<String>,
    pub country: Option<String>,
    pub auth_type: Option<String>,
    pub gateways: Vec<String>,
}

impl AnalyticsQuery {
    #[allow(clippy::too_many_arguments)]
    pub fn from_request(
        merchant_id: String,
        range: Option<String>,
        start_ms: Option<i64>,
        end_ms: Option<i64>,
        page: Option<u32>,
        page_size: Option<u32>,
        payment_method_type: Option<String>,
        payment_method: Option<String>,
        card_network: Option<String>,
        card_is_in: Option<String>,
        currency: Option<String>,
        country: Option<String>,
        auth_type: Option<String>,
        gateways: Option<String>,
    ) -> Self {
        let range = AnalyticsRange::from_query(range.as_deref());
        let (start_ms, end_ms) = match (start_ms, end_ms) {
            (Some(start_ms), Some(end_ms)) if start_ms >= 0 && end_ms > start_ms => {
                (Some(start_ms), Some(end_ms))
            }
            _ => (None, None),
        };

        Self {
            merchant_id,
            range,
            start_ms,
            end_ms,
            page: normalise_page(page),
            page_size: normalise_page_size(page_size, DEFAULT_ANALYTICS_PAGE_SIZE),
            payment_method_type: payment_method_type.filter(|value| !value.is_empty()),
            payment_method: payment_method.filter(|value| !value.is_empty()),
            card_network: card_network.filter(|value| !value.is_empty()),
            card_is_in: card_is_in.filter(|value| !value.is_empty()),
            currency: currency.filter(|value| !value.is_empty()),
            country: country.filter(|value| !value.is_empty()),
            auth_type: auth_type.filter(|value| !value.is_empty()),
            gateways: normalise_gateways(gateways),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnalyticsRange {
    M15,
    H1,
    H12,
    D1,
    W1,
}

impl AnalyticsRange {
    pub fn from_query(value: Option<&str>) -> Self {
        match value {
            Some("15m") => Self::M15,
            Some("12h") => Self::H12,
            Some("1d") => Self::D1,
            Some("1w") => Self::W1,
            _ => Self::H1,
        }
    }

    pub fn window_ms(&self) -> i64 {
        match self {
            Self::M15 => 15 * 60 * 1000,
            Self::H1 => 60 * 60 * 1000,
            Self::H12 => 12 * 60 * 60 * 1000,
            Self::D1 => 24 * 60 * 60 * 1000,
            Self::W1 => 7 * 24 * 60 * 60 * 1000,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsKpi {
    pub label: String,
    pub value: String,
    pub subtitle: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartRetryTrigger {
    pub gateway: String,
    pub error_code: Option<String>,
    pub count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartRetryFallback {
    pub gateway: String,
    pub retried: u64,
    pub recovered: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartRetryStats {
    pub retried_count: u64,
    pub recovered_count: u64,
    pub by_trigger: Vec<SmartRetryTrigger>,
    pub by_fallback: Vec<SmartRetryFallback>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsOverviewResponse {
    pub merchant_id: String,
    pub kpis: Vec<AnalyticsKpi>,
    pub route_hits: Vec<AnalyticsRouteHit>,
    pub top_scores: Vec<GatewayScoreSnapshot>,
    pub top_errors: Vec<AnalyticsErrorSummary>,
    pub top_rules: Vec<AnalyticsRuleHit>,
    pub smart_retry_stats: SmartRetryStats,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsRouteHit {
    pub route: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayScoreSnapshot {
    pub merchant_id: Option<String>,
    pub payment_method_type: Option<String>,
    pub payment_method: Option<String>,
    pub gateway: Option<String>,
    pub score_value: Option<f64>,
    pub sigma_factor: Option<f64>,
    pub average_latency: Option<f64>,
    pub tp99_latency: Option<f64>,
    pub transaction_count: Option<i64>,
    pub last_updated_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayScoreSeriesPoint {
    pub bucket_ms: i64,
    pub merchant_id: Option<String>,
    pub payment_method_type: Option<String>,
    pub payment_method: Option<String>,
    pub gateway: Option<String>,
    pub score_value: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsGatewayScoresResponse {
    pub merchant_id: String,
    pub range: String,
    pub snapshots: Vec<GatewayScoreSnapshot>,
    pub series: Vec<GatewayScoreSeriesPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsDecisionPoint {
    pub bucket_ms: i64,
    pub routing_approach: Option<String>,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsDecisionResponse {
    pub merchant_id: String,
    pub range: String,
    pub tiles: Vec<AnalyticsKpi>,
    pub series: Vec<AnalyticsDecisionPoint>,
    pub approaches: Vec<AnalyticsRuleHit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsGatewaySharePoint {
    pub bucket_ms: i64,
    pub gateway: Option<String>,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsRoutingStatsResponse {
    pub merchant_id: String,
    pub range: String,
    pub gateway_share: Vec<AnalyticsGatewaySharePoint>,
    pub top_rules: Vec<AnalyticsRuleHit>,
    pub sr_trend: Vec<GatewayScoreSeriesPoint>,
    pub available_filters: RoutingFilterOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsAvailableCurrency {
    pub currency: String,
    pub decision_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsCostSavingsTrendPoint {
    pub bucket_ms: i64,
    pub saved_value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsCostSavingsTotals {
    pub saved_value: f64,
    pub cost_won_count: u64,
    pub total_decisions: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsCostSavingsResponse {
    pub merchant_id: String,
    pub range: String,
    pub currency: Option<String>,
    pub available_currencies: Vec<AnalyticsAvailableCurrency>,
    pub trend: Vec<AnalyticsCostSavingsTrendPoint>,
    pub totals: AnalyticsCostSavingsTotals,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingFilterOptions {
    pub dimensions: Vec<RoutingFilterDimension>,
    pub missing_dimensions: Vec<RoutingFilterDimensionHint>,
    pub gateways: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingFilterDimension {
    pub key: String,
    pub label: String,
    pub values: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingFilterDimensionHint {
    pub key: String,
    pub label: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsErrorSummary {
    pub route: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub count: i64,
    pub last_seen_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsLogSample {
    pub route: Option<String>,
    pub merchant_id: Option<String>,
    pub payment_id: Option<String>,
    pub request_id: Option<String>,
    pub global_request_id: Option<String>,
    pub trace_id: Option<String>,
    pub gateway: Option<String>,
    pub routing_approach: Option<String>,
    pub status: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub flow_type: Option<String>,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsLogSummariesResponse {
    pub merchant_id: String,
    pub range: String,
    pub total_errors: i64,
    pub errors: Vec<AnalyticsErrorSummary>,
    pub samples: Vec<AnalyticsLogSample>,
    pub page: usize,
    pub page_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsRuleHit {
    pub rule_name: Option<String>,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentAuditQuery {
    pub merchant_id: String,
    pub range: AnalyticsRange,
    pub start_ms: Option<i64>,
    pub end_ms: Option<i64>,
    pub page: usize,
    pub page_size: usize,
    pub payment_id: Option<String>,
    pub request_id: Option<String>,
    pub gateway: Option<String>,
    pub route: Option<String>,
    pub status: Option<String>,
    pub flow_type: Option<String>,
    pub routing_approach: Option<String>,
    pub exclude_routing_approach: Option<String>,
    pub error_code: Option<String>,
}

impl PaymentAuditQuery {
    fn normalise_route_filter(route: Option<String>) -> Option<String> {
        route.and_then(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return None;
            }

            AnalyticsRoute::from_filter_value(trimmed).map(|route| route.as_str().to_string())
        })
    }

    fn normalise_status_filter(status: Option<String>) -> Option<String> {
        status.and_then(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                return None;
            }

            Some(match trimmed.to_ascii_lowercase().as_str() {
                "success" => "success".to_string(),
                "failure" => "FAILURE".to_string(),
                _ => trimmed.to_string(),
            })
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_request(
        merchant_id: String,
        range: Option<String>,
        start_ms: Option<i64>,
        end_ms: Option<i64>,
        page: Option<u32>,
        page_size: Option<u32>,
        payment_id: Option<String>,
        request_id: Option<String>,
        gateway: Option<String>,
        route: Option<String>,
        status: Option<String>,
        flow_type: Option<String>,
        routing_approach: Option<String>,
        exclude_routing_approach: Option<String>,
        error_code: Option<String>,
    ) -> Self {
        let range = AnalyticsRange::from_query(range.as_deref());
        let (start_ms, end_ms) = match (start_ms, end_ms) {
            (Some(start_ms), Some(end_ms)) if start_ms >= 0 && end_ms > start_ms => {
                (Some(start_ms), Some(end_ms))
            }
            _ => (None, None),
        };

        Self {
            merchant_id,
            range,
            start_ms,
            end_ms,
            page: normalise_page(page),
            page_size: normalise_page_size(page_size, DEFAULT_PAYMENT_AUDIT_PAGE_SIZE),
            payment_id,
            request_id,
            gateway,
            route: Self::normalise_route_filter(route),
            status: Self::normalise_status_filter(status),
            flow_type,
            routing_approach,
            exclude_routing_approach,
            error_code,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentAuditSummary {
    pub lookup_key: String,
    pub payment_id: Option<String>,
    pub request_id: Option<String>,
    pub merchant_id: Option<String>,
    pub first_seen_ms: i64,
    pub last_seen_ms: i64,
    pub event_count: usize,
    pub latest_status: Option<String>,
    pub latest_gateway: Option<String>,
    pub latest_stage: Option<String>,
    pub gateways: Vec<String>,
    pub routes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentAuditEvent {
    pub id: String,
    pub flow_type: String,
    pub event_stage: Option<String>,
    pub route: Option<String>,
    pub merchant_id: Option<String>,
    pub payment_id: Option<String>,
    pub request_id: Option<String>,
    pub global_request_id: Option<String>,
    pub trace_id: Option<String>,
    pub payment_method_type: Option<String>,
    pub payment_method: Option<String>,
    pub gateway: Option<String>,
    pub routing_approach: Option<String>,
    pub rule_name: Option<String>,
    pub status: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub score_value: Option<f64>,
    pub sigma_factor: Option<f64>,
    pub average_latency: Option<f64>,
    pub tp99_latency: Option<f64>,
    pub transaction_count: Option<i64>,
    pub details: Option<String>,
    pub details_json: Option<serde_json::Value>,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentAuditResponse {
    pub merchant_id: String,
    pub range: String,
    pub payment_id: Option<String>,
    pub request_id: Option<String>,
    pub gateway: Option<String>,
    pub route: Option<String>,
    pub status: Option<String>,
    pub flow_type: Option<String>,
    pub routing_approach: Option<String>,
    pub error_code: Option<String>,
    pub page: usize,
    pub page_size: usize,
    pub total_results: usize,
    pub total_success: usize,
    pub total_failure: usize,
    pub results: Vec<PaymentAuditSummary>,
    pub timeline: Vec<PaymentAuditEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentArmMetrics {
    pub arm: String,
    pub transaction_count: i64,
    pub success_count: i64,
    pub failure_count: i64,
    pub auth_rate: f64,
    pub avg_latency_ms: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ExperimentVerdict {
    /// Not enough transactions yet to make a judgment.
    CollectingData,
    /// Enough data; difference is not statistically significant.
    NotSignificant,
    /// Variant is statistically significantly better than control.
    VariantWins,
    /// Variant is statistically significantly worse than control.
    VariantLoses,
    /// Variant auth rate dropped beyond the guardrail threshold — merchant should pause.
    GuardrailBreached,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentResultsResponse {
    pub experiment_id: String,
    pub merchant_id: String,
    pub control: ExperimentArmMetrics,
    pub variant: ExperimentArmMetrics,
    /// Auth rate delta in percentage points (variant - control).
    pub delta_pp: f64,
    pub p_value: Option<f64>,
    pub confidence_interval: Option<(f64, f64)>,
    pub verdict: ExperimentVerdict,
    /// Min sample size from experiment config; used to show progress.
    pub min_sample_size: u32,
}

pub struct ExperimentResultsQuery {
    pub experiment_id: String,
    pub merchant_id: String,
    pub start_ms: Option<i64>,
    pub end_ms: Option<i64>,
    pub min_sample_size: u32,
    pub guardrail_threshold_pp: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentTransaction {
    pub payment_id: String,
    pub variant_arm: String,
    pub gateway: Option<String>,
    pub status: Option<String>,
    pub created_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExperimentTransactionsResponse {
    pub experiment_id: String,
    pub total: u64,
    pub transactions: Vec<ExperimentTransaction>,
}

pub struct ExperimentTransactionsQuery {
    pub experiment_id: String,
    pub merchant_id: String,
    pub start_ms: Option<i64>,
    pub page: u64,
    pub page_size: u64,
}

pub const ROUTING_EVENTS_BUCKET_MS: i64 = 5 * 60 * 1000;
pub const ROUTING_EVENTS_FAST_BUCKET_MS: i64 = 60 * 1000;
pub const ROUTING_EVENTS_SECOND_BUCKET_MS: i64 = 1000;
pub const ROUTING_EVENTS_STALENESS_BUCKETS: i64 = 12;
// Floor so tiny buckets don't age gateways out within seconds of quiet.
pub const ROUTING_EVENTS_STALENESS_FLOOR_MS: i64 = 10 * 60 * 1000;
// Second-granularity scans are row-heavy; cap the window in that mode.
pub const ROUTING_EVENTS_SECOND_BUCKET_MAX_WINDOW_MS: i64 = 60 * 60 * 1000;
pub const DEFAULT_ROUTING_EVENTS_MIN_TXN_COUNT: i64 = 10;
// SR scores are on a 0..1 scale (see gateway_scoring_service success_rate).
pub const DEFAULT_ROUTING_EVENTS_MIN_SCORE_DELTA: f64 = 0.01;
pub const DEFAULT_ROUTING_EVENTS_SWING_THRESHOLD: f64 = 0.1;
pub const DEFAULT_ROUTING_EVENTS_LIMIT: usize = 50;
pub const MAX_ROUTING_EVENTS_LIMIT: usize = 200;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RoutingEventType {
    /// The top-scored gateway for a dimension changed.
    LeaderChanged,
    /// A gateway newly appeared in the score map for a dimension.
    GatewayEntered,
    /// A gateway's score moved by more than the swing threshold between snapshots.
    ScoreSwing,
}

impl RoutingEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::LeaderChanged => "leader_changed",
            Self::GatewayEntered => "gateway_entered",
            Self::ScoreSwing => "score_swing",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingEvent {
    /// Deterministic composite ID, stable across polls; clients dedupe on it.
    pub id: String,
    pub event_type: RoutingEventType,
    pub merchant_id: String,
    pub payment_method_type: Option<String>,
    pub payment_method: Option<String>,
    pub bucket_ms: i64,
    pub gateway: String,
    pub previous_gateway: Option<String>,
    pub score: Option<f64>,
    pub previous_score: Option<f64>,
    pub transaction_count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingEventsResponse {
    pub merchant_id: String,
    pub range: String,
    pub events: Vec<RoutingEvent>,
    pub generated_at_ms: i64,
}

#[derive(Debug, Clone)]
pub struct RoutingEventsQuery {
    pub merchant_id: String,
    pub range: AnalyticsRange,
    pub start_ms: Option<i64>,
    pub end_ms: Option<i64>,
    pub payment_method_type: Option<String>,
    pub payment_method: Option<String>,
    pub min_transaction_count: i64,
    pub min_score_delta: f64,
    pub swing_threshold: f64,
    pub limit: usize,
    /// Bucket granularity: 5-min default, 1-min opt-in ("bucket=1m").
    /// Event IDs embed bucket_ms, so each granularity has its own stable ID space.
    pub bucket_ms: i64,
}

impl RoutingEventsQuery {
    #[allow(clippy::too_many_arguments)]
    pub fn from_request(
        merchant_id: String,
        range: Option<String>,
        start_ms: Option<i64>,
        end_ms: Option<i64>,
        payment_method_type: Option<String>,
        payment_method: Option<String>,
        min_transaction_count: Option<i64>,
        min_score_delta: Option<f64>,
        swing_threshold: Option<f64>,
        limit: Option<u32>,
        bucket: Option<String>,
    ) -> Self {
        let range = AnalyticsRange::from_query(range.as_deref());
        let (start_ms, end_ms) = match (start_ms, end_ms) {
            (Some(start_ms), Some(end_ms)) if start_ms >= 0 && end_ms > start_ms => {
                (Some(start_ms), Some(end_ms))
            }
            _ => (None, None),
        };

        Self {
            merchant_id,
            range,
            start_ms,
            end_ms,
            payment_method_type: payment_method_type.filter(|value| !value.is_empty()),
            payment_method: payment_method.filter(|value| !value.is_empty()),
            min_transaction_count: min_transaction_count
                .unwrap_or(DEFAULT_ROUTING_EVENTS_MIN_TXN_COUNT)
                .max(0),
            min_score_delta: min_score_delta
                .unwrap_or(DEFAULT_ROUTING_EVENTS_MIN_SCORE_DELTA)
                .max(0.0),
            swing_threshold: swing_threshold
                .unwrap_or(DEFAULT_ROUTING_EVENTS_SWING_THRESHOLD)
                .max(0.0),
            limit: limit
                .map(|limit| limit as usize)
                .unwrap_or(DEFAULT_ROUTING_EVENTS_LIMIT)
                .clamp(1, MAX_ROUTING_EVENTS_LIMIT),
            bucket_ms: match bucket.as_deref() {
                Some("1s") => ROUTING_EVENTS_SECOND_BUCKET_MS,
                Some("1m") => ROUTING_EVENTS_FAST_BUCKET_MS,
                _ => ROUTING_EVENTS_BUCKET_MS,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        normalise_page, normalise_page_size, DEFAULT_ANALYTICS_PAGE_SIZE,
        DEFAULT_PAYMENT_AUDIT_PAGE_SIZE, MAX_ANALYTICS_PAGE_SIZE, MIN_ANALYTICS_PAGE,
    };

    #[test]
    fn normalise_page_defaults_and_bounds() {
        assert_eq!(normalise_page(None), MIN_ANALYTICS_PAGE);
        assert_eq!(normalise_page(Some(0)), MIN_ANALYTICS_PAGE);
        assert_eq!(normalise_page(Some(3)), 3);
    }

    #[test]
    fn normalise_page_size_uses_default_and_clamps_to_bounds() {
        assert_eq!(
            normalise_page_size(None, DEFAULT_ANALYTICS_PAGE_SIZE),
            DEFAULT_ANALYTICS_PAGE_SIZE
        );
        assert_eq!(
            normalise_page_size(Some(0), DEFAULT_PAYMENT_AUDIT_PAGE_SIZE),
            1
        );
        assert_eq!(
            normalise_page_size(Some(500), DEFAULT_ANALYTICS_PAGE_SIZE),
            MAX_ANALYTICS_PAGE_SIZE
        );
    }
}
