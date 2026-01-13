#[cfg(feature = "mysql")]
use crate::storage::schema::routing_algorithm::dsl;
#[cfg(feature = "postgres")]
use crate::storage::schema_pg::routing_algorithm::dsl;
use crate::{
    error::ApiErrorResponse,
    euclid::{
        ast::{ComparisonType, ConnectorInfo, Output, ValueType},
        cgraph,
        interpreter::{evaluate_output, InterpreterBackend},
        types::{
            ActivateRoutingConfigRequest, Context, JsonifiedRoutingAlgorithm, KeyDataType,
            RoutingAlgorithmMapperNew, RoutingDictionaryRecord, RoutingEvaluateResponse,
            RoutingRequest, RoutingRule, SrDimensionConfig, StaticRoutingAlgorithm,
            ELIGIBLE_DIMENSIONS,
        },
        utils::{generate_random_id, is_valid_enum_value, validate_routing_rule},
    },
    types::service_configuration::{find_config_by_name, insert_config, update_config},
};

use crate::euclid::{
    errors::EuclidErrors,
    types::{RoutingAlgorithmMapper, RoutingAlgorithmMapperUpdate},
};
use crate::{euclid::types::RoutingAlgorithm, logger, metrics};
use axum::{extract::Path, Json};
use diesel::{associations::HasTable, BoolExpressionMethods, ExpressionMethods};
use error_stack::ResultExt;

use crate::app::get_tenant_app_state;

use crate::error::ContainerError;
use crate::metrics::{API_LATENCY_HISTOGRAM, API_REQUEST_COUNTER, API_REQUEST_TOTAL_COUNTER};
use serde_json::{json, Value};

