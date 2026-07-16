#[cfg(feature = "mysql")]
use crate::storage::schema::routing_algorithm::dsl;
#[cfg(feature = "postgres")]
use crate::storage::schema_pg::routing_algorithm::dsl;
use crate::{
    error::ApiErrorResponse,
    euclid::{
        ast::{ConnectorInfo, Output, ValueType},
        interpreter::{evaluate_output, InterpreterBackend},
        pm_filter_graph,
        types::{
            ActivateRoutingConfigRequest, Context, DeactivateRoutingConfigRequest,
            JsonifiedRoutingAlgorithm, KeyDataType, RoutingAlgorithmMapperNew,
            RoutingDictionaryRecord, RoutingEvaluateResponse, RoutingRequest, RoutingRule,
            SrDimensionConfig, StaticRoutingAlgorithm, ELIGIBLE_DIMENSIONS,
        },
        utils::{generate_random_id, is_valid_enum_value, validate_routing_rule},
    },
    types::service_configuration::{find_config_by_name, insert_config, update_config},
};

use crate::euclid::{
    errors::EuclidErrors,
    errors::ValidationErrorDetails,
    types::{RoutingAlgorithmMapper, RoutingAlgorithmMapperUpdate},
};
use crate::{euclid::types::RoutingAlgorithm, logger, metrics};
use axum::{extract::Path, response::IntoResponse, Json};
use diesel::{associations::HasTable, BoolExpressionMethods, ExpressionMethods};
use error_stack::ResultExt;

use crate::app::get_tenant_app_state;

use crate::error::ContainerError;
use crate::metrics::{API_LATENCY_HISTOGRAM, API_REQUEST_COUNTER, API_REQUEST_TOTAL_COUNTER};

// ── Routing algorithm Redis cache ─────────────────────────────────────────────
//
// Caches the active routing algorithm per merchant to avoid 2 DB queries on
// every /routing/evaluate call. Written on activate, deleted on deactivate,
// read-with-fallback on evaluate.
//
// Key  : DE_routing_algo_eval:{merchant_id}
// Value: JSON { id, algorithm_data }
// TTL  : cache_config.service_config_ttl (default 300s)

const ROUTING_ALGO_CACHE_PREFIX: &str = "DE_routing_algo_eval:";

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct CachedRoutingAlgorithm {
    id: String,
    algorithm_data: String,
}

fn routing_algo_cache_key(merchant_id: &str) -> String {
    format!("{}{}", ROUTING_ALGO_CACHE_PREFIX, merchant_id)
}

async fn cache_routing_algorithm(
    state: &crate::app::TenantAppState,
    merchant_id: &str,
    algorithm: &RoutingAlgorithm,
) {
    let key = routing_algo_cache_key(merchant_id);
    let value = CachedRoutingAlgorithm {
        id: algorithm.id.clone(),
        algorithm_data: algorithm.algorithm_data.clone(),
    };
    let ttl = state.config.cache_config.service_config_ttl;
    if let Err(e) = state.redis_conn.set_key_with_ttl(&key, value, ttl).await {
        logger::warn!(error = ?e, merchant_id = %merchant_id, "Failed to cache routing algorithm in Redis — falling back to DB on next evaluate");
    }
}

async fn invalidate_routing_algorithm_cache(state: &crate::app::TenantAppState, merchant_id: &str) {
    let key = routing_algo_cache_key(merchant_id);
    if let Err(e) = state.redis_conn.delete_key(&key).await {
        logger::warn!(error = ?e, merchant_id = %merchant_id, "Failed to invalidate routing algorithm cache");
    }
}
use serde::Serialize;
use serde_json::{json, Value};

#[allow(dead_code)]
const DEFAULT_FALLBACK_IDENTIFIER: &str = "default_fallback_enabled";

#[derive(Debug, Serialize)]
struct RoutingCreateAnalyticsDetails<'a> {
    request: &'a Value,
    response: &'a RoutingDictionaryRecord,
    algorithm_name: &'a str,
}

#[derive(Debug, Serialize)]
struct RoutingEvaluateAnalyticsDetails<'a> {
    request: &'a RoutingRequest,
    response: &'a RoutingEvaluateResponse,
    rule_name: Option<&'a str>,
    preview_kind: &'static str,
}

#[derive(Debug, Serialize)]
struct RoutingEvaluateErrorResponseDetails<'a> {
    status: &'a str,
    error_message: &'a str,
    api_error: &'a Option<Value>,
}

#[derive(Debug, Serialize)]
struct RoutingEvaluateErrorAnalyticsDetails<'a> {
    request: &'a RoutingRequest,
    response: RoutingEvaluateErrorResponseDetails<'a>,
    preview_kind: &'static str,
}

