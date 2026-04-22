use crate::metrics::{API_LATENCY_HISTOGRAM, API_REQUEST_COUNTER, API_REQUEST_TOTAL_COUNTER};
use crate::types::merchant as ETM;
use crate::{error, logger, types};
use axum::Json;
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

#[axum::debug_handler]
pub async fn create_rule_config(
    headers: axum::http::HeaderMap,
    Json(payload): Json<types::routing_configuration::RoutingRule>,
) -> Result<Json<RuleConfigResponse>, error::ContainerError<error::RuleConfigurationError>> {
    let timer = API_LATENCY_HISTOGRAM
        .with_label_values(&["create_rule_config"])
        .start_timer();
    API_REQUEST_TOTAL_COUNTER
        .with_label_values(&["create_rule_config"])
        .inc();
    logger::debug!("Received rule configuration: {:?}", payload);

    let merchant_id = payload.merchant_id.clone();
    let config = payload.config.clone();
    let analytics_payload = payload.clone();
    let analytics_merchant_id = merchant_id.clone();
    let analytics_config = config.clone();
    let request_id = headers
        .get(crate::storage::consts::X_REQUEST_ID)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let global_request_id = crate::analytics::global_request_id_from_headers(&headers);
    let trace_id = crate::analytics::trace_id_from_headers(&headers);

    let mid = payload.merchant_id.clone();

    // Check if merchant exists
    if ETM::merchant_account::load_merchant_by_merchant_id(mid.clone())
        .await
        .is_none()
    {
        API_REQUEST_COUNTER
            .with_label_values(&["create_rule_config", "failure"])
            .inc();
        timer.observe_duration();
        return Err(error::RuleConfigurationError::MerchantNotFound.into());
    }

    let result = match payload.config {
        types::routing_configuration::ConfigVariant::SuccessRate(success_config) => {
            let name = format!("SR_V3_INPUT_CONFIG_{}", mid);
            let serialized_config = serde_json::to_string(&success_config)
                .map_err(|_| error::RuleConfigurationError::StorageError)?;

            // Check if config already exists
            let result = types::service_configuration::find_config_by_name(name.clone())
                .await
                .change_context(error::RuleConfigurationError::StorageError)?;

            match result {
                Some(_) => {
                    API_REQUEST_COUNTER
                        .with_label_values(&["sr_create_rule_config", "failure"])
                        .inc();
                    Err(error::RuleConfigurationError::ConfigurationAlreadyExists.into())
                }
                None => {
                    match types::service_configuration::insert_config(name, Some(serialized_config))
                        .await
                        .change_context(error::RuleConfigurationError::StorageError)
                    {
                        Ok(_) => {
                            API_REQUEST_COUNTER
                                .with_label_values(&["sr_create_rule_config", "success"])
                                .inc();
                            Ok(Json(RuleConfigResponse {
                                message: "Success Rate Configuration created successfully"
                                    .to_string(),
                                merchant_id,
                                config,
                            }))
                        }
                        Err(e) => {
                            API_REQUEST_COUNTER
                                .with_label_values(&["sr_create_rule_config", "failure"])
                                .inc();
                            Err(e.into())
                        }
                    }
                }
            }
        }
        types::routing_configuration::ConfigVariant::Elimination(elimination_config) => {
            let db_config = types::gateway_routing_input::GatewaySuccessRateBasedRoutingInput::from_elimination_threshold(elimination_config);
            let serialized_config = serde_json::to_string(&db_config)
                .map_err(|_| error::RuleConfigurationError::StorageError)?;

            let result = types::merchant::merchant_account::update_merchant_account(
                mid,
                Some(serialized_config),
            )
            .await
            .change_context(error::RuleConfigurationError::StorageError);

            match result {
                Ok(_) => {
                    API_REQUEST_COUNTER
                        .with_label_values(&["elimination_create_rule_config", "success"])
                        .inc();
                    Ok(Json(RuleConfigResponse {
                        message: "Elimination Configuration created successfully".to_string(),
                        merchant_id,
                        config,
                    }))
                }
                Err(e) => {
                    API_REQUEST_COUNTER
                        .with_label_values(&["elimination_create_rule_config", "failure"])
                        .inc();
                    Err(e.into())
                }
            }
        }
        types::routing_configuration::ConfigVariant::DebitRouting(debit_config) => {
            let name = format!("DEBIT_ROUTING_CONFIG_{}", mid);
            let serialized_config = serde_json::to_string(&debit_config)
                .map_err(|_| error::RuleConfigurationError::StorageError)?;

            // Check if config already exists
            let result = types::service_configuration::find_config_by_name(name.clone())
                .await
                .change_context(error::RuleConfigurationError::StorageError)?;

            match result {
                Some(_) => {
                    API_REQUEST_COUNTER
                        .with_label_values(&["debit_routing_create_rule_config", "failure"])
                        .inc();
                    Err(error::RuleConfigurationError::ConfigurationAlreadyExists.into())
                }
                None => {
                    match types::service_configuration::insert_config(name, Some(serialized_config))
                        .await
                        .change_context(error::RuleConfigurationError::StorageError)
                    {
                        Ok(_) => {
                            API_REQUEST_COUNTER
                                .with_label_values(&["debit_routing_create_rule_config", "success"])
                                .inc();
                            Ok(Json(RuleConfigResponse {
                                message: "Debit Routing Configuration created successfully"
                                    .to_string(),
                                merchant_id,
                                config,
                            }))
                        }
                        Err(e) => {
                            API_REQUEST_COUNTER
                                .with_label_values(&["debit_routing_create_rule_config", "failure"])
                                .inc();
                            Err(e.into())
                        }
                    }
                }
            }
        }
    };

    if let Ok(response) = &result {
        crate::analytics::record_operation_event(
            crate::analytics::AnalyticsFlowContext::new(
                crate::analytics::ApiFlow::RuleBasedRouting,
                crate::analytics::refine_rule_config_create_flow_type(&analytics_config),
            ),
            crate::analytics::AnalyticsRoute::RuleConfigCreate,
            Some(analytics_merchant_id),
            None,
            request_id,
            global_request_id,
            trace_id,
            Some("success".to_string()),
            serde_json::to_string(&serde_json::json!({
                "request": &analytics_payload,
                "response": &response.0,
            }))
            .ok(),
            Some("rule_config_created".to_string()),
        );
    }
    timer.observe_duration();
    result
}

