use crate::analytics::{
    decisions as fetch_decisions, gateway_scores as fetch_gateway_scores,
    log_summaries as fetch_log_summaries, overview as fetch_overview, parse_payment_audit_query,
    parse_query, payment_audit as fetch_payment_audit, preview_trace as fetch_preview_trace,
    routing_stats as fetch_routing_stats,
};
use crate::custom_extractors::{AuthenticatedAnalyticsContext, TenantStateResolver};
use crate::error;
use axum::extract::Query;
use axum::Json;
use serde::Deserialize;
use std::sync::Arc;

#[derive(Debug, Clone, Deserialize)]
pub struct AnalyticsQueryParams {
    pub range: Option<String>,
    pub start_ms: Option<i64>,
    pub end_ms: Option<i64>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
    pub payment_method_type: Option<String>,
    pub payment_method: Option<String>,
    pub card_network: Option<String>,
    pub card_is_in: Option<String>,
    pub currency: Option<String>,
    pub country: Option<String>,
    pub auth_type: Option<String>,
    pub gateway: Option<String>,
    pub payment_id: Option<String>,
    pub request_id: Option<String>,
    pub route: Option<String>,
    pub status: Option<String>,
    pub flow_type: Option<String>,
    pub error_code: Option<String>,
}

pub fn serve() -> axum::Router<Arc<crate::tenant::GlobalAppState>> {
    axum::Router::<Arc<crate::tenant::GlobalAppState>>::new()
        .route("/", axum::routing::get(overview))
        .route("/overview", axum::routing::get(overview))
        .route("/gateway-scores", axum::routing::get(gateway_scores))
        .route("/decisions", axum::routing::get(decisions))
        .route("/routing-stats", axum::routing::get(routing_stats))
        .route("/log-summaries", axum::routing::get(log_summaries))
        .route("/payment-audit", axum::routing::get(payment_audit))
        .route("/preview-trace", axum::routing::get(preview_trace))
}

pub async fn overview(
    TenantStateResolver(state): TenantStateResolver,
    AuthenticatedAnalyticsContext(auth_context): AuthenticatedAnalyticsContext,
    Query(params): Query<AnalyticsQueryParams>,
) -> Result<Json<crate::analytics::AnalyticsOverviewResponse>, error::ContainerError<error::ApiError>>
{
    let query = parse_query(
        auth_context.merchant_id.clone(),
        params.range,
        params.start_ms,
        params.end_ms,
        params.page,
        params.page_size,
        params.payment_method_type,
        params.payment_method,
        params.card_network,
        params.card_is_in,
        params.currency,
        params.country,
        params.auth_type,
        params.gateway,
    );
    let response = fetch_overview(&state, &query).await?;
    Ok(Json(response))
}

pub async fn gateway_scores(
    TenantStateResolver(state): TenantStateResolver,
    AuthenticatedAnalyticsContext(auth_context): AuthenticatedAnalyticsContext,
    Query(params): Query<AnalyticsQueryParams>,
) -> Result<
    Json<crate::analytics::AnalyticsGatewayScoresResponse>,
    error::ContainerError<error::ApiError>,
> {
    let query = parse_query(
        auth_context.merchant_id.clone(),
        params.range,
        params.start_ms,
        params.end_ms,
        params.page,
        params.page_size,
        params.payment_method_type,
        params.payment_method,
        params.card_network,
        params.card_is_in,
        params.currency,
        params.country,
        params.auth_type,
        params.gateway,
    );
    Ok(Json(fetch_gateway_scores(&state, &query).await?))
}

pub async fn decisions(
    TenantStateResolver(state): TenantStateResolver,
    AuthenticatedAnalyticsContext(auth_context): AuthenticatedAnalyticsContext,
    Query(params): Query<AnalyticsQueryParams>,
) -> Result<Json<crate::analytics::AnalyticsDecisionResponse>, error::ContainerError<error::ApiError>>
{
    let query = parse_query(
        auth_context.merchant_id.clone(),
        params.range,
        params.start_ms,
        params.end_ms,
        params.page,
        params.page_size,
        params.payment_method_type,
        params.payment_method,
        params.card_network,
        params.card_is_in,
        params.currency,
        params.country,
        params.auth_type,
        params.gateway,
    );
    Ok(Json(fetch_decisions(&state, &query).await?))
}

pub async fn routing_stats(
    TenantStateResolver(state): TenantStateResolver,
    AuthenticatedAnalyticsContext(auth_context): AuthenticatedAnalyticsContext,
    Query(params): Query<AnalyticsQueryParams>,
) -> Result<
    Json<crate::analytics::AnalyticsRoutingStatsResponse>,
    error::ContainerError<error::ApiError>,
> {
    let query = parse_query(
        auth_context.merchant_id.clone(),
        params.range,
        params.start_ms,
        params.end_ms,
        params.page,
        params.page_size,
        params.payment_method_type,
        params.payment_method,
        params.card_network,
        params.card_is_in,
        params.currency,
        params.country,
        params.auth_type,
        params.gateway,
    );
    Ok(Json(fetch_routing_stats(&state, &query).await?))
}

pub async fn log_summaries(
    TenantStateResolver(state): TenantStateResolver,
    AuthenticatedAnalyticsContext(auth_context): AuthenticatedAnalyticsContext,
    Query(params): Query<AnalyticsQueryParams>,
) -> Result<
    Json<crate::analytics::AnalyticsLogSummariesResponse>,
    error::ContainerError<error::ApiError>,
> {
    let query = parse_query(
        auth_context.merchant_id.clone(),
        params.range,
        params.start_ms,
        params.end_ms,
        params.page,
        params.page_size,
        params.payment_method_type,
        params.payment_method,
        params.card_network,
        params.card_is_in,
        params.currency,
        params.country,
        params.auth_type,
        params.gateway,
    );
    Ok(Json(fetch_log_summaries(&state, &query).await?))
}

pub async fn payment_audit(
    TenantStateResolver(state): TenantStateResolver,
    AuthenticatedAnalyticsContext(auth_context): AuthenticatedAnalyticsContext,
    Query(params): Query<AnalyticsQueryParams>,
) -> Result<Json<crate::analytics::PaymentAuditResponse>, error::ContainerError<error::ApiError>> {
    let query = parse_payment_audit_query(
        auth_context.merchant_id.clone(),
        params.range,
        params.start_ms,
        params.end_ms,
        params.page,
        params.page_size,
        params.payment_id,
        params.request_id,
        params.gateway,
        params.route,
        params.status,
        params.flow_type,
        params.error_code,
    );
    Ok(Json(fetch_payment_audit(&state, &query).await?))
}

pub async fn preview_trace(
    TenantStateResolver(state): TenantStateResolver,
    AuthenticatedAnalyticsContext(auth_context): AuthenticatedAnalyticsContext,
    Query(params): Query<AnalyticsQueryParams>,
) -> Result<Json<crate::analytics::PaymentAuditResponse>, error::ContainerError<error::ApiError>> {
    let query = parse_payment_audit_query(
        auth_context.merchant_id.clone(),
        params.range,
        params.start_ms,
        params.end_ms,
        params.page,
        params.page_size,
        params.payment_id,
        params.request_id,
        params.gateway,
        params.route,
        params.status,
        params.flow_type,
        params.error_code,
    );
    Ok(Json(fetch_preview_trace(&state, &query).await?))
}
