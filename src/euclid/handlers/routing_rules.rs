#[cfg(feature = "mysql")]
use crate::storage::schema::routing_algorithm::dsl;
#[cfg(feature = "postgres")]
use crate::storage::schema_pg::routing_algorithm::dsl;
use crate::{
    error::ApiErrorResponse,
    euclid::{
        ast::{self, ComparisonType, ConnectorInfo, Output, ValueType},
        cgraph,
        interpreter::InterpreterBackend,
        types::{
            ActivateRoutingConfigRequest, Context, JsonifiedRoutingAlgorithm,
            RoutingDictionaryRecord, RoutingEvaluateResponse, RoutingRequest, RoutingRule,
            StaticRoutingAlgorithm,
        },
        utils::{generate_random_id, is_valid_enum_value, validate_routing_rule},
    },
};

use crate::euclid::{
    errors::EuclidErrors,
    types::{RoutingAlgorithmMapper, RoutingAlgorithmMapperUpdate},
};
use crate::{euclid::types::RoutingAlgorithm, logger};
use axum::{extract::Path, Json};
use diesel::{associations::HasTable, ExpressionMethods};
use error_stack::ResultExt;

use crate::app::get_tenant_app_state;

use crate::error::{self, ContainerError};
use serde_json::{json, Value};

pub async fn routing_create(
    Json(payload): Json<Value>,
) -> Result<Json<RoutingDictionaryRecord>, ContainerError<EuclidErrors>> {
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

    let response = RoutingDictionaryRecord::new(algorithm_id, config.name, timestamp, timestamp);
    logger::info!("Response: {response:?}");

    Ok(Json(response))
}

#[cfg(feature = "mysql")]
use crate::storage::schema::routing_algorithm_mapper::dsl as mapper_dsl;
#[cfg(feature = "postgres")]
use crate::storage::schema_pg::routing_algorithm_mapper::dsl as mapper_dsl;
pub async fn activate_routing_rule(
    Json(payload): Json<ActivateRoutingConfigRequest>,
) -> Result<(), ContainerError<EuclidErrors>> {
    let state = get_tenant_app_state().await;
    // Update the RoutingAlgorithmMapper table here with new rule_id
    // Find whether this creator previously has an entry in mapper table
    // If yes go on with updating the rule_id inplace.
    // If not create a new entry in RoutingAlgorithmMapper table.

    let conn = &state
        .db
        .get_conn()
        .await
        .map_err(|_| EuclidErrors::StorageError)?;
    let predicate = mapper_dsl::created_by.eq(payload.created_by.clone());
    let values = RoutingAlgorithmMapperUpdate {
        routing_algorithm_id: payload.routing_algorithm_id.clone(),
    };

    let rows_affected = crate::generics::generic_update_if_present::<
        <RoutingAlgorithmMapper as HasTable>::Table,
        RoutingAlgorithmMapperUpdate,
        _,
    >(conn, predicate, values)
    .await
    .change_context(EuclidErrors::StorageError)?;

    if rows_affected > 0 {
        return Ok(());
    } else {
        let mapper_entry =
            RoutingAlgorithmMapper::new(payload.created_by, payload.routing_algorithm_id);
        crate::generics::generic_insert(&state.db, mapper_entry)
            .await
            .change_context(EuclidErrors::StorageError)?;
        return Ok(());
    }
}

pub async fn list_all_routing_algorithm_id(
    Path(created_by): Path<String>,
) -> Result<Json<Vec<JsonifiedRoutingAlgorithm>>, ContainerError<EuclidErrors>> {
    let state = get_tenant_app_state().await;
    Ok(Json(
        crate::generics::generic_find_all::<
            <RoutingAlgorithm as HasTable>::Table,
            _,
            RoutingAlgorithm,
        >(&state.db, dsl::created_by.eq(created_by))
        .await
        .change_context(EuclidErrors::StorageError)?
        .into_iter()
        .map(Into::into)
        .collect(),
    ))
}

