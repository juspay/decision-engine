use std::sync::Arc;

use error_stack::{Report, ResultExt};
use futures::future;
use redis_interface::errors::RedisError;

use crate::{
    app::AppState,
    errors::{ApiError, ContainerError},
    handlers::simulate::types::{SimulationOutcomeOfEachTxn, SimulationSummary},
};

const SUMMARY_JSON_FILE_NAME: &str = "summary";
const DETAILED_REPORT_DIRECTORY_NAME: &str = "detailed_transaction_report";
const DETAILED_REPORT_FILE_PREFIX: &str = "report_chunk";

async fn upload_simulation_summary(
    state: &Arc<AppState>,
    key_prefix: &str,
    simulation_summary: SimulationSummary,
) -> Result<String, ContainerError<ApiError>> {
    let json_str = serde_json::to_string_pretty(&simulation_summary)
        .change_context(ApiError::FailedToSerializeToJson)?;

    let key = format!(
        "{}/{}/{SUMMARY_JSON_FILE_NAME}.json",
        state.config.environment, key_prefix
    );

    state
        .file_storage_client
        .upload_file(&key, json_str.into_bytes())
        .await
        .change_context(ApiError::FailedToUploadSimulationSummary)?;

    Ok(key)
}

async fn upload_transaction_outcomes(
    state: &Arc<AppState>,
    key_prefix: &str,
    simulation_outcome_of_each_txn: Vec<SimulationOutcomeOfEachTxn>,
) -> Result<Vec<String>, ContainerError<ApiError>> {
    let total_records_per_json = state.config.parameters.total_records_per_json;

    let total_files = simulation_outcome_of_each_txn
        .len()
        .div_ceil(total_records_per_json);

    let mut upload_futures = Vec::with_capacity(total_files);

    invalidate_old_report_chunks(state, key_prefix).await?;

    for (chunk_idx, chunk) in simulation_outcome_of_each_txn
        .chunks(total_records_per_json)
        .enumerate()
    {
        let filename = format!("{}_{}.json", DETAILED_REPORT_FILE_PREFIX, chunk_idx);
        let key = format!(
            "{}/{}/{}/{}",
            state.config.environment, key_prefix, DETAILED_REPORT_DIRECTORY_NAME, filename
        );

        let json_str = serde_json::to_string_pretty(chunk)
            .change_context(ApiError::FailedToSerializeToJson)?;

        let state_clone = Arc::clone(state);
        let key_clone = key.clone();
        let json_bytes = json_str.into_bytes();

        let upload_future = async move {
            state_clone
                .file_storage_client
                .upload_file(&key_clone, json_bytes)
                .await
                .change_context(ApiError::FailedToUploadSimulationReport)?;

            Ok::<String, Report<ApiError>>(key_clone)
        };

        upload_futures.push(upload_future);
    }

    let uploaded_keys = future::try_join_all(upload_futures).await?;

    Ok(uploaded_keys)
}

pub async fn upload_reports(
    state: &Arc<AppState>,
    merchant_id: &str,
    simulation_summary: SimulationSummary,
    simulation_outcome_of_each_txn: Vec<SimulationOutcomeOfEachTxn>,
) -> Result<(), ContainerError<ApiError>> {
    let summary_future = upload_simulation_summary(state, merchant_id, simulation_summary);

    let transactions_future =
        upload_transaction_outcomes(state, merchant_id, simulation_outcome_of_each_txn);

    let (_summary_key, _transaction_keys) =
        future::try_join(summary_future, transactions_future).await?;

    Ok(())
}

pub async fn fetch_simulation_summary_data(
    state: &Arc<AppState>,
    key_prefix: &str,
) -> Result<SimulationSummary, Report<ApiError>> {
    let file_key = format!(
        "{}/{}/{SUMMARY_JSON_FILE_NAME}.json",
        state.config.environment, key_prefix
    );
    let cache_key = format!("{}:{}", key_prefix, SUMMARY_JSON_FILE_NAME);

    let summary = get_or_populate_in_cache::<SimulationSummary>(state, file_key, cache_key).await?;

    Ok(summary)
}

