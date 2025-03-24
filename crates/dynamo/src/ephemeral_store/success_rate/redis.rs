use error_stack::ResultExt;
use fred::types::RedisMap;
use redis_interface::errors::RedisError;

use crate::{
    ephemeral_store::{success_rate::SuccessRateEphemeralStoreInterface, RedisEphemeralStore},
    logger,
    success_rate::{
        block::{Block, CurrentBlock},
        error::SuccessRateError,
        types::{BlockFields, KeyDeletionStatus},
    },
};

#[async_trait::async_trait]
impl SuccessRateEphemeralStoreInterface for RedisEphemeralStore {
    async fn set_aggregates(
        &self,
        aggregates_key: &str,
        aggregates: Vec<Block>,
    ) -> error_stack::Result<(), SuccessRateError> {
        self.redis_conn
            .serialize_and_set_key_with_expiry(aggregates_key, aggregates, self.ttl.aggregates)
            .await
            .change_context(SuccessRateError::RedisError(
                "Failed while setting aggregates in Redis",
            ))
    }

    async fn fetch_aggregates(
        &self,
        aggregates_key: &str,
    ) -> error_stack::Result<Vec<Block>, SuccessRateError> {
        let aggregates = self
            .redis_conn
            .get_and_deserialize_key(aggregates_key, "Aggregates")
            .await;

        let resultant_aggregates = match aggregates {
            Ok(aggregates) => Ok(aggregates),
            Err(err) => match err.current_context() {
                RedisError::NotFound => Ok(Vec::default()),
                _ => Err(err),
            },
        }
        .change_context(SuccessRateError::RedisError(
            "Failed to fetch aggregates from Redis",
        ))?;

        Ok(resultant_aggregates)
    }

    async fn initialize_current_block(
        &self,
        current_block_key: &str,
        success_count: u64,
        total_count: u64,
    ) -> error_stack::Result<(), SuccessRateError> {
        let initial_block: RedisMap = Block::new(success_count, total_count)?.try_into()?;

        self.redis_conn
            .set_hash_fields(
                current_block_key,
                initial_block,
                Some(self.ttl.current_block),
            )
            .await
            .change_context(SuccessRateError::RedisError(
                "Failed to set initial current_block in Redis",
            ))
    }

    async fn fetch_current_block(
        &self,
        current_block_key: &str,
    ) -> error_stack::Result<CurrentBlock, SuccessRateError> {
        self.redis_conn
            .get_hash_fields(current_block_key)
            .await
            .change_context(SuccessRateError::RedisError(
                "Failed while fetching current_block from Redis",
            ))
    }

    async fn incr_current_block_fields(
        &self,
        current_block_key: &str,
        fields_to_increment: &[(BlockFields, i64)],
    ) -> error_stack::Result<Vec<usize>, SuccessRateError> {
        let res = self
            .redis_conn
            .increment_fields_in_hash(current_block_key, fields_to_increment)
            .await
            .change_context(SuccessRateError::RedisError(
                "Failed while incrementing hash field in Redis",
            ))?;

        self.redis_conn
            .set_expiry(current_block_key, self.ttl.current_block)
            .await
            .change_context(SuccessRateError::RedisError(
                "Failed while setting expiry of current_block in redis",
            ))?;

        Ok(res)
    }

    async fn delete_key(
        &self,
        key: &str,
    ) -> error_stack::Result<KeyDeletionStatus, SuccessRateError> {
        self.redis_conn
            .delete_key(key)
            .await
            .change_context(SuccessRateError::RedisError(
                "Failed to delete key in redis",
            ))
            .map(KeyDeletionStatus::from)
    }

    async fn delete_keys_matching_prefix(
        &self,
        prefix: &str,
    ) -> error_stack::Result<Vec<(String, KeyDeletionStatus)>, SuccessRateError> {
        let pattern = format!("{prefix}*");
        logger::debug!("Pattern for searching keys in redis for invalidation: {pattern}");

        let keys = self
            .redis_conn
            .scan(&pattern, None, None)
            .await
            .change_context(SuccessRateError::RedisError(
                "Failed to get keys matching a pattern in redis",
            ))?;

        let redis_del_reply = self
            .redis_conn
            .delete_multiple_keys(keys.clone())
            .await
            .change_context(SuccessRateError::RedisError(
                "Failed to delete multiple keys in redis",
            ))?
            .into_iter()
            .map(KeyDeletionStatus::from)
            .collect::<Vec<_>>();

        let keys_deletion_resp = keys.into_iter().zip(redis_del_reply).collect::<Vec<_>>();

        Ok(keys_deletion_resp)
    }
}
