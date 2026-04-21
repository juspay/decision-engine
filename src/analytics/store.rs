use async_trait::async_trait;

use crate::analytics::events::{ApiEvent, DomainAnalyticsEvent};
use crate::analytics::models::{
    AnalyticsDecisionResponse, AnalyticsGatewayScoresResponse, AnalyticsLogSummariesResponse,
    AnalyticsOverviewResponse, AnalyticsQuery, AnalyticsRoutingStatsResponse, PaymentAuditQuery,
    PaymentAuditResponse,
};
use crate::error::ApiError;

#[async_trait]
pub trait AnalyticsWriteStore: Send + Sync {
    async fn persist_domain_events(&self, events: &[DomainAnalyticsEvent]) -> Result<(), ApiError>;
    async fn persist_api_events(&self, events: &[ApiEvent]) -> Result<(), ApiError>;

    fn sink_name(&self) -> &'static str;
}

#[async_trait]
pub trait AnalyticsReadStore: Send + Sync {
    async fn overview(
        &self,
        tenant_id: &str,
        query: &AnalyticsQuery,
    ) -> Result<AnalyticsOverviewResponse, ApiError>;

    async fn gateway_scores(
        &self,
        tenant_id: &str,
        query: &AnalyticsQuery,
    ) -> Result<AnalyticsGatewayScoresResponse, ApiError>;

    async fn decisions(
        &self,
        tenant_id: &str,
        query: &AnalyticsQuery,
    ) -> Result<AnalyticsDecisionResponse, ApiError>;

    async fn routing_stats(
        &self,
        tenant_id: &str,
        query: &AnalyticsQuery,
    ) -> Result<AnalyticsRoutingStatsResponse, ApiError>;

    async fn log_summaries(
        &self,
        tenant_id: &str,
        query: &AnalyticsQuery,
    ) -> Result<AnalyticsLogSummariesResponse, ApiError>;

    async fn payment_audit(
        &self,
        tenant_id: &str,
        query: &PaymentAuditQuery,
    ) -> Result<PaymentAuditResponse, ApiError>;

    async fn preview_trace(
        &self,
        tenant_id: &str,
        query: &PaymentAuditQuery,
    ) -> Result<PaymentAuditResponse, ApiError>;
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
        _tenant_id: &str,
        _query: &AnalyticsQuery,
    ) -> Result<AnalyticsOverviewResponse, ApiError> {
        Err(ApiError::DatabaseError)
    }

    async fn gateway_scores(
        &self,
        _tenant_id: &str,
        _query: &AnalyticsQuery,
    ) -> Result<AnalyticsGatewayScoresResponse, ApiError> {
        Err(ApiError::DatabaseError)
    }

    async fn decisions(
        &self,
        _tenant_id: &str,
        _query: &AnalyticsQuery,
    ) -> Result<AnalyticsDecisionResponse, ApiError> {
        Err(ApiError::DatabaseError)
    }

    async fn routing_stats(
        &self,
        _tenant_id: &str,
        _query: &AnalyticsQuery,
    ) -> Result<AnalyticsRoutingStatsResponse, ApiError> {
        Err(ApiError::DatabaseError)
    }

    async fn log_summaries(
        &self,
        _tenant_id: &str,
        _query: &AnalyticsQuery,
    ) -> Result<AnalyticsLogSummariesResponse, ApiError> {
        Err(ApiError::DatabaseError)
    }

    async fn payment_audit(
        &self,
        _tenant_id: &str,
        _query: &PaymentAuditQuery,
    ) -> Result<PaymentAuditResponse, ApiError> {
        Err(ApiError::DatabaseError)
    }

    async fn preview_trace(
        &self,
        _tenant_id: &str,
        _query: &PaymentAuditQuery,
    ) -> Result<PaymentAuditResponse, ApiError> {
        Err(ApiError::DatabaseError)
    }
}