#[axum::debug_handler]
pub async fn get_rule_config(
    headers: axum::http::HeaderMap,
    Json(payload): Json<types::routing_configuration::FetchRoutingRule>,
) -> Result<
    Json<types::routing_configuration::RoutingRule>,
    error::ContainerError<error::RuleConfigurationError>,
> {
    let timer = API_LATENCY_HISTOGRAM
        .with_label_values(&["get_rule_config"])
        .start_timer();
    API_REQUEST_TOTAL_COUNTER
        .with_label_values(&["get_rule_config"])
        .inc();
    logger::debug!("Received rule fetch request: {:?}", payload);
    let analytics_payload = payload.clone();
    let request_id = headers
        .get(crate::storage::consts::X_REQUEST_ID)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let global_request_id = crate::analytics::global_request_id_from_headers(&headers);
    let trace_id = crate::analytics::trace_id_from_headers(&headers);

    let mid = payload.merchant_id.clone();
    let analytics_mid = mid.clone();
    let merchant_account = ETM::merchant_account::load_merchant_by_merchant_id(mid.clone())
        .await
        .ok_or(error::RuleConfigurationError::MerchantNotFound);

    let result = match payload.algorithm {
        types::routing_configuration::AlgorithmType::SuccessRate => {
            let config_name = format!("SR_V3_INPUT_CONFIG_{}", mid);
            let result = types::service_configuration::find_config_by_name(config_name.clone())
                .await
                .change_context(error::RuleConfigurationError::StorageError)
                .and_then(|opt_config| {
                    opt_config
                        .and_then(|config| config.value)
                        .ok_or(error::RuleConfigurationError::ConfigurationNotFound.into())
                })
                .and_then(|config| {
                    serde_json::from_str::<types::routing_configuration::SuccessRateData>(&config)
                        .map_err(|_| error::RuleConfigurationError::StorageError.into())
                });

            match result {
                Ok(success_rate_config) => {
                    API_REQUEST_COUNTER
                        .with_label_values(&["sr_get_rule_config", "success"])
                        .inc();
                    Ok(Json(types::routing_configuration::RoutingRule {
                        merchant_id: mid,
                        config: types::routing_configuration::ConfigVariant::SuccessRate(
                            success_rate_config,
                        ),
                    }))
                }
                Err(e) => {
                    API_REQUEST_COUNTER
                        .with_label_values(&["sr_get_rule_config", "failure"])
                        .inc();
                    Err(e.into())
                }
            }
        }
        types::routing_configuration::AlgorithmType::Elimination => {
            let result = merchant_account.and_then(|account| {
                serde_json::from_str::<
                    types::gateway_routing_input::GatewaySuccessRateBasedRoutingInput,
                >(&account.gatewaySuccessRateBasedDeciderInput)
                .map_err(|_| error::RuleConfigurationError::ConfigurationNotFound)
                .map(|config| types::routing_configuration::EliminationData {
                    threshold: config.defaultEliminationThreshold,
                    txnLatency: config.txnLatency,
                })
            });

            match result {
                Ok(elimination_config) => {
                    API_REQUEST_COUNTER
                        .with_label_values(&["elimination_get_rule_config", "success"])
                        .inc();
                    Ok(Json(types::routing_configuration::RoutingRule {
                        merchant_id: mid,
                        config: types::routing_configuration::ConfigVariant::Elimination(
                            elimination_config,
                        ),
                    }))
                }
                Err(e) => {
                    API_REQUEST_COUNTER
                        .with_label_values(&["elimination_get_rule_config", "failure"])
                        .inc();
                    Err(e.into())
                }
            }
        }
        types::routing_configuration::AlgorithmType::DebitRouting => {
            let config_name = format!("DEBIT_ROUTING_CONFIG_{}", mid);
            let result = types::service_configuration::find_config_by_name(config_name.clone())
                .await
                .change_context(error::RuleConfigurationError::StorageError)
                .and_then(|opt_config| {
                    opt_config
                        .and_then(|config| config.value)
                        .ok_or(error::RuleConfigurationError::ConfigurationNotFound.into())
                })
                .and_then(|config| {
                    serde_json::from_str::<types::routing_configuration::DebitRoutingData>(&config)
                        .map_err(|_| error::RuleConfigurationError::StorageError.into())
                });

            match result {
                Ok(debit_routing_config) => {
                    API_REQUEST_COUNTER
                        .with_label_values(&["debit_routing_get_rule_config", "success"])
                        .inc();
                    Ok(Json(types::routing_configuration::RoutingRule {
                        merchant_id: mid,
                        config: types::routing_configuration::ConfigVariant::DebitRouting(
                            debit_routing_config,
                        ),
                    }))
                }
                Err(e) => {
                    API_REQUEST_COUNTER
                        .with_label_values(&["debit_routing_get_rule_config", "failure"])
                        .inc();
                    Err(e.into())
                }
            }
        }
    };
    if let Ok(response) = &result {
        crate::analytics::record_operation_event(
            crate::analytics::AnalyticsFlowContext::new(
                crate::analytics::ApiFlow::RuleBasedRouting,
                crate::analytics::flow_type_for_rule_config_algorithm_get(&payload.algorithm),
            ),
            crate::analytics::AnalyticsRoute::RuleConfigGet,
            Some(analytics_mid),
            None,
            request_id,
            global_request_id,
            trace_id,
            Some("success".to_string()),
            serde_json::to_string(&serde_json::json!({
                "request": &analytics_payload,
                "response": &response.0,
            }))
            .ok(),
            Some("rule_config_loaded".to_string()),
        );
    }
    timer.observe_duration();
    result
}

