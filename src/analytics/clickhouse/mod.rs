use async_trait::async_trait;
use clickhouse::{Client, Row};
use masking::PeekInterface;
use serde::Deserialize;

use crate::analytics::models::*;
use crate::analytics::store::AnalyticsReadStore;
use crate::config::ClickHouseAnalyticsConfig;
use crate::error::ApiError;

pub mod common;
pub mod endpoints;
pub mod filters;
pub mod metrics;
pub mod query;
pub mod time;

#[derive(Clone)]
pub struct ClickHouseAnalyticsStore {
    client: Client,
}

#[derive(Debug, Clone, Deserialize, Row)]
struct StartupProbeRow {
    value: u8,
}

impl ClickHouseAnalyticsStore {
    pub async fn new(config: ClickHouseAnalyticsConfig) -> Result<Self, ApiError> {
        let mut client = Client::default()
            .with_url(config.url.clone())
            .with_database(config.database.clone())
            .with_user(config.user.clone());
        if let Some(password) = &config.password {
            client = client.with_password(password.peek().clone());
        }

        verify_connectivity(&client).await?;

        Ok(Self { client })
    }
}

async fn verify_connectivity(client: &Client) -> Result<(), ApiError> {
    // Fail fast on bad ClickHouse config instead of deferring the error to the first dashboard read.
    let probe = common::fetch_one::<StartupProbeRow>(client.query("SELECT 1 AS value"))
        .await
        .map_err(|_| ApiError::DatabaseError)?;
    let _ = probe.value;
    Ok(())
}

#[async_trait]
impl AnalyticsReadStore for ClickHouseAnalyticsStore {
    async fn overview(
        &self,
        query: &AnalyticsQuery,
    ) -> Result<AnalyticsOverviewResponse, ApiError> {
        endpoints::overview::load(&self.client, query).await
    }

    async fn gateway_scores(
        &self,
        query: &AnalyticsQuery,
    ) -> Result<AnalyticsGatewayScoresResponse, ApiError> {
        endpoints::gateway_scores::load(&self.client, query).await
    }

    async fn decisions(
        &self,
        query: &AnalyticsQuery,
    ) -> Result<AnalyticsDecisionResponse, ApiError> {
        endpoints::decisions::load(&self.client, query).await
    }

    async fn routing_stats(
        &self,
        query: &AnalyticsQuery,
    ) -> Result<AnalyticsRoutingStatsResponse, ApiError> {
        endpoints::routing_stats::load(&self.client, query).await
    }

    async fn log_summaries(
        &self,
        query: &AnalyticsQuery,
    ) -> Result<AnalyticsLogSummariesResponse, ApiError> {
        endpoints::log_summaries::load(&self.client, query).await
    }

    async fn payment_audit(
        &self,
        query: &PaymentAuditQuery,
    ) -> Result<PaymentAuditResponse, ApiError> {
        endpoints::payment_audit::load(&self.client, query, false).await
    }

    async fn preview_trace(
        &self,
        query: &PaymentAuditQuery,
    ) -> Result<PaymentAuditResponse, ApiError> {
        endpoints::preview_trace::load(&self.client, query).await
    }
}
