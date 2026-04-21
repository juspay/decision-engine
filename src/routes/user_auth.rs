use crate::app::{get_tenant_app_state, APP_STATE};
use crate::auth;
use crate::error::{self, UserAuthError};
use crate::storage::types::{NewUser, User};
use crate::utils::date_time;
use axum::http::HeaderMap;
use axum::Json;
use diesel::associations::HasTable;
use diesel::ExpressionMethods;
use serde::{Deserialize, Serialize};

#[cfg(feature = "mysql")]
use crate::storage::schema::users::dsl;
#[cfg(feature = "postgres")]
use crate::storage::schema_pg::users::dsl;

const JWT_DENYLIST_PREFIX: &str = "jwt_revoked:";

#[derive(Debug, Deserialize)]
pub struct SignupRequest {
    pub email: String,
    pub password: String,
    pub merchant_id: String,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user_id: String,
    pub email: String,
    pub merchant_id: String,
    pub role: String,
}

#[derive(Debug, Serialize)]
pub struct MeResponse {
    pub user_id: String,
    pub email: String,
    pub merchant_id: String,
    pub role: String,
    pub email_verified: bool,
}

#[axum::debug_handler]
pub async fn signup(
    Json(payload): Json<SignupRequest>,
) -> Result<Json<AuthResponse>, error::ContainerError<UserAuthError>> {
    let app_state = get_tenant_app_state().await;
    let global_config = APP_STATE
        .get()
        .map(|s| s.global_config.clone())
        .ok_or(UserAuthError::StorageError)?;

    // Check merchant exists
    {
        #[cfg(feature = "mysql")]
        use crate::storage::schema::merchant_account::dsl as ma_dsl;
        #[cfg(feature = "postgres")]
        use crate::storage::schema_pg::merchant_account::dsl as ma_dsl;

        let exists = crate::generics::generic_find_all::<
            <crate::storage::types::MerchantAccount as HasTable>::Table,
            _,
            crate::storage::types::MerchantAccount,
        >(&app_state.db, ma_dsl::merchant_id.eq(payload.merchant_id.clone()))
        .await
        .map_err(|_| UserAuthError::StorageError)?;

        if exists.is_empty() {
            return Err(error::ContainerError::from(UserAuthError::MerchantNotFound));
        }
    }

    // Check email uniqueness
    let existing = crate::generics::generic_find_all::<
        <User as HasTable>::Table,
        _,
        User,
    >(&app_state.db, dsl::email.eq(payload.email.clone()))
    .await
    .map_err(|_| UserAuthError::StorageError)?;

    if !existing.is_empty() {
        return Err(error::ContainerError::from(UserAuthError::EmailAlreadyExists));
    }

    let password_hash = auth::hash_password(&payload.password)
        .map_err(|_| UserAuthError::PasswordHashingFailed)?;

    let user_id = uuid::Uuid::new_v4().to_string();
    let now = date_time::now();

    let new_user = NewUser {
        user_id: user_id.clone(),
        email: payload.email.clone(),
        password_hash,
        merchant_id: payload.merchant_id.clone(),
        role: "admin".to_string(),
        #[cfg(feature = "mysql")]
        is_active: 1,
        #[cfg(feature = "postgres")]
        is_active: true,
        // Email verification skipped for local; in production set to 0/false and send email
        #[cfg(feature = "mysql")]
        email_verified: if global_config.user_auth.email_verification_enabled { 0 } else { 1 },
        #[cfg(feature = "postgres")]
        email_verified: !global_config.user_auth.email_verification_enabled,
        created_at: now,
    };

    crate::generics::generic_insert(&app_state.db, new_user)
        .await
        .map_err(|_| UserAuthError::StorageError)?;

    if global_config.user_auth.email_verification_enabled {
        // TODO: send verification email via email provider
        return Err(error::ContainerError::from(UserAuthError::EmailNotVerified));
    }

    let token = auth::generate_jwt(
        &user_id,
        &payload.email,
        &payload.merchant_id,
        "admin",
        &global_config.user_auth.jwt_secret,
        global_config.user_auth.jwt_expiry_seconds,
    )
    .map_err(|_| UserAuthError::TokenGenerationFailed)?;

    Ok(Json(AuthResponse {
        token,
        user_id,
        email: payload.email,
        merchant_id: payload.merchant_id,
        role: "admin".to_string(),
    }))
}

