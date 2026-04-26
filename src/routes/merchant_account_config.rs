use crate::app::APP_STATE;
use crate::metrics::{API_LATENCY_HISTOGRAM, API_REQUEST_COUNTER, API_REQUEST_TOTAL_COUNTER};
use crate::types::merchant as ETM;
use crate::types::service_configuration;
use crate::{error, logger};
use axum::{extract::Path, http::HeaderMap, Json};
use error_stack::ResultExt;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MerchantAccountCreateResponse {
    pub message: String,
    pub merchant_id: String,
    pub gateway_success_rate_based_decider_input: Option<String>,
    pub api_key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MerchantAccountDeleteResponse {
    pub message: String,
    pub merchant_id: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DebitRoutingRequest {
    pub enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DebitRoutingResponse {
    pub merchant_id: String,
    pub debit_routing_enabled: bool,
}

fn debit_routing_config_name(merchant_id: &str) -> String {
    format!("DEBIT_ROUTING_ENABLED_{}", merchant_id)
}

#[axum::debug_handler]
pub async fn get_debit_routing(
    Path(merchant_id): Path<String>,
) -> Result<
    Json<DebitRoutingResponse>,
    error::ContainerError<error::MerchantAccountConfigurationError>,
> {
    API_REQUEST_TOTAL_COUNTER
        .with_label_values(&["merchant_debit_routing_get"])
        .inc();
    let timer = API_LATENCY_HISTOGRAM
        .with_label_values(&["merchant_debit_routing_get"])
        .start_timer();

    logger::debug!(
        "Received request to get debit routing flag for merchant {}",
        merchant_id
    );

    let response = async {
        ETM::merchant_account::load_merchant_by_merchant_id(merchant_id.clone())
            .await
            .ok_or(error::MerchantAccountConfigurationError::MerchantNotFound)?;

        let config_name = debit_routing_config_name(&merchant_id);
        let debit_routing_enabled = service_configuration::find_config_by_name(config_name)
            .await
            .change_context(error::MerchantAccountConfigurationError::StorageError)?
            .and_then(|config| config.value)
            .and_then(|value| value.parse::<bool>().ok())
            .unwrap_or(false);

        Ok(Json(DebitRoutingResponse {
            merchant_id,
            debit_routing_enabled,
        }))
    }
    .await;

    match &response {
        Ok(_) => API_REQUEST_COUNTER
            .with_label_values(&["merchant_debit_routing_get", "success"])
            .inc(),
        Err(_) => API_REQUEST_COUNTER
            .with_label_values(&["merchant_debit_routing_get", "failure"])
            .inc(),
    }

    timer.observe_duration();
    response
}

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
    headers: HeaderMap,
    Json(payload): Json<ETM::merchant_account::MerchantAccountCreateRequest>,
) -> Result<
    Json<MerchantAccountCreateResponse>,
    error::ContainerError<error::MerchantAccountConfigurationError>,
> {
    let global_config = APP_STATE
        .get()
        .map(|s| s.global_config.clone())
        .ok_or(error::MerchantAccountConfigurationError::StorageError)?;

    let provided = headers
        .get("x-admin-secret")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if provided != global_config.admin_secret.secret {
        return Err(error::MerchantAccountConfigurationError::Unauthorized.into());
    }
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

    let merchant_id = payload.merchant_id.clone();
    let gateway_success_rate_based_decider_input =
        payload.gateway_success_rate_based_decider_input.clone();

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
            let api_key = crate::routes::api_key::insert_api_key_for_merchant(
                &merchant_id,
                Some("Default API key".to_string()),
            )
            .await;
            Ok(Json(MerchantAccountCreateResponse {
                message: "Merchant account created successfully".to_string(),
                merchant_id,
                gateway_success_rate_based_decider_input,
                api_key,
            }))
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
pub async fn update_debit_routing(
    Path(merchant_id): Path<String>,
    Json(payload): Json<DebitRoutingRequest>,
) -> Result<
    Json<DebitRoutingResponse>,
    error::ContainerError<error::MerchantAccountConfigurationError>,
> {
    API_REQUEST_TOTAL_COUNTER
        .with_label_values(&["merchant_debit_routing_update"])
        .inc();
    let timer = API_LATENCY_HISTOGRAM
        .with_label_values(&["merchant_debit_routing_update"])
        .start_timer();

    logger::debug!(
        "Received request to update debit routing for merchant {}: enabled={}",
        merchant_id,
        payload.enabled
    );

    // Verify merchant exists
    ETM::merchant_account::load_merchant_by_merchant_id(merchant_id.clone())
        .await
        .ok_or(error::MerchantAccountConfigurationError::MerchantNotFound)?;

    let config_name = debit_routing_config_name(&merchant_id);
    let config_value = payload.enabled.to_string();

    // Check if config already exists
    let existing_config = service_configuration::find_config_by_name(config_name.clone())
        .await
        .change_context(error::MerchantAccountConfigurationError::StorageError)?;

    let result = if existing_config.is_some() {
        // Update existing config
        service_configuration::update_config(config_name, Some(config_value))
            .await
            .change_context(error::MerchantAccountConfigurationError::StorageError)
    } else {
        // Insert new config
        service_configuration::insert_config(config_name, Some(config_value))
            .await
            .change_context(error::MerchantAccountConfigurationError::StorageError)
    };

    let response = match result {
        Ok(_) => {
            API_REQUEST_COUNTER
                .with_label_values(&["merchant_debit_routing_update", "success"])
                .inc();
            Ok(Json(DebitRoutingResponse {
                merchant_id: merchant_id.clone(),
                debit_routing_enabled: payload.enabled,
            }))
        }
        Err(e) => {
            API_REQUEST_COUNTER
                .with_label_values(&["merchant_debit_routing_update", "failure"])
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
) -> Result<
    Json<MerchantAccountDeleteResponse>,
    error::ContainerError<error::MerchantAccountConfigurationError>,
> {
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

    let result = ETM::merchant_account::delete_merchant_account(merchant_id.clone())
        .await
        .change_context(error::MerchantAccountConfigurationError::MerchantDeletionFailed);

    let response = match result {
        Ok(_) => {
            API_REQUEST_COUNTER
                .with_label_values(&["merchant_account_delete", "success"])
                .inc();
            Ok(Json(MerchantAccountDeleteResponse {
                message: "Merchant account deleted successfully".to_string(),
                merchant_id,
            }))
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
