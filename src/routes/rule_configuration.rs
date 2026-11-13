use crate::auth::AuthorizedMerchant;
use crate::custom_extractors::{Json, RequestIds, ScopedJson};
use crate::types::merchant as ETM;
use crate::types::routing_configuration::{AlgorithmType, ConfigVariant, RuleConfigGetResponse};
use crate::types::service_configuration::{
    self as config_store, ConfigError, DebitRoutingRule, SrV3Input,
};
use crate::{error, logger, types};
use error_stack::ResultExt;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuleConfigResponse {
    pub message: String,
    pub merchant_id: String,
    pub config: types::routing_configuration::ConfigVariant,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RuleConfigDeleteResponse {
    pub message: String,
    pub merchant_id: String,
}

#[derive(Debug, Serialize)]
struct RuleConfigAnalyticsDetails<'a, Request, Response> {
    request: &'a Request,
    response: &'a Response,
}

/// The audit event every rule-config operation records once it succeeds.
fn record_rule_config_event<Request: Serialize, Response: Serialize>(
    ids: RequestIds,
    flow_type: crate::analytics::FlowType,
    route: crate::analytics::AnalyticsRoute,
    merchant: &AuthorizedMerchant,
    (request, response): (&Request, &Response),
    event_stage: &str,
) {
    crate::analytics::DomainAnalyticsEvent::record_operation(
        crate::analytics::AnalyticsFlowContext::new(
            crate::analytics::ApiFlow::RuleBasedRouting,
            flow_type,
        ),
        route,
        Some(merchant.as_str().to_string()),
        None,
        ids.request_id,
        ids.global_request_id,
        ids.trace_id,
        Some("success".to_string()),
        crate::analytics::serialize_details(&RuleConfigAnalyticsDetails { request, response }),
        Some(event_stage.to_string()),
    );
}

/// The dashboard's name for each configuration, as the response messages spell it.
fn label(config: &ConfigVariant) -> &'static str {
    match config {
        ConfigVariant::SuccessRate(_) => "Success Rate",
        ConfigVariant::Elimination(_) => "Elimination",
        ConfigVariant::DebitRouting(_) => "Debit Routing",
    }
}

fn algorithm_label(algorithm: &AlgorithmType) -> &'static str {
    match algorithm {
        AlgorithmType::SuccessRate => "Success Rate",
        AlgorithmType::Elimination => "Elimination",
        AlgorithmType::DebitRouting => "Debit Routing",
    }
}

/// Elimination thresholds live on the merchant account row, not in `service_configuration`.
async fn store_elimination(
    merchant: &AuthorizedMerchant,
    elimination: types::routing_configuration::EliminationData,
) -> error_stack::Result<(), crate::generics::MeshError> {
    let input = types::gateway_routing_input::GatewaySuccessRateBasedRoutingInput::from_elimination_threshold(elimination);
    let serialized = serde_json::to_string(&input).map_err(|error| {
        error_stack::report!(error).change_context(crate::generics::MeshError::Others)
    })?;
    ETM::merchant_account::update_merchant_account(merchant.as_str().to_string(), Some(serialized))
        .await
}

/// Creating a configuration that already exists is a conflict; any other failure is storage.
fn on_create(
    error: error_stack::Report<ConfigError>,
) -> error_stack::Report<error::RuleConfigurationError> {
    let context = match error.current_context() {
        ConfigError::AlreadyExists => error::RuleConfigurationError::ConfigurationAlreadyExists,
        _ => error::RuleConfigurationError::StorageError,
    };
    error.change_context(context)
}

async fn ensure_merchant(
    merchant: &AuthorizedMerchant,
) -> Result<
    ETM::merchant_account::MerchantAccount,
    error::ContainerError<error::RuleConfigurationError>,
> {
    ETM::merchant_account::load_authorized(merchant)
        .await
        .ok_or_else(|| error::RuleConfigurationError::MerchantNotFound.into())
}

