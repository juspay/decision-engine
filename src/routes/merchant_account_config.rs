use crate::metrics::{API_LATENCY_HISTOGRAM, API_REQUEST_COUNTER, API_REQUEST_TOTAL_COUNTER};
use crate::types::merchant as ETM;
use crate::{error, logger, metrics, types};
use axum::{extract::Path, Json};
use error_stack::ResultExt;
#[axum::debug_handler]
pub async fn get_merchant_config(
    Path(merchant_id): Path<String>,
) -> Result<
    Json<ETM::merchant_account::MerchantAccountResponse>,
    error::ContainerError<error::MerchantAccountConfigurationError>,
> {
    // Record total request count and start timer
    API_REQUEST_TOTAL_COUNTER
        .with_label_values(&["merchant_account_get"])
        .inc();
    let timer = API_LATENCY_HISTOGRAM
        .with_label_values(&["merchant_account_get"])
        .start_timer();

    logger::debug!(
        "Received request to get merchant account configuration for ID: {}",
        merchant_id
    );

    let result = ETM::merchant_account::load_merchant_by_merchant_id(merchant_id)
        .await
        .ok_or(error::MerchantAccountConfigurationError::MerchantNotFound);

    let response = match result {
        Ok(merchant_account) => {
            API_REQUEST_COUNTER
                .with_label_values(&["merchant_account_get", "success"])
                .inc();
            Ok(Json(merchant_account.into()))
        }
        Err(e) => {
            API_REQUEST_COUNTER
                .with_label_values(&["merchant_account_get", "failure"])
                .inc();
            Err(e.into())
        }
    };

    timer.observe_duration();
    response
}

#[axum::debug_handler]
pub async fn create_merchant_config(
    Json(payload): Json<ETM::merchant_account::MerchantAccountCreateRequest>,
) -> Result<Json<String>, error::ContainerError<error::MerchantAccountConfigurationError>> {
    // Record total request count and start timer
    API_REQUEST_TOTAL_COUNTER
        .with_label_values(&["merchant_account_create"])
        .inc();
    let timer = API_LATENCY_HISTOGRAM
        .with_label_values(&["merchant_account_create"])
        .start_timer();

    logger::debug!(
        "Received request to create merchant account configuration: {:?}",
        payload
    );

    let merchant_account =
        ETM::merchant_account::load_merchant_by_merchant_id(payload.merchant_id.clone()).await;

    if merchant_account.is_some() {
        API_REQUEST_COUNTER
            .with_label_values(&["merchant_account_create", "failure"])
            .inc();
        timer.observe_duration();
        return Err(error::MerchantAccountConfigurationError::MerchantAlreadyExists.into());
    }

    let result = ETM::merchant_account::insert_merchant_account(payload)
        .await
        .change_context(error::MerchantAccountConfigurationError::MerchantInsertionFailed);

    let response = match result {
        Ok(_) => {
            logger::debug!("Merchant account configuration created successfully");
            API_REQUEST_COUNTER
                .with_label_values(&["merchant_account_create", "success"])
                .inc();
            Ok(Json("Merchant account created successfully".to_string()))
        }
        Err(e) => {
            API_REQUEST_COUNTER
                .with_label_values(&["merchant_account_create", "failure"])
                .inc();
            Err(e.into())
        }
    };

    timer.observe_duration();
    response
}

#[axum::debug_handler]
pub async fn delete_merchant_config(
    Path(merchant_id): Path<String>,
) -> Result<Json<String>, error::ContainerError<error::MerchantAccountConfigurationError>> {
    // Record total request count and start timer
    API_REQUEST_TOTAL_COUNTER
        .with_label_values(&["merchant_account_delete"])
        .inc();
    let timer = API_LATENCY_HISTOGRAM
        .with_label_values(&["merchant_account_delete"])
        .start_timer();

    logger::debug!(
        "Received request to delete merchant account configuration for ID: {}",
        merchant_id
    );

    let result = ETM::merchant_account::delete_merchant_account(merchant_id)
        .await
        .change_context(error::MerchantAccountConfigurationError::MerchantDeletionFailed);

    let response = match result {
        Ok(_) => {
            API_REQUEST_COUNTER
                .with_label_values(&["merchant_account_delete", "success"])
                .inc();
            Ok(Json("Merchant account deleted successfully".to_string()))
        }
        Err(e) => {
            API_REQUEST_COUNTER
                .with_label_values(&["merchant_account_delete", "failure"])
                .inc();
            Err(e.into())
        }
    };

    timer.observe_duration();
    response
}