#[axum::debug_handler]
pub async fn update_rule_config(
    headers: axum::http::HeaderMap,
    Json(payload): Json<types::routing_configuration::RoutingRule>,
) -> Result<Json<RuleConfigResponse>, error::ContainerError<error::RuleConfigurationError>> {
    let timer = API_LATENCY_HISTOGRAM
        .with_label_values(&["update_rule_config"])
        .start_timer();
    API_REQUEST_TOTAL_COUNTER
        .with_label_values(&["update_rule_config"])
        .inc();
    logger::debug!("Received rule update configuration: {:?}", payload);

    let merchant_id = payload.merchant_id.clone();
    let config = payload.config.clone();
    let analytics_payload = payload.clone();
    let analytics_merchant_id = merchant_id.clone();
    let analytics_config = config.clone();
    let request_id = headers
        .get(crate::storage::consts::X_REQUEST_ID)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let global_request_id = crate::analytics::global_request_id_from_headers(&headers);
    let trace_id = crate::analytics::trace_id_from_headers(&headers);

    let mid = payload.merchant_id.clone();
    ETM::merchant_account::load_merchant_by_merchant_id(mid.clone())
        .await
        .ok_or(error::RuleConfigurationError::MerchantNotFound)?;

    // Update DB call for updating the rule configuration
    let result = match payload.config {
        types::routing_configuration::ConfigVariant::SuccessRate(success_config) => {
            let name = format!("SR_V3_INPUT_CONFIG_{}", mid);
            let serialized_config = serde_json::to_string(&success_config)
                .map_err(|_| error::RuleConfigurationError::StorageError)?;

            let result = types::service_configuration::update_config(name, Some(serialized_config))
                .await
                .change_context(error::RuleConfigurationError::ConfigurationNotFound);

            match result {
                Ok(_) => {
                    API_REQUEST_COUNTER
                        .with_label_values(&["sr_update_rule_config", "success"])
                        .inc();
                    Ok(Json(RuleConfigResponse {
                        message: "Success Rate Configuration updated successfully".to_string(),
                        merchant_id,
                        config,
                    }))
                }
                Err(e) => {
                    API_REQUEST_COUNTER
                        .with_label_values(&["sr_update_rule_config", "failure"])
                        .inc();
                    Err(e.into())
                }
            }
        }
        types::routing_configuration::ConfigVariant::Elimination(elimination_config) => {
            let db_config = types::gateway_routing_input::GatewaySuccessRateBasedRoutingInput::from_elimination_threshold(elimination_config);
            let serialized_config = serde_json::to_string(&db_config)
                .map_err(|_| error::RuleConfigurationError::StorageError)?;

            let result = types::merchant::merchant_account::update_merchant_account(
                mid,
                Some(serialized_config),
            )
            .await
            .change_context(error::RuleConfigurationError::ConfigurationNotFound);

            match result {
                Ok(_) => {
                    API_REQUEST_COUNTER
                        .with_label_values(&["elimination_update_rule_config", "success"])
                        .inc();
                    Ok(Json(RuleConfigResponse {
                        message: "Elimination Configuration updated successfully".to_string(),
                        merchant_id,
                        config,
                    }))
                }
                Err(e) => {
                    API_REQUEST_COUNTER
                        .with_label_values(&["elimination_update_rule_config", "failure"])
                        .inc();
                    Err(e.into())
                }
            }
        }
        types::routing_configuration::ConfigVariant::DebitRouting(debit_config) => {
            let name = format!("DEBIT_ROUTING_CONFIG_{}", mid);
            let serialized_config = serde_json::to_string(&debit_config)
                .map_err(|_| error::RuleConfigurationError::StorageError)?;

            let result = types::service_configuration::update_config(name, Some(serialized_config))
                .await
                .change_context(error::RuleConfigurationError::ConfigurationNotFound);

            match result {
                Ok(_) => {
                    API_REQUEST_COUNTER
                        .with_label_values(&["debit_routing_update_rule_config", "success"])
                        .inc();
                    Ok(Json(RuleConfigResponse {
                        message: "Debit Routing Configuration updated successfully".to_string(),
                        merchant_id,
                        config,
                    }))
                }
                Err(e) => {
                    API_REQUEST_COUNTER
                        .with_label_values(&["debit_routing_update_rule_config", "failure"])
                        .inc();
                    Err(e.into())
                }
            }
        }
    };
    if let Ok(response) = &result {
        crate::analytics::record_operation_event(
            crate::analytics::AnalyticsFlowContext::new(
                crate::analytics::ApiFlow::RuleBasedRouting,
                crate::analytics::refine_rule_config_update_flow_type(&analytics_config),
            ),
            crate::analytics::AnalyticsRoute::RuleConfigUpdate,
            Some(analytics_merchant_id),
            None,
            request_id,
            global_request_id,
            trace_id,
            Some("success".to_string()),
            serde_json::to_string(&serde_json::json!({
                "request": &analytics_payload,
                "response": &response.0,
            }))
            .ok(),
            Some("rule_config_updated".to_string()),
        );
    }
    timer.observe_duration();
    result
}

