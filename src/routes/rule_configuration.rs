use crate::app::get_tenant_app_state;
use crate::types::merchant as ETM;
use crate::{error, logger, storage::types::ServiceConfigurationNew, types};
use axum::Json;
use error_stack::ResultExt;

#[axum::debug_handler]
pub async fn create_rule_config(
    Json(payload): Json<types::routing_configuration::RoutingRule>,
) -> Result<(), error::ContainerError<error::RuleConfigurationError>> {
    logger::debug!("Received rule configuration: {:?}", payload);
    let state = get_tenant_app_state().await;

    let mid = payload.merchant_id.clone();
    ETM::merchant_iframe_preferences::getMerchantIPrefsByMId(mid.clone())
        .await
        .ok_or(error::RuleConfigurationError::MerchantNotFound)?;

    let (name, config) = match payload.config {
        types::routing_configuration::ConfigVariant::SuccessRate(config) => (
            format!("SR_V3_INPUT_CONFIG_{}", mid),
            serde_json::to_string(&config)
                .map_err(|_| error::RuleConfigurationError::StorageError)
                .ok(),
        ),
        types::routing_configuration::ConfigVariant::Elimination(config) => {
            let db_config = types::gateway_routing_input::GatewaySuccessRateBasedRoutingInput::from_elimination_threshold(config.threshold);
            (
                format!("DEFAULT_SR_BASED_GATEWAY_ELIMINATION_INPUT"), // Need to decide on key name
                serde_json::to_string(&db_config)
                    .map_err(|_| error::RuleConfigurationError::StorageError)
                    .ok(),
            )
        }
        types::routing_configuration::ConfigVariant::DebitRouting(config) => (
            format!("DEBIT_ROUTING_CONFIG_{}", mid),
            serde_json::to_string(&config)
                .map_err(|_| error::RuleConfigurationError::StorageError)
                .ok(),
        ),
    };

    let config = ServiceConfigurationNew {
        name,
        value: config,
        new_value: None,
        previous_value: None,
        new_value_status: None,
    };

    crate::generics::generic_insert(&state.db, config)
        .await
        .change_context(error::RuleConfigurationError::StorageError)?;

    Ok(())
}

#[axum::debug_handler]
pub async fn get_rule_config(
    Json(payload): Json<types::routing_configuration::FetchRoutingRule>,
) -> Result<
    Json<types::routing_configuration::RoutingRule>,
    error::ContainerError<error::RuleConfigurationError>,
> {
    logger::debug!("Received rule fetch request: {:?}", payload);

    let mid = payload.merchant_id.clone();
    ETM::merchant_iframe_preferences::getMerchantIPrefsByMId(mid.clone())
        .await
        .ok_or(error::RuleConfigurationError::MerchantNotFound)?;

    match payload.algorithm {
        types::routing_configuration::AlgorithmType::SuccessRate => {
            let config_name = format!("SR_V3_INPUT_CONFIG_{}", mid);
            let config = types::service_configuration::find_config_by_name(config_name.clone())
                .await
                .change_context(error::RuleConfigurationError::StorageError)?
                .and_then(|config| config.value)
                .ok_or(error::RuleConfigurationError::ConfigurationNotFound)?;

            let success_rate_config: types::routing_configuration::SuccessRateData =
                serde_json::from_str(&config)
                    .map_err(|_| error::RuleConfigurationError::StorageError)?;

            Ok(Json(types::routing_configuration::RoutingRule {
                name: config_name,
                description: "Success Rate Configuration".to_string(),
                merchant_id: mid,
                config: types::routing_configuration::ConfigVariant::SuccessRate(
                    success_rate_config,
                ),
            }))
        }
        types::routing_configuration::AlgorithmType::Elimination => {
            let config_name = format!("DEFAULT_SR_BASED_GATEWAY_ELIMINATION_INPUT");
            let config = types::service_configuration::find_config_by_name(config_name.clone())
                .await
                .change_context(error::RuleConfigurationError::StorageError)?
                .and_then(|config| config.value)
                .ok_or(error::RuleConfigurationError::ConfigurationNotFound)?;

            let elimination_config: types::routing_configuration::EliminationData =
                serde_json::from_str(&config)
                    .map_err(|_| error::RuleConfigurationError::StorageError)?;

            Ok(Json(types::routing_configuration::RoutingRule {
                name: config_name,
                description: "Elimination Configuration".to_string(),
                merchant_id: mid,
                config: types::routing_configuration::ConfigVariant::Elimination(
                    elimination_config,
                ),
            }))
        }
        types::routing_configuration::AlgorithmType::DebitRouting => {
            let config_name = format!("DEBIT_ROUTING_CONFIG_{}", mid);
            let config = types::service_configuration::find_config_by_name(config_name.clone())
                .await
                .change_context(error::RuleConfigurationError::StorageError)?
                .and_then(|config| config.value)
                .ok_or(error::RuleConfigurationError::ConfigurationNotFound)?;

            let debit_routing_config: types::routing_configuration::DebitRoutingData =
                serde_json::from_str(&config)
                    .map_err(|_| error::RuleConfigurationError::StorageError)?;

            Ok(Json(types::routing_configuration::RoutingRule {
                name: config_name,
                description: "Debit Routing Configuration".to_string(),
                merchant_id: mid,
                config: types::routing_configuration::ConfigVariant::DebitRouting(
                    debit_routing_config,
                ),
            }))
        }
    }
}

#[axum::debug_handler]
pub async fn update_rule_config(
    Json(payload): Json<types::routing_configuration::RoutingRule>,
) -> Result<(), error::ContainerError<error::RuleConfigurationError>> {
    logger::debug!("Received rule configuration: {:?}", payload);
    let state = get_tenant_app_state().await;

    let mid = payload.merchant_id.clone();
    ETM::merchant_iframe_preferences::getMerchantIPrefsByMId(mid.clone())
        .await
        .ok_or(error::RuleConfigurationError::MerchantNotFound)?;

    // Update DB call for updating the rule configuration
    Ok(())
}

#[axum::debug_handler]
pub async fn delete_rule_config(
    Json(payload): Json<types::routing_configuration::FetchRoutingRule>,
) -> Result<(), error::ContainerError<error::RuleConfigurationError>> {
    logger::debug!("Received rule fetch request: {:?}", payload);

    let mid = payload.merchant_id.clone();
    ETM::merchant_iframe_preferences::getMerchantIPrefsByMId(mid.clone())
        .await
        .ok_or(error::RuleConfigurationError::MerchantNotFound)?;

    // Delete DB call for deleting the rule configuration
    Ok(())
}
