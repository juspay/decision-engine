use std::sync::Arc;

use error_stack::ResultExt;
use redis_interface::{errors::RedisError, DelReply, RedisConnectionPool};
use serde::{de::DeserializeOwned, Serialize};

use crate::{
    configs::KeysTtl,
    elimination::{configs::BucketSettings, error::EliminationError, leaky_bucket::LeakyBucket},
    logger,
};

pub const ELIMINATION_PREFIX_IN_REDIS: &str = "elimination";
pub const ELIMINATION_GLOBAL_ENTITY_ID: &str = "global";

pub struct Elimination {
    pub config: EliminationConfig,
    pub redis_conn: Arc<RedisConnectionPool>,
}

impl Elimination {
    pub fn new(config: EliminationConfig, redis_conn: Arc<RedisConnectionPool>) -> Self {
        Self { config, redis_conn }
    }

    pub(super) async fn fetch_buckets_from_redis<T>(
        &self,
        elimination_key: &str,
    ) -> error_stack::Result<T, EliminationError>
    where
        T: IntoIterator<Item = LeakyBucket> + Default + DeserializeOwned,
    {
        let bucket = self
            .redis_conn
            .get_and_deserialize_key(elimination_key, "LeakyBucket")
            .await;

        let resultant_bucket = match bucket {
            Ok(aggregates) => Ok(aggregates),
            Err(err) => match err.current_context() {
                RedisError::NotFound => Ok(T::default()),
                _ => Err(err),
            },
        }
        .change_context(EliminationError::RedisError(
            "Failed to fetch buckets from Redis",
        ))?;

        Ok(resultant_bucket)
    }

    pub(super) async fn delete_keys_matching_prefix(
        &self,
        prefix: &str,
    ) -> error_stack::Result<Vec<(String, DelReply)>, EliminationError> {
        let pattern = format!("{prefix}*");
        logger::debug!("Pattern for searching keys in redis for invalidation: {pattern}");

        let keys = self
            .redis_conn
            .scan(&pattern, None, None)
            .await
            .change_context(EliminationError::RedisError(
                "Failed to get keys matching a pattern in redis",
            ))?;

        let redis_del_reply = self
            .redis_conn
            .delete_multiple_keys(keys.clone())
            .await
            .change_context(EliminationError::RedisError(
                "Failed to delete multiple keys in redis",
            ))?;

        let keys_deletion_resp = keys.into_iter().zip(redis_del_reply).collect::<Vec<_>>();

        Ok(keys_deletion_resp)
    }

    pub(super) async fn set_buckets_in_redis<T>(
        &self,
        elimination_key: &str,
        buckets: T,
    ) -> error_stack::Result<(), EliminationError>
    where
        T: IntoIterator<Item = LeakyBucket> + Serialize + std::fmt::Debug,
    {
        self.redis_conn
            .serialize_and_set_key_with_expiry(
                elimination_key,
                buckets,
                self.config.keys_ttl.elimination_bucket,
            )
            .await
            .change_context(EliminationError::RedisError(
                "Failed while setting buckets in Redis",
            ))
    }
}

pub struct EliminationConfig {
    pub keys_ttl: KeysTtl,
    pub is_multi_tenancy_enabled: bool,
    pub global_er_config: BucketSettings,
}

impl EliminationConfig {
    pub fn new(
        keys_ttl: KeysTtl,
        is_multi_tenancy_enabled: bool,
        global_er_config: BucketSettings,
    ) -> Self {
        Self {
            keys_ttl,
            is_multi_tenancy_enabled,
            global_er_config,
        }
    }
}

pub struct EliminationStatus {
    pub should_update_leaks_in_redis: bool,
    pub should_eliminate: bool,
    pub bucket_names: Vec<String>,
}
