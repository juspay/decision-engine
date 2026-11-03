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
    ScopedJson {
        merchant,
        mut payload,
        ..
    }: ScopedJson<DecideRequest>,
) -> Result<Response, ContainerError<EuclidErrors>> {
    // A named profile is the scope routed for; everything downstream reads `merchant_id`.
    if payload.profile_id.take().is_some() {
        payload.merchant_id = merchant.to_string();
    }
    let state = crate::app::get_tenant_app_state().await;
    let config = state
        .config
        .routing_config
        .as_ref()
        .ok_or(EuclidErrors::GlobalRoutingConfigsUnavailable)?;
    let request = payload
        .to_hybrid_request(config)
        .map_err(EuclidErrors::InvalidRequest)?;
    let invalid_inputs = payload.invalid_rule_inputs(config);
    let payment_id = payload.payment_id.clone();
    evaluation::evaluate(
        ids,
        request,
        Some(payload),
        move |evaluation, request_id| {
            let decision =
                DecideResponse::from_evaluation(evaluation, invalid_inputs, payment_id, request_id);
            serde_json::to_value(decision)
                .map(Some)
                .map_err(|_| EuclidErrors::FailedToSerializeJsonToString.into())
        },
    )
    .await
}