#[derive(Debug, Serialize)]
struct ValidationErrorsPayload<'a> {
    validation_errors: &'a [ValidationErrorDetails],
}

fn serialize_routing_create_analytics_details(
    request: &Value,
    response: &RoutingDictionaryRecord,
    algorithm_name: &str,
) -> Option<String> {
    crate::analytics::serialize_details(&RoutingCreateAnalyticsDetails {
        request,
        response,
        algorithm_name,
    })
}

fn serialize_routing_evaluate_analytics_details(
    request: &RoutingRequest,
    response: &RoutingEvaluateResponse,
    rule_name: Option<&str>,
) -> Option<String> {
    crate::analytics::serialize_details(&RoutingEvaluateAnalyticsDetails {
        request,
        response,
        rule_name,
        preview_kind: "routing_evaluate",
    })
}

fn serialize_routing_evaluate_error_analytics_details(
    request: &RoutingRequest,
    status: &str,
    error_message: &str,
    api_error: &Option<Value>,
) -> Option<String> {
    crate::analytics::serialize_details(&RoutingEvaluateErrorAnalyticsDetails {
        request,
        response: RoutingEvaluateErrorResponseDetails {
            status,
            error_message,
            api_error,
        },
        preview_kind: "routing_evaluate",
    })
}

fn validation_errors_payload(
    validation_errors: &[ValidationErrorDetails],
) -> Option<serde_json::Value> {
    serde_json::to_value(ValidationErrorsPayload { validation_errors }).ok()
}

pub async fn config_sr_dimensions(
    Json(payload): Json<SrDimensionConfig>,
) -> Result<Json<String>, ContainerError<EuclidErrors>> {
    let timer = metrics::API_LATENCY_HISTOGRAM
        .with_label_values(&["config_sr_dimensions"])
        .start_timer();
    metrics::API_REQUEST_TOTAL_COUNTER
        .with_label_values(&["config_sr_dimensions"])
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
            .with_label_values(&["config_sr_dimensions", "failure"])
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
            .with_label_values(&["config_sr_dimensions", "failure"])
            .inc();
        timer.observe_duration();
        logger::error!(
            "Failed to insert or update SR Dimension config for merchant: {}",
            mid
        );
        return Err(ContainerError::from(EuclidErrors::StorageError));
    }
    metrics::API_REQUEST_COUNTER
        .with_label_values(&["config_sr_dimensions", "success"])
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

/// Returns the merchant's SR dimension config (which dimensions clusters are split on). Empty
/// `fields` when nothing is configured yet — so the dashboard can render the selector.
pub async fn get_sr_dimensions(
    Path(merchant_id): Path<String>,
) -> Result<Json<SrDimensionConfig>, ContainerError<EuclidErrors>> {
    let name = format!("SR_DIMENSION_CONFIG_{}", merchant_id);
    let stored = find_config_by_name(name)
        .await
        .change_context(EuclidErrors::StorageError)?;
    let config = stored
        .and_then(|c| c.value)
        .and_then(|v| serde_json::from_str::<SrDimensionConfig>(&v).ok())
        .unwrap_or(SrDimensionConfig {
            merchant_id,
            ..Default::default()
        });
    Ok(Json(config))
}

