use crate::custom_extractors::{RequestIds, ScopedJson};
use crate::error::ContainerError;
use crate::euclid::errors::EuclidErrors;
use crate::types::decide::response::DecideResponse;
use crate::types::decide::DecideRequest;
use axum::response::Response;

pub mod evaluation;

#[axum::debug_handler]
pub async fn decide(
    ids: RequestIds,
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
    let payment_id = payload.payment_id.clone();
    evaluation::evaluate(
        ids,
        request,
        Some(payload),
        move |evaluation, request_id| {
            let decision = DecideResponse::from_evaluation(evaluation, payment_id, request_id);
            serde_json::to_value(decision)
                .map(Some)
                .map_err(|_| EuclidErrors::FailedToSerializeJsonToString.into())
        },
    )
    .await
}
