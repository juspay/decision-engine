use std::sync::Arc;

use axum::{extract::FromRequestParts, http::request::Parts};
use dynamo::{authentication::types::ApiKeyInformation, logger};
use error_stack::ResultExt;
use hyper::HeaderMap;
use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use masking::PeekInterface;

use crate::{
    app::AppState,
    consts,
    errors::{ApiError, ContainerError},
};

#[derive(Clone)]
pub struct AuthResolver(pub Headers);

impl FromRequestParts<Arc<AppState>> for AuthResolver {
    type Rejection = ContainerError<ApiError>;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let headers = &parts.headers;

        // Try JWT authentication first
        match authenticate_jwt_key(state, headers).await {
            Ok(resolver) => Ok(resolver),
            Err(jwt_error) => {
                // If JWT fails, try API key authentication
                match authenticate_api_key(state, headers).await {
                    Ok(resolver) => Ok(resolver),
                    Err(api_key_error) => {
                        // Both authentication methods failed
                        logger::error!(
                            "Authentication failed: JWT error: {:?}, API key error: {:?}",
                            jwt_error,
                            api_key_error
                        );
                        Err(ContainerError::from(ApiError::UnAuthenticated))
                    }
                }
            }
        }
    }
}

async fn authenticate_api_key(
    state: &Arc<AppState>,
    headers: &HeaderMap,
) -> Result<AuthResolver, ContainerError<ApiError>> {
    let tenant_id = if state.config.multi_tenancy.enabled {
        headers
            .get(consts::X_TENANT_ID)
            .and_then(|h| h.to_str().ok())
            .ok_or(ApiError::HeadersError("x-tenant-id not found in headers"))?
            .to_string()
    } else {
        consts::DEFAULT_TENANT_ID.to_string()
    };

    let api_key = headers
        .get(consts::API_KEY_HEADER_KEY)
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
        .ok_or(ApiError::HeadersError("x-api-key not found in headers"))?;

    let api_key_info: ApiKeyInformation = state
        .sr_algorithm
        .window_based
        .storage
        .as_ref()
        .ok_or(ApiError::StorageNotFound)?
        .fetch_key(
            &tenant_id,
            api_key,
            &state.sr_algorithm.window_based.hash_key,
        )
        .await
        .change_context(ApiError::UnAuthenticated)?;

    Ok(AuthResolver(Headers {
        tenant_id,
        merchant_id: api_key_info.merchant_id,
    }))
}

async fn authenticate_jwt_key(
    state: &Arc<AppState>,
    headers: &HeaderMap,
) -> Result<AuthResolver, ContainerError<ApiError>> {
    let jwt_key = match headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| {
            if h.is_empty() {
                None
            } else {
                h.strip_prefix("Bearer ").map(|token| token.to_string())
            }
        }) {
        Some(key) => key,
        None => {
            println!("JWT header not found or empty");
            return Err(ContainerError::from(ApiError::HeadersError("JWT key")));
        }
    };

    match decode_jwt(&jwt_key, state).await {
        Ok(jwt_claims) => Ok(AuthResolver(jwt_claims)),
        Err(e) => {
            println!("JWT authentication failed: {:?}", e);
            Err(e)
        }
    }
}

async fn decode_jwt(
    token: &str,
    state: &Arc<AppState>,
) -> Result<Headers, ContainerError<ApiError>> {
    let secret = state.jwt_secret.peek().as_bytes();
    let key = DecodingKey::from_secret(secret);
    Ok(
        decode::<Headers>(token, &key, &Validation::new(Algorithm::HS256))
            .map(|token_data| token_data.claims)
            .change_context(ApiError::UnAuthenticated)?,
    )
}

#[derive(Clone, Debug, serde::Deserialize)]
pub struct Headers {
    pub tenant_id: String,
    pub merchant_id: String,
}
