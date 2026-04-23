use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsQuery {
    pub merchant_id: Option<String>,
    pub scope: AnalyticsScope,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnalyticsScope {
    Current,
    All,
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

impl AnalyticsScope {
    pub fn from_query(value: Option<&str>) -> Self {
        match value {
            Some("all") => Self::All,
            _ => Self::Current,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::All => "all",
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
pub struct AnalyticsOverviewResponse {
    pub scope: String,
    pub merchant_id: Option<String>,
    pub kpis: Vec<AnalyticsKpi>,
    pub route_hits: Vec<AnalyticsRouteHit>,
    pub top_scores: Vec<GatewayScoreSnapshot>,
    pub top_errors: Vec<AnalyticsErrorSummary>,
    pub top_rules: Vec<AnalyticsRuleHit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsRouteHit {
    pub route: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayScoreSnapshot {
    pub merchant_id: String,
    pub payment_method_type: String,
    pub payment_method: String,
    pub gateway: String,
    pub score_value: f64,
    pub sigma_factor: f64,
    pub average_latency: f64,
    pub tp99_latency: f64,
    pub transaction_count: i64,
    pub last_updated_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayScoreSeriesPoint {
    pub bucket_ms: i64,
    pub merchant_id: String,
    pub payment_method_type: String,
    pub payment_method: String,
    pub gateway: String,
    pub score_value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsGatewayScoresResponse {
    pub scope: String,
    pub merchant_id: Option<String>,
    pub range: String,
    pub snapshots: Vec<GatewayScoreSnapshot>,
    pub series: Vec<GatewayScoreSeriesPoint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsDecisionPoint {
    pub bucket_ms: i64,
    pub routing_approach: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsDecisionResponse {
    pub scope: String,
    pub merchant_id: Option<String>,
    pub range: String,
    pub tiles: Vec<AnalyticsKpi>,
    pub series: Vec<AnalyticsDecisionPoint>,
    pub approaches: Vec<AnalyticsRuleHit>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsGatewaySharePoint {
    pub bucket_ms: i64,
    pub gateway: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsRoutingStatsResponse {
    pub scope: String,
    pub merchant_id: Option<String>,
    pub range: String,
    pub gateway_share: Vec<AnalyticsGatewaySharePoint>,
    pub top_rules: Vec<AnalyticsRuleHit>,
    pub sr_trend: Vec<GatewayScoreSeriesPoint>,
    pub available_filters: RoutingFilterOptions,
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
    pub route: String,
    pub error_code: String,
    pub error_message: String,
    pub count: i64,
    pub last_seen_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsLogSample {
    pub route: String,
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
    pub scope: String,
    pub merchant_id: Option<String>,
    pub range: String,
    pub total_errors: i64,
    pub errors: Vec<AnalyticsErrorSummary>,
    pub samples: Vec<AnalyticsLogSample>,
    pub page: usize,
    pub page_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsRuleHit {
    pub rule_name: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentAuditQuery {
    pub merchant_id: Option<String>,
    pub scope: AnalyticsScope,
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
    pub error_code: Option<String>,
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
    pub id: i64,
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
    pub scope: String,
    pub merchant_id: Option<String>,
    pub range: String,
    pub payment_id: Option<String>,
    pub request_id: Option<String>,
    pub gateway: Option<String>,
    pub route: Option<String>,
    pub status: Option<String>,
    pub flow_type: Option<String>,
    pub error_code: Option<String>,
    pub page: usize,
    pub page_size: usize,
    pub total_results: usize,
    pub results: Vec<PaymentAuditSummary>,
    pub timeline: Vec<PaymentAuditEvent>,
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
