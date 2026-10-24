use crate::custom_extractors::Json;
use crate::metrics::{API_LATENCY_HISTOGRAM, API_REQUEST_COUNTER, API_REQUEST_TOTAL_COUNTER};
use crate::types::merchant as ETM;
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

fn serialize_rule_config_analytics_details<Request: Serialize, Response: Serialize>(
    request: &Request,
    response: &Response,
) -> Option<String> {
    crate::analytics::serialize_details(&RuleConfigAnalyticsDetails { request, response })
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
            let success_config = success_config
                .normalised()
                .map_err(error::RuleConfigurationError::InvalidPaymentMethod)?;
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
        crate::analytics::DomainAnalyticsEvent::record_operation(
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
            serialize_rule_config_analytics_details(&analytics_payload, &response.0),
            Some("rule_config_created".to_string()),
        );
    }
    timer.observe_duration();
    result
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
    headers: axum::http::HeaderMap,
    Json(payload): Json<types::routing_configuration::FetchRoutingRule>,
) -> Result<
    Json<types::routing_configuration::RuleConfigGetResponse>,
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
    // Resolve the scope before dispatching on algorithm. Only the elimination config lives on the
    // merchant row; the others are looked up by a name derived from the merchant id, so without
    // this an unknown merchant is indistinguishable from one that simply has no config yet.
    let merchant_account =
        match ETM::merchant_account::load_merchant_by_merchant_id(mid.clone()).await {
            Some(account) => account,
            None => {
                API_REQUEST_COUNTER
                    .with_label_values(&["get_rule_config", "failure"])
                    .inc();
                timer.observe_duration();
                return Err(error::RuleConfigurationError::MerchantNotFound.into());
            }
        };

    use types::routing_configuration::{AlgorithmType, ConfigVariant, RuleConfigGetResponse};
    let (result, metric): (
        Result<Option<ConfigVariant>, error_stack::Report<error::RuleConfigurationError>>,
        &str,
    ) = match payload.algorithm {
        AlgorithmType::SuccessRate | AlgorithmType::DebitRouting => {
            let (config_name, metric) = match payload.algorithm {
                AlgorithmType::SuccessRate => {
                    (format!("SR_V3_INPUT_CONFIG_{mid}"), "sr_get_rule_config")
                }
                _ => (
                    format!("DEBIT_ROUTING_CONFIG_{mid}"),
                    "debit_routing_get_rule_config",
                ),
            };
            let result = types::service_configuration::find_config_by_name(config_name)
                .await
                .change_context(error::RuleConfigurationError::ConfigurationNotFound)
                .and_then(|stored| {
                    let value = stored.and_then(|config| config.value);
                    match payload.algorithm {
                        AlgorithmType::SuccessRate => parse_optional_rule_config(value.as_deref())
                            .map(|config| config.map(ConfigVariant::SuccessRate)),
                        _ => parse_optional_rule_config(value.as_deref())
                            .map(|config| config.map(ConfigVariant::DebitRouting)),
                    }
                });
            (result, metric)
        }
        AlgorithmType::Elimination => {
            // A missing DB value is represented as an empty string by MerchantAccount.
            let stored = merchant_account
                .gatewaySuccessRateBasedDeciderInput
                .as_str();
            let value = (!stored.trim().is_empty()).then_some(stored);
            let result = parse_optional_rule_config::<
                types::gateway_routing_input::GatewaySuccessRateBasedRoutingInput,
            >(value)
            .map(|config| {
                config.map(|config| {
                    ConfigVariant::Elimination(types::routing_configuration::EliminationData {
                        threshold: config.defaultEliminationThreshold,
                        txnLatency: config.txnLatency,
                    })
                })
            });
            (result, "elimination_get_rule_config")
        }
    };
    API_REQUEST_COUNTER
        .with_label_values(&[metric, if result.is_ok() { "success" } else { "failure" }])
        .inc();
    let result: Result<_, error::ContainerError<error::RuleConfigurationError>> = result
        .map(|config| {
            Json(RuleConfigGetResponse {
                merchant_id: mid,
                config,
            })
        })
        .map_err(Into::into);
    if let Ok(response) = &result {
        crate::analytics::DomainAnalyticsEvent::record_operation(
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
            serialize_rule_config_analytics_details(&analytics_payload, &response.0),
            Some(
                if response.0.config.is_some() {
                    "rule_config_loaded"
                } else {
                    "rule_config_not_configured"
                }
                .to_string(),
            ),
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
            let success_config = success_config
                .normalised()
                .map_err(error::RuleConfigurationError::InvalidPaymentMethod)?;
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
        crate::analytics::DomainAnalyticsEvent::record_operation(
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
            serialize_rule_config_analytics_details(&analytics_payload, &response.0),
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
        crate::analytics::DomainAnalyticsEvent::record_operation(
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
            serialize_rule_config_analytics_details(&analytics_payload, &response.0),
            Some("rule_config_deleted".to_string()),
        );
    }
    timer.observe_duration();
    result
}

#[cfg(test)]
mod fetch_response_tests {
    use super::parse_optional_rule_config;
    use crate::types::routing_configuration::{RuleConfigGetResponse, SuccessRateData};

    #[test]
    fn missing_config_is_a_successful_null_response() -> Result<(), serde_json::Error> {
        assert!(matches!(
            parse_optional_rule_config::<SuccessRateData>(None),
            Ok(None)
        ));
        let response = RuleConfigGetResponse {
            merchant_id: "merchant_defaults".to_string(),
            config: None,
        };
        assert_eq!(
            serde_json::to_value(response)?,
            serde_json::json!({
                "merchant_id": "merchant_defaults", "config": null
            })
        );
        Ok(())
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
