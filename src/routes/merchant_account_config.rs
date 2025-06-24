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
    let start_time = std::time::Instant::now();
    metrics::MERCHANT_CONFIG_GET_METRICS_REQUEST.inc();

    logger::debug!(
        "Received request to get merchant account configuration for ID: {}",
        merchant_id
    );

    let result = ETM::merchant_account::load_merchant_by_merchant_id(merchant_id)
        .await
        .ok_or(error::MerchantAccountConfigurationError::MerchantNotFound);

    let response = match result {
        Ok(merchant_account) => {
            metrics::MERCHANT_CONFIG_GET_SUCCESSFUL_RESPONSE_COUNT.inc();
            Ok(Json(merchant_account.into()))
        }
        Err(e) => {
            metrics::MERCHANT_CONFIG_GET_UNSUCCESSFUL_RESPONSE_COUNT.inc();
            Err(e.into())
        }
    };

    metrics::MERCHANT_CONFIG_GET_METRICS_DECISION_REQUEST_TIME
        .observe(start_time.elapsed().as_secs_f64());
    response
}

#[axum::debug_handler]
pub async fn create_merchant_config(
    Json(payload): Json<ETM::merchant_account::MerchantAccountCreateRequest>,
) -> Result<Json<String>, error::ContainerError<error::MerchantAccountConfigurationError>> {
    let start_time = std::time::Instant::now();
    metrics::MERCHANT_CONFIG_CREATE_METRICS_REQUEST.inc();

    logger::debug!(
        "Received request to create merchant account configuration: {:?}",
        payload
    );

    let merchant_account =
        ETM::merchant_account::load_merchant_by_merchant_id(payload.merchant_id.clone()).await;

    if merchant_account.is_some() {
        metrics::MERCHANT_CONFIG_CREATE_UNSUCCESSFUL_RESPONSE_COUNT.inc();
        metrics::MERCHANT_CONFIG_CREATE_METRICS_DECISION_REQUEST_TIME
            .observe(start_time.elapsed().as_secs_f64());
        return Err(error::MerchantAccountConfigurationError::MerchantAlreadyExists.into());
    }

    let result = ETM::merchant_account::insert_merchant_account(payload)
        .await
        .change_context(error::MerchantAccountConfigurationError::MerchantInsertionFailed);

    let response = match result {
        Ok(_) => {
            logger::debug!("Merchant account configuration created successfully");
            metrics::MERCHANT_CONFIG_CREATE_SUCCESSFUL_RESPONSE_COUNT.inc();
            Ok(Json("Merchant account created successfully".to_string()))
        }
        Err(e) => {
            metrics::MERCHANT_CONFIG_CREATE_UNSUCCESSFUL_RESPONSE_COUNT.inc();
            Err(e.into())
        }
    };

    metrics::MERCHANT_CONFIG_CREATE_METRICS_DECISION_REQUEST_TIME
        .observe(start_time.elapsed().as_secs_f64());
    response
}

#[axum::debug_handler]
pub async fn delete_merchant_config(
    Path(merchant_id): Path<String>,
) -> Result<Json<String>, error::ContainerError<error::MerchantAccountConfigurationError>> {
    let start_time = std::time::Instant::now();
    metrics::MERCHANT_CONFIG_DELETE_METRICS_REQUEST.inc();

    logger::debug!(
        "Received request to delete merchant account configuration for ID: {}",
        merchant_id
    );

    let result = ETM::merchant_account::delete_merchant_account(merchant_id)
        .await
        .change_context(error::MerchantAccountConfigurationError::MerchantDeletionFailed);

    let response = match result {
        Ok(_) => {
            metrics::MERCHANT_CONFIG_DELETE_SUCCESSFUL_RESPONSE_COUNT.inc();
            Ok(Json("Merchant account deleted successfully".to_string()))
        }
        Err(e) => {
            metrics::MERCHANT_CONFIG_DELETE_UNSUCCESSFUL_RESPONSE_COUNT.inc();
            Err(e.into())
        }
    };

    metrics::MERCHANT_CONFIG_DELETE_METRICS_DECISION_REQUEST_TIME
        .observe(start_time.elapsed().as_secs_f64());
    response
}
