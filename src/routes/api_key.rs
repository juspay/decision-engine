use crate::app::get_tenant_app_state;
use crate::auth;
use crate::custom_extractors::{Json, MerchantFromPath, ScopedJson};
use crate::error::{self, ApiKeyError};
use crate::storage::api_keys;
use crate::storage::types::{MerchantApiKey, MerchantApiKeyNew};
use crate::utils::date_time;
use axum::extract::Path;
use serde::{Deserialize, Serialize};
use time::PrimitiveDateTime;

#[derive(Debug, Deserialize)]
pub struct CreateApiKeyRequest {
    pub merchant_id: String,
    pub description: Option<String>,
}

impl crate::auth::MerchantScoped for CreateApiKeyRequest {
    fn merchant_ids(&self) -> Vec<&str> {
        vec![&self.merchant_id]
    }
}

#[derive(Debug, Serialize)]
pub struct CreateApiKeyResponse {
    pub key_id: String,
    pub api_key: String,
    pub key_prefix: String,
    pub merchant_id: String,
    pub description: Option<String>,
    pub created_at: PrimitiveDateTime,
}

#[derive(Debug, Serialize)]
pub struct ApiKeyListItem {
    pub key_id: String,
    pub key_prefix: String,
    pub merchant_id: String,
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: PrimitiveDateTime,
}

#[derive(Debug, Serialize)]
pub struct RevokeApiKeyResponse {
    pub key_id: String,
    pub message: String,
}

impl From<MerchantApiKey> for ApiKeyListItem {
    fn from(k: MerchantApiKey) -> Self {
        Self {
            key_id: k.key_id,
            key_prefix: k.key_prefix,
            merchant_id: k.merchant_id,
            description: k.description,
            #[cfg(feature = "mysql")]
            is_active: k.is_active != 0,
            #[cfg(feature = "postgres")]
            is_active: k.is_active,
            created_at: k.created_at,
        }
    }
}

#[axum::debug_handler]
pub async fn create_api_key(
    // A per-merchant session (JWT / api key) may only mint a key for its own merchant — otherwise a
    // body-supplied `merchant_id` would let any authenticated caller create a working key for a
    // victim merchant and impersonate it. The admin-secret lane inserts no session and may mint for
    // any merchant (that is how Hyperswitch provisions keys).
    ScopedJson { merchant, payload }: ScopedJson<CreateApiKeyRequest>,
) -> Result<Json<CreateApiKeyResponse>, error::ContainerError<ApiKeyError>> {
    let created = mint_api_key(&merchant, payload.description)
        .await
        .ok_or(ApiKeyError::CreationFailed)?;
    Ok(Json(created))
}

#[axum::debug_handler]
pub async fn list_api_keys(
    MerchantFromPath(merchant): MerchantFromPath,
) -> Result<Json<Vec<ApiKeyListItem>>, error::ContainerError<ApiKeyError>> {
    let app_state = get_tenant_app_state().await;
    let keys = api_keys::list(&app_state.db, &merchant)
        .await
        .map_err(|_| ApiKeyError::StorageError)?;

    Ok(Json(keys.into_iter().map(ApiKeyListItem::from).collect()))
}

#[axum::debug_handler]
pub async fn revoke_api_key(
    session: Option<axum::Extension<auth::AuthContext>>,
    Path(key_id): Path<String>,
) -> Result<Json<RevokeApiKeyResponse>, error::ContainerError<ApiKeyError>> {
    let app_state = get_tenant_app_state().await;

    // The route names no merchant. A session revokes among its own merchant's keys; the admin-secret
    // lane (no session) acts for whichever merchant owns the key.
    let merchant = match auth::session_merchant(session.as_deref()) {
        Some(merchant) => merchant,
        None => {
            let owner = api_keys::owner_of(&app_state.db, &key_id)
                .await
                .map_err(|_| ApiKeyError::StorageError)?
                .ok_or(ApiKeyError::NotFound)?;
            auth::authorize_merchant_id(None, &owner).map_err(|_| ApiKeyError::NotFound)?
        }
    };

    // Another merchant's key reads exactly like a missing one — no cross-merchant existence oracle.
    let key = api_keys::find(&app_state.db, &merchant, &key_id)
        .await
        .map_err(|_| ApiKeyError::StorageError)?
        .ok_or(ApiKeyError::NotFound)?;

    api_keys::revoke(&app_state.db, &merchant, &key.key_id)
        .await
        .map_err(|_| ApiKeyError::RevocationFailed)?;

    let cache_key = format!("api_key:{}", key.key_hash);
    let _ = app_state.redis_conn.conn.delete_key(&cache_key).await;

    Ok(Json(RevokeApiKeyResponse {
        key_id,
        message: "API key revoked successfully".to_string(),
    }))
}

/// Mint a key for the merchant and return it, raw key included — the only time it is shown.
pub async fn mint_api_key(
    merchant: &auth::AuthorizedMerchant,
    description: Option<String>,
) -> Option<CreateApiKeyResponse> {
    let raw_key = auth::generate_api_key();
    let key_hash = auth::hash_api_key(&raw_key);
    let key_prefix = auth::extract_key_prefix(&raw_key);
    let key_id = uuid::Uuid::new_v4().to_string();
    let now = date_time::now();

    let new_key = MerchantApiKeyNew {
        key_id: key_id.clone(),
        merchant_id: merchant.as_str().to_string(),
        key_hash,
        key_prefix: key_prefix.clone(),
        description: description.clone(),
        #[cfg(feature = "mysql")]
        is_active: 1,
        #[cfg(feature = "postgres")]
        is_active: true,
        created_at: now,
    };

    let app_state = get_tenant_app_state().await;
    api_keys::insert(&app_state.db, merchant, new_key)
        .await
        .ok()?;
    Some(CreateApiKeyResponse {
        key_id,
        api_key: raw_key,
        key_prefix,
        merchant_id: merchant.as_str().to_string(),
        description,
        created_at: now,
    })
}
