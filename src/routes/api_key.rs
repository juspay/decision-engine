use crate::app::get_tenant_app_state;
use crate::auth;
use crate::custom_extractors::Json;
use crate::error::{self, ApiKeyError};
use crate::storage::types::{MerchantApiKey, MerchantApiKeyNew, MerchantApiKeyRevoke};
use crate::utils::date_time;
use axum::extract::Path;
use diesel::associations::HasTable;
use diesel::ExpressionMethods;
use serde::{Deserialize, Serialize};
use time::PrimitiveDateTime;

#[cfg(feature = "mysql")]
use crate::storage::schema::merchant_api_keys::dsl;
#[cfg(feature = "postgres")]
use crate::storage::schema_pg::merchant_api_keys::dsl;

#[derive(Debug, Deserialize)]
pub struct CreateApiKeyRequest {
    pub merchant_id: String,
    pub description: Option<String>,
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
    session: Option<axum::Extension<auth::AuthContext>>,
    Json(payload): Json<CreateApiKeyRequest>,
) -> Result<Json<CreateApiKeyResponse>, error::ContainerError<ApiKeyError>> {
    // A per-merchant session (JWT / api key) may only mint a key for its own merchant — otherwise a
    // body-supplied `merchant_id` would let any authenticated caller create a working key for a
    // victim merchant and impersonate it. The admin-secret lane inserts no session and may mint for
    // any merchant (that is how Hyperswitch provisions keys).
    if let Some(axum::Extension(context)) = session.as_ref() {
        if payload.merchant_id != context.merchant_id {
            return Err(ApiKeyError::Forbidden.into());
        }
    }

    let raw_key = auth::generate_api_key();
    let key_hash = auth::hash_api_key(&raw_key);
    let key_prefix = auth::extract_key_prefix(&raw_key);
    let key_id = uuid::Uuid::new_v4().to_string();
    let now = date_time::now();

    let new_key = MerchantApiKeyNew {
        key_id: key_id.clone(),
        merchant_id: payload.merchant_id.clone(),
        key_hash,
        key_prefix: key_prefix.clone(),
        description: payload.description.clone(),
        #[cfg(feature = "mysql")]
        is_active: 1,
        #[cfg(feature = "postgres")]
        is_active: true,
        created_at: now,
    };

    let app_state = get_tenant_app_state().await;
    crate::generics::generic_insert(&app_state.db, new_key)
        .await
        .map_err(|_| ApiKeyError::CreationFailed)?;

    Ok(Json(CreateApiKeyResponse {
        key_id,
        api_key: raw_key,
        key_prefix,
        merchant_id: payload.merchant_id,
        description: payload.description,
        created_at: now,
    }))
}

#[axum::debug_handler]
pub async fn list_api_keys(
    Path(merchant_id): Path<String>,
) -> Result<Json<Vec<ApiKeyListItem>>, error::ContainerError<ApiKeyError>> {
    let app_state = get_tenant_app_state().await;
    let keys = crate::generics::generic_find_all::<
        <MerchantApiKey as HasTable>::Table,
        _,
        MerchantApiKey,
    >(&app_state.db, dsl::merchant_id.eq(merchant_id))
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

    // This route is keyed by `key_id`, not `:merchant-id`, so the scope middleware cannot bind it.
    // A per-merchant session may only revoke a key that belongs to its own merchant; resolve the
    // key first and check ownership. A missing key reads as NotFound either way, so a caller cannot
    // probe which key ids exist under other merchants. The admin-secret lane (no session) may
    // revoke any key.
    let existing = crate::generics::generic_find_all::<
        <MerchantApiKey as HasTable>::Table,
        _,
        MerchantApiKey,
    >(&app_state.db, dsl::key_id.eq(key_id.clone()))
    .await
    .map_err(|_| ApiKeyError::StorageError)?;
    let key_record = existing.into_iter().next().ok_or(ApiKeyError::NotFound)?;

    if let Some(axum::Extension(context)) = session.as_ref() {
        if key_record.merchant_id != context.merchant_id {
            // Same response shape as a genuinely missing key — no cross-merchant existence oracle.
            return Err(ApiKeyError::NotFound.into());
        }
    }

    let conn = &app_state
        .db
        .get_conn()
        .await
        .map_err(|_| ApiKeyError::StorageError)?;

    let revoke = MerchantApiKeyRevoke {
        #[cfg(feature = "mysql")]
        is_active: 0,
        #[cfg(feature = "postgres")]
        is_active: false,
    };

    crate::generics::generic_update::<<MerchantApiKey as HasTable>::Table, _, _>(
        conn,
        dsl::key_id.eq(key_id.clone()),
        revoke,
    )
    .await
    .map_err(|_| ApiKeyError::RevocationFailed)?;

    // Remove from Redis cache
    if let Ok(keys) = crate::generics::generic_find_all::<
        <MerchantApiKey as HasTable>::Table,
        _,
        MerchantApiKey,
    >(&app_state.db, dsl::key_id.eq(key_id.clone()))
    .await
    {
        for key in keys {
            let cache_key = format!("api_key:{}", key.key_hash);
            let _ = app_state.redis_conn.conn.delete_key(&cache_key).await;
        }
    }

    Ok(Json(RevokeApiKeyResponse {
        key_id,
        message: "API key revoked successfully".to_string(),
    }))
}

pub async fn insert_api_key_for_merchant(
    merchant_id: &str,
    description: Option<String>,
) -> Option<String> {
    let raw_key = auth::generate_api_key();
    let key_hash = auth::hash_api_key(&raw_key);
    let key_prefix = auth::extract_key_prefix(&raw_key);
    let key_id = uuid::Uuid::new_v4().to_string();
    let now = date_time::now();

    let new_key = MerchantApiKeyNew {
        key_id,
        merchant_id: merchant_id.to_string(),
        key_hash,
        key_prefix,
        description,
        #[cfg(feature = "mysql")]
        is_active: 1,
        #[cfg(feature = "postgres")]
        is_active: true,
        created_at: now,
    };

    let app_state = get_tenant_app_state().await;
    match crate::generics::generic_insert(&app_state.db, new_key).await {
        Ok(_) => Some(raw_key),
        Err(_) => None,
    }
}