pub async fn routing_create(
    headers: axum::http::HeaderMap,
    Json(payload): Json<Value>,
) -> Result<Json<RoutingDictionaryRecord>, ContainerError<EuclidErrors>> {
    let timer = metrics::API_LATENCY_HISTOGRAM
        .with_label_values(&["routing_create"])
        .start_timer();
    metrics::API_REQUEST_TOTAL_COUNTER
        .with_label_values(&["routing_create"])
        .inc();

    let state = get_tenant_app_state().await;
    let request_id = headers
        .get(crate::storage::consts::X_REQUEST_ID)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let global_request_id = crate::analytics::global_request_id_from_headers(&headers);
    let trace_id = crate::analytics::trace_id_from_headers(&headers);

    let config: RoutingRule = serde_json::from_value(payload.clone())
        .change_context(EuclidErrors::InvalidRuleConfiguration)?;
    let create_flow_type = crate::analytics::refine_routing_create_flow_type(&config.algorithm);
    let analytics_created_by = config.created_by.clone();
    let analytics_config_name = config.name.clone();

    logger::debug!("Received routing config: {:?}", config);

    match validate_routing_rule(&config, &state.config.routing_config) {
        Ok(validation_result) => {
            if !validation_result.is_valid {
                for error in &validation_result.errors {
                    logger::error!(
                        field = %error.field,
                        error_type = %error.error_type,
                        message = %error.message,
                        "Field validation error during routing rule creation"
                    );
                }

                let detailed_error = validation_result.to_error_message();

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
                        validation_errors_payload(&validation_result.errors),
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
            return Err(err);
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
    crate::analytics::DomainAnalyticsEvent::record_operation(
        crate::analytics::AnalyticsFlowContext::new(
            crate::analytics::ApiFlow::RuleBasedRouting,
            create_flow_type,
        ),
        crate::analytics::AnalyticsRoute::RoutingCreate,
        Some(analytics_created_by),
        None,
        request_id,
        global_request_id,
        trace_id,
        Some("success".to_string()),
        serialize_routing_create_analytics_details(&payload, &response, &analytics_config_name),
        Some("routing_created".to_string()),
    );

    metrics::API_REQUEST_COUNTER
        .with_label_values(&["routing_create", "success"])
        .inc();
    timer.observe_duration();
    Ok(Json(response))
}

// Fetches the active routing algorithm from DB (2 queries) and back-fills Redis cache.
// Extracted to keep `routing_evaluate` readable and to reuse on cache miss / parse error.
async fn fetch_algorithm_from_db_and_cache(
    state: &crate::app::TenantAppState,
    merchant_id: &str,
) -> Result<RoutingAlgorithm, ContainerError<EuclidErrors>> {
    #[cfg(feature = "mysql")]
    use crate::storage::schema::routing_algorithm_mapper::dsl as db_mapper_dsl;
    #[cfg(feature = "postgres")]
    use crate::storage::schema_pg::routing_algorithm_mapper::dsl as db_mapper_dsl;

    let active_routing_algorithm_id = crate::generics::generic_find_one::<
        <RoutingAlgorithmMapper as HasTable>::Table,
        _,
        RoutingAlgorithmMapper,
    >(
        &state.db,
        db_mapper_dsl::created_by.eq(merchant_id.to_string()),
    )
    .await
    .change_context(EuclidErrors::ActiveRoutingAlgorithmNotFound(
        merchant_id.to_string(),
    ))
    .map_err(ContainerError::from)?
    .routing_algorithm_id;

    let algorithm = crate::generics::generic_find_one::<
        <RoutingAlgorithm as HasTable>::Table,
        _,
        RoutingAlgorithm,
    >(&state.db, dsl::id.eq(active_routing_algorithm_id.clone()))
    .await
    .inspect_err(|&e| {
        logger::error!(
            ?e,
            "Failed to fetch RoutingAlgorithm for ID {:?}",
            active_routing_algorithm_id
        );
    })
    .change_context(EuclidErrors::StorageError)
    .map_err(ContainerError::from)?;

    // Back-fill cache so subsequent requests hit Redis
    cache_routing_algorithm(state, merchant_id, &algorithm).await;
    Ok(algorithm)
}

pub async fn routing_evaluate(
    headers: axum::http::HeaderMap,
    Json(payload): Json<RoutingRequest>,
) -> Result<Json<RoutingEvaluateResponse>, ContainerError<EuclidErrors>> {
    let mut timer = Some(
        metrics::API_LATENCY_HISTOGRAM
            .with_label_values(&["routing_evaluate"])
            .start_timer(),
    );

    API_REQUEST_TOTAL_COUNTER
        .with_label_values(&["routing_evaluate"])
        .inc();

    let state = get_tenant_app_state().await;
    let request_id = headers
        .get(crate::storage::consts::X_REQUEST_ID)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let global_request_id = crate::analytics::global_request_id_from_headers(&headers);
    let trace_id = crate::analytics::trace_id_from_headers(&headers);
    logger::debug!(
        payment_id = ?payload.payment_id,
        created_by = %payload.created_by,
        "Received routing evaluation request"
    );
    crate::analytics::DomainAnalyticsEvent::record_request_hit(
        crate::analytics::AnalyticsFlowContext::new(
            crate::analytics::ApiFlow::RuleBasedRouting,
            crate::analytics::FlowType::RoutingEvaluateRequestHit,
        ),
        crate::analytics::AnalyticsRoute::RoutingEvaluate,
        Some(payload.created_by.clone()),
        payload.payment_id.clone(),
        request_id.clone(),
        global_request_id.clone(),
        trace_id.clone(),
        None,
    );

    let update_failure_metrics = || {
        API_REQUEST_COUNTER
            .with_label_values(&["routing_evaluate", "failure"])
            .inc();
    };
    let mut fail_preview = |err: ContainerError<EuclidErrors>, stage: &'static str| {
        record_routing_evaluate_preview_error(
            &payload,
            &err,
            stage,
            request_id.clone(),
            global_request_id.clone(),
            trace_id.clone(),
        );
        update_failure_metrics();
        if let Some(timer) = timer.take() {
            timer.observe_duration();
        }
        Err(err)
    };

    // Check for the fallback_output in evaluate request:
    let default_output_present = payload
        .fallback_output
        .as_ref()
        .is_some_and(|output| !output.is_empty());

    // ── Parameter validation ────────────────────────────────────────────────
    let parameters = payload.parameters.clone();

    let routing_config = match state
        .config
        .routing_config
        .as_ref()
        .ok_or(EuclidErrors::GlobalRoutingConfigsUnavailable)
    {
        Ok(config) => config,
        Err(e) => return fail_preview(e.into(), "routing_config_unavailable"),
    };

    for (key, value) in &parameters {
        if !routing_config.keys.keys.contains_key(key)
            && value.as_ref().is_some_and(|val| !val.is_metadata())
        {
            return fail_preview(
                EuclidErrors::InvalidRequestParameter(key.clone()).into(),
                "parameter_validation_failed",
            );
        }

        if let Some(key_config) = routing_config.keys.keys.get(key) {
            if key_config.data_type == KeyDataType::Enum {
                if let Some(Some(ValueType::EnumVariant(value))) = parameters.get(key) {
                    if !is_valid_enum_value(routing_config, key, value) {
                        return fail_preview(
                            EuclidErrors::InvalidRequest(format!(
                                "Invalid enum value '{}' for key '{}'",
                                value, key
                            ))
                            .into(),
                            "parameter_validation_failed",
                        );
                    }
                } else {
                    return fail_preview(
                        EuclidErrors::InvalidRequest(format!(
                            "Expected enum value for key '{}'",
                            key
                        ))
                        .into(),
                        "parameter_validation_failed",
                    );
                }
            }
        }
    }

    // ── Fetch active routing algorithm (Redis cache → DB fallback) ──────────
    //
    // Cache hit  : skip both DB queries entirely.
    // Cache miss : run the original 2-query DB path and back-fill the cache.
    let cache_key = routing_algo_cache_key(&payload.created_by);

    let algorithm: RoutingAlgorithm = match state
        .redis_conn
        .get_key::<CachedRoutingAlgorithm>(&cache_key, "CachedRoutingAlgorithm")
        .await
    {
        Ok(cached) => {
            logger::debug!(
                merchant_id = %payload.created_by,
                algorithm_id = %cached.id,
                "routing_evaluate: cache hit"
            );
            RoutingAlgorithm {
                id: cached.id,
                created_by: payload.created_by.clone(),
                name: String::new(),
                description: String::new(),
                metadata: None,
                algorithm_data: cached.algorithm_data,
                algorithm_for: String::new(),
                created_at: time::PrimitiveDateTime::MIN,
                modified_at: time::PrimitiveDateTime::MIN,
            }
        }
        Err(_) => {
            // Cache miss or stale entry — fetch from DB and back-fill
            match fetch_algorithm_from_db_and_cache(&state, &payload.created_by).await {
                Ok(algo) => algo,
                Err(e) => return fail_preview(e, "active_routing_lookup_failed"),
            }
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
            Err(e) => return fail_preview(e.into(), "routing_algorithm_parse_failed"),
        };
    let mut preview_flow_type =
        crate::analytics::refine_routing_evaluate_flow_type(&algorithm_data);

    // Populated by the AbTest arm to tag analytics events with experiment context.
    let mut ab_experiment_id: Option<String> = None;
    let mut ab_variant_arm: Option<String> = None;

    let (output, evaluated_output, rule_name): (Output, Vec<ConnectorInfo>, Option<String>) =
        match algorithm_data {
            StaticRoutingAlgorithm::Single(conn) => {
                let out_enum = Output::Single(*conn.clone());
                match evaluate_output(&out_enum).map_err(|_| {
                    EuclidErrors::FailedToEvaluateOutput(format!(
                        "{}",
                        StaticRoutingAlgorithm::Single(conn.clone())
                    ))
                }) {
                    Ok((_, eval)) => (out_enum, eval, Some("straight_through_rule".into())),
                    Err(e) => return fail_preview(e.into(), "preview_output_evaluation_failed"),
                }
            }

            StaticRoutingAlgorithm::Priority(connectors) => {
                let out_enum = Output::Priority(connectors.clone());
                match evaluate_output(&out_enum).map_err(|_| {
                    EuclidErrors::FailedToEvaluateOutput(format!(
                        "{}",
                        StaticRoutingAlgorithm::Priority(connectors.clone())
                    ))
                }) {
                    Ok((_, eval)) => (out_enum, eval, Some("priority_rule".into())),
                    Err(e) => return fail_preview(e.into(), "preview_output_evaluation_failed"),
                }
            }

            StaticRoutingAlgorithm::VolumeSplit(splits) => {
                let out_enum = Output::VolumeSplit(splits.clone());
                match evaluate_output(&out_enum).map_err(|_| {
                    EuclidErrors::FailedToEvaluateOutput(format!(
                        "{}",
                        StaticRoutingAlgorithm::VolumeSplit(splits.clone())
                    ))
                }) {
                    Ok((_, eval)) => (out_enum, eval, Some("volume_split_rule".into())),
                    Err(e) => return fail_preview(e.into(), "preview_output_evaluation_failed"),
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
                    Err(e) => return fail_preview(e.into(), "preview_interpreter_failed"),
                }
            }

            StaticRoutingAlgorithm::AbTest(ab_data) => {
                let payment_id = payload.payment_id.as_deref().unwrap_or("");
                let arm = crate::decider::gatewaydecider::ab_test::assign_arm(
                    payment_id,
                    ab_data.variant_split_pct,
                );
                let arm_algorithm_id = if arm == "variant" {
                    ab_data.variant_algorithm_id.as_str()
                } else {
                    ab_data.control_algorithm_id.as_str()
                };
                logger::debug!(
                    "A/B test routing evaluate: payment_id={:?} arm={} algorithm={}",
                    payload.payment_id,
                    arm,
                    arm_algorithm_id
                );
                ab_experiment_id = Some(algorithm.id.clone());
                ab_variant_arm = Some(arm.to_string());

                let result = crate::decider::gatewaydecider::ab_test::preview::evaluate_arm(
                    arm,
                    arm_algorithm_id,
                    &payload,
                    default_output_present,
                    &state.db,
                )
                .await;
                match result {
                    Ok(r) => {
                        preview_flow_type = r.flow_type;
                        (r.output, r.evaluated_output, r.rule_name)
                    }
                    Err(e) => return fail_preview(e, "ab_test_evaluation_failed"),
                }
            }
        };

    let pm_filter_bundle = if pm_filter_graph::has_payment_method_type(&parameters) {
        state.get_pm_filter_graph_bundle().await
    } else {
        None
    };

    let connectors_for_eligibility = extract_connectors_for_eligibility(&output);
    let eligible_connectors = eligibility_for_output(
        pm_filter_bundle.as_deref(),
        &parameters,
        &connectors_for_eligibility,
    );

    let response = RoutingEvaluateResponse {
        payment_id: payload.payment_id.clone(),
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
    let analytics_details = match (&ab_experiment_id, &ab_variant_arm) {
        (Some(exp_id), Some(arm)) => {
            crate::decider::gatewaydecider::ab_test::preview::serialize_analytics_details(
                &payload,
                &response,
                rule_name.as_deref(),
                exp_id,
                arm,
            )
        }
        _ => {
            serialize_routing_evaluate_analytics_details(&payload, &response, rule_name.as_deref())
        }
    };
    crate::analytics::DomainAnalyticsEvent::record_rule_evaluation_preview(
        crate::analytics::AnalyticsFlowContext::new(
            crate::analytics::ApiFlow::RuleBasedRouting,
            preview_flow_type,
        ),
        Some(payload.created_by.clone()),
        payload.payment_id.clone(),
        preview_gateway(&response),
        rule_name.clone(),
        Some(response.status.clone()),
        analytics_details,
        request_id,
        global_request_id,
        trace_id,
    );

    API_REQUEST_COUNTER
        .with_label_values(&["routing_evaluate", "success"])
        .inc();
    if let Some(timer) = timer.take() {
        timer.observe_duration();
    }
    Ok(Json(response))
}

fn record_routing_evaluate_preview_error(
    payload: &RoutingRequest,
    error: &ContainerError<EuclidErrors>,
    event_stage: &str,
    request_id: Option<String>,
    global_request_id: Option<String>,
    trace_id: Option<String>,
) {
    let response_payload = error
        .downcast_ref::<ApiErrorResponse>()
        .and_then(|payload| serde_json::to_value(payload).ok());
    let status = error
        .get_inner()
        .clone()
        .into_response()
        .status()
        .as_u16()
        .to_string();
    let error_code = response_payload
        .as_ref()
        .and_then(|value| value.get("code"))
        .and_then(|value| value.as_str())
        .unwrap_or("ROUTING_EVALUATE_FAILED")
        .to_string();
    let error_message = response_payload
        .as_ref()
        .and_then(|value| value.get("message"))
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| error.get_inner().to_string());

    crate::analytics::DomainAnalyticsEvent::record_error(
        crate::analytics::AnalyticsFlowContext::new(
            crate::analytics::ApiFlow::RuleBasedRouting,
            crate::analytics::FlowType::RoutingEvaluateError,
        ),
        crate::analytics::AnalyticsRoute::RoutingEvaluate,
        Some(payload.created_by.clone()),
        payload.payment_id.clone(),
        request_id,
        global_request_id,
        trace_id,
        None,
        Some("RULE_EVALUATE_PREVIEW".to_string()),
        error_code,
        error_message.clone(),
        serialize_routing_evaluate_error_analytics_details(
            payload,
            &status,
            &error_message,
            &response_payload,
        ),
        Some(event_stage.to_string()),
        None,
    );
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
    // Keep the full struct so we can populate the Redis cache after a successful activate.
    let algorithm = match crate::generics::generic_find_one::<
        <RoutingAlgorithm as HasTable>::Table,
        _,
        RoutingAlgorithm,
    >(&state.db, dsl::id.eq(payload.routing_algorithm_id.clone()))
    .await
    .change_context(EuclidErrors::RoutingAlgorithmNotFound(
        payload.routing_algorithm_id.clone(),
    )) {
        Ok(algo) => algo,
        Err(e) => {
            update_failure_metrics();
            timer.observe_duration();
            return Err(e.into());
        }
    };
    let algorithm_for = algorithm.algorithm_for.clone();

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
                    cache_routing_algorithm(&state, &payload.created_by, &algorithm).await;
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
        // Already active with the same algorithm — refresh the cache TTL
        cache_routing_algorithm(&state, &payload.created_by, &algorithm).await;
        API_REQUEST_COUNTER
            .with_label_values(&["activate_routing_rule", "success"])
            .inc();
        timer.observe_duration();
        return Ok(());
    }

    // === Step 3b: Insert new if not present ===
    let merchant_id_for_cache = payload.created_by.clone();
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
            cache_routing_algorithm(&state, &merchant_id_for_cache, &algorithm).await;
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

pub async fn deactivate_routing_rule(
    Json(payload): Json<DeactivateRoutingConfigRequest>,
) -> Result<(), ContainerError<EuclidErrors>> {
    let timer = API_LATENCY_HISTOGRAM
        .with_label_values(&["deactivate_routing_rule"])
        .start_timer();
    API_REQUEST_TOTAL_COUNTER
        .with_label_values(&["deactivate_routing_rule"])
        .inc();

    let update_failure_metrics = || {
        API_REQUEST_COUNTER
            .with_label_values(&["deactivate_routing_rule", "failure"])
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

    // === Step 2: Find the active mapping for (created_by, routing_algorithm_id, algorithm_for) ===
    let existing_mapping = crate::generics::generic_find_one::<
        <RoutingAlgorithmMapper as HasTable>::Table,
        _,
        RoutingAlgorithmMapper,
    >(
        &state.db,
        mapper_dsl::created_by
            .eq(payload.created_by.clone())
            .and(mapper_dsl::routing_algorithm_id.eq(payload.routing_algorithm_id.clone()))
            .and(mapper_dsl::algorithm_for.eq(algorithm_for.clone())),
    )
    .await
    .ok();

    // === Step 3: Delete the mapping if found (idempotent - return success if not found) ===
    if let Some(mapping) = existing_mapping {
        let predicate = mapper_dsl::id.eq(mapping.id);

        match crate::generics::generic_delete::<<RoutingAlgorithmMapper as HasTable>::Table, _>(
            &conn, predicate,
        )
        .await
        .change_context(EuclidErrors::StorageError)
        {
            Ok(_) => {
                logger::debug!(
                    "Deactivated routing algorithm {} for merchant {}",
                    payload.routing_algorithm_id,
                    payload.created_by
                );
                invalidate_routing_algorithm_cache(&state, &payload.created_by).await;
                API_REQUEST_COUNTER
                    .with_label_values(&["deactivate_routing_rule", "success"])
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
    } else {
        // Idempotent: if the mapping doesn't exist, return success
        logger::debug!(
            "No active mapping found for routing algorithm {} and merchant {} - already deactivated",
            payload.routing_algorithm_id,
            payload.created_by
        );
        API_REQUEST_COUNTER
            .with_label_values(&["deactivate_routing_rule", "success"])
            .inc();
        timer.observe_duration();
        Ok(())
    }
}

/// Guard for the edit/delete flows: reject the operation if this algorithm is the merchant's
/// currently active routing algorithm. Editing/deleting a running experiment would corrupt its
/// collected results, so the caller must Stop (deactivate) it first.
async fn ensure_routing_algorithm_inactive(
    state: &crate::app::TenantAppState,
    created_by: &str,
    routing_algorithm_id: &str,
) -> Result<(), ContainerError<EuclidErrors>> {
    let active = crate::generics::generic_find_one_optional::<
        <RoutingAlgorithmMapper as HasTable>::Table,
        _,
        RoutingAlgorithmMapper,
    >(
        &state.db,
        mapper_dsl::created_by
            .eq(created_by.to_string())
            .and(mapper_dsl::routing_algorithm_id.eq(routing_algorithm_id.to_string())),
    )
    .await
    .change_context(EuclidErrors::StorageError)?;

    if active.is_some() {
        return Err(EuclidErrors::InvalidRequest(
            "This experiment is active. Stop it before editing or deleting.".to_string(),
        )
        .into());
    }
    Ok(())
}

/// Edit an existing inactive routing algorithm in place (name/description/definition). Used by the
/// A/B Testing dashboard's Edit action. Keeps the same id so history/links remain valid.
pub async fn update_routing_rule(
    Json(payload): Json<crate::euclid::types::UpdateRoutingConfigRequest>,
) -> Result<Json<RoutingDictionaryRecord>, ContainerError<EuclidErrors>> {
    let timer = API_LATENCY_HISTOGRAM
        .with_label_values(&["update_routing_rule"])
        .start_timer();
    API_REQUEST_TOTAL_COUNTER
        .with_label_values(&["update_routing_rule"])
        .inc();
    let fail = || {
        API_REQUEST_COUNTER
            .with_label_values(&["update_routing_rule", "failure"])
            .inc();
    };

    let run = async {
        let state = get_tenant_app_state().await;
        let conn = state
            .db
            .get_conn()
            .await
            .map_err(|_| EuclidErrors::StorageError)?;

        // Only inactive experiments can be edited.
        ensure_routing_algorithm_inactive(
            &state,
            &payload.created_by,
            &payload.routing_algorithm_id,
        )
        .await?;

        // Load the existing row (also verifies ownership) so we can preserve created_at/algorithm_for.
        let existing = crate::generics::generic_find_one::<
            <RoutingAlgorithm as HasTable>::Table,
            _,
            RoutingAlgorithm,
        >(&state.db, dsl::id.eq(payload.routing_algorithm_id.clone()))
        .await
        .change_context(EuclidErrors::RoutingAlgorithmNotFound(
            payload.routing_algorithm_id.clone(),
        ))?;

        if existing.created_by != payload.created_by {
            return Err(ContainerError::from(EuclidErrors::InvalidRequest(
                "Routing algorithm does not belong to this merchant".to_string(),
            )));
        }

        let utc_date_time = time::OffsetDateTime::now_utc();
        let timestamp = time::PrimitiveDateTime::new(utc_date_time.date(), utc_date_time.time());
        let algorithm_data = serde_json::to_string(&payload.algorithm)
            .change_context(EuclidErrors::FailedToSerializeJsonToString)?;

        crate::generics::generic_update::<<RoutingAlgorithm as HasTable>::Table, _, _>(
            &conn,
            dsl::id.eq(payload.routing_algorithm_id.clone()),
            (
                dsl::name.eq(payload.name.clone()),
                dsl::description.eq(payload.description.clone()),
                dsl::algorithm_data.eq(algorithm_data),
                dsl::modified_at.eq(timestamp),
            ),
        )
        .await
        .change_context(EuclidErrors::StorageError)?;

        invalidate_routing_algorithm_cache(&state, &payload.created_by).await;

        Ok(RoutingDictionaryRecord::new(
            payload.routing_algorithm_id.clone(),
            payload.name.clone(),
            existing.algorithm_for,
            existing.created_at,
            timestamp,
        ))
    };

    match run.await {
        Ok(record) => {
            API_REQUEST_COUNTER
                .with_label_values(&["update_routing_rule", "success"])
                .inc();
            timer.observe_duration();
            Ok(Json(record))
        }
        Err(e) => {
            fail();
            timer.observe_duration();
            Err(e)
        }
    }
}

/// Delete an existing inactive routing algorithm. Used by the A/B Testing dashboard's Delete
/// action. Removes the `routing_algorithm` row (results already collected remain in analytics).
pub async fn delete_routing_rule(
    Json(payload): Json<crate::euclid::types::DeleteRoutingConfigRequest>,
) -> Result<Json<serde_json::Value>, ContainerError<EuclidErrors>> {
    let timer = API_LATENCY_HISTOGRAM
        .with_label_values(&["delete_routing_rule"])
        .start_timer();
    API_REQUEST_TOTAL_COUNTER
        .with_label_values(&["delete_routing_rule"])
        .inc();
    let fail = || {
        API_REQUEST_COUNTER
            .with_label_values(&["delete_routing_rule", "failure"])
            .inc();
    };

    let run = async {
        let state = get_tenant_app_state().await;
        let conn = state
            .db
            .get_conn()
            .await
            .map_err(|_| EuclidErrors::StorageError)?;

        // Only inactive experiments can be deleted.
        ensure_routing_algorithm_inactive(
            &state,
            &payload.created_by,
            &payload.routing_algorithm_id,
        )
        .await?;

        crate::generics::generic_delete::<<RoutingAlgorithm as HasTable>::Table, _>(
            &conn,
            dsl::id.eq(payload.routing_algorithm_id.clone()),
        )
        .await
        .change_context(EuclidErrors::StorageError)?;

        invalidate_routing_algorithm_cache(&state, &payload.created_by).await;
        Ok(())
    };

    match run.await {
        Ok(()) => {
            API_REQUEST_COUNTER
                .with_label_values(&["delete_routing_rule", "success"])
                .inc();
            timer.observe_duration();
            Ok(Json(serde_json::json!({
                "status": "deleted",
                "routing_algorithm_id": payload.routing_algorithm_id,
            })))
        }
        Err(e) => {
            fail();
            timer.observe_duration();
            Err(e)
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

fn preview_gateway(response: &RoutingEvaluateResponse) -> Option<String> {
    response
        .evaluated_output
        .first()
        .map(|connector| connector.gateway_name.clone())
        .or_else(|| {
            response
                .eligible_connectors
                .first()
                .map(|connector| connector.gateway_name.clone())
        })
}

pub(crate) fn eligibility_for_output(
    pm_filter_bundle: Option<&pm_filter_graph::PmFilterGraphBundle>,
    parameters: &std::collections::HashMap<String, Option<ValueType>>,
    connectors: &[ConnectorInfo],
) -> Vec<ConnectorInfo> {
    if !pm_filter_graph::has_payment_method_type(parameters) {
        logger::debug!("Skipping pm_filters eligibility; payment_method_type missing");
        return connectors.to_vec();
    }

    apply_pm_filter_eligibility(pm_filter_bundle, parameters, connectors)
}

pub fn compute_routing_evaluate_eligibility(
    pm_filter_bundle: Option<&pm_filter_graph::PmFilterGraphBundle>,
    parameters: &std::collections::HashMap<String, Option<ValueType>>,
    connectors: &[ConnectorInfo],
) -> Vec<ConnectorInfo> {
    eligibility_for_output(pm_filter_bundle, parameters, connectors)
}

pub(crate) fn apply_pm_filter_eligibility(
    bundle: Option<&pm_filter_graph::PmFilterGraphBundle>,
    parameters: &std::collections::HashMap<String, Option<ValueType>>,
    eligible_connectors: &[ConnectorInfo],
) -> Vec<ConnectorInfo> {
    let Some(bundle) = bundle else {
        logger::debug!("Skipping pm_filters eligibility; graph unavailable");
        return eligible_connectors.to_vec();
    };

    pm_filter_graph::filter_eligible_connectors(bundle, parameters, eligible_connectors)
}

pub(crate) fn extract_connectors_for_eligibility(output: &Output) -> Vec<ConnectorInfo> {
    let mut connectors = Vec::<ConnectorInfo>::new();
    let mut push_unique = |connector: &ConnectorInfo| {
        if !connectors.iter().any(|existing| existing == connector) {
            connectors.push(connector.clone());
        }
    };

    match output {
        Output::Single(connector) => push_unique(connector),
        Output::Priority(priority_connectors) => {
            for connector in priority_connectors {
                push_unique(connector);
            }
        }
        Output::VolumeSplit(splits) => {
            for split in splits {
                push_unique(&split.output);
            }
        }
        Output::VolumeSplitPriority(splits) => {
            for split in splits {
                for connector in &split.output {
                    push_unique(connector);
                }
            }
        }
    }

    connectors
}

/// GET endpoint to serve routing keys configuration
/// Returns the routing config with all available keys and their enum values
/// This allows the dashboard to dynamically fetch valid routing keys
pub async fn get_routing_config(
) -> Result<Json<crate::euclid::types::TomlConfig>, ContainerError<EuclidErrors>> {
    let timer = metrics::API_LATENCY_HISTOGRAM
        .with_label_values(&["get_routing_config"])
        .start_timer();
    metrics::API_REQUEST_TOTAL_COUNTER
        .with_label_values(&["get_routing_config"])
        .inc();

    let tenant_state = get_tenant_app_state().await;

    // Clone the routing config to return it
    let config = tenant_state
        .config
        .routing_config
        .clone()
        .ok_or(EuclidErrors::GlobalRoutingConfigsUnavailable)?;

    metrics::API_REQUEST_COUNTER
        .with_label_values(&["get_routing_config", "success"])
        .inc();
    timer.observe_duration();

    logger::info!("Successfully served routing config");

    Ok(Json(config))
}