#[axum::debug_handler]
pub async fn create_rule_config(
    ids: RequestIds,
    ScopedJson {
        merchant, payload, ..
    }: ScopedJson<types::routing_configuration::RoutingRule>,
) -> Result<Json<RuleConfigResponse>, error::ContainerError<error::RuleConfigurationError>> {
    logger::debug!("Received rule configuration: {:?}", payload);
    ensure_merchant(&merchant).await?;

    match payload.config.clone() {
        ConfigVariant::SuccessRate(success_rate) => {
            let success_rate = success_rate
                .normalised()
                .map_err(error::RuleConfigurationError::InvalidPaymentMethod)?;
            config_store::create::<SrV3Input>(&merchant, &success_rate)
                .await
                .map_err(on_create)?;
        }
        ConfigVariant::Elimination(elimination) => store_elimination(&merchant, elimination)
            .await
            .change_context(error::RuleConfigurationError::StorageError)?,
        ConfigVariant::DebitRouting(debit_routing) => {
            config_store::create::<DebitRoutingRule>(&merchant, &debit_routing)
                .await
                .map_err(on_create)?;
        }
    }

    let response = RuleConfigResponse {
        message: format!(
            "{} Configuration created successfully",
            label(&payload.config)
        ),
        merchant_id: merchant.as_str().to_string(),
        config: payload.config.clone(),
    };
    record_rule_config_event(
        ids,
        crate::analytics::refine_rule_config_create_flow_type(&payload.config),
        crate::analytics::AnalyticsRoute::RuleConfigCreate,
        &merchant,
        (&payload, &response),
        "rule_config_created",
    );
    Ok(Json(response))
}

// Absence is a normal read result. Failed reads retain their source error in the report
// while using the same 404 context as the other configuration operations.
fn parse_optional_rule_config<T: serde::de::DeserializeOwned>(
    value: Option<&str>,
) -> Result<Option<T>, error_stack::Report<error::RuleConfigurationError>> {
    value
        .map(serde_json::from_str)
        .transpose()
        .change_context(error::RuleConfigurationError::ConfigurationNotFound)
}

#[axum::debug_handler]
pub async fn get_rule_config(
    ids: RequestIds,
    ScopedJson {
        merchant, payload, ..
    }: ScopedJson<types::routing_configuration::FetchRoutingRule>,
) -> Result<
    Json<types::routing_configuration::RuleConfigGetResponse>,
    error::ContainerError<error::RuleConfigurationError>,
> {
    logger::debug!("Received rule fetch request: {:?}", payload);
    // Resolve the scope before dispatching on algorithm. Only the elimination config lives on the
    // merchant row; the others are looked up by a name derived from the merchant id, so without
    // this an unknown merchant is indistinguishable from one that simply has no config yet.
    let merchant_account = ensure_merchant(&merchant).await?;

    let not_found = error::RuleConfigurationError::ConfigurationNotFound;
    let config = match payload.algorithm {
        AlgorithmType::SuccessRate => config_store::get::<SrV3Input>(&merchant)
            .await
            .change_context(not_found)?
            .map(ConfigVariant::SuccessRate),
        AlgorithmType::DebitRouting => config_store::get::<DebitRoutingRule>(&merchant)
            .await
            .change_context(not_found)?
            .map(ConfigVariant::DebitRouting),
        AlgorithmType::Elimination => {
            // A missing DB value is represented as an empty string by MerchantAccount.
            let stored = merchant_account
                .gatewaySuccessRateBasedDeciderInput
                .as_str();
            let value = (!stored.trim().is_empty()).then_some(stored);
            parse_optional_rule_config::<
                types::gateway_routing_input::GatewaySuccessRateBasedRoutingInput,
            >(value)?
            .map(|config| {
                ConfigVariant::Elimination(types::routing_configuration::EliminationData {
                    threshold: config.defaultEliminationThreshold,
                    txnLatency: config.txnLatency,
                })
            })
        }
    };

    let response = RuleConfigGetResponse {
        merchant_id: merchant.as_str().to_string(),
        config,
    };
    record_rule_config_event(
        ids,
        crate::analytics::flow_type_for_rule_config_algorithm_get(&payload.algorithm),
        crate::analytics::AnalyticsRoute::RuleConfigGet,
        &merchant,
        (&payload, &response),
        if response.config.is_some() {
            "rule_config_loaded"
        } else {
            "rule_config_not_configured"
        },
    );
    Ok(Json(response))
}

