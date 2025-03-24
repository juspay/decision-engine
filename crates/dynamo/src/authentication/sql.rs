use error_stack::{Report, ResultExt};
use masking::PeekInterface;
use serde_json::Value;
use std::collections::HashMap;

use super::storage::StorageError;
use super::types::{ApiKeyInformation, SuccessBasedRoutingConfigBody};
use crate::authentication::{
    hashing::{time::now, Blake3, Hashing},
    types::SuccessBasedRoutingConfig,
};

#[derive(Clone)]
pub struct SqlStorage {
    pool: HashMap<String, sqlx::postgres::PgPool>,
}

impl SqlStorage {
    pub async fn new(
        config: crate::configs::DatabaseConfigs,
        password: String,
    ) -> Result<Self, SqlStorageError> {
        let new_config = config.clone();

        let pool_builder = |schema: String,
                            config: crate::configs::DatabaseConfigs,
                            password: String| async move {
            sqlx::postgres::PgPoolOptions::new()
                .max_connections(config.max_connections)
                .connect(&format!(
                    "postgres://{}:{}@{}:{}/{}?application_name={}&options=-c search_path%3D{}",
                    config.user,
                    password,
                    config.host,
                    config.port,
                    config.database,
                    &schema,
                    &schema
                ))
                .await
                .map(|pool| (schema.to_string(), pool))
        };

        let pool = futures::future::try_join_all(
            config
                .tenants
                .clone()
                .into_iter()
                .map(|tenant| pool_builder(tenant, new_config.clone(), password.clone())),
        )
        .await?
        .into_iter()
        .collect();

        Ok(Self { pool })
    }

    pub async fn fetch_api_key(
        &self,
        tenant: &str,
        api_key: &str,
        hash_key: &masking::Secret<[u8; 32]>,
    ) -> Result<ApiKeyInformation, SqlStorageError> {
        let pool_ctx = self
            .pool
            .get(tenant)
            .ok_or(SqlStorageError::InvalidTenant)?;

        let hashed_key = Blake3::generate(api_key.to_string(), hash_key.peek());

        let query =
            "SELECT merchant_id, key_id, expires_at FROM api_keys WHERE hashed_api_key = $1";
        let row: (String, String, Option<time::PrimitiveDateTime>) = sqlx::query_as(query)
            .bind(hashed_key)
            .fetch_one(pool_ctx)
            .await?;

        tracing::info!(?row, "api key fetched");
        let (merchant_id, key_id, time_to_expire) = row;

        match time_to_expire {
            Some(expiry_time) if expiry_time < now() => {
                tracing::info!("Key expired");
                Err(SqlStorageError::ApiKeyExpired)
            }
            None => Ok(ApiKeyInformation {
                tenant_id: tenant.to_string(),
                merchant_id,
                key_id,
                expires_at: None,
            }),
            Some(expiry_time) => Ok(ApiKeyInformation {
                tenant_id: tenant.to_string(),
                merchant_id,
                key_id,
                expires_at: Some(expiry_time),
            }),
        }
    }

    pub async fn fetch_dynamic_routing_configs(
        &self,
        tenant: &str,
        profile_id: &str,
        merchant_id: &str,
    ) -> Result<SuccessBasedRoutingConfigBody, Report<SqlStorageError>> {
        let pool_ctx = self
            .pool
            .get(tenant)
            .ok_or(SqlStorageError::InvalidTenant)?;

        let query = "SELECT dynamic_routing_algorithm FROM business_profile WHERE profile_id=$1 AND merchant_id=$2";

        let row: (Option<Value>,) = sqlx::query_as(query)
            .bind(profile_id)
            .bind(merchant_id)
            .fetch_one(pool_ctx)
            .await
            .map_err(SqlStorageError::SqlxError)?;

        tracing::info!(?row, "dynamic_routing_ref fetched from business_profile");

        let algorithm_id = match row.0 {
            Some(json) => json
                .get("success_based_algorithm")
                .and_then(|algo| algo.get("algorithm_id_with_timestamp"))
                .and_then(|timestamp| timestamp.get("algorithm_id"))
                .map(|id| serde_json::from_value::<String>(id.clone())),
            None => None,
        }
        .transpose()
        .change_context(SqlStorageError::DeserializationError(
            "id from algorithm_id_with_timestamp".to_string(),
        ))?;

        match &algorithm_id {
            Some(routing_id) => {
                tracing::info!(?routing_id, "Routing algorithm ID fetched");
                let query = "SELECT algorithm_data FROM routing_algorithm WHERE algorithm_id=$1";

                let row: (Option<Value>,) = sqlx::query_as(query)
                    .bind(routing_id)
                    .fetch_one(pool_ctx)
                    .await
                    .map_err(SqlStorageError::SqlxError)?;

                if let Some(json) = row.0 {
                    serde_json::from_value::<SuccessBasedRoutingConfig>(json)
                        .change_context(SqlStorageError::DeserializationError(
                            "SuccessBasedRoutingConfig".to_string(),
                        ))?
                        .config
                        .ok_or(
                            SqlStorageError::SuccessBasedRoutingConfigsNotFound(
                                routing_id.to_string(),
                            )
                            .into(),
                        )
                } else {
                    Err(
                        SqlStorageError::SuccessBasedRoutingConfigsNotFound(routing_id.to_string())
                            .into(),
                    )
                }
            }
            None => Err(SqlStorageError::SuccessBasedRoutingAlgorithmIdNotFound(
                profile_id.to_string(),
            )
            .into()),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SqlStorageError {
    #[error("SQL error: {0}")]
    SqlxError(#[from] sqlx::Error),
    #[error("Invalid Tenant")]
    InvalidTenant,
    #[error("Api key expired")]
    ApiKeyExpired,
    #[error("Success based routing configs not found for profile_id: {0}")]
    SuccessBasedRoutingConfigsNotFound(String),
    #[error("Success based routing algorithm_id not found for profile_id: {0}")]
    SuccessBasedRoutingAlgorithmIdNotFound(String),
    #[error("Unable to deserialize algorithm_data: {0}")]
    DeserializationError(String),
}

#[async_trait::async_trait]
impl super::storage::Storage for SqlStorage {
    async fn fetch_key(
        &self,
        tenant: &str,
        key: &str,
        hash_key: &masking::Secret<[u8; 32]>,
    ) -> Result<ApiKeyInformation, StorageError> {
        Ok(self.fetch_api_key(tenant, key, hash_key).await?)
    }

    async fn fetch_dynamic_routing_configs(
        &self,
        tenant: &str,
        profile_id: &str,
        merchant_id: &str,
    ) -> Result<SuccessBasedRoutingConfigBody, Report<StorageError>> {
        Ok(self
            .fetch_dynamic_routing_configs(tenant, profile_id, merchant_id)
            .await
            .change_context(StorageError::DynamicRoutingConfigError)?)
    }
}