const DEFAULT_FALLBACK_IDENTIFIER: &str = "default_fallback_enabled";
pub async fn config_sr_dimentions(
    Json(payload): Json<SrDimensionConfig>,
) -> Result<Json<String>, ContainerError<EuclidErrors>> {
    let timer = metrics::API_LATENCY_HISTOGRAM
        .with_label_values(&["config_sr_dimentions"])
        .start_timer();
    metrics::API_REQUEST_TOTAL_COUNTER
        .with_label_values(&["config_sr_dimentions"])
        .inc();
    logger::debug!("Received SR Dimension config: {:?}", payload);

    // Validate dimensions against ELIGIBLE_DIMENSIONS
    let invalid_dimensions: Vec<&String> = payload
        .paymentInfo
        .fields
        .as_ref()
        .map(|fields| {
            fields
                .iter()
                .filter(|field| !ELIGIBLE_DIMENSIONS.contains(&field.as_str()))
                .collect()
        })
        .unwrap_or_default();

    if !invalid_dimensions.is_empty() {
        metrics::API_REQUEST_COUNTER
            .with_label_values(&["config_sr_dimentions", "failure"])
            .inc();
        timer.observe_duration();

        logger::error!(
            "Invalid dimensions found for merchant {}: {:?}",
            payload.merchant_id,
            invalid_dimensions.clone()
        );

        return Err(EuclidErrors::InvalidSrDimensionConfig(format!(
            "Invalid dimensions: {:?}. Valid dimensions are: {}",
            invalid_dimensions.clone(),
            ELIGIBLE_DIMENSIONS.join(", ")
        ))
        .into());
    }

    let mid = payload.merchant_id.clone();
    let config = serde_json::to_string(&payload)
        .change_context(EuclidErrors::FailedToSerializeJsonToString)?;
    let name = format!("SR_DIMENSION_CONFIG_{}", mid);

    let service_config = find_config_by_name(name.clone())
        .await
        .change_context(EuclidErrors::StorageError)?;
    let result = match service_config {
        Some(_) => {
            logger::debug!(
                "Updating existing SR Dimension config for merchant: {}",
                mid
            );
            update_config(name, Some(config))
                .await
                .change_context(EuclidErrors::StorageError)
        }
        None => {
            logger::debug!("Inserting new SR Dimension config for merchant: {}", mid);
            insert_config(name, Some(config))
                .await
                .change_context(EuclidErrors::StorageError)
        }
    };

    if let Err(_) = result {
        metrics::API_REQUEST_COUNTER
            .with_label_values(&["config_sr_dimentions", "failure"])
            .inc();
        timer.observe_duration();
        logger::error!(
            "Failed to insert or update SR Dimension config for merchant: {}",
            mid
        );
        return Err(ContainerError::from(EuclidErrors::StorageError));
    }
    metrics::API_REQUEST_COUNTER
        .with_label_values(&["config_sr_dimentions", "success"])
        .inc();
    timer.observe_duration();
    logger::debug!(
        "SR Dimension configuration updated successfully for merchant: {}",
        mid
    );
    Ok(Json(
        "SR Dimension configuration updated successfully".to_string(),
    ))
}
pub async fn routing_create(
    Json(payload): Json<Value>,
) -> Result<Json<RoutingDictionaryRecord>, ContainerError<EuclidErrors>> {
    let timer = metrics::API_LATENCY_HISTOGRAM
        .with_label_values(&["routing_create"])
        .start_timer();
    metrics::API_REQUEST_TOTAL_COUNTER
        .with_label_values(&["routing_create"])
        .inc();

    let state = get_tenant_app_state().await;

    let config: RoutingRule = serde_json::from_value(payload.clone())
        .change_context(EuclidErrors::InvalidRuleConfiguration)?;

    logger::debug!("Received routing config: {:?}", config);

    match validate_routing_rule(&config, &state.config.routing_config) {
        Ok(validation_result) => {
            if !validation_result.is_valid {
                for error in &validation_result.errors {
                    logger::error!(
                        field = %error.field,
                        error_type = %error.error_type,
                        message = %error.message,
                        expected = ?error.expected,
                        actual = ?error.actual,
                        "Field validation error during routing rule creation"
                    );
                }

                let error_details: Vec<serde_json::Value> = validation_result
                    .errors
                    .iter()
                    .map(|e| {
                        let mut detail = serde_json::json!({
                            "field": e.field,
                            "error_type": e.error_type,
                            "message": e.message,
                        });
                        if let Some(ref expected) = e.expected {
                            detail["expected"] = serde_json::json!(expected);
                        }
                        if let Some(ref actual) = e.actual {
                            detail["actual"] = serde_json::json!(actual);
                        }
                        detail
                    })
                    .collect();

                let detailed_error = validation_result.to_error_message();
                logger::error!(
                    error_count = validation_result.errors.len(),
                    "Routing rule validation failed: {}",
                    detailed_error
                );

                metrics::API_REQUEST_COUNTER
                    .with_label_values(&["routing_create", "failure"])
                    .inc();
                timer.observe_duration();

                return Err(ContainerError::new_with_status_code_and_payload(
                    EuclidErrors::FieldValidationFailed(detailed_error.clone()),
                    axum::http::StatusCode::BAD_REQUEST,
                    ApiErrorResponse::new(
                        "FIELD_VALIDATION_FAILED",
                        format!("Routing rule validation failed: {}", detailed_error),
                        Some(serde_json::json!({ "validation_errors": error_details })),
                    ),
                ));
            }
            logger::debug!("Routing rule validation passed successfully");
        }
        Err(err) => {
            logger::error!(error = ?err, "Failed to validate routing rule configuration");
            metrics::API_REQUEST_COUNTER
                .with_label_values(&["routing_create", "failure"])
                .inc();
            timer.observe_duration();
            return Err(err.into());
        }
    }

    let utc_date_time = time::OffsetDateTime::now_utc();
    let timestamp = time::PrimitiveDateTime::new(utc_date_time.date(), utc_date_time.time());

    let algorithm_id = config
        .rule_id
        .unwrap_or_else(|| generate_random_id("routing"));

    let new_algo = RoutingAlgorithm {
        id: algorithm_id.clone(),
        created_by: config.created_by,
        name: config.name.clone(),
        description: config.description,
        #[cfg(feature = "mysql")]
        metadata: Some(
            serde_json::to_string(&config.metadata)
                .change_context(EuclidErrors::FailedToSerializeJsonToString)?,
        ),
        #[cfg(feature = "postgres")]
        metadata: config.metadata.clone(),
        algorithm_data: serde_json::to_string(&config.algorithm)
            .change_context(EuclidErrors::FailedToSerializeJsonToString)?,
        algorithm_for: config.algorithm_for.to_string(),
        created_at: timestamp,
        modified_at: timestamp,
    };

    crate::generics::generic_insert(&state.db, new_algo)
        .await
        .map_err(|e| {
            logger::error!("{:?}", e);
            ContainerError::from(EuclidErrors::StorageError)
        })?;

    let response = RoutingDictionaryRecord::new(
        algorithm_id,
        config.name,
        config.algorithm_for.to_string(),
        timestamp,
        timestamp,
    );
    logger::debug!("Response: {response:?}");

    metrics::API_REQUEST_COUNTER
        .with_label_values(&["routing_create", "success"])
        .inc();
    timer.observe_duration();
    Ok(Json(response))
}

