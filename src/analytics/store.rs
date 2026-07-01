use async_trait::async_trait;

use crate::analytics::events::{ApiEvent, DomainAnalyticsEvent};
use crate::analytics::models::{
    AnalyticsCostSavingsResponse, AnalyticsDecisionResponse, AnalyticsGatewayScoresResponse,
    AnalyticsLogSummariesResponse, AnalyticsOverviewResponse, AnalyticsQuery,
    AnalyticsRoutingStatsResponse, ExperimentResultsQuery, ExperimentResultsResponse,
    ExperimentTransactionsQuery, ExperimentTransactionsResponse, PaymentAuditQuery,
    PaymentAuditResponse, RoutingEventsQuery, RoutingEventsResponse,
};
use crate::error::ApiError;

#[async_trait]
pub trait AnalyticsWriteStore: Send + Sync {
    async fn persist_domain_events(&self, events: &[DomainAnalyticsEvent]) -> Result<(), ApiError>;
    async fn persist_api_events(&self, events: &[ApiEvent]) -> Result<(), ApiError>;

    fn sink_name(&self) -> &'static str;
}

/// Runtime calibration inputs for one cluster, derived from analytics. The cluster is keyed by
/// (payment_method_type, payment_method) plus whatever low-cardinality dimensions the merchant
/// has active (card scheme / currency / country / auth type — `None` when inactive). `volume` is
/// the decision count over the lookback window and `gateway_count` the distinct PSPs that saw it.
#[derive(Debug, Clone)]
pub struct SegmentTraffic {
    pub payment_method_type: String,
    pub payment_method: String,
    pub card_network: Option<String>,
    pub currency: Option<String>,
    pub country: Option<String>,
    pub auth_type: Option<String>,
    pub volume: i64,
    pub gateway_count: i64,
}

#[async_trait]
pub trait AnalyticsReadStore: Send + Sync {
    async fn overview(&self, query: &AnalyticsQuery)
        -> Result<AnalyticsOverviewResponse, ApiError>;

    /// Per-cluster decision volume + PSP count for the auto-calibrator, grouped by
    /// (pmt, pm) plus `active_dims` (a subset of card_network/currency/country/auth_type).
    /// Defaults to empty so stores without analytics simply contribute no inputs.
    async fn merchant_segment_traffic(
        &self,
        _merchant_id: &str,
        _since_ms: i64,
        _active_dims: &[&str],
    ) -> Result<Vec<SegmentTraffic>, ApiError> {
        Ok(Vec::new())
    }

    async fn gateway_scores(
        &self,
        query: &AnalyticsQuery,
    ) -> Result<AnalyticsGatewayScoresResponse, ApiError>;

    async fn decisions(
        &self,
        query: &AnalyticsQuery,
    ) -> Result<AnalyticsDecisionResponse, ApiError>;

    async fn routing_stats(
        &self,
        query: &AnalyticsQuery,
    ) -> Result<AnalyticsRoutingStatsResponse, ApiError>;

    async fn cost_savings(
        &self,
        query: &AnalyticsQuery,
    ) -> Result<AnalyticsCostSavingsResponse, ApiError>;

    async fn log_summaries(
        &self,
        query: &AnalyticsQuery,
    ) -> Result<AnalyticsLogSummariesResponse, ApiError>;

    async fn payment_audit(
        &self,
        query: &PaymentAuditQuery,
    ) -> Result<PaymentAuditResponse, ApiError>;

    async fn preview_trace(
        &self,
        query: &PaymentAuditQuery,
    ) -> Result<PaymentAuditResponse, ApiError>;

    async fn experiment_results(
        &self,
        query: &ExperimentResultsQuery,
    ) -> Result<ExperimentResultsResponse, ApiError>;

    async fn experiment_transactions(
        &self,
        query: &ExperimentTransactionsQuery,
    ) -> Result<ExperimentTransactionsResponse, ApiError>;

    async fn routing_events(
        &self,
        query: &RoutingEventsQuery,
    ) -> Result<RoutingEventsResponse, ApiError>;
}

#[derive(Clone)]
pub struct NoopAnalyticsWriteStore;

#[async_trait]
impl AnalyticsWriteStore for NoopAnalyticsWriteStore {
    async fn persist_domain_events(
        &self,
        _events: &[DomainAnalyticsEvent],
    ) -> Result<(), ApiError> {
        Ok(())
    }

    async fn persist_api_events(&self, _events: &[ApiEvent]) -> Result<(), ApiError> {
        Ok(())
    }

    fn sink_name(&self) -> &'static str {
        "noop"
    }
}

#[derive(Clone)]
pub struct UnavailableAnalyticsReadStore;

#[async_trait]
impl AnalyticsReadStore for UnavailableAnalyticsReadStore {
    async fn overview(
        &self,
        _query: &AnalyticsQuery,
    ) -> Result<AnalyticsOverviewResponse, ApiError> {
        Err(ApiError::DatabaseError)
    }

    async fn gateway_scores(
        &self,
        _query: &AnalyticsQuery,
    ) -> Result<AnalyticsGatewayScoresResponse, ApiError> {
        Err(ApiError::DatabaseError)
    }

    async fn decisions(
        &self,
        _query: &AnalyticsQuery,
    ) -> Result<AnalyticsDecisionResponse, ApiError> {
        Err(ApiError::DatabaseError)
    }

    async fn routing_stats(
        &self,
        _query: &AnalyticsQuery,
    ) -> Result<AnalyticsRoutingStatsResponse, ApiError> {
        Err(ApiError::DatabaseError)
    }

    async fn cost_savings(
        &self,
        _query: &AnalyticsQuery,
    ) -> Result<AnalyticsCostSavingsResponse, ApiError> {
        Err(ApiError::DatabaseError)
    }

    async fn log_summaries(
        &self,
        _query: &AnalyticsQuery,
    ) -> Result<AnalyticsLogSummariesResponse, ApiError> {
        Err(ApiError::DatabaseError)
    }

    async fn payment_audit(
        &self,
        _query: &PaymentAuditQuery,
    ) -> Result<PaymentAuditResponse, ApiError> {
        Err(ApiError::DatabaseError)
    }

    async fn preview_trace(
        &self,
        _query: &PaymentAuditQuery,
    ) -> Result<PaymentAuditResponse, ApiError> {
        Err(ApiError::DatabaseError)
    }

    async fn experiment_results(
        &self,
        _query: &ExperimentResultsQuery,
    ) -> Result<ExperimentResultsResponse, ApiError> {
        Err(ApiError::DatabaseError)
    }

    async fn experiment_transactions(
        &self,
        _query: &ExperimentTransactionsQuery,
    ) -> Result<ExperimentTransactionsResponse, ApiError> {
        Err(ApiError::DatabaseError)
    }

    async fn routing_events(
        &self,
        _query: &RoutingEventsQuery,
    ) -> Result<RoutingEventsResponse, ApiError> {
        Err(ApiError::DatabaseError)
    }
}
