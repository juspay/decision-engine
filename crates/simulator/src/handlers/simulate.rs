pub mod baseline;
pub mod file_uploader;
pub mod model;
pub mod multipart_extractor;
pub mod types;

use std::{collections::HashMap, sync::Arc};

use axum::{
    extract::{Path, Query, State},
    Json,
};

use dynamo::{helpers, logger, success_rate::types::SUCCESS_RATE_PREFIX_IN_REDIS};
use error_stack::ResultExt;
use futures::future;

use crate::{
    app::AppState,
    errors::{ApiError, ContainerError},
    handlers::simulate::{
        multipart_extractor::UploadDataResolver,
        types::{
            AlgorithmType, BaselineData, CsvRecord, Faar, FailedPayments, Payment, Revenue,
            SimulateDataResponse, SimulationOutcomeOfEachTxn, SimulationReportResponse,
            SimulationSummary, SuccessBasedRoutingResponse, TimeSeriesData,
            VolumeDistributionAsPerSr,
        },
    },
    headers_extractors::{AuthResolver, Headers},
    utils,
};

pub async fn simulate(
    State(state): State<Arc<AppState>>,
    Path(merchant_id): Path<String>,
    AuthResolver(headers): AuthResolver,
    request: UploadDataResolver,
) -> Result<Json<SimulateDataResponse>, ContainerError<ApiError>> {
    let algo_type = request.json_data.algo_type;
    if request.upload_data {
        if let Some(csv_data) = request.csv_data {
            records_handler(state, merchant_id, headers, csv_data.as_ref(), algo_type).await
        } else {
            Err(ApiError::MultipartError("Missing multipart data").into())
        }
    } else {
        let baseline_data = state
            .file_storage_client
            .retrieve_file(&state.config.baseline_static_data.file_name)
            .await
            .change_context(ApiError::FailedToRetrieveBaselineData)?;

        records_handler(
            state,
            merchant_id,
            headers,
            baseline_data.as_slice(),
            algo_type,
        )
        .await
    }
}

async fn records_handler<T: std::io::Read>(
    state: Arc<AppState>,
    merchant_id: String,
    headers: Headers,
    records: T,
    algo_type: AlgorithmType,
) -> Result<Json<SimulateDataResponse>, ContainerError<ApiError>> {
    let records = csv::Reader::from_reader(records)
        .deserialize()
        .collect::<Result<Vec<CsvRecord>, _>>()
        .change_context(ApiError::FailedToDeserializeCsv)?;

    let records = utils::reorder_records(records.clone());

    process_records(merchant_id, records, state, headers, algo_type).await?;

    Ok(Json(SimulateDataResponse {
        message: "Simulation successful".to_string(),
    }))
}

async fn process_records(
    merchant_id: String,
    payments: Vec<Payment>,
    state: Arc<AppState>,
    headers: Headers,
    algo_type: AlgorithmType,
) -> Result<(), ContainerError<ApiError>> {
    let model_configs = state.config.model_configs.clone();
    let total_chunks = state.config.parameters.total_chunks;

    let baseline_data_result = baseline::analyze_baseline_data(payments, total_chunks)?;

    let connectors = baseline_data_result.connectors.clone();

    let mut simulation_results_for_each_config = Vec::with_capacity(model_configs.len());

    for model_config in model_configs {
        let connectors = connectors.clone();
        let state = state.clone();
        let unique_param_suffix = utils::generate_random_id(); // So that keys are unique across multiple configs simulation

        let each_config_simulation_future = model::perform_success_based_routing(
            merchant_id.clone(),
            headers.clone(),
            model_config,
            connectors,
            state,
            &baseline_data_result,
            unique_param_suffix,
            &algo_type,
        );
        simulation_results_for_each_config.push(each_config_simulation_future);
    }

    let simulation_results_for_each_config =
        future::try_join_all(simulation_results_for_each_config).await?;

    let (simulation_outcome_of_each_txn, simulation_summary) =
        get_report_of_best_performing_config(
            simulation_results_for_each_config,
            baseline_data_result,
            connectors.clone(),
            state.config.parameters.total_chunks,
        )?;

    logger::debug!("simulation summary: {:?}", simulation_summary);

    file_uploader::upload_reports(
        &state,
        &merchant_id,
        simulation_summary,
        simulation_outcome_of_each_txn,
    )
    .await?;

    invalidate_metrics(state, merchant_id, Some(headers.tenant_id));

    Ok(())
}