#[axum::debug_handler]
pub async fn list_active_routing_algorithm(
    Path(created_by): Path<String>,
) -> Result<Json<JsonifiedRoutingAlgorithm>, ContainerError<EuclidErrors>> {
    let state = get_tenant_app_state().await;
    let active_routing_algorithm_id =
        crate::generics::generic_find_one::<
            <RoutingAlgorithmMapper as HasTable>::Table,
            _,
            RoutingAlgorithmMapper,
        >(&state.db, mapper_dsl::created_by.eq(created_by.clone()))
        .await
        .change_context(EuclidErrors::ActiveRoutingAlgorithmNotFound(
            created_by.clone(),
        ))?
        .routing_algorithm_id;

    Ok(Json(
        crate::generics::generic_find_one::<
            <RoutingAlgorithm as HasTable>::Table,
            _,
            RoutingAlgorithm,
        >(&state.db, dsl::id.eq(active_routing_algorithm_id))
        .await
        .change_context(EuclidErrors::StorageError)?
        .into(),
    ))
}

pub async fn routing_evaluate(
    Json(payload): Json<RoutingRequest>,
) -> Result<Json<RoutingEvaluateResponse>, ContainerError<EuclidErrors>> {
    let state = get_tenant_app_state().await;
    logger::debug!(
        "Received routing evaluation request for ID: {}",
        payload.created_by
    );

    // fetch the active routing_algorithm of the merchant
    let active_routing_algorithm_id = crate::generics::generic_find_one::<
        <RoutingAlgorithmMapper as HasTable>::Table,
        _,
        RoutingAlgorithmMapper,
    >(
        &state.db,
        mapper_dsl::created_by.eq(payload.created_by.clone()),
    )
    .await
    .change_context(EuclidErrors::ActiveRoutingAlgorithmNotFound(
        payload.created_by,
    ))?
    .routing_algorithm_id;

    let parameters = payload.parameters.clone();

    let routing_config = state
        .config
        .routing_config
        .as_ref()
        .ok_or(EuclidErrors::GlobalRoutingConfigsUnavailable)?;

    for (key, _) in &parameters {
        if !routing_config.keys.keys.contains_key(key) {
            return Err(EuclidErrors::InvalidRequestParameter(key.clone()).into());
        }

        if let Some(key_config) = routing_config.keys.keys.get(key) {
            if key_config.data_type == "enum" {
                if let Some(Some(ValueType::EnumVariant(value))) = parameters.get(key) {
                    if !is_valid_enum_value(routing_config, key, value) {
                        return Err(EuclidErrors::InvalidRequest(format!(
                            "Invalid enum value '{}' for key '{}'",
                            value, key
                        ))
                        .into());
                    }
                } else {
                    return Err(EuclidErrors::InvalidRequest(format!(
                        "Expected enum value for key '{}'",
                        key
                    ))
                    .into());
                }
            }
        }
    }

    let algorithm = crate::generics::generic_find_one::<
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
    .change_context(EuclidErrors::StorageError)?;

    logger::debug!("Fetched routing algorithm: {:?}", algorithm);
    let algorithm_data: StaticRoutingAlgorithm = serde_json::from_str(&algorithm.algorithm_data)
        .map_err(|e| {
            logger::error!(
                error = ?e,
                raw_data = %algorithm.algorithm_data,
                "Failed to parse algorithm_data into StaticRoutingAlgorithm"
            );
            EuclidErrors::InvalidRequest(format!("Invalid algorithm data format: {}", e))
        })?;

    let program = match algorithm_data {
        StaticRoutingAlgorithm::Advanced(p) => p,
    };

    let context = Context::new(parameters.clone());

    logger::debug!("routing_evaluation: context keys = {:?}", parameters.keys());
    let interpreter_result = InterpreterBackend::eval_program(&program, &context).map_err(|e| {
        EuclidErrors::InvalidRequest(format!("Interpreter error: {:?}", e.error_type))
    })?;

    let eligible_connectors = if let Some(ref config) = state.config.routing_config {
        let ctx = cgraph::CheckCtx::from(parameters);
        perform_eligibility_analysis(
            &config.constraint_graph,
            ctx,
            &interpreter_result.evaluated_output,
        )
    } else {
        interpreter_result.evaluated_output.clone()
    };

    let response = RoutingEvaluateResponse {
        status: if interpreter_result.rule_name.is_some() {
            "success".to_string()
        } else {
            "default_selection".to_string()
        },
        output: format_output(&interpreter_result.output),
        evaluated_output: interpreter_result.evaluated_output.clone(),
        eligible_connectors,
    };
    logger::info!("Response: {response:?}");

    Ok(Json(response))
}

fn format_output(output: &Output) -> Value {
    match output {
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