pub async fn routing_evaluate(
    Json(payload): Json<RoutingRequest>,
) -> Result<Json<RoutingEvaluateResponse>, ContainerError<EuclidErrors>> {
    let timer = metrics::API_LATENCY_HISTOGRAM
        .with_label_values(&["routing_evaluate"])
        .start_timer();

    API_REQUEST_TOTAL_COUNTER
        .with_label_values(&["routing_evaluate"])
        .inc();

    let state = get_tenant_app_state().await;
    logger::debug!(
        "Received routing evaluation request for ID: {}",
        payload.created_by
    );

    let config_identifier = format!(
        "{}_{}",
        DEFAULT_FALLBACK_IDENTIFIER,
        payload.created_by.clone()
    );

    let update_failure_metrics = || {
        API_REQUEST_COUNTER
            .with_label_values(&["routing_evaluate", "failure"])
            .inc();
    };

    // Check for the fallback_output in evaluate request:
    let default_output_present = payload
        .fallback_output
        .as_ref()
        .map_or(false, |output| !output.is_empty());

    // fetch the active routing_algorithm of the merchant
    let active_routing_algorithm_id = match crate::generics::generic_find_one::<
        <RoutingAlgorithmMapper as HasTable>::Table,
        _,
        RoutingAlgorithmMapper,
    >(
        &state.db,
        mapper_dsl::created_by.eq(payload.created_by.clone()),
    )
    .await
    .change_context(EuclidErrors::ActiveRoutingAlgorithmNotFound(
        payload.created_by.clone(),
    )) {
        Ok(mapper) => mapper.routing_algorithm_id,
        Err(e) => {
            update_failure_metrics();
            timer.observe_duration();
            return Err(e.into());
        }
    };

    let parameters = payload.parameters.clone();

    let routing_config = match state
        .config
        .routing_config
        .as_ref()
        .ok_or(EuclidErrors::GlobalRoutingConfigsUnavailable)
    {
        Ok(config) => config,
        Err(e) => {
            update_failure_metrics();
            timer.observe_duration();
            return Err(e.into());
        }
    };

    for (key, value) in &parameters {
        if !routing_config.keys.keys.contains_key(key)
            && value.as_ref().is_some_and(|val| !val.is_metadata())
        {
            update_failure_metrics();
            timer.observe_duration();
            return Err(EuclidErrors::InvalidRequestParameter(key.clone()).into());
        }

        if let Some(key_config) = routing_config.keys.keys.get(key) {
            if key_config.data_type == KeyDataType::Enum {
                if let Some(Some(ValueType::EnumVariant(value))) = parameters.get(key) {
                    if !is_valid_enum_value(routing_config, key, value) {
                        update_failure_metrics();
                        timer.observe_duration();
                        return Err(EuclidErrors::InvalidRequest(format!(
                            "Invalid enum value '{}' for key '{}'",
                            value, key
                        ))
                        .into());
                    }
                } else {
                    update_failure_metrics();
                    timer.observe_duration();
                    return Err(EuclidErrors::InvalidRequest(format!(
                        "Expected enum value for key '{}'",
                        key
                    ))
                    .into());
                }
            }
        }
    }

    let algorithm = match crate::generics::generic_find_one::<
        <RoutingAlgorithm as HasTable>::Table,
        _,
        RoutingAlgorithm,
    >(&state.db, dsl::id.eq(active_routing_algorithm_id.clone()))
    .await
    .map_err(|e| {
        logger::error!(
            ?e,
            "Failed to fetch RoutingAlgorithm for ID {:?}",
            active_routing_algorithm_id
        );
        e
    })
    .change_context(EuclidErrors::StorageError)
    {
        Ok(algo) => algo,
        Err(e) => {
            update_failure_metrics();
            timer.observe_duration();
            return Err(e.into());
        }
    };

    logger::debug!("Fetched routing algorithm: {:?}", algorithm);
    let algorithm_data: StaticRoutingAlgorithm =
        match serde_json::from_str(&algorithm.algorithm_data).map_err(|e| {
            logger::error!(
                error = ?e,
                raw_data = %algorithm.algorithm_data,
                "Failed to parse algorithm_data into StaticRoutingAlgorithm"
            );
            EuclidErrors::InvalidRequest(format!("Invalid algorithm data format: {}", e))
        }) {
            Ok(data) => data,
            Err(e) => {
                update_failure_metrics();
                timer.observe_duration();
                return Err(e.into());
            }
        };

    let (output, evaluated_output, rule_name): (Output, Vec<ConnectorInfo>, Option<String>) =
        match algorithm_data {
            StaticRoutingAlgorithm::Single(conn) => {
                let out_enum = Output::Single(*conn.clone());
                match evaluate_output(&out_enum).map_err(|_| {
                    EuclidErrors::FailedToEvaluateOutput(format!(
                        "{}",
                        StaticRoutingAlgorithm::Single(conn.clone()).to_string()
                    ))
                }) {
                    Ok((_, eval)) => (out_enum, eval, Some("straight_through_rule".into())),
                    Err(e) => {
                        update_failure_metrics();
                        timer.observe_duration();
                        return Err(e.into());
                    }
                }
            }

            StaticRoutingAlgorithm::Priority(connectors) => {
                let out_enum = Output::Priority(connectors.clone());
                match evaluate_output(&out_enum).map_err(|_| {
                    EuclidErrors::FailedToEvaluateOutput(format!(
                        "{}",
                        StaticRoutingAlgorithm::Priority(connectors.clone()).to_string()
                    ))
                }) {
                    Ok((_, eval)) => (out_enum, eval, Some("priority_rule".into())),
                    Err(e) => {
                        update_failure_metrics();
                        timer.observe_duration();
                        return Err(e.into());
                    }
                }
            }

            StaticRoutingAlgorithm::VolumeSplit(splits) => {
                let out_enum = Output::VolumeSplit(splits.clone());
                match evaluate_output(&out_enum).map_err(|_| {
                    EuclidErrors::FailedToEvaluateOutput(format!(
                        "{}",
                        StaticRoutingAlgorithm::VolumeSplit(splits.clone()).to_string()
                    ))
                }) {
                    Ok((_, eval)) => (out_enum, eval, Some("volume_split_rule".into())),
                    Err(e) => {
                        update_failure_metrics();
                        timer.observe_duration();
                        return Err(e.into());
                    }
                }
            }

            StaticRoutingAlgorithm::Advanced(program) => {
                let ctx = Context::new(payload.parameters.clone());
                logger::debug!("routing_evaluation: context keys = {:?}", parameters.keys());

                match InterpreterBackend::eval_program(&program, &ctx).map_err(|e| {
                    EuclidErrors::InvalidRequest(format!("Interpreter error: {:?}", e.error_type))
                }) {
                    Ok(mut ir) => {
                        // Check if fallback is enabled
                        if default_output_present && ir.output == program.default_selection {
                            logger::debug!(
                                "Default fallback triggered: Overriding with fallback connector"
                            );

                            // Replace output with fallback connector from request
                            if let Some(fallback_connector) = payload.fallback_output.clone() {
                                ir.rule_name = Some("default_fallback".to_string());
                                ir.output = Output::Priority(fallback_connector.clone());
                                ir.evaluated_output =
                                    vec![fallback_connector.first().cloned().unwrap_or_default()];
                            }
                        }
                        (ir.output, ir.evaluated_output, ir.rule_name)
                    }
                    Err(e) => {
                        update_failure_metrics();
                        timer.observe_duration();
                        return Err(e.into());
                    }
                }
            }
        };

    let eligible_connectors = if let Some(ref cfg) = state.config.routing_config {
        let ctx = cgraph::CheckCtx::from(payload.parameters.clone());
        perform_eligibility_analysis(&cfg.constraint_graph, ctx, &evaluated_output)
    } else {
        evaluated_output.clone()
    };

    let response = RoutingEvaluateResponse {
        status: match rule_name.as_deref() {
            Some("default_selection") | Some("default_fallback") => "default_selection".into(),
            Some(_) => "success".into(),
            None => "default_selection".into(),
        },
        output: format_output(&output),
        evaluated_output,
        eligible_connectors,
    };

    logger::debug!("Response: {response:?}");

    API_REQUEST_COUNTER
        .with_label_values(&["routing_evaluate", "success"])
        .inc();
    timer.observe_duration();
    Ok(Json(response))
}