#[axum::debug_handler]
pub async fn update_rule_config(
    ids: RequestIds,
    ScopedJson {
        merchant, payload, ..
    }: ScopedJson<types::routing_configuration::RoutingRule>,
) -> Result<Json<RuleConfigResponse>, error::ContainerError<error::RuleConfigurationError>> {
    logger::debug!("Received rule update configuration: {:?}", payload);
    ensure_merchant(&merchant).await?;

    let not_found = error::RuleConfigurationError::ConfigurationNotFound;
    match payload.config.clone() {
        ConfigVariant::SuccessRate(success_rate) => {
            let success_rate = success_rate
                .normalised()
                .map_err(error::RuleConfigurationError::InvalidPaymentMethod)?;
            config_store::update::<SrV3Input>(&merchant, &success_rate)
                .await
                .change_context(not_found)?;
        }
        ConfigVariant::Elimination(elimination) => store_elimination(&merchant, elimination)
            .await
            .change_context(not_found)?,
        ConfigVariant::DebitRouting(debit_routing) => {
            config_store::update::<DebitRoutingRule>(&merchant, &debit_routing)
                .await
                .change_context(not_found)?;
        }
    }

    let response = RuleConfigResponse {
        message: format!(
            "{} Configuration updated successfully",
            label(&payload.config)
        ),
        merchant_id: merchant.as_str().to_string(),
        config: payload.config.clone(),
    };
    record_rule_config_event(
        ids,
        crate::analytics::refine_rule_config_update_flow_type(&payload.config),
        crate::analytics::AnalyticsRoute::RuleConfigUpdate,
        &merchant,
        (&payload, &response),
        "rule_config_updated",
    );
    Ok(Json(response))
}

#[axum::debug_handler]
pub async fn delete_rule_config(
    ids: RequestIds,
    ScopedJson {
        merchant, payload, ..
    }: ScopedJson<types::routing_configuration::FetchRoutingRule>,
) -> Result<Json<RuleConfigDeleteResponse>, error::ContainerError<error::RuleConfigurationError>> {
    logger::debug!("Received rule delete request: {:?}", payload);
    let merchant_account = ensure_merchant(&merchant).await?;

    let not_found = error::RuleConfigurationError::ConfigurationNotFound;
    match payload.algorithm {
        AlgorithmType::SuccessRate => config_store::delete::<SrV3Input>(&merchant)
            .await
            .change_context(not_found)?,
        AlgorithmType::DebitRouting => config_store::delete::<DebitRoutingRule>(&merchant)
            .await
            .change_context(not_found)?,
        AlgorithmType::Elimination => {
            if merchant_account
                .gatewaySuccessRateBasedDeciderInput
                .is_empty()
            {
                return Err(not_found.into());
            }
            ETM::merchant_account::update_merchant_account(
                merchant.as_str().to_string(),
                Some(String::new()),
            )
            .await
            .change_context(error::RuleConfigurationError::StorageError)?;
        }
    }

    let response = RuleConfigDeleteResponse {
        message: format!(
            "{} Configuration deleted successfully",
            algorithm_label(&payload.algorithm)
        ),
        merchant_id: merchant.as_str().to_string(),
    };
    record_rule_config_event(
        ids,
        crate::analytics::flow_type_for_rule_config_algorithm_delete(&payload.algorithm),
        crate::analytics::AnalyticsRoute::RuleConfigDelete,
        &merchant,
        (&payload, &response),
        "rule_config_deleted",
    );
    Ok(Json(response))
}

#[cfg(test)]
mod fetch_response_tests {
    use super::parse_optional_rule_config;
    use crate::types::routing_configuration::{RuleConfigGetResponse, SuccessRateData};

    #[test]
    fn missing_config_is_a_successful_null_response() {
        assert!(matches!(
            parse_optional_rule_config::<SuccessRateData>(None),
            Ok(None)
        ));
        let response = RuleConfigGetResponse {
            merchant_id: "merchant_defaults".to_string(),
            config: None,
        };
        assert_eq!(
            serde_json::to_value(response).ok(),
            Some(serde_json::json!({
                "merchant_id": "merchant_defaults", "config": null
            }))
        );
    }

    #[test]
    fn saved_config_is_preserved() {
        let result =
            parse_optional_rule_config::<SuccessRateData>(Some(r#"{"defaultBucketSize":200}"#));
        assert!(matches!(result, Ok(Some(config)) if config.default_bucket_size == Some(200)));
    }

    #[test]
    fn invalid_saved_config_is_not_treated_as_absent() {
        for value in ["not json", "", "null", r#"{"defaultBucketSize":"invalid"}"#] {
            let result = parse_optional_rule_config::<SuccessRateData>(Some(value));
            assert!(result.as_ref().is_err_and(|error| {
                error.downcast_ref::<serde_json::Error>().is_some()
                    && axum::response::IntoResponse::into_response(error.current_context().clone())
                        .status()
                        == axum::http::StatusCode::NOT_FOUND
            }));
        }
    }
}