#[axum::debug_handler]
pub async fn login(
    Json(payload): Json<LoginRequest>,
) -> Result<Json<AuthResponse>, error::ContainerError<UserAuthError>> {
    let app_state = get_tenant_app_state().await;
    let global_config = APP_STATE
        .get()
        .map(|s| s.global_config.clone())
        .ok_or(UserAuthError::StorageError)?;

    let mut users = crate::generics::generic_find_all::<
        <User as HasTable>::Table,
        _,
        User,
    >(&app_state.db, dsl::email.eq(payload.email.clone()))
    .await
    .map_err(|_| UserAuthError::StorageError)?;

    let user = users.pop().ok_or(UserAuthError::UserNotFound)?;

    let is_active = {
        #[cfg(feature = "mysql")]
        { user.is_active != 0 }
        #[cfg(feature = "postgres")]
        { user.is_active }
    };
    if !is_active {
        return Err(error::ContainerError::from(UserAuthError::AccountInactive));
    }

    let email_verified = {
        #[cfg(feature = "mysql")]
        { user.email_verified != 0 }
        #[cfg(feature = "postgres")]
        { user.email_verified }
    };
    if global_config.user_auth.email_verification_enabled && !email_verified {
        return Err(error::ContainerError::from(UserAuthError::EmailNotVerified));
    }

    let valid = auth::verify_password(&payload.password, &user.password_hash)
        .map_err(|_| UserAuthError::StorageError)?;
    if !valid {
        return Err(error::ContainerError::from(UserAuthError::InvalidPassword));
    }

    let token = auth::generate_jwt(
        &user.user_id,
        &user.email,
        &user.merchant_id,
        &user.role,
        &global_config.user_auth.jwt_secret,
        global_config.user_auth.jwt_expiry_seconds,
    )
    .map_err(|_| UserAuthError::TokenGenerationFailed)?;

    Ok(Json(AuthResponse {
        token,
        user_id: user.user_id,
        email: user.email,
        merchant_id: user.merchant_id,
        role: user.role,
    }))
}

#[axum::debug_handler]
pub async fn logout(
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, error::ContainerError<UserAuthError>> {
    let token = extract_bearer_token(&headers)?;
    let global_config = APP_STATE
        .get()
        .map(|s| s.global_config.clone())
        .ok_or(UserAuthError::StorageError)?;

    let claims = auth::verify_jwt(token, &global_config.user_auth.jwt_secret)
        .map_err(|_| UserAuthError::InvalidToken)?;

    let app_state = get_tenant_app_state().await;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| UserAuthError::StorageError)?
        .as_secs();
    let remaining_ttl = claims.exp.saturating_sub(now) as i64;

    if remaining_ttl > 0 {
        let deny_key = format!("{}{}", JWT_DENYLIST_PREFIX, claims.jti);
        let _ = app_state
            .redis_conn
            .set_key_with_ttl(&deny_key, "1", remaining_ttl)
            .await;
    }

    Ok(Json(serde_json::json!({ "message": "Logged out successfully" })))
}

#[axum::debug_handler]
pub async fn me(
    headers: HeaderMap,
) -> Result<Json<MeResponse>, error::ContainerError<UserAuthError>> {
    let token = extract_bearer_token(&headers)?;
    let global_config = APP_STATE
        .get()
        .map(|s| s.global_config.clone())
        .ok_or(UserAuthError::StorageError)?;

    let claims = verify_jwt_not_revoked(token, &global_config.user_auth.jwt_secret).await?;

    let app_state = get_tenant_app_state().await;
    let mut users = crate::generics::generic_find_all::<
        <User as HasTable>::Table,
        _,
        User,
    >(&app_state.db, dsl::user_id.eq(claims.user_id.clone()))
    .await
    .map_err(|_| UserAuthError::StorageError)?;

    let user = users.pop().ok_or(UserAuthError::UserNotFound)?;

    Ok(Json(MeResponse {
        user_id: user.user_id,
        email: user.email,
        merchant_id: user.merchant_id,
        role: user.role,
        #[cfg(feature = "mysql")]
        email_verified: user.email_verified != 0,
        #[cfg(feature = "postgres")]
        email_verified: user.email_verified,
    }))
}

fn extract_bearer_token(headers: &HeaderMap) -> Result<&str, error::ContainerError<UserAuthError>> {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| error::ContainerError::from(UserAuthError::InvalidToken))
}

pub async fn verify_jwt_not_revoked(
    token: &str,
    secret: &str,
) -> Result<auth::JwtClaims, UserAuthError> {
    let claims = auth::verify_jwt(token, secret).map_err(|_| UserAuthError::InvalidToken)?;

    let app_state = get_tenant_app_state().await;
    let deny_key = format!("{}{}", JWT_DENYLIST_PREFIX, claims.jti);
    if let Ok(val) = app_state.redis_conn.get_key_string(&deny_key).await {
        if !val.is_empty() {
            return Err(UserAuthError::InvalidToken);
        }
    }

    Ok(claims)
}
