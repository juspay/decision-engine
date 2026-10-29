use crate::custom_extractors::ScopedJson;
use crate::error::ContainerError;
use crate::euclid::errors::EuclidErrors;
use crate::types::decide::DecideRequest;
use axum::response::Response;

#[axum::debug_handler]
pub async fn decide(
    headers: axum::http::HeaderMap,
    ScopedJson { payload, .. }: ScopedJson<DecideRequest>,
) -> Result<Response, ContainerError<EuclidErrors>> {
    let state = crate::app::get_tenant_app_state().await;
    let config = state
        .config
        .routing_config
        .as_ref()
        .ok_or(EuclidErrors::GlobalRoutingConfigsUnavailable)?;
    let request = payload
        .to_hybrid_request(config)
        .map_err(EuclidErrors::InvalidRequest)?;
    super::hybrid_routing::evaluate_routing(headers, request, Some(payload)).await
}
