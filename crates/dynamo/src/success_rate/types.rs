use std::collections::VecDeque;

use redis_interface::DelReply;

use crate::{
    authentication::storage::Storage,
    configs::{GlobalSrConfig, KeysTtl},
    ephemeral_store::success_rate::SuccessRateEphemeralStoreInterface,
    success_rate::{block::Block, error::SuccessRateError},
};

pub const SUCCESS_RATE_PREFIX_IN_REDIS: &str = "success_rate";
pub const SUCCESS_RATE_AGGREGATES_SUFFIX_IN_REDIS: &str = "aggregates";
pub const SUCCESS_RATE_CURRENT_BLOCK_SUFFIX_IN_REDIS: &str = "current_block";
pub const SUCCESS_RATE_GLOBAL_ENTITY_ID: &str = "global";

#[derive(Clone)]
pub struct SuccessRate {
    pub config: SuccessRateConfig,
    pub ephemeral_store: Box<dyn SuccessRateEphemeralStoreInterface>,
    pub hash_key: masking::Secret<[u8; 32]>,
    pub storage: Option<Box<dyn Storage>>,
}

#[derive(Clone)]
pub struct SuccessRateConfig {
    pub keys_ttl: KeysTtl,
    pub is_multi_tenancy_enabled: bool,
    pub global_sr_config: GlobalSrConfig,
}

impl SuccessRateConfig {
    pub fn new(
        keys_ttl: KeysTtl,
        is_multi_tenancy_enabled: bool,
        global_sr_config: GlobalSrConfig,
    ) -> Self {
        Self {
            keys_ttl,
            is_multi_tenancy_enabled,
            global_sr_config,
        }
    }
}

impl SuccessRate {
    pub async fn new(
        config: SuccessRateConfig,
        ephemeral_store: Box<dyn SuccessRateEphemeralStoreInterface>,
        hash_key: masking::Secret<[u8; 32]>,
        storage: Option<Box<dyn Storage>>,
    ) -> Self {
        Self {
            config,
            ephemeral_store,
            hash_key,
            storage,
        }
    }

    pub(super) async fn move_current_block_to_aggregates_in_redis(
        &self,
        aggregates_key: &str,
        current_block: Block,
        max_aggregates_size: usize,
    ) -> error_stack::Result<(), SuccessRateError> {
        let mut aggregates: VecDeque<Block> = self
            .ephemeral_store
            .fetch_aggregates(aggregates_key)
            .await?
            .into();

        if aggregates.len() >= max_aggregates_size {
            aggregates.pop_front();
        }

        aggregates.push_back(current_block);

        self.ephemeral_store
            .set_aggregates(aggregates_key, aggregates.into())
            .await
    }
}

#[derive(Debug, strum::Display)]
#[strum(serialize_all = "snake_case")]
pub enum BlockFields {
    TotalCount,
    SuccessCount,
    CreatedAt,
}

#[derive(Debug, strum::Display)]
pub enum KeyDeletionStatus {
    Deleted,
    NotDeleted,
}

impl From<DelReply> for KeyDeletionStatus {
    fn from(reply: DelReply) -> Self {
        match reply {
            DelReply::KeyDeleted => Self::Deleted,
            DelReply::KeyNotDeleted => Self::NotDeleted,
        }
    }
}
