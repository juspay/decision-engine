use crate::custom_extractors::{ConnectedMerchant, Json, RequestIds};
use crate::error::RouteError;
use crate::euclid::errors::EuclidErrors;
use crate::types::decide::response::DecideResponse;
use crate::types::decide::DecideRequest;
use axum::response::Response;

pub mod evaluation;

#[axum::debug_handler]
pub async fn decide(
    ids: RequestIds,
    ConnectedMerchant(merchant): ConnectedMerchant,
    Json(payload): Json<DecideRequest>,
) -> Result<Response, RouteError> {
    let response = decide_payment(ids, payload.for_merchant(&merchant)).await?;
    let status = response.status();
    if status.is_success() {
        Ok(response)
    } else if status.is_client_error() {
        // The inner engine's error names internal routing keys; it is logged, not returned.
        Err(RouteError::bad_request(
            "Unable to decide a connector for this payment",
        ))
    } else {
        Err(RouteError::internal(status))
    }
}

async fn decide_payment(ids: RequestIds, payload: DecideRequest) -> Result<Response, RouteError> {
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
    evaluation::evaluate(
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
            serde_json::to_value(decision)
                .map(Some)
                .map_err(|_| EuclidErrors::FailedToSerializeJsonToString.into())
        },
    )
    .await
    .map_err(RouteError::internal)
}
