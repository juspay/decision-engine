#[cfg(feature = "mysql")]
use crate::storage::schema::routing_algorithm::dsl;
#[cfg(feature = "postgres")]
use crate::storage::schema_pg::routing_algorithm::dsl;
use crate::{
    error::ApiErrorResponse,
    euclid::{
        ast::{self, ComparisonType, ConnectorInfo, Output, ValueType},
        cgraph,
        interpreter::{evaluate_output, InterpreterBackend},
        types::{
            ActivateRoutingConfigRequest, Context, JsonifiedRoutingAlgorithm,
            RoutingAlgorithmMapperNew, RoutingDictionaryRecord, RoutingEvaluateResponse,
            RoutingRequest, RoutingRule, StaticRoutingAlgorithm,
        },
        utils::{generate_random_id, is_valid_enum_value, validate_routing_rule},
    },
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

use crate::error::{self, ContainerError};
use crate::metrics::{API_LATENCY_HISTOGRAM, API_REQUEST_COUNTER, API_REQUEST_TOTAL_COUNTER};
use serde_json::{json, Value};

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

    if let Err(err) = validate_routing_rule(&config, &state.config.routing_config) {
        let source = err.get_inner();

        if let EuclidErrors::FailedToValidateRoutingRule = source {
            if let Some(validation_messages) = err.downcast_ref::<Vec<String>>() {
                let detailed_error = validation_messages.join("; ");
                logger::error!("Routing rule validation failed with errors: {detailed_error}");

                metrics::API_REQUEST_COUNTER
                    .with_label_values(&["routing_create", "failure"])
                    .inc();
                timer.observe_duration();
                return Err(ContainerError::new_with_status_code_and_payload(
                    EuclidErrors::FailedToValidateRoutingRule,
                    axum::http::StatusCode::BAD_REQUEST,
                    ApiErrorResponse::new(
                        "INVALID_REQUEST_DATA",
                        format!("Routing rule validation failed: {}", detailed_error),
                        None,
                    ),
                ));
            }
        }
        metrics::API_REQUEST_COUNTER
            .with_label_values(&["routing_create", "failure"])
            .inc();
        timer.observe_duration();
        return Err(err.into());
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
    logger::info!("Response: {response:?}");

    metrics::API_REQUEST_COUNTER
        .with_label_values(&["routing_create", "success"])
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

    let state = get_tenant_app_state().await;
    let conn = match state
        .db
        .get_conn()
        .await
        .map_err(|_| EuclidErrors::StorageError)
    {
        Ok(connection) => connection,
        Err(e) => {
            API_REQUEST_COUNTER
                .with_label_values(&["activate_routing_rule", "failure"])
                .inc();
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
            API_REQUEST_COUNTER
                .with_label_values(&["activate_routing_rule", "failure"])
                .inc();
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
                    API_REQUEST_COUNTER
                        .with_label_values(&["activate_routing_rule", "failure"])
                        .inc();
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
            API_REQUEST_COUNTER
                .with_label_values(&["activate_routing_rule", "failure"])
                .inc();
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
            metrics::API_REQUEST_COUNTER
                .with_label_values(&["list_active_routing_algorithm", "failure"])
                .inc();
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
            metrics::API_REQUEST_COUNTER
                .with_label_values(&["list_active_routing_algorithm", "failure"])
                .inc();
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
            API_REQUEST_COUNTER
                .with_label_values(&["routing_evaluate", "failure"])
                .inc();
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
            API_REQUEST_COUNTER
                .with_label_values(&["routing_evaluate", "failure"])
                .inc();
            timer.observe_duration();
            return Err(e.into());
        }
    };

    for (key, _) in &parameters {
        if !routing_config.keys.keys.contains_key(key) {
            API_REQUEST_COUNTER
                .with_label_values(&["routing_evaluate", "failure"])
                .inc();
            timer.observe_duration();
            return Err(EuclidErrors::InvalidRequestParameter(key.clone()).into());
        }

        if let Some(key_config) = routing_config.keys.keys.get(key) {
            if key_config.data_type == "enum" {
                if let Some(Some(ValueType::EnumVariant(value))) = parameters.get(key) {
                    if !is_valid_enum_value(routing_config, key, value) {
                        API_REQUEST_COUNTER
                            .with_label_values(&["routing_evaluate", "failure"])
                            .inc();
                        timer.observe_duration();
                        return Err(EuclidErrors::InvalidRequest(format!(
                            "Invalid enum value '{}' for key '{}'",
                            value, key
                        ))
                        .into());
                    }
                } else {
                    API_REQUEST_COUNTER
                        .with_label_values(&["routing_evaluate", "failure"])
                        .inc();
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
            API_REQUEST_COUNTER
                .with_label_values(&["routing_evaluate", "failure"])
                .inc();
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
                API_REQUEST_COUNTER
                    .with_label_values(&["routing_evaluate", "failure"])
                    .inc();
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
                    Ok((_, eval)) => (out_enum, eval, Some("straight_through".into())),
                    Err(e) => {
                        API_REQUEST_COUNTER
                            .with_label_values(&["routing_evaluate", "failure"])
                            .inc();
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
                    Ok((_, eval)) => (out_enum, eval, Some("priority".into())),
                    Err(e) => {
                        API_REQUEST_COUNTER
                            .with_label_values(&["routing_evaluate", "failure"])
                            .inc();
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
                    Ok((_, eval)) => (out_enum, eval, Some("volume_split".into())),
                    Err(e) => {
                        API_REQUEST_COUNTER
                            .with_label_values(&["routing_evaluate", "failure"])
                            .inc();
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
                    Ok(ir) => (ir.output, ir.evaluated_output, ir.rule_name),
                    Err(e) => {
                        API_REQUEST_COUNTER
                            .with_label_values(&["routing_evaluate", "failure"])
                            .inc();
                        timer.observe_duration();
                        return Err(e.into());
                    }
                }
            }

            StaticRoutingAlgorithm::DefaultFallback(connectors) => {
                let out_enum = Output::DefaultFallback(connectors.clone());
                match evaluate_output(&out_enum).map_err(|_| {
                    EuclidErrors::FailedToEvaluateOutput(format!(
                        "{}",
                        StaticRoutingAlgorithm::DefaultFallback(connectors.clone()).to_string()
                    ))
                }) {
                    Ok((_, eval)) => (out_enum, eval, Some("default_fallback".into())),
                    Err(e) => {
                        API_REQUEST_COUNTER
                            .with_label_values(&["routing_evaluate", "failure"])
                            .inc();
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
        status: if rule_name.is_some() {
            "success".into()
        } else {
            "default_selection".into()
        },
        output: format_output(&output),
        evaluated_output,
        eligible_connectors,
    };
    logger::info!("Response: {response:?}");

    API_REQUEST_COUNTER
        .with_label_values(&["routing_evaluate", "success"])
        .inc();
    timer.observe_duration();
    Ok(Json(response))
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
        Output::DefaultFallback(connectors) => {
            json!({
                "type": "default_fallback",
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