#[cfg(feature = "mysql")]
use crate::storage::schema::routing_algorithm_mapper::dsl as mapper_dsl;
#[cfg(feature = "postgres")]
use crate::storage::schema_pg::routing_algorithm_mapper::dsl as mapper_dsl;

pub async fn activate_routing_rule(
    Json(payload): Json<ActivateRoutingConfigRequest>,
) -> Result<(), ContainerError<EuclidErrors>> {
    let timer = API_LATENCY_HISTOGRAM
        .with_label_values(&["activate_routing_rule"])
        .start_timer();
    API_REQUEST_TOTAL_COUNTER
        .with_label_values(&["activate_routing_rule"])
        .inc();

    let update_failure_metrics = || {
        API_REQUEST_COUNTER
            .with_label_values(&["activate_routing_rule", "failure"])
            .inc();
    };

    let state = get_tenant_app_state().await;
    let conn = match state
        .db
        .get_conn()
        .await
        .map_err(|_| EuclidErrors::StorageError)
    {
        Ok(connection) => connection,
        Err(e) => {
            update_failure_metrics();
            timer.observe_duration();
            return Err(e.into());
        }
    };

    // === Step 1: Find algorithm_for from RoutingAlgorithm table ===
    let algorithm_for = match crate::generics::generic_find_one::<
        <RoutingAlgorithm as HasTable>::Table,
        _,
        RoutingAlgorithm,
    >(&state.db, dsl::id.eq(payload.routing_algorithm_id.clone()))
    .await
    .change_context(EuclidErrors::RoutingAlgorithmNotFound(
        payload.routing_algorithm_id.clone(),
    )) {
        Ok(algorithm) => algorithm.algorithm_for,
        Err(e) => {
            update_failure_metrics();
            timer.observe_duration();
            return Err(e.into());
        }
    };

    // === Step 2: Try to find existing entry for (created_by, algorithm_for) ===
    let maybe_existing = crate::generics::generic_find_one::<
        <RoutingAlgorithmMapper as HasTable>::Table,
        _,
        RoutingAlgorithmMapper,
    >(
        &state.db,
        mapper_dsl::created_by
            .eq(payload.created_by.clone())
            .and(mapper_dsl::algorithm_for.eq(algorithm_for.clone())),
    )
    .await
    .ok();

    if let Some(existing) = maybe_existing {
        if existing.routing_algorithm_id != payload.routing_algorithm_id {
            // === Step 3a: Update routing_algorithm_id in place ===
            let predicate = mapper_dsl::created_by
                .eq(payload.created_by.clone())
                .and(mapper_dsl::algorithm_for.eq(algorithm_for.clone()));

            let values = RoutingAlgorithmMapperUpdate {
                routing_algorithm_id: payload.routing_algorithm_id.clone(),
                algorithm_for: algorithm_for.clone(),
            };

            match crate::generics::generic_update_if_present::<
                <RoutingAlgorithmMapper as HasTable>::Table,
                RoutingAlgorithmMapperUpdate,
                _,
            >(&conn, predicate, values)
            .await
            .change_context(EuclidErrors::StorageError)
            {
                Ok(_) => {
                    API_REQUEST_COUNTER
                        .with_label_values(&["activate_routing_rule", "success"])
                        .inc();
                    timer.observe_duration();
                    return Ok(());
                }
                Err(e) => {
                    update_failure_metrics();
                    timer.observe_duration();
                    return Err(e.into());
                }
            }
        }
        API_REQUEST_COUNTER
            .with_label_values(&["activate_routing_rule", "success"])
            .inc();
        timer.observe_duration();
        return Ok(());
    }

    // === Step 3b: Insert new if not present ===
    let mapper_entry = RoutingAlgorithmMapperNew::new(
        payload.created_by,
        payload.routing_algorithm_id,
        algorithm_for,
    );

    match crate::generics::generic_insert(&state.db, mapper_entry)
        .await
        .change_context(EuclidErrors::StorageError)
    {
        Ok(_) => {
            API_REQUEST_COUNTER
                .with_label_values(&["activate_routing_rule", "success"])
                .inc();
            timer.observe_duration();
            Ok(())
        }
        Err(e) => {
            update_failure_metrics();
            timer.observe_duration();
            Err(e.into())
        }
    }
}