pub async fn fetch_transaction_outcomes(
    state: &Arc<AppState>,
    key_prefix: &str,
    limit: usize,
    offset: usize,
) -> Result<Vec<SimulationOutcomeOfEachTxn>, Report<ApiError>> {
    error_stack::ensure!(
        limit != 0,
        ApiError::InvalidRequest("limit cannot be set as 0")
    );
    // perform some index calculations
    let total_records_per_json = state.config.parameters.total_records_per_json;
    let (chunk_indices, new_offset) =
        calculate_chunk_indices(offset, limit, total_records_per_json);
    let mut fetch_futures = Vec::new();

    for chunk_idx in chunk_indices {
        let future = async move {
            let cache_key = format!(
                "{}:{}_{}",
                key_prefix, DETAILED_REPORT_FILE_PREFIX, chunk_idx
            );
            let filename = format!("{}_{}.json", DETAILED_REPORT_FILE_PREFIX, chunk_idx);
            let file_key = format!(
                "{}/{}/{DETAILED_REPORT_DIRECTORY_NAME}/{}",
                state.config.environment, &key_prefix, filename
            );

            let records = get_or_populate_in_cache::<Vec<SimulationOutcomeOfEachTxn>>(
                state, file_key, cache_key,
            )
            .await?;
            Ok::<Vec<SimulationOutcomeOfEachTxn>, Report<ApiError>>(records)
        };

        fetch_futures.push(future);
    }

    let fetched_records = future::try_join_all(fetch_futures)
        .await?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

    Ok(fetched_records[new_offset..(new_offset + limit)].to_vec())
}

async fn get_or_populate_in_cache<T>(
    state: &Arc<AppState>,
    file_key: String,
    cache_key: String,
) -> Result<T, Report<ApiError>>
where
    T: serde::Serialize + serde::de::DeserializeOwned + Clone + std::fmt::Debug,
{
    // get cached records
    let type_name = std::any::type_name::<T>();
    let cached_records_result = state
        .redis_conn
        .get_and_deserialize_key::<T>(&cache_key, type_name)
        .await;

    let records = match cached_records_result {
        Ok(rec) => {
            state
                .redis_conn
                .set_expiry(&cache_key, state.config.redis_simulation_keys.ttl)
                .await
                .change_context(ApiError::Unexpected(
                    "Failed to set redis expiry of simulation data key",
                ))?;
            rec
        }
        Err(err) => match err.current_context() {
            // Fetch from file storage and populate cache on miss
            RedisError::NotFound => {
                let records_from_file_storage: T =
                    get_data_from_file_storage(state, file_key).await?;
                state
                    .redis_conn
                    .serialize_and_set_key_with_expiry(
                        &cache_key,
                        records_from_file_storage.clone(),
                        state.config.redis_simulation_keys.ttl,
                    )
                    .await
                    .change_context(ApiError::FailedToRetrieveSimulationReport)?;
                records_from_file_storage
            }
            _ => return Err(err.change_context(ApiError::FailedToRetrieveSimulationReport)),
        },
    };
    Ok::<T, Report<ApiError>>(records)
}

async fn get_data_from_file_storage<T>(
    state: &Arc<AppState>,
    file_key: String,
) -> Result<T, Report<ApiError>>
where
    T: serde::Serialize + serde::de::DeserializeOwned,
{
    let data = state
        .file_storage_client
        .retrieve_file(&file_key)
        .await
        .change_context(ApiError::FailedToRetrieveSimulationReport)?;

    let simulation_records =
        serde_json::from_slice::<T>(&data).change_context(ApiError::FailedToDeserializeCsv)?;

    Ok(simulation_records)
}

fn calculate_chunk_indices(
    offset: usize,
    limit: usize,
    total_records_per_json: usize,
) -> (Vec<usize>, usize) {
    // Calculate the start and end index of the range to process
    let start_index = offset;
    let end_index = (offset + limit).saturating_sub(1);
    let new_offset = (start_index % total_records_per_json).saturating_sub(1);

    // Calculate the chunk start index
    let start_chunk_idx = start_index / total_records_per_json;
    let end_chunk_idx = end_index / total_records_per_json;

    // Return the list of chunk indices
    (
        (start_chunk_idx..=end_chunk_idx).collect::<Vec<usize>>(),
        new_offset,
    )
}

pub async fn invalidate_old_report_chunks(
    state: &Arc<AppState>,
    key_prefix: &str,
) -> Result<(), Report<ApiError>> {
    // Invalidate in persistent storage
    let dir_name = format!(
        "{}/{}/{}",
        state.config.environment, key_prefix, DETAILED_REPORT_DIRECTORY_NAME
    );
    let mut keys_to_be_deleted_in_redis = state
        .file_storage_client
        .delete_directory(&dir_name)
        .await
        .change_context(ApiError::ChunkInvalidationError("persistent storage"))?;

    // Invalidate in redis
    keys_to_be_deleted_in_redis.push(format!("{}:{}", key_prefix, SUMMARY_JSON_FILE_NAME)); // add summary key
    let mut deletion_futures = Vec::with_capacity(keys_to_be_deleted_in_redis.len());
    for key in keys_to_be_deleted_in_redis {
        let key_clone = key.clone();
        let deletion_future = async move {
            state
                .redis_conn
                .delete_key(&key_clone)
                .await
                .change_context(ApiError::ChunkInvalidationError("redis"))?;

            Ok::<(), Report<ApiError>>(())
        };
        deletion_futures.push(deletion_future);
    }

    future::try_join_all(deletion_futures).await?;

    Ok(())
}
