use crate::custom_extractors::{ConnectedMerchant, Json, RequestIds};
use crate::error::RouteError;
use crate::euclid::errors::EuclidErrors;
use crate::types::decide::response::DecideResponse;
use crate::types::decide::DecideRequest;
use axum::response::Response;
use error_stack::ResultExt;

pub mod evaluation;

#[axum::debug_handler]
pub async fn decide(
    ids: RequestIds,
    ConnectedMerchant(merchant): ConnectedMerchant,
    Json(payload): Json<DecideRequest>,
) -> Result<Response, RouteError> {
    let payload = payload.for_merchant(&merchant);
    let state = crate::app::get_tenant_app_state().await;
    let config = state
        .config
        .routing_config
        .as_ref()
        .ok_or_else(|| RouteError::internal("routing configuration is unavailable"))?;
    let request = payload
        .to_hybrid_request(config)
        .map_err(RouteError::bad_request)?;
    let invalid_inputs = payload.invalid_rule_inputs(config);
    let payment_id = payload.payment_id.clone();
    let merchant_reference_id = payload.merchant_reference_id.clone();
    let response = evaluation::evaluate(
        ids,
        request,
        Some(payload),
        move |evaluation, _request_id| {
            let decision = DecideResponse::from_evaluation(
                evaluation,
                invalid_inputs,
                payment_id,
                merchant_reference_id,
            );
            let value = serde_json::to_value(decision)
                .change_context(EuclidErrors::FailedToSerializeJsonToString)?;
            Ok(Some(value))
        },
    )
    .await?;
    Ok(response)
}