pub async fn list_all_routing_algorithm_id(
    Path(created_by): Path<String>,
) -> Result<Json<Vec<JsonifiedRoutingAlgorithm>>, ContainerError<EuclidErrors>> {
    let timer = API_LATENCY_HISTOGRAM
        .with_label_values(&["list_all_routing_algorithm_id"])
        .start_timer();
    API_REQUEST_TOTAL_COUNTER
        .with_label_values(&["list_all_routing_algorithm_id"])
        .inc();

    let state = get_tenant_app_state().await;

    match crate::generics::generic_find_all::<
        <RoutingAlgorithm as HasTable>::Table,
        _,
        RoutingAlgorithm,
    >(&state.db, dsl::created_by.eq(created_by))
    .await
    .change_context(EuclidErrors::StorageError)
    {
        Ok(algorithms) => {
            API_REQUEST_COUNTER
                .with_label_values(&["list_all_routing_algorithm_id", "success"])
                .inc();
            timer.observe_duration();
            Ok(Json(algorithms.into_iter().map(Into::into).collect()))
        }
        Err(e) => {
            API_REQUEST_COUNTER
                .with_label_values(&["list_all_routing_algorithm_id", "failure"])
                .inc();
            timer.observe_duration();
            Err(e.into())
        }
    }
}

