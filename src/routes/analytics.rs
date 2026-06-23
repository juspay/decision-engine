use crate::analytics::{
    cost_savings as fetch_cost_savings, decisions as fetch_decisions,
    experiment_results as fetch_experiment_results,
    experiment_transactions as fetch_experiment_transactions,
    gateway_scores as fetch_gateway_scores, log_summaries as fetch_log_summaries,
    overview as fetch_overview, payment_audit as fetch_payment_audit,
    preview_trace as fetch_preview_trace, routing_events as fetch_routing_events,
    routing_stats as fetch_routing_stats, AnalyticsQuery, ExperimentResultsQuery,
    ExperimentTransactionsQuery, PaymentAuditQuery, RoutingEventsQuery,
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
    pub routing_approach: Option<String>,
    pub exclude_routing_approach: Option<String>,
    pub error_code: Option<String>,
}

fn analytics_query_from_params(
    merchant_id: String,
    params: &AnalyticsQueryParams,
) -> AnalyticsQuery {
    AnalyticsQuery::from_request(
        merchant_id,
        params.range.clone(),
        params.start_ms,
        params.end_ms,
        params.page,
        params.page_size,
        params.payment_method_type.clone(),
        params.payment_method.clone(),
        params.card_network.clone(),
        params.card_is_in.clone(),
        params.currency.clone(),
        params.country.clone(),
        params.auth_type.clone(),
        params.gateway.clone(),
    )
}

fn payment_audit_query_from_params(
    merchant_id: String,
    params: &AnalyticsQueryParams,
) -> PaymentAuditQuery {
    PaymentAuditQuery::from_request(
        merchant_id,
        params.range.clone(),
        params.start_ms,
        params.end_ms,
        params.page,
        params.page_size,
        params.payment_id.clone(),
        params.request_id.clone(),
        params.gateway.clone(),
        params.route.clone(),
        params.status.clone(),
        params.flow_type.clone(),
        params.routing_approach.clone(),
        params.exclude_routing_approach.clone(),
        params.error_code.clone(),
    )
}

pub fn serve() -> axum::Router<Arc<crate::tenant::GlobalAppState>> {
    axum::Router::<Arc<crate::tenant::GlobalAppState>>::new()
        .route("/", axum::routing::get(overview))
        .route("/overview", axum::routing::get(overview))
        .route("/gateway-scores", axum::routing::get(gateway_scores))
        .route("/decisions", axum::routing::get(decisions))
        .route("/routing-stats", axum::routing::get(routing_stats))
        .route("/cost-savings", axum::routing::get(cost_savings))
        .route("/routing-events", axum::routing::get(routing_events))
        .route("/log-summaries", axum::routing::get(log_summaries))
        .route("/payment-audit", axum::routing::get(payment_audit))
        .route("/preview-trace", axum::routing::get(preview_trace))
        .route(
            "/experiment/:experiment_id/results",
            axum::routing::get(experiment_results),
        )
        .route(
            "/experiment/:experiment_id/transactions",
            axum::routing::get(experiment_transactions),
        )
}

pub async fn overview(
    TenantStateResolver(state): TenantStateResolver,
    AuthenticatedAnalyticsContext(auth_context): AuthenticatedAnalyticsContext,
    Query(params): Query<AnalyticsQueryParams>,
) -> Result<Json<crate::analytics::AnalyticsOverviewResponse>, error::ContainerError<error::ApiError>>
{
    let query = analytics_query_from_params(auth_context.merchant_id.clone(), &params);
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
    let query = analytics_query_from_params(auth_context.merchant_id.clone(), &params);
    Ok(Json(fetch_gateway_scores(&state, &query).await?))
}

pub async fn decisions(
    TenantStateResolver(state): TenantStateResolver,
    AuthenticatedAnalyticsContext(auth_context): AuthenticatedAnalyticsContext,
    Query(params): Query<AnalyticsQueryParams>,
) -> Result<Json<crate::analytics::AnalyticsDecisionResponse>, error::ContainerError<error::ApiError>>
{
    let query = analytics_query_from_params(auth_context.merchant_id.clone(), &params);
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
    let query = analytics_query_from_params(auth_context.merchant_id.clone(), &params);
    Ok(Json(fetch_routing_stats(&state, &query).await?))
}

pub async fn cost_savings(
    TenantStateResolver(state): TenantStateResolver,
    AuthenticatedAnalyticsContext(auth_context): AuthenticatedAnalyticsContext,
    Query(params): Query<AnalyticsQueryParams>,
) -> Result<
    Json<crate::analytics::AnalyticsCostSavingsResponse>,
    error::ContainerError<error::ApiError>,
> {
    let query = analytics_query_from_params(auth_context.merchant_id.clone(), &params);
    Ok(Json(fetch_cost_savings(&state, &query).await?))
}

pub async fn log_summaries(
    TenantStateResolver(state): TenantStateResolver,
    AuthenticatedAnalyticsContext(auth_context): AuthenticatedAnalyticsContext,
    Query(params): Query<AnalyticsQueryParams>,
) -> Result<
    Json<crate::analytics::AnalyticsLogSummariesResponse>,
    error::ContainerError<error::ApiError>,
> {
    let query = analytics_query_from_params(auth_context.merchant_id.clone(), &params);
    Ok(Json(fetch_log_summaries(&state, &query).await?))
}

