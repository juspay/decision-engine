use error_stack::Report;
use masking::PeekInterface;

use super::hashing::{Blake3, Hashing};
use super::storage::StorageError;
use super::types::{ApiKeyInformation, SuccessBasedRoutingConfigBody};

#[derive(Clone)]
pub struct CachingStorage<T: Clone + super::storage::Storage> {
    storage: T,
    api_key_cache: moka::future::Cache<TenantHashKey, ApiKeyInformation>, // tenant_id, key
    success_based_routing_config_cache:
        moka::future::Cache<TenantProfileKey, SuccessBasedRoutingConfigBody>,
}

#[derive(Eq, PartialEq, Hash)]
pub struct TenantHashKey {
    pub tenant_id: String,
    pub hashed_api_key: String,
}

#[derive(Eq, PartialEq, Hash)]
pub struct TenantProfileKey {
    pub tenant_id: String,
    pub profile_id: String,
}

#[async_trait::async_trait]
impl<T: super::storage::Storage + Clone> super::storage::Storage for CachingStorage<T> {
    async fn fetch_key(
        &self,
        tenant: &str,
        api_key: &str,
        hash_key: &masking::Secret<[u8; 32]>,
    ) -> Result<ApiKeyInformation, StorageError> {
        let hashed_key = Blake3::generate(api_key.to_string(), hash_key.peek());

        let cache_key = TenantHashKey {
            tenant_id: tenant.to_string(),
            hashed_api_key: hashed_key,
        };

        match self.api_key_cache.get(&cache_key).await {
            Some(ident) => {
                tracing::info!(?ident, "cache hit");
                Ok(ident)
            }
            None => {
                let ident = self.storage.fetch_key(tenant, api_key, hash_key).await;
                match ident {
                    Ok(ident) => {
                        self.api_key_cache.insert(cache_key, ident.clone()).await;
                        Ok(ident)
                    }
                    Err(err) => Err(err),
                }
            }
        }
    }

    async fn fetch_dynamic_routing_configs(
        &self,
        tenant: &str,
        profile_id: &str,
        merchant_id: &str,
    ) -> Result<SuccessBasedRoutingConfigBody, Report<StorageError>> {
        let cache_key = TenantProfileKey {
            tenant_id: tenant.to_string(),
            profile_id: profile_id.to_string(),
        };

        match self
            .success_based_routing_config_cache
            .get(&cache_key)
            .await
        {
            Some(config) => {
                tracing::info!(?config, "cache hit");
                Ok(config)
            }
            None => {
                tracing::info!("cache missed: falling back to storage");
                let config = self
                    .storage
                    .fetch_dynamic_routing_configs(tenant, profile_id, merchant_id)
                    .await;
                match config {
                    Ok(config) => {
                        self.success_based_routing_config_cache
                            .insert(cache_key, config.clone())
                            .await;
                        Ok(config)
                    }
                    Err(err) => Err(err),
                }
            }
        }
    }
}

impl<T: super::storage::Storage + Clone> CachingStorage<T> {
    pub fn new(storage: T, config: crate::configs::CacheConfigs) -> Self {
        let api_key_cache = moka::future::CacheBuilder::new(config.max_cache_size)
            .time_to_live(std::time::Duration::from_secs(config.ttl_in_seconds))
            .time_to_idle(std::time::Duration::from_secs(config.tti_in_seconds))
            .build();

        let success_based_routing_config_cache =
            moka::future::CacheBuilder::new(config.max_cache_size)
                .time_to_live(std::time::Duration::from_secs(config.ttl_in_seconds))
                .time_to_idle(std::time::Duration::from_secs(config.tti_in_seconds))
                .build();

        Self {
            storage,
            api_key_cache,
            success_based_routing_config_cache,
        }
    }
}
