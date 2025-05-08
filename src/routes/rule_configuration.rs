use crate::types::merchant as ETM;
use crate::{error, logger, types};
use axum::Json;
use error_stack::ResultExt;

#[axum::debug_handler]
pub async fn create_rule_config(
    Json(payload): Json<types::routing_configuration::RoutingRule>,
) -> Result<Json<String>, error::ContainerError<error::RuleConfigurationError>> {
    logger::debug!("Received rule configuration: {:?}", payload);

    let mid = payload.merchant_id.clone();
    ETM::merchant_account::load_merchant_by_merchant_id(mid.clone())
        .await
        .ok_or(error::RuleConfigurationError::MerchantNotFound)?;

    match payload.config {
        types::routing_configuration::ConfigVariant::SuccessRate(config) => {
            let name = format!("SR_V3_INPUT_CONFIG_{}", mid);
            let config = serde_json::to_string(&config)
                .map_err(|_| error::RuleConfigurationError::StorageError)?;

            match types::service_configuration::find_config_by_name(name.clone())
                .await
                .change_context(error::RuleConfigurationError::StorageError)?
            {
                Some(_) => {
                    return Err(error::RuleConfigurationError::ConfigurationAlreadyExists.into());
                }
                None => types::service_configuration::insert_config(name, Some(config))
                    .await
                    .change_context(error::RuleConfigurationError::StorageError)?,
            }

            Ok(Json(
                "Success Rate Configuration created successfully".to_string(),
            ))
        }
        types::routing_configuration::ConfigVariant::Elimination(config) => {
            let db_config = types::gateway_routing_input::GatewaySuccessRateBasedRoutingInput::from_elimination_threshold(config.threshold);
            let config = serde_json::to_string(&db_config)
                .map_err(|_| error::RuleConfigurationError::StorageError)?;

            types::merchant::merchant_account::update_merchant_account(mid, Some(config))
                .await
                .change_context(error::RuleConfigurationError::StorageError)?;

            Ok(Json(
                "Elimination Configuration created successfully".to_string(),
            ))
        }
        types::routing_configuration::ConfigVariant::DebitRouting(config) => {
            let name = format!("DEBIT_ROUTING_CONFIG_{}", mid);
            let config = serde_json::to_string(&config)
                .map_err(|_| error::RuleConfigurationError::StorageError)?;

            match types::service_configuration::find_config_by_name(name.clone())
                .await
                .change_context(error::RuleConfigurationError::StorageError)?
            {
                Some(_) => {
                    return Err(error::RuleConfigurationError::ConfigurationAlreadyExists.into());
                }
                None => types::service_configuration::insert_config(name, Some(config))
                    .await
                    .change_context(error::RuleConfigurationError::StorageError)?,
            }

            Ok(Json(
                "Debit Routing Configuration created successfully".to_string(),
            ))
        }
    }
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
    let merchant_account = ETM::merchant_account::load_merchant_by_merchant_id(mid.clone())
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
                merchant_id: mid,
                config: types::routing_configuration::ConfigVariant::SuccessRate(
                    success_rate_config,
                ),
            }))
        }
        types::routing_configuration::AlgorithmType::Elimination => {
            let db_config = merchant_account.gatewaySuccessRateBasedDeciderInput;

            let config = serde_json::from_str::<
                types::gateway_routing_input::GatewaySuccessRateBasedRoutingInput,
            >(&db_config)
            .map_err(|_| error::RuleConfigurationError::DeserializationError)?;

            let elimination_config = types::routing_configuration::EliminationData {
                threshold: config.defaultEliminationThreshold,
            };

            Ok(Json(types::routing_configuration::RoutingRule {
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
) -> Result<Json<String>, error::ContainerError<error::RuleConfigurationError>> {
    logger::debug!("Received rule update configuration: {:?}", payload);

    let mid = payload.merchant_id.clone();
    ETM::merchant_account::load_merchant_by_merchant_id(mid.clone())
        .await
        .ok_or(error::RuleConfigurationError::MerchantNotFound)?;

    // Update DB call for updating the rule configuration
    match payload.config {
        types::routing_configuration::ConfigVariant::SuccessRate(config) => {
            let name = format!("SR_V3_INPUT_CONFIG_{}", mid);
            let config = serde_json::to_string(&config)
                .map_err(|_| error::RuleConfigurationError::StorageError)?;

            types::service_configuration::update_config(name, Some(config))
                .await
                .change_context(error::RuleConfigurationError::StorageError)?;

            Ok(Json(
                "Success Rate Configuration updated successfully".to_string(),
            ))
        }
        types::routing_configuration::ConfigVariant::Elimination(config) => {
            let db_config = types::gateway_routing_input::GatewaySuccessRateBasedRoutingInput::from_elimination_threshold(config.threshold);
            let config = serde_json::to_string(&db_config)
                .map_err(|_| error::RuleConfigurationError::StorageError)?;

            types::merchant::merchant_account::update_merchant_account(mid, Some(config))
                .await
                .change_context(error::RuleConfigurationError::StorageError)?;

            Ok(Json(
                "Elimination Configuration created successfully".to_string(),
            ))
        }
        types::routing_configuration::ConfigVariant::DebitRouting(config) => {
            let name = format!("DEBIT_ROUTING_CONFIG_{}", mid);
            let config = serde_json::to_string(&config)
                .map_err(|_| error::RuleConfigurationError::StorageError)?;

            types::service_configuration::update_config(name, Some(config))
                .await
                .change_context(error::RuleConfigurationError::StorageError)?;

            Ok(Json(
                "Debit Routing Configuration updated successfully".to_string(),
            ))
        }
    }
}

#[axum::debug_handler]
pub async fn delete_rule_config(
    Json(payload): Json<types::routing_configuration::FetchRoutingRule>,
) -> Result<Json<String>, error::ContainerError<error::RuleConfigurationError>> {
    logger::debug!("Received rule delete request: {:?}", payload);

    let mid = payload.merchant_id.clone();
    ETM::merchant_account::load_merchant_by_merchant_id(mid.clone())
        .await
        .ok_or(error::RuleConfigurationError::MerchantNotFound)?;

    // Delete DB call for deleting the rule configuration
    match payload.algorithm {
        types::routing_configuration::AlgorithmType::SuccessRate => {
            let config_name = format!("SR_V3_INPUT_CONFIG_{}", mid);
            types::service_configuration::delete_config(config_name)
                .await
                .change_context(error::RuleConfigurationError::StorageError)?;

            Ok(Json(
                "Success Rate Configuration deleted successfully".to_string(),
            ))
        }
        types::routing_configuration::AlgorithmType::Elimination => {
            types::merchant::merchant_account::update_merchant_account(mid, Some("".to_string())) // update to empty string
                .await
                .change_context(error::RuleConfigurationError::StorageError)?;

            Ok(Json(
                "Elimination Configuration deleted successfully".to_string(),
            ))
        }
        types::routing_configuration::AlgorithmType::DebitRouting => {
            let config_name = format!("DEBIT_ROUTING_CONFIG_{}", mid);
            types::service_configuration::delete_config(config_name)
                .await
                .change_context(error::RuleConfigurationError::StorageError)?;

            Ok(Json(
                "Debit Routing Configuration deleted successfully".to_string(),
            ))
        }
    }
}
