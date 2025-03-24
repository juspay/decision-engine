pub mod imc;
pub mod redis;

use crate::success_rate::{
    block::{Block, CurrentBlock},
    error::SuccessRateError,
    types::{BlockFields, KeyDeletionStatus},
};

#[async_trait::async_trait]
pub trait SuccessRateEphemeralStoreInterface: dyn_clone::DynClone + Sync + Send {
    async fn set_aggregates(
        &self,
        aggregates_key: &str,
        aggregates: Vec<Block>,
    ) -> error_stack::Result<(), SuccessRateError>;

    async fn fetch_aggregates(
        &self,
        aggregates_key: &str,
    ) -> error_stack::Result<Vec<Block>, SuccessRateError>;

    async fn initialize_current_block(
        &self,
        current_block_key: &str,
        success_count: u64,
        total_count: u64,
    ) -> error_stack::Result<(), SuccessRateError>;

    async fn fetch_current_block(
        &self,
        current_block_key: &str,
    ) -> error_stack::Result<CurrentBlock, SuccessRateError>;

    async fn incr_current_block_fields(
        &self,
        current_block_key: &str,
        fields_to_increment: &[(BlockFields, i64)],
    ) -> error_stack::Result<Vec<usize>, SuccessRateError>;

    async fn delete_key(
        &self,
        key: &str,
    ) -> error_stack::Result<KeyDeletionStatus, SuccessRateError>;

    async fn delete_keys_matching_prefix(
        &self,
        prefix: &str,
    ) -> error_stack::Result<Vec<(String, KeyDeletionStatus)>, SuccessRateError>;
}

dyn_clone::clone_trait_object!(SuccessRateEphemeralStoreInterface);
