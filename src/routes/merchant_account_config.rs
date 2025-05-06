use crate::types::merchant as ETM;
use crate::{error, logger, types};
use axum::{extract::Path, Json};
use error_stack::ResultExt;

#[axum::debug_handler]
pub async fn get_merchant_config(
    Path(merchant_id): Path<String>,
) -> Result<
    Json<ETM::merchant_account::MerchantAccount>,
    error::ContainerError<error::MerchantAccountConfigurationError>,
> {
    logger::debug!(
        "Received request to get merchant account configuration for ID: {}",
        merchant_id
    );
    let merchant_account = ETM::merchant_account::load_merchant_by_merchant_id(merchant_id)
        .await
        .ok_or(error::MerchantAccountConfigurationError::MerchantNotFound)?;

    Ok(Json(merchant_account))
}

#[axum::debug_handler]
pub async fn create_merchant_config(
    Json(payload): Json<ETM::merchant_account::MerchantAccountCreateRequest>,
) -> Result<(), error::ContainerError<error::MerchantAccountConfigurationError>> {
    logger::debug!(
        "Received request to create merchant account configuration: {:?}",
        payload
    );

    ETM::merchant_account::insert_merchant_account(payload)
        .await
        .change_context(error::MerchantAccountConfigurationError::MerchantInsertionFailed)?;

    logger::debug!("Merchant account configuration created successfully");

    Ok(())
}

#[axum::debug_handler]
pub async fn delete_merchant_config(
    Path(merchant_id): Path<String>,
) -> Result<Json<String>, error::ContainerError<error::MerchantAccountConfigurationError>> {
    logger::debug!(
        "Received request to delete merchant account configuration for ID: {}",
        merchant_id
    );
    ETM::merchant_account::delete_merchant_account(merchant_id)
        .await
        .change_context(error::MerchantAccountConfigurationError::MerchantDeletionFailed)?;

    Ok(Json("Merchant account deleted successfully".to_string()))
}