#[axum::debug_handler]
pub async fn delete_rule_config(
    headers: axum::http::HeaderMap,
    Json(payload): Json<types::routing_configuration::FetchRoutingRule>,
) -> Result<Json<RuleConfigDeleteResponse>, error::ContainerError<error::RuleConfigurationError>> {
    let timer = API_LATENCY_HISTOGRAM
        .with_label_values(&["delete_rule_config"])
        .start_timer();
    API_REQUEST_TOTAL_COUNTER
        .with_label_values(&["delete_rule_config"])
        .inc();
    logger::debug!("Received rule delete request: {:?}", payload);
    let analytics_payload = payload.clone();
    let request_id = headers
        .get(crate::storage::consts::X_REQUEST_ID)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let global_request_id = crate::analytics::global_request_id_from_headers(&headers);
    let trace_id = crate::analytics::trace_id_from_headers(&headers);

    let mid = payload.merchant_id.clone();
    let analytics_mid = mid.clone();
    let merchant_account = ETM::merchant_account::load_merchant_by_merchant_id(mid.clone())
        .await
        .ok_or(error::RuleConfigurationError::MerchantNotFound)?;

    // Delete DB call for deleting the rule configuration
    let result = match payload.algorithm {
        types::routing_configuration::AlgorithmType::SuccessRate => {
            let config_name = format!("SR_V3_INPUT_CONFIG_{}", mid);
            let result = types::service_configuration::delete_config(config_name)
                .await
                .change_context(error::RuleConfigurationError::ConfigurationNotFound);

            match result {
                Ok(_) => {
                    API_REQUEST_COUNTER
                        .with_label_values(&["sr_delete_rule_config", "success"])
                        .inc();
                    Ok(Json(RuleConfigDeleteResponse {
                        message: "Success Rate Configuration deleted successfully".to_string(),
                        merchant_id: mid,
                    }))
                }
                Err(e) => {
                    API_REQUEST_COUNTER
                        .with_label_values(&["sr_delete_rule_config", "failure"])
                        .inc();
                    Err(e.into())
                }
            }
        }
        types::routing_configuration::AlgorithmType::Elimination => {
            if merchant_account
                .gatewaySuccessRateBasedDeciderInput
                .is_empty()
            {
                API_REQUEST_COUNTER
                    .with_label_values(&["elimination_delete_rule_config", "failure"])
                    .inc();
                return Err(error::RuleConfigurationError::ConfigurationNotFound.into());
            }

            let result = types::merchant::merchant_account::update_merchant_account(
                mid.clone(),
                Some("".to_string()),
            ) // update to empty string
            .await
            .change_context(error::RuleConfigurationError::StorageError);

            match result {
                Ok(_) => {
                    API_REQUEST_COUNTER
                        .with_label_values(&["elimination_delete_rule_config", "success"])
                        .inc();
                    Ok(Json(RuleConfigDeleteResponse {
                        message: "Elimination Configuration deleted successfully".to_string(),
                        merchant_id: mid,
                    }))
                }
                Err(e) => {
                    API_REQUEST_COUNTER
                        .with_label_values(&["elimination_delete_rule_config", "failure"])
                        .inc();
                    Err(e.into())
                }
            }
        }
        types::routing_configuration::AlgorithmType::DebitRouting => {
            let config_name = format!("DEBIT_ROUTING_CONFIG_{}", mid);
            let result = types::service_configuration::delete_config(config_name)
                .await
                .change_context(error::RuleConfigurationError::ConfigurationNotFound);

            match result {
                Ok(_) => {
                    API_REQUEST_COUNTER
                        .with_label_values(&["debit_routing_delete_rule_config", "success"])
                        .inc();
                    Ok(Json(RuleConfigDeleteResponse {
                        message: "Debit Routing Configuration deleted successfully".to_string(),
                        merchant_id: mid,
                    }))
                }
                Err(e) => {
                    API_REQUEST_COUNTER
                        .with_label_values(&["debit_routing_delete_rule_config", "failure"])
                        .inc();
                    Err(e.into())
                }
            }
        }
    };

    if let Ok(response) = &result {
        crate::analytics::record_operation_event(
            crate::analytics::AnalyticsFlowContext::new(
                crate::analytics::ApiFlow::RuleBasedRouting,
                crate::analytics::flow_type_for_rule_config_algorithm_delete(&payload.algorithm),
            ),
            crate::analytics::AnalyticsRoute::RuleConfigDelete,
            Some(analytics_mid),
            None,
            request_id,
            global_request_id,
            trace_id,
            Some("success".to_string()),
            serde_json::to_string(&serde_json::json!({
                "request": &analytics_payload,
                "response": &response.0,
            }))
            .ok(),
            Some("rule_config_deleted".to_string()),
        );
    }
    timer.observe_duration();
    result
}
