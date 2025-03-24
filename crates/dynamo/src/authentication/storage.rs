use async_trait::async_trait;
use error_stack::Report;

use super::sql;
use super::types::{ApiKeyInformation, SuccessBasedRoutingConfigBody};

#[async_trait]
pub trait Storage: dyn_clone::DynClone + Sync + Send {
    async fn fetch_key(
        &self,
        tenant: &str,
        key: &str,
        hash_key: &masking::Secret<[u8; 32]>,
    ) -> Result<ApiKeyInformation, StorageError>;

    async fn fetch_dynamic_routing_configs(
        &self,
        tenant: &str,
        profile_id: &str,
        merchant_id: &str,
    ) -> Result<SuccessBasedRoutingConfigBody, Report<StorageError>>;
}

dyn_clone::clone_trait_object!(Storage);

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("custom sqlx error: {0}")]
    SqlxError(#[from] sql::SqlStorageError),
    #[error("Error in fetching dynamic routing configs")]
    DynamicRoutingConfigError,
}
