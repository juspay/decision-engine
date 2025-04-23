use crate::euclid::{
    ast::{self, ComparisonType, Output, ValueType},
    cgraph,
    interpreter::InterpreterBackend,
    types::{
        Context, RoutingDictionaryRecord, RoutingEvaluateResponse, RoutingRequest, RoutingRule,
    },
    utils::{generate_random_id, is_valid_enum_value, validate_routing_rule},
};
use crate::storage::schema::routing_algorithm::dsl;
use crate::{
    euclid::{
        errors::EuclidErrors,
        types::{RoutingAlgorithmMapper, RoutingAlgorithmMapperUpdate},
    },
    storage::schema::routing_algorithm_mapper,
};
use crate::{logger, euclid::types::RoutingAlgorithm};
use axum::{extract::Path, Json};
use diesel::{associations::HasTable, ExpressionMethods};
use error_stack::ResultExt;

use crate::app::get_tenant_app_state;

use crate::error::{self, ContainerError};
use serde_json::{json, Value};

pub async fn routing_create(
    Json(payload): Json<Value>,
) -> Result<Json<RoutingDictionaryRecord>, error::ContainerError<EuclidErrors>> {
    let state = get_tenant_app_state().await;
    let config: RoutingRule = serde_json::from_value(payload.clone())
        .change_context(EuclidErrors::InvalidRuleConfiguration)?;

    logger::debug!("Received routing config: {}", config.name);

    validate_routing_rule(&config, &state.config.routing_config)?;

    let utc_date_time = time::OffsetDateTime::now_utc();
    let timestamp = time::PrimitiveDateTime::new(utc_date_time.date(), utc_date_time.time());
    let data = serde_json::to_value(config.algorithm.clone());
    let algorithm_id = generate_random_id("routing");
    if let Ok(data) = data {
        let new_algo = RoutingAlgorithm {
            id: algorithm_id.clone(),
            created_by: config.created_by,
            name: "My Algo".into(),
            description: Some("Test algo".into()),
            algorithm_data: serde_json::to_string(&data).unwrap(),
            created_at: timestamp,
            modified_at: timestamp,
        };

        crate::generics::generic_insert(&state.db, new_algo)
            .await
            .map_err(|_| ContainerError::from(EuclidErrors::StorageError))?;

        let response =
            RoutingDictionaryRecord::new(algorithm_id, config.name, timestamp, timestamp);
        Ok(Json(response))
    } else {
        Err(ContainerError::from(EuclidErrors::StorageError))
    }
}

use crate::storage::schema::routing_algorithm_mapper::dsl as mapper_dsl;
pub async fn activate_routing_rule(
    Json(payload): Json<RoutingAlgorithmMapper>,
) -> Result<(), ContainerError<EuclidErrors>> {
    let state = get_tenant_app_state().await;
    // Update the RoutingAlgorithmMapper table here with new rule_id
    // Find whether this creator previously has an entry in mapper table
    // If yes go on with updating the rule_id inplace.
    // If not create a new entry in RoutingAlgorithmMapper table.

    let mut conn = &state
        .db
        .get_conn()
        .await
        .map_err(|_| EuclidErrors::StorageError)?;
    let predicate = mapper_dsl::created_by.eq(payload.created_by.clone());
    let values = RoutingAlgorithmMapperUpdate {
        routing_algorithm_id: payload.routing_algorithm_id.clone(),
    };

    match crate::generics::generic_update::<
        <RoutingAlgorithmMapper as HasTable>::Table,
        RoutingAlgorithmMapperUpdate,
        _,
    >(conn, predicate, values)
    .await
    {
        Ok(rows_affected) if rows_affected > 0 => Ok(()),
        Ok(_) => {
            // Creator is non-existent in mapper table
            let mapper_entry = RoutingAlgorithmMapper::new(
                payload.created_by,
                payload.routing_algorithm_id
            );
            crate::generics::generic_insert(&state.db, mapper_entry)
                .await
                .map_err(|_| ContainerError::from(EuclidErrors::StorageError))?;
            return Ok(());
        }
        Err(err) => return Err(EuclidErrors::StorageError.into()),
    }
}

pub async fn list_all_routing_algorithm_id(
    Path(created_by): Path<String>,
// ) -> Result<Vec<RoutingAlgorithm>, ContainerError<EuclidErrors>> {
) -> Result<(), ContainerError<EuclidErrors>> {
    let state = get_tenant_app_state().await;
    let res = crate::generics::generic_find_all::<
            <RoutingAlgorithm as HasTable>::Table,
            _,
            RoutingAlgorithm,
        >(&state.db, dsl::created_by.eq(created_by))
        .await
        .change_context(EuclidErrors::StorageError)?;
    println!(">>>>>>>>>>>>>>>{:?}",res);
    Ok(())
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
    let routing_algorithm_id = crate::generics::generic_find_one::<
        <RoutingAlgorithmMapper as HasTable>::Table,
        _,
        RoutingAlgorithmMapper,
    >(&state.db, mapper_dsl::created_by.eq(payload.created_by.clone()))
    .await
    //PKTODO: add error to let merchant know that he didn't activate a routing rule.
    .change_context(EuclidErrors::StorageError)?.routing_algorithm_id;

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

    let payload_clone = payload.created_by.clone();

    let algorithm = crate::generics::generic_find_one::<
        <RoutingAlgorithm as HasTable>::Table,
        _,
        RoutingAlgorithm,
    >(&state.db, dsl::id.eq(routing_algorithm_id))
    .await
    .change_context(EuclidErrors::StorageError)?;

    let program: ast::Program = serde_json::from_str(&algorithm.algorithm_data)
        .map_err(|_| EuclidErrors::InvalidRequest("Invalid algorithm data format".into()))?;

    let context = Context::new(parameters.clone());
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
    output: &[String],
) -> Vec<String> {
    let mut eligible_connectors = Vec::<String>::with_capacity(output.len());

    for out in output {
        let clause = cgraph::Clause {
            key: "output".to_string(),
            comparison: ComparisonType::Equal,
            value: ValueType::EnumVariant(out.clone()),
        };

        if let Ok(true) = constraint_graph.check_clause_validity(clause, &ctx) {
            eligible_connectors.push(out.clone());
        }
    }

    eligible_connectors
}