// generate report based on the simulation result of best performing config
fn get_report_of_best_performing_config(
    simulation_results_for_each_config: Vec<SuccessBasedRoutingResponse>,
    baseline_data_result: BaselineData,
    connectors: Vec<String>,
    total_time_series_chunks: usize,
) -> Result<(Vec<SimulationOutcomeOfEachTxn>, SimulationSummary), ContainerError<ApiError>> {
    // Get the model response with highest success rate
    let optimal_response = simulation_results_for_each_config
        .into_iter()
        .max_by(|resp1, resp2| {
            resp1
                .model_success_rate
                .partial_cmp(&resp2.model_success_rate)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .ok_or(ApiError::Unexpected("Failed to find best performing model"))?;

    let simulation_summary = generate_summary(
        &optimal_response,
        baseline_data_result,
        connectors,
        total_time_series_chunks,
    );

    Ok((
        optimal_response.simulation_outcome_of_each_txn,
        simulation_summary,
    ))
}

fn generate_summary(
    optimal_response: &SuccessBasedRoutingResponse,
    baseline_data_result: BaselineData,
    connectors: Vec<String>,
    total_time_series_chunks: usize,
) -> SimulationSummary {
    let simulation_outcome_of_each_txn = &optimal_response.simulation_outcome_of_each_txn;

    let overall_success_rate = types::SuccessRate {
        baseline: utils::to_percentage(baseline_data_result.success_rate),
        model: utils::to_percentage(optimal_response.model_success_rate),
    };

    let total_failed_payments = FailedPayments {
        baseline: baseline_data_result.total_failed_payments,
        model: optimal_response.total_failed_payments,
    };

    let total_revenue = Revenue {
        baseline: baseline_data_result.total_revenue,
        model: optimal_response.total_revenue,
    };

    let faar = Faar {
        baseline: utils::to_percentage(baseline_data_result.faar),
        model: utils::to_percentage(optimal_response.faar),
    };

    let mut index = 0;

    let mut overall_baseline_success_count = 0;
    let mut overall_model_success_count = 0;

    let mut overall_baseline_revenue;
    let mut overall_model_revenue;

    let mut time_stamps = Vec::with_capacity(total_time_series_chunks);
    let mut time_series_success_rates = Vec::with_capacity(total_time_series_chunks);
    let mut time_series_revenue = Vec::with_capacity(total_time_series_chunks);
    let mut volume_distribution = Vec::with_capacity(total_time_series_chunks);

    let total_attempts = baseline_data_result.total_attempts;
    let each_chunk_size = (total_attempts as f64 / total_time_series_chunks as f64).ceil() as usize;

    for current_chunk_count in 0..total_time_series_chunks {
        let mut connectors_stats_of_current_chunk = HashMap::new();

        connectors.iter().for_each(|connector| {
            connectors_stats_of_current_chunk.insert(
                connector.clone(),
                types::VolumeDistributionWithStats::default(),
            );
        });

        let current_chunk_end_index = if current_chunk_count == total_time_series_chunks - 1 {
            total_attempts
        } else {
            std::cmp::min(index + each_chunk_size, total_attempts)
        };
        time_stamps.push(
            simulation_outcome_of_each_txn[current_chunk_end_index - 1]
                .baseline_record
                .created_at,
        );

        overall_baseline_revenue = 0.0;
        overall_model_revenue = 0.0;

        while index < current_chunk_end_index {
            let record = &simulation_outcome_of_each_txn[index];

            if let Some(baseline_connector_stats) =
                connectors_stats_of_current_chunk.get_mut(&record.baseline_record.payment_gateway)
            {
                baseline_connector_stats.baseline_volume += 1;

                if record.baseline_record.payment_status {
                    baseline_connector_stats.connector_success_count += 1;
                    overall_baseline_revenue += record.baseline_record.amount;
                    overall_baseline_success_count += 1;
                }
            }

            if let Some(model_connector_stats) =
                connectors_stats_of_current_chunk.get_mut(&record.model_connector)
            {
                model_connector_stats.model_volume += 1;

                if record.model_status {
                    overall_model_revenue += record.baseline_record.amount;
                    overall_model_success_count += 1;
                }
            }

            index += 1;
        }

        let mut connectors_sr_of_current_chunk = HashMap::new();
        connectors_stats_of_current_chunk
            .into_iter()
            .for_each(|(connector, stats)| {
                connectors_sr_of_current_chunk.insert(
                    connector,
                    VolumeDistributionAsPerSr {
                        success_rate: utils::to_percentage(
                            stats.connector_success_count as f64 / stats.baseline_volume as f64,
                        ),
                        baseline_volume: stats.baseline_volume,
                        model_volume: stats.model_volume,
                    },
                );
            });

        volume_distribution.push(connectors_sr_of_current_chunk);
        time_series_success_rates.push(types::SuccessRate {
            baseline: utils::to_percentage(
                overall_baseline_success_count as f64 / current_chunk_end_index as f64,
            ),
            model: utils::to_percentage(
                overall_model_success_count as f64 / current_chunk_end_index as f64,
            ),
        });
        time_series_revenue.push(Revenue {
            baseline: overall_baseline_revenue,
            model: overall_model_revenue,
        });
    }

    let mut time_series_data = vec![];
    for i in 0..total_time_series_chunks {
        time_series_data.push(TimeSeriesData {
            time_stamp: time_stamps[i].clone().to_string(),
            volume_distribution_as_per_sr: volume_distribution[i].clone(),
            success_rate: time_series_success_rates[i].clone(),
            revenue: time_series_revenue[i].clone(),
        });
    }

    SimulationSummary {
        overall_success_rate,
        total_failed_payments,
        total_revenue,
        faar,
        time_series_data,
        overall_success_rate_improvement: utils::to_percentage(optimal_response.improvement),
        total_payment_count: optimal_response.total_txn_count,
    }
}

fn invalidate_metrics(state: Arc<AppState>, merchant_id: String, tenant_id: Option<String>) {
    let ephemeral_store = state.sr_algorithm.window_based.ephemeral_store.clone();

    tokio::spawn(async move {
        let key_prefix = helpers::redis_key_create_for_metrics_invalidation(
            SUCCESS_RATE_PREFIX_IN_REDIS,
            &tenant_id,
            &merchant_id,
        );

        ephemeral_store
            .delete_keys_matching_prefix(&key_prefix)
            .await
            .ok();
    });
}

pub async fn fetch_simulated_summary(
    State(state): State<Arc<AppState>>,
    Path(merchant_id): Path<String>,
    AuthResolver(_headers): AuthResolver,
) -> Result<Json<SimulationSummary>, ContainerError<ApiError>> {
    let summary_data = file_uploader::fetch_simulation_summary_data(&state, &merchant_id).await?;

    Ok(Json(summary_data))
}

pub async fn fetch_simulated_report(
    State(state): State<Arc<AppState>>,
    Path(merchant_id): Path<String>,
    Query(params): Query<types::SearchParams>,
    AuthResolver(_headers): AuthResolver,
) -> Result<Json<SimulationReportResponse>, ContainerError<ApiError>> {
    let records = file_uploader::fetch_transaction_outcomes(
        &state,
        &merchant_id,
        params.limit,
        params.offset,
    )
    .await?;

    let summary_data = file_uploader::fetch_simulation_summary_data(&state, &merchant_id).await?;

    Ok(Json(SimulationReportResponse::new(
        summary_data.total_payment_count,
        records,
    )))
}
