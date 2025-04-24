use crate::app::get_tenant_app_state;
use crate::types::merchant as ETM;
use crate::{error, logger, storage::types::ServiceConfigurationNew, types};
use axum::Json;
use chrono::format;
use error_stack::ResultExt;
use serde_json::{json, Value};

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
                format!("DEFAULT_SR_BASED_GATEWAY_ELIMINATION_INPUT"),
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
        version: 0,
    };

    crate::generics::generic_insert(&state.db, config)
        .await
        .change_context(error::RuleConfigurationError::StorageError)?;

    Ok(())
}
