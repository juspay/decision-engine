use crate::app::APP_STATE;
use crate::auth;
use crate::custom_extractors::TenantStateResolver;
use crate::error::{self, ContainerError};
use axum::body::Body;
use axum::response::{IntoResponse, Response};
use axum::{http::Request, http::StatusCode, middleware::Next};
use diesel::ExpressionMethods;

const API_KEY_CACHE_TTL: i64 = 300;

/// Middleware providing implementation to perform JWE + JWS encryption and decryption around the
/// card APIs
pub async fn middleware(
    TenantStateResolver(_tenant_state): TenantStateResolver,
    req: Request<Body>,
    next: Next,
) -> Result<Response<Body>, ContainerError<error::ApiError>> {
    let response = next.run(req).await;
    Ok(response)
}

/// Middleware to authenticate requests using either:
/// - `x-api-key` header (service-to-service / programmatic access)
/// - `Authorization: Bearer <jwt>` header (dashboard / user sessions)
///
/// When `api_key_auth_enabled` is false in config, all requests pass through (backward compat mode).
pub async fn authenticate(
    req: Request<Body>,
    next: Next,
) -> Result<Response<Body>, ContainerError<error::ApiError>> {
    let app_state = match APP_STATE.get() {
        Some(s) => s,
        None => return Ok((StatusCode::INTERNAL_SERVER_ERROR, "Server not ready").into_response()),
    };

    if !app_state.global_config.api_key_auth_enabled {
        return Ok(next.run(req).await);
    }

    // Accept JWT Bearer token (dashboard sessions)
    if let Some(bearer) = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
    {
        use crate::routes::user_auth::verify_jwt_not_revoked;
        match verify_jwt_not_revoked(bearer, &app_state.global_config.user_auth.jwt_secret).await {
            Ok(_) => return Ok(next.run(req).await),
            Err(_) => {
                return Ok((StatusCode::UNAUTHORIZED, "Invalid or expired token").into_response())
            }
        }
    }

    let api_key = match req.headers().get("x-api-key").and_then(|v| v.to_str().ok()) {
        Some(k) => k.to_owned(),
        None => {
            return Ok(
                (StatusCode::UNAUTHORIZED, "Missing authentication credentials").into_response(),
            )
        }
    };

    let key_hash = auth::hash_api_key(&api_key);
    let cache_key = format!("api_key:{}", key_hash);

    let tenant_state =
        match crate::tenant::GlobalAppState::get_app_state_of_tenant(app_state, "public").await {
            Ok(s) => s,
            Err(_) => {
                return Ok((StatusCode::INTERNAL_SERVER_ERROR, "Tenant not found").into_response())
            }
        };

    // Check Redis cache first
    if let Ok(cached) = tenant_state.redis_conn.get_key_string(&cache_key).await {
        if !cached.is_empty() {
            return Ok(next.run(req).await);
        }
    }

    // Cache miss — query DB
    use crate::storage::types::MerchantApiKey;
    use diesel::associations::HasTable;

    #[cfg(feature = "mysql")]
    use crate::storage::schema::merchant_api_keys::dsl;
    #[cfg(feature = "postgres")]
    use crate::storage::schema_pg::merchant_api_keys::dsl;

    let results = crate::generics::generic_find_all::<
        <MerchantApiKey as HasTable>::Table,
        _,
        MerchantApiKey,
    >(&tenant_state.db, dsl::key_hash.eq(key_hash.clone()))
    .await;

    let key_record = match results {
        Ok(mut rows) => rows.pop(),
        Err(_) => None,
    };

    match key_record {
        Some(record) => {
            let is_active = {
                #[cfg(feature = "mysql")]
                {
                    record.is_active != 0
                }
                #[cfg(feature = "postgres")]
                {
                    record.is_active
                }
            };

            if !is_active {
                return Ok((StatusCode::UNAUTHORIZED, "API key is revoked").into_response());
            }

            // Populate Redis cache
            let _ = tenant_state
                .redis_conn
                .set_key_with_ttl(&cache_key, &record.merchant_id, API_KEY_CACHE_TTL)
                .await;

            Ok(next.run(req).await)
        }
        None => Ok((StatusCode::UNAUTHORIZED, "Invalid API key").into_response()),
    }
}
