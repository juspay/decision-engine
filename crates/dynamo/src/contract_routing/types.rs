use crate::logger;
use crate::{
    configs::KeysTtl,
    contract_routing::{configs::TimeScale, errors::ContractRoutingError},
    success_rate::utils,
};
use error_stack::ResultExt;
use redis_interface::{errors::RedisError, DelReply, RedisConnectionPool};
use serde::{de::DeserializeOwned, Serialize};
use std::sync::Arc;

pub const CONTRACT_ROUTING_PREFIX_IN_REDIS: &str = "contract_routing";
pub const CONTRACT_ROUTING_MAP_SUFFIX_IN_REDIS: &str = "contract_map";
const MAXIMUM_GRADIENT: f64 = 100000.0;
const MINIMUM_GRADIENT: f64 = 0.0;
const MIN_RANGE: f64 = 0.0;
const MAX_RANGE: f64 = 5.0;
const SECS_IN_DAY: f64 = 86400.0;
const SECS_IN_MONTH: f64 = 86400.0 * 30.0;

pub struct ContractRoutingConfig {
    pub keys_ttl: KeysTtl,
    pub is_multi_tenancy_enabled: bool,
}

impl ContractRoutingConfig {
    pub fn new(keys_ttl: KeysTtl, is_multi_tenancy_enabled: bool) -> Self {
        Self {
            keys_ttl,
            is_multi_tenancy_enabled,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, serde::Serialize, serde::Deserialize)]
pub struct ContractMap {
    pub label: String,
    pub target_count: u64,
    pub target_time: u64,
    pub current_count: u64,
}

pub struct ContractRouting {
    pub config: ContractRoutingConfig,
    pub redis_conn: Arc<RedisConnectionPool>,
}

impl ContractRouting {
    pub fn new(config: ContractRoutingConfig, redis_conn: Arc<RedisConnectionPool>) -> Self {
        Self { config, redis_conn }
    }

    pub(super) async fn fetch_contract_map_from_redis<T: DeserializeOwned>(
        &self,
        key: &str,
    ) -> error_stack::Result<Option<T>, ContractRoutingError> {
        let contract_map_result = self
            .redis_conn
            .get_and_deserialize_key(key, "ContractMap")
            .await;

        let final_map = contract_map_result
            .map(Some)
            .or_else(|err| match err.current_context() {
                RedisError::NotFound => Ok(None),
                _ => Err(err),
            })
            .change_context(ContractRoutingError::RedisError(
                "Failed to fetch contract map from Redis",
            ))?;

        Ok(final_map)
    }

    pub(super) async fn set_contract_map_in_redis<T: Serialize + std::fmt::Debug>(
        &self,
        key: &str,
        contract_map: T,
    ) -> error_stack::Result<(), ContractRoutingError> {
        self.redis_conn
            .serialize_and_set_key_with_expiry(key, contract_map, self.config.keys_ttl.contract_ttl)
            .await
            .change_context(ContractRoutingError::RedisError(
                "Failed while setting aggregates in Redis",
            ))
    }

    pub(super) async fn delete_keys_matching_prefix(
        &self,
        prefix: &str,
    ) -> error_stack::Result<Vec<(String, DelReply)>, ContractRoutingError> {
        let pattern = format!("{prefix}*");
        logger::debug!("Pattern for searching keys in redis for invalidation: {pattern}");

        let keys = self
            .redis_conn
            .scan(&pattern, None, None)
            .await
            .change_context(ContractRoutingError::RedisError(
                "Failed to get keys matching a pattern in redis",
            ))?;

        let redis_del_reply = self
            .redis_conn
            .delete_multiple_keys(keys.clone())
            .await
            .change_context(ContractRoutingError::RedisError(
                "Failed to delete multiple keys in redis",
            ))?;

        let keys_deletion_resp = keys.into_iter().zip(redis_del_reply).collect::<Vec<_>>();

        Ok(keys_deletion_resp)
    }

    pub(super) fn calculate_ct_score(
        &self,
        target_count: f64,
        current_count: f64,
        target_time: f64,
        constants: Vec<f64>,
        time_scale: Option<TimeScale>,
    ) -> error_stack::Result<f64, ContractRoutingError> {
        let delta_y = target_count - current_count;

        // default score
        if delta_y == 0.0 {
            return Ok(f64::default());
        }

        #[allow(clippy::as_conversions)]
        let current_time = utils::get_current_time_in_secs()
            .change_context(ContractRoutingError::FailedToGetCurrentTime)?
            as f64;

        let final_time = time_scale.map(|scale| match scale {
            TimeScale::Day => (target_time / SECS_IN_DAY, current_time / SECS_IN_DAY),
            TimeScale::Month => (target_time / SECS_IN_MONTH, current_time / SECS_IN_MONTH),
        });

        let delta_x = final_time.map(|time| time.0).unwrap_or(target_time)
            - final_time.map(|time| time.1).unwrap_or(current_time);
        let gradient = delta_y / delta_x;
        let normalized_gradient = MIN_RANGE
            + (MAX_RANGE - MIN_RANGE)
                * ((gradient - MINIMUM_GRADIENT) / (MAXIMUM_GRADIENT - MINIMUM_GRADIENT));

        let linear_multiplier = constants
            .first()
            .ok_or(ContractRoutingError::ConfigError("constant a not found"))?;
        let quadratic_multiplier = constants
            .get(1)
            .ok_or(ContractRoutingError::ConfigError("constant b not found"))?;

        let ct_score = linear_multiplier * normalized_gradient
            + quadratic_multiplier * normalized_gradient.powf(2.0);
        Ok(ct_score)
    }
}