#[axum::debug_handler]
pub async fn list_active_routing_algorithm(
    Path(created_by): Path<String>,
) -> Result<Json<Vec<JsonifiedRoutingAlgorithm>>, ContainerError<EuclidErrors>> {
    let timer = metrics::API_LATENCY_HISTOGRAM
        .with_label_values(&["list_active_routing_algorithm"])
        .start_timer();
    metrics::API_REQUEST_TOTAL_COUNTER
        .with_label_values(&["list_active_routing_algorithm"])
        .inc();

    let update_failure_metrics = || {
        API_REQUEST_COUNTER
            .with_label_values(&["list_active_routing_algorithm", "failure"])
            .inc();
    };

    let state = get_tenant_app_state().await;

    let active_mappings = match crate::generics::generic_find_all::<
        <RoutingAlgorithmMapper as HasTable>::Table,
        _,
        RoutingAlgorithmMapper,
    >(&state.db, mapper_dsl::created_by.eq(created_by.clone()))
    .await
    .change_context(EuclidErrors::ActiveRoutingAlgorithmNotFound(
        created_by.clone(),
    )) {
        Ok(mappings) => mappings,
        Err(e) => {
            update_failure_metrics();
            timer.observe_duration();
            return Err(e.into());
        }
    };

    let ids: Vec<String> = active_mappings
        .into_iter()
        .map(|m| m.routing_algorithm_id)
        .collect();

    let routing_algorithms = match crate::generics::generic_find_all::<
        <RoutingAlgorithm as HasTable>::Table,
        _,
        RoutingAlgorithm,
    >(&state.db, dsl::id.eq_any(ids))
    .await
    .change_context(EuclidErrors::StorageError)
    {
        Ok(algos) => algos,
        Err(e) => {
            update_failure_metrics();
            timer.observe_duration();
            return Err(e.into());
        }
    };
    let result = routing_algorithms
        .into_iter()
        .map(JsonifiedRoutingAlgorithm::from)
        .collect();

    API_REQUEST_COUNTER
        .with_label_values(&["list_active_routing_algorithm", "success"])
        .inc();
    timer.observe_duration();

    Ok(Json(result))
}

fn format_output(output: &Output) -> Value {
    match output {
        Output::Single(connector) => {
            json!({
                "type": "straight_through",
                "connector": connector
            })
        }
        Output::Priority(connectors) => {
            json!({
                "type": "priority",
                "connectors": connectors
            })
        }
        Output::VolumeSplit(splits) => {
            let formatted_splits: Vec<Value> = splits
                .iter()
                .map(|split| {
                    json!({
                        "connector": split.output,
                        "split": split.split
                    })
                })
                .collect();
            json!({
                "type": "volume_split",
                "splits": formatted_splits
            })
        }
        Output::VolumeSplitPriority(splits) => {
            let formatted_splits: Vec<Value> = splits
                .iter()
                .map(|split| {
                    json!({
                        "connectors": split.output,
                        "split": split.split
                    })
                })
                .collect();
            json!({
                "type": "volume_split_priority",
                "splits": formatted_splits
            })
        }
    }
}

fn perform_eligibility_analysis(
    constraint_graph: &cgraph::ConstraintGraph,
    ctx: cgraph::CheckCtx,
    output: &[ConnectorInfo],
) -> Vec<ConnectorInfo> {
    let mut eligible_connectors = Vec::<ConnectorInfo>::with_capacity(output.len());

    for out in output {
        let clause = cgraph::Clause {
            key: "output".to_string(),
            comparison: ComparisonType::Equal,
            value: ValueType::EnumVariant(out.gateway_name.clone()),
        };

        if let Ok(true) = constraint_graph.check_clause_validity(clause, &ctx) {
            eligible_connectors.push(out.clone());
        }
    }

    eligible_connectors
}
