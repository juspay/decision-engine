use error_stack::{Report, ResultExt};
use tonic::metadata::MetadataMap;

use super::{
    errors::AuthenticationError,
    types::{ApiKeyInformation, SuccessBasedRoutingConfigBody},
};
use crate::success_rate::types::SuccessRate;
const API_KEY_HEADER_KEY: &str = "x-api-key";
const TENANT_ID_HEADER_KEY: &str = "x-tenant-id";
const PROFILE_ID_HEADER_KEY: &str = "x-profile-id";

#[async_trait::async_trait()]
pub trait Authenticate {
    type Config;
    async fn authenticate(
        &self,
        headers: &MetadataMap,
    ) -> Result<ApiKeyInformation, Report<AuthenticationError>>;
    async fn fetch_configs(
        &self,
        headers: &MetadataMap,
        tenant_id: &str,
        merchant_id: &str,
    ) -> Result<Self::Config, Report<AuthenticationError>>;
}

#[async_trait::async_trait()]
impl Authenticate for SuccessRate {
    type Config = SuccessBasedRoutingConfigBody;
    async fn authenticate(
        &self,
        headers: &MetadataMap,
    ) -> Result<ApiKeyInformation, Report<AuthenticationError>> {
        let tenant_id = headers
            .get(TENANT_ID_HEADER_KEY)
            .and_then(|h| match h.to_str() {
                Ok(h) => {
                    if h.is_empty() {
                        None
                    } else {
                        Some(h)
                    }
                }
                Err(_) => None,
            })
            .ok_or(AuthenticationError::MissingHeader("tenant_id"))?;

        let api_key = headers
            .get(API_KEY_HEADER_KEY)
            .and_then(|h| match h.to_str() {
                Ok(h) => {
                    if h.is_empty() {
                        None
                    } else {
                        Some(h)
                    }
                }
                Err(_) => None,
            })
            .ok_or(AuthenticationError::MissingHeader("api_key"))?;

        self.storage
            .as_ref()
            .ok_or(AuthenticationError::StorageNotFound)?
            .fetch_key(tenant_id, api_key, &self.hash_key)
            .await
            .change_context(AuthenticationError::UnAuthenticated)
    }

    async fn fetch_configs(
        &self,
        headers: &MetadataMap,
        tenant_id: &str,
        merchant_id: &str,
    ) -> Result<Self::Config, Report<AuthenticationError>> {
        let profile_id = headers
            .get(PROFILE_ID_HEADER_KEY)
            .and_then(|h| match h.to_str() {
                Ok(h) => {
                    if h.is_empty() {
                        None
                    } else {
                        Some(h)
                    }
                }
                Err(_) => None,
            })
            .ok_or(AuthenticationError::MissingHeader("profile_id"))?;

        self.storage
            .as_ref()
            .ok_or(AuthenticationError::StorageNotFound)?
            .fetch_dynamic_routing_configs(tenant_id, profile_id, merchant_id)
            .await
            .change_context(AuthenticationError::RoutingConfigsNotFound(
                profile_id.to_string(),
            ))
    }
}