pub async fn payment_audit(
    TenantStateResolver(state): TenantStateResolver,
    AuthenticatedAnalyticsContext(auth_context): AuthenticatedAnalyticsContext,
    Query(params): Query<AnalyticsQueryParams>,
) -> Result<Json<crate::analytics::PaymentAuditResponse>, error::ContainerError<error::ApiError>> {
    let query = payment_audit_query_from_params(auth_context.merchant_id.clone(), &params);
    Ok(Json(fetch_payment_audit(&state, &query).await?))
}

pub async fn preview_trace(
    TenantStateResolver(state): TenantStateResolver,
    AuthenticatedAnalyticsContext(auth_context): AuthenticatedAnalyticsContext,
    Query(params): Query<AnalyticsQueryParams>,
) -> Result<Json<crate::analytics::PaymentAuditResponse>, error::ContainerError<error::ApiError>> {
    let query = payment_audit_query_from_params(auth_context.merchant_id.clone(), &params);
    Ok(Json(fetch_preview_trace(&state, &query).await?))
}

#[derive(Debug, Clone, Deserialize)]
pub struct RoutingEventsParams {
    pub range: Option<String>,
    pub start_ms: Option<i64>,
    pub end_ms: Option<i64>,
    pub payment_method_type: Option<String>,
    pub payment_method: Option<String>,
    pub min_transaction_count: Option<i64>,
    pub min_score_delta: Option<f64>,
    pub tolerance_pp: Option<f64>,
    pub limit: Option<u32>,
    pub bucket: Option<String>,
}

pub async fn routing_events(
    TenantStateResolver(state): TenantStateResolver,
    AuthenticatedAnalyticsContext(auth_context): AuthenticatedAnalyticsContext,
    Query(params): Query<RoutingEventsParams>,
) -> Result<Json<crate::analytics::RoutingEventsResponse>, error::ContainerError<error::ApiError>> {
    // Auth-band events are a multi-objective concept, so only surface them when MO
    // routing is enabled for the merchant (same feature flag the decider checks).
    // When on, mirror the live decision band: caller override → configured tolerance.
    // When off, leave tolerance unset so only LeaderChanged events are emitted.
    let tolerance_pp =
        resolve_auth_band_tolerance(&auth_context.merchant_id, params.tolerance_pp).await;
    let query = RoutingEventsQuery::from_request(
        auth_context.merchant_id.clone(),
        params.range,
        params.start_ms,
        params.end_ms,
        params.payment_method_type,
        params.payment_method,
        params.min_transaction_count,
        params.min_score_delta,
        tolerance_pp,
        params.limit,
        params.bucket,
    );
    Ok(Json(fetch_routing_events(&state, &query).await?))
}

/// Auth-band tolerance for routing-events detection. `None` when multi-objective
/// routing is off for the merchant (the band is meaningless, so no entered/exited
/// events fire). When on, an explicit caller override wins; otherwise the merchant's
/// configured `default_tolerance_pp`, falling back to the multi-objective default —
/// matching exactly what the live decider applies.
async fn resolve_auth_band_tolerance(merchant_id: &str, explicit: Option<f64>) -> Option<f64> {
    let multi_objective_on = crate::redis::feature::is_feature_enabled(
        "multi_objective_routing_enabled".to_string(),
        merchant_id.to_string(),
        crate::feedback::constants::kvRedis(),
    )
    .await;
    if !multi_objective_on {
        return None;
    }
    let tolerance = match explicit {
        Some(value) => value,
        None => crate::decider::gatewaydecider::flow_new::load_default_tolerance_pp(merchant_id)
            .await
            .unwrap_or(crate::decider::gatewaydecider::multi_objective::DEFAULT_TOLERANCE_BAND_PP),
    };
    Some(tolerance)
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExperimentResultsParams {
    pub start_ms: Option<i64>,
    pub end_ms: Option<i64>,
    pub min_sample_size: Option<u32>,
    pub guardrail_threshold_pp: Option<f64>,
}

pub async fn experiment_results(
    TenantStateResolver(state): TenantStateResolver,
    AuthenticatedAnalyticsContext(auth_context): AuthenticatedAnalyticsContext,
    axum::extract::Path(experiment_id): axum::extract::Path<String>,
    Query(params): Query<ExperimentResultsParams>,
) -> Result<Json<crate::analytics::ExperimentResultsResponse>, error::ContainerError<error::ApiError>>
{
    let query = ExperimentResultsQuery {
        experiment_id,
        merchant_id: auth_context.merchant_id.clone(),
        start_ms: params.start_ms,
        end_ms: params.end_ms,
        min_sample_size: params.min_sample_size.unwrap_or(1000),
        guardrail_threshold_pp: params.guardrail_threshold_pp.unwrap_or(3.0),
    };
    Ok(Json(fetch_experiment_results(&state, &query).await?))
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExperimentTransactionsParams {
    pub start_ms: Option<i64>,
    pub page: Option<u64>,
    pub page_size: Option<u64>,
}

pub async fn experiment_transactions(
    TenantStateResolver(state): TenantStateResolver,
    AuthenticatedAnalyticsContext(auth_context): AuthenticatedAnalyticsContext,
    axum::extract::Path(experiment_id): axum::extract::Path<String>,
    Query(params): Query<ExperimentTransactionsParams>,
) -> Result<
    Json<crate::analytics::ExperimentTransactionsResponse>,
    error::ContainerError<error::ApiError>,
> {
    let query = ExperimentTransactionsQuery {
        experiment_id,
        merchant_id: auth_context.merchant_id.clone(),
        start_ms: params.start_ms,
        page: params.page.unwrap_or(1),
        page_size: params.page_size.unwrap_or(50).min(100),
    };
    Ok(Json(fetch_experiment_transactions(&state, &query).await?))
}
