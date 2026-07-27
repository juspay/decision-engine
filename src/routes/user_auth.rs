use crate::app::{get_tenant_app_state, APP_STATE};
use crate::auth;
use crate::error::{self, ContainerError, ResultContainerExt, UserAuthError};
use crate::storage::types::{
    MerchantAccountNew, NewUser, NewUserMerchant, User, UserEmailVerifiedUpdate, UserMerchant,
    UserMerchantIdUpdate,
};
use crate::utils::date_time;
use axum::extract::Query;
use axum::http::HeaderMap;
use axum::Json;
use diesel::associations::HasTable;
use diesel::{BoolExpressionMethods, ExpressionMethods};
use error_stack::ResultExt;
use serde::{Deserialize, Serialize};

#[cfg(feature = "mysql")]
use crate::storage::schema::users::dsl;
#[cfg(feature = "postgres")]
use crate::storage::schema_pg::users::dsl;

#[cfg(feature = "mysql")]
use crate::storage::schema::user_merchants::dsl as um_dsl;
#[cfg(feature = "postgres")]
use crate::storage::schema_pg::user_merchants::dsl as um_dsl;

const JWT_DENYLIST_PREFIX: &str = "jwt_revoked:";

#[derive(Debug, Deserialize)]
pub struct SignupRequest {
    pub email: String,
    pub password: String,
    pub merchant_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateMerchantRequest {
    pub merchant_name: String,
}

#[derive(Debug, Deserialize)]
pub struct SwitchMerchantRequest {
    pub merchant_id: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct MerchantInfo {
    pub merchant_id: String,
    pub merchant_name: String,
    pub role: String,
}

#[derive(Debug, Serialize)]
pub struct AuthResponse {
    pub token: String,
    pub user_id: String,
    pub email: String,
    pub merchant_id: String,
    pub role: String,
    pub merchants: Vec<MerchantInfo>,
}

#[derive(Debug, Serialize)]
pub struct MeResponse {
    pub user_id: String,
    pub email: String,
    pub merchant_id: String,
    pub role: String,
    pub email_verified: bool,
    pub merchants: Vec<MerchantInfo>,
}

#[derive(Debug, Serialize)]
pub struct CreateMerchantResponse {
    pub token: String,
    pub merchant_id: String,
    pub merchant_name: String,
    pub merchants: Vec<MerchantInfo>,
}

const EMAIL_VERIFICATION_PREFIX: &str = "email_verification:";
const EMAIL_VERIFICATION_TTL_SECONDS: i64 = 86400; // 24 hours
const PENDING_SIGNUP_PREFIX: &str = "pending_signup:";
const PENDING_SIGNUP_TTL_SECONDS: i64 = 300; // 5 minutes

/// One-time password-reset codes. Stored by SHA-256 of the code (never the code itself),
/// so a leaked Redis snapshot cannot yield live reset links. Single-use via atomic DEL.
const PASSWORD_RESET_CODE_PREFIX: &str = "password_reset_code:";
const PASSWORD_RESET_CODE_TTL_SECONDS: i64 = 1800; // 30 minutes

/// Unix timestamp of the user's last password change/reset. Any JWT minted before it is
/// rejected in `verify_jwt_not_revoked`; the key self-expires once all such tokens would
/// have expired anyway.
const PASSWORD_RESET_AT_PREFIX: &str = "user_pwreset_at:";

const FORGOT_PASSWORD_RATE_PREFIX: &str = "forgot_password_rate:";
const FORGOT_PASSWORD_RATE_WINDOW_SECONDS: i64 = 3600;
const FORGOT_PASSWORD_RATE_MAX_PER_EMAIL: i64 = 3;
const FORGOT_PASSWORD_RATE_MAX_PER_IP: i64 = 30;

#[axum::debug_handler]
pub async fn signup(
    Json(payload): Json<SignupRequest>,
) -> Result<Json<SignupResponse>, error::ContainerError<UserAuthError>> {
    let app_state = get_tenant_app_state().await;
    let global_config = APP_STATE
        .get()
        .map(|s| s.global_config.clone())
        .ok_or(UserAuthError::StorageError)?;

    let existing = crate::generics::generic_find_all::<<User as HasTable>::Table, _, User>(
        &app_state.db,
        dsl::email.eq(payload.email.clone()),
    )
    .await
    .change_error(UserAuthError::StorageError)?;

    if !existing.is_empty() {
        return Err(error::ContainerError::from(
            UserAuthError::EmailAlreadyExists,
        ));
    }

    auth::validate_password_strength(&payload.password)
        .change_context(UserAuthError::WeakPassword)?;

    let password_hash = auth::hash_password(&payload.password)
        .change_context(UserAuthError::PasswordHashingFailed)?;

    let user_id = uuid::Uuid::new_v4().to_string();
    let now = date_time::now();

    let requested_merchant_id = payload
        .merchant_id
        .as_ref()
        .map(|merchant_id| merchant_id.trim())
        .filter(|merchant_id| !merchant_id.is_empty())
        .map(str::to_string);

    if let Some(merchant_id) = requested_merchant_id.as_ref() {
        #[cfg(feature = "mysql")]
        use crate::storage::schema::merchant_account::dsl as ma_dsl;
        #[cfg(feature = "postgres")]
        use crate::storage::schema_pg::merchant_account::dsl as ma_dsl;

        let existing_merchant = crate::generics::generic_find_all::<
            <crate::storage::types::MerchantAccount as HasTable>::Table,
            _,
            crate::storage::types::MerchantAccount,
        >(
            &app_state.db,
            ma_dsl::merchant_id.eq(Some(merchant_id.clone())),
        )
        .await
        .change_error(UserAuthError::StorageError)?;

        if existing_merchant.is_empty() {
            return Err(error::ContainerError::from(UserAuthError::MerchantNotFound));
        }
    }

    if global_config.user_auth.email_verification_enabled {
        let token = uuid::Uuid::new_v4().to_string();
        let redis_key = format!("{}{}", EMAIL_VERIFICATION_PREFIX, token);
        let pending_key = format!("{}{}", PENDING_SIGNUP_PREFIX, payload.email);

        let verification_url = format!(
            "{}/verify-email?token={}",
            global_config.email.base_url, token
        );

        // Acquire a short-lived NX lock to prevent concurrent duplicate signups
        // for the same email from each racing past the DB uniqueness check above.
        let acquired = app_state
            .redis_conn
            .set_key_if_not_exists(&pending_key, "1", PENDING_SIGNUP_TTL_SECONDS)
            .await
            .change_context(UserAuthError::StorageError)?;
        if !acquired {
            return Err(error::ContainerError::from(
                UserAuthError::EmailAlreadyExists,
            ));
        }

        let email_client = APP_STATE
            .get()
            .map(|s| s.email_client.clone())
            .ok_or(UserAuthError::StorageError)?;

        // Send the email before any DB or Redis writes so that a delivery failure
        // leaves no records behind and the user can retry registration immediately.
        let send_result = email_client
            .send_email(
                crate::email::templates::EmailVerificationTemplate {
                    user_email: payload.email.clone(),
                    verification_url,
                }
                .into_message(),
            )
            .await;
        if send_result.is_err() {
            let _ = app_state.redis_conn.delete_key(&pending_key).await;
        }
        send_result.change_context(UserAuthError::EmailSendFailed)?;

        // Write the verification token before creating DB records so that if a
        // DB insert fails the token does not reference a non-existent user.
        let token_result = app_state
            .redis_conn
            .set_key_with_ttl(&redis_key, &user_id, EMAIL_VERIFICATION_TTL_SECONDS)
            .await;
        if token_result.is_err() {
            let _ = app_state.redis_conn.delete_key(&pending_key).await;
        }
        token_result.change_context(UserAuthError::StorageError)?;

        let new_user = NewUser {
            user_id: user_id.clone(),
            email: payload.email.clone(),
            password_hash,
            merchant_id: requested_merchant_id.clone(),
            role: "admin".to_string(),
            #[cfg(feature = "mysql")]
            is_active: 1,
            #[cfg(feature = "postgres")]
            is_active: true,
            #[cfg(feature = "mysql")]
            email_verified: 0,
            #[cfg(feature = "postgres")]
            email_verified: false,
            created_at: now,
        };

        let user_insert_result = crate::generics::generic_insert(&app_state.db, new_user).await;
        if user_insert_result.is_err() {
            let _ = app_state.redis_conn.delete_key(&redis_key).await;
            let _ = app_state.redis_conn.delete_key(&pending_key).await;
        }
        user_insert_result.change_context(UserAuthError::StorageError)?;

        if let Some(merchant_id) = requested_merchant_id.as_ref() {
            let new_user_merchant = NewUserMerchant {
                user_id: user_id.clone(),
                merchant_id: merchant_id.clone(),
                role: "admin".to_string(),
                created_at: now,
            };

            let merchant_insert_result =
                crate::generics::generic_insert(&app_state.db, new_user_merchant).await;
            if merchant_insert_result.is_err() {
                let conn = app_state.db.get_conn().await.ok();
                if let Some(conn) = conn {
                    let _ = crate::generics::generic_delete::<
                        <User as diesel::associations::HasTable>::Table,
                        _,
                    >(&conn, dsl::user_id.eq(user_id.clone()))
                    .await;
                }
                let _ = app_state.redis_conn.delete_key(&redis_key).await;
                let _ = app_state.redis_conn.delete_key(&pending_key).await;
            }
            merchant_insert_result.change_context(UserAuthError::StorageError)?;
        }

        let _ = app_state.redis_conn.delete_key(&pending_key).await;

        return Ok(Json(SignupResponse::VerificationPending(
            SignupVerificationPendingResponse {
                message: "Account created. Please check your email to verify your address before logging in.".to_string(),
                email_verification_required: true,
            },
        )));
    }

    let new_user = NewUser {
        user_id: user_id.clone(),
        email: payload.email.clone(),
        password_hash,
        merchant_id: requested_merchant_id.clone(),
        role: "admin".to_string(),
        #[cfg(feature = "mysql")]
        is_active: 1,
        #[cfg(feature = "postgres")]
        is_active: true,
        #[cfg(feature = "mysql")]
        email_verified: 1,
        #[cfg(feature = "postgres")]
        email_verified: true,
        created_at: now,
    };

    crate::generics::generic_insert(&app_state.db, new_user)
        .await
        .change_context(UserAuthError::StorageError)?;

    if let Some(merchant_id) = requested_merchant_id.as_ref() {
        let new_user_merchant = NewUserMerchant {
            user_id: user_id.clone(),
            merchant_id: merchant_id.clone(),
            role: "admin".to_string(),
            created_at: now,
        };

        crate::generics::generic_insert(&app_state.db, new_user_merchant)
            .await
            .change_context(UserAuthError::StorageError)?;
    }

    let token = auth::generate_jwt(
        &user_id,
        &payload.email,
        requested_merchant_id.as_deref().unwrap_or(""),
        "admin",
        &global_config.user_auth.jwt_secret,
        global_config.user_auth.jwt_expiry_seconds,
    )
    .change_context(UserAuthError::TokenGenerationFailed)?;

    let merchants = fetch_user_merchants(&app_state, &user_id).await?;

    Ok(Json(SignupResponse::Authenticated(AuthResponse {
        token,
        user_id,
        email: payload.email,
        merchant_id: requested_merchant_id.unwrap_or_default(),
        role: "admin".to_string(),
        merchants,
    })))
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

    let mut users = crate::generics::generic_find_all::<<User as HasTable>::Table, _, User>(
        &app_state.db,
        dsl::email.eq(payload.email.clone()),
    )
    .await
    .change_error(UserAuthError::StorageError)?;

    let user = users.pop().ok_or(UserAuthError::UserNotFound)?;

    let is_active = {
        #[cfg(feature = "mysql")]
        {
            user.is_active != 0
        }
        #[cfg(feature = "postgres")]
        {
            user.is_active
        }
    };
    if !is_active {
        return Err(error::ContainerError::from(UserAuthError::AccountInactive));
    }

    let email_verified = {
        #[cfg(feature = "mysql")]
        {
            user.email_verified != 0
        }
        #[cfg(feature = "postgres")]
        {
            user.email_verified
        }
    };
    if global_config.user_auth.email_verification_enabled && !email_verified {
        return Err(error::ContainerError::from(UserAuthError::EmailNotVerified));
    }

    if !auth::verify_password(&payload.password, &user.password_hash)
        .change_context(UserAuthError::StorageError)?
    {
        return Err(error::ContainerError::from(UserAuthError::InvalidPassword));
    }

    let merchants = fetch_user_merchants(&app_state, &user.user_id).await?;
    let active_merchant_id = user.merchant_id.clone().unwrap_or_else(|| {
        merchants
            .first()
            .map(|m| m.merchant_id.clone())
            .unwrap_or_default()
    });

    let token = auth::generate_jwt(
        &user.user_id,
        &user.email,
        &active_merchant_id,
        &user.role,
        &global_config.user_auth.jwt_secret,
        global_config.user_auth.jwt_expiry_seconds,
    )
    .change_context(UserAuthError::TokenGenerationFailed)?;

    Ok(Json(AuthResponse {
        token,
        user_id: user.user_id,
        email: user.email,
        merchant_id: active_merchant_id,
        role: user.role,
        merchants,
    }))
}

#[axum::debug_handler]
pub async fn create_merchant(
    headers: HeaderMap,
    Json(payload): Json<CreateMerchantRequest>,
) -> Result<Json<CreateMerchantResponse>, error::ContainerError<UserAuthError>> {
    let token = extract_bearer_token(&headers)?;
    let global_config = APP_STATE
        .get()
        .map(|s| s.global_config.clone())
        .ok_or(UserAuthError::StorageError)?;

    let claims = verify_jwt_not_revoked(token, &global_config.user_auth.jwt_secret).await?;
    let app_state = get_tenant_app_state().await;

    let merchant_id = format!(
        "merchant_{}",
        &uuid::Uuid::new_v4().to_string().replace('-', "")[..12]
    );
    let now = date_time::now();

    let new_merchant = MerchantAccountNew {
        merchant_id: Some(merchant_id.clone()),
        merchant_name: Some(payload.merchant_name.clone()),
        date_created: now,
        use_code_for_gateway_priority: crate::storage::types::BitBoolWrite(false),
        gateway_success_rate_based_decider_input: None,
        internal_metadata: None,
        enabled: crate::storage::types::BitBoolWrite(true),
    };

    crate::generics::generic_insert(&app_state.db, new_merchant)
        .await
        .change_context(UserAuthError::StorageError)?;

    let new_user_merchant = NewUserMerchant {
        user_id: claims.user_id.clone(),
        merchant_id: merchant_id.clone(),
        role: "admin".to_string(),
        created_at: now,
    };

    crate::generics::generic_insert(&app_state.db, new_user_merchant)
        .await
        .change_context(UserAuthError::StorageError)?;

    // Update users.merchant_id to the newly created merchant
    {
        #[cfg(feature = "mysql")]
        use crate::storage::schema::users::dsl as u_dsl;
        #[cfg(feature = "postgres")]
        use crate::storage::schema_pg::users::dsl as u_dsl;

        let conn = &app_state
            .db
            .get_conn()
            .await
            .change_error(UserAuthError::StorageError)?;
        crate::generics::generic_update_if_present::<
            <User as diesel::associations::HasTable>::Table,
            UserMerchantIdUpdate,
            _,
        >(
            conn,
            u_dsl::user_id.eq(claims.user_id.clone()),
            UserMerchantIdUpdate {
                merchant_id: Some(merchant_id.clone()),
            },
        )
        .await
        .change_context(UserAuthError::StorageError)?;
    }

    let merchants = fetch_user_merchants(&app_state, &claims.user_id).await?;

    let new_token = auth::generate_jwt(
        &claims.user_id,
        &claims.email,
        &merchant_id,
        &claims.role,
        &global_config.user_auth.jwt_secret,
        global_config.user_auth.jwt_expiry_seconds,
    )
    .change_context(UserAuthError::TokenGenerationFailed)?;

    Ok(Json(CreateMerchantResponse {
        token: new_token,
        merchant_id,
        merchant_name: payload.merchant_name,
        merchants,
    }))
}

#[axum::debug_handler]
pub async fn list_merchants(
    headers: HeaderMap,
) -> Result<Json<Vec<MerchantInfo>>, error::ContainerError<UserAuthError>> {
    let token = extract_bearer_token(&headers)?;
    let global_config = APP_STATE
        .get()
        .map(|s| s.global_config.clone())
        .ok_or(UserAuthError::StorageError)?;

    let claims = verify_jwt_not_revoked(token, &global_config.user_auth.jwt_secret).await?;
    let app_state = get_tenant_app_state().await;

    let merchants = fetch_user_merchants(&app_state, &claims.user_id).await?;
    Ok(Json(merchants))
}

#[axum::debug_handler]
pub async fn switch_merchant(
    headers: HeaderMap,
    Json(payload): Json<SwitchMerchantRequest>,
) -> Result<Json<AuthResponse>, error::ContainerError<UserAuthError>> {
    let token = extract_bearer_token(&headers)?;
    let global_config = APP_STATE
        .get()
        .map(|s| s.global_config.clone())
        .ok_or(UserAuthError::StorageError)?;

    let claims = verify_jwt_not_revoked(token, &global_config.user_auth.jwt_secret).await?;
    let app_state = get_tenant_app_state().await;

    let merchants = fetch_user_merchants(&app_state, &claims.user_id).await?;
    let target = merchants
        .iter()
        .find(|m| m.merchant_id == payload.merchant_id)
        .ok_or_else(|| error::ContainerError::from(UserAuthError::MerchantNotFound))?;

    let new_token = auth::generate_jwt(
        &claims.user_id,
        &claims.email,
        &target.merchant_id,
        &target.role,
        &global_config.user_auth.jwt_secret,
        global_config.user_auth.jwt_expiry_seconds,
    )
    .change_context(UserAuthError::TokenGenerationFailed)?;

    Ok(Json(AuthResponse {
        token: new_token,
        user_id: claims.user_id,
        email: claims.email,
        merchant_id: target.merchant_id.clone(),
        role: target.role.clone(),
        merchants,
    }))
}

#[derive(Debug, Deserialize)]
pub struct ChangePasswordRequest {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Serialize)]
pub struct ChangePasswordResponse {
    pub message: String,
    pub token: String,
}

#[derive(Debug, Deserialize)]
pub struct ForgotPasswordRequest {
    pub email: String,
}

#[derive(Debug, Serialize)]
pub struct ForgotPasswordResponse {
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct ResetPasswordRequest {
    pub token: String,
    pub new_password: String,
}

#[derive(Debug, Serialize)]
pub struct ResetPasswordResponse {
    pub message: String,
}

#[axum::debug_handler]
pub async fn change_password(
    headers: HeaderMap,
    Json(payload): Json<ChangePasswordRequest>,
) -> Result<Json<ChangePasswordResponse>, error::ContainerError<UserAuthError>> {
    let token = extract_bearer_token(&headers)?;
    let global_config = APP_STATE
        .get()
        .map(|s| s.global_config.clone())
        .ok_or(UserAuthError::StorageError)?;

    let claims = verify_jwt_not_revoked(token, &global_config.user_auth.jwt_secret).await?;

    let app_state = get_tenant_app_state().await;

    let mut users = crate::generics::generic_find_all::<<User as HasTable>::Table, _, User>(
        &app_state.db,
        dsl::user_id.eq(claims.sub.clone()),
    )
    .await
    .change_error(UserAuthError::StorageError)?;

    let user = users.pop().ok_or(UserAuthError::UserNotFound)?;

    if !auth::verify_password(&payload.current_password, &user.password_hash)
        .change_context(UserAuthError::StorageError)?
    {
        return Err(error::ContainerError::from(UserAuthError::InvalidPassword));
    }

    auth::validate_password_strength(&payload.new_password)
        .change_context(UserAuthError::WeakPassword)?;

    let new_hash = auth::hash_password(&payload.new_password)
        .change_context(UserAuthError::PasswordHashingFailed)?;

    // Revoke every session minted before this change — fail-closed and BEFORE the DB
    // update: if Redis is down nothing has changed and the user retries; if the DB update
    // below fails, sessions were revoked but the password is unchanged, which is safe.
    // The fresh token is minted after, so its iat never precedes the cutoff.
    record_password_reset_timestamp(&app_state, &claims.user_id, &global_config).await?;

    let conn = app_state
        .db
        .get_conn()
        .await
        .change_error(UserAuthError::StorageError)?;

    crate::generics::generic_update_if_present::<
        <User as HasTable>::Table,
        crate::storage::types::UserPasswordUpdate,
        _,
    >(
        &conn,
        dsl::user_id.eq(claims.sub),
        crate::storage::types::UserPasswordUpdate {
            password_hash: new_hash,
        },
    )
    .await
    .change_context(UserAuthError::StorageError)?;

    let new_token = auth::generate_jwt(
        &claims.user_id,
        &claims.email,
        &claims.merchant_id,
        &claims.role,
        &global_config.user_auth.jwt_secret,
        global_config.user_auth.jwt_expiry_seconds,
    )
    .change_context(UserAuthError::TokenGenerationFailed)?;

    Ok(Json(ChangePasswordResponse {
        message: "Password updated successfully.".to_string(),
        token: new_token,
    }))
}

/// Record "passwords changed at" so `verify_jwt_not_revoked` rejects all JWTs minted
/// earlier and `reset_password` rejects reset codes issued earlier. Fail-closed: a
/// password change must not report success while old sessions remain revocable-in-name-
/// only, so a Redis write failure is surfaced as an error (the read side stays fail-open,
/// matching the jti denylist). The key outlives both every pre-change JWT and every
/// outstanding reset code.
async fn record_password_reset_timestamp(
    app_state: &crate::app::TenantAppState,
    user_id: &str,
    global_config: &crate::config::GlobalConfig,
) -> Result<(), error::ContainerError<UserAuthError>> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| error::ContainerError::from(UserAuthError::StorageError))?;
    let reset_at_key = format!("{}{}", PASSWORD_RESET_AT_PREFIX, user_id);
    let ttl = std::cmp::max(
        global_config.user_auth.jwt_expiry_seconds as i64,
        PASSWORD_RESET_CODE_TTL_SECONDS,
    );
    if let Err(err) = app_state
        .redis_conn
        .set_key_with_ttl(&reset_at_key, &now.as_secs().to_string(), ttl)
        .await
    {
        crate::logger::warn!(
            error = ?err,
            "Failed to record password change timestamp; refusing to complete the change"
        );
        return Err(error::ContainerError::from(UserAuthError::StorageError));
    }
    Ok(())
}

/// INCR-first fixed-window counter; returns true when the bucket is over `max`. The
/// window TTL is attached when this INCR created the key (count == 1); if attaching it
/// fails the counter is deleted so a TTL-less key can never lock a bucket out
/// permanently. Redis errors fail open (never throttle).
async fn is_forgot_password_rate_limited(
    app_state: &crate::app::TenantAppState,
    key: &str,
    max: i64,
) -> bool {
    let Ok(count) = app_state.redis_conn.increment_key(key).await else {
        return false;
    };
    if count == 1 {
        if let Err(err) = app_state
            .redis_conn
            .expire_key(key, FORGOT_PASSWORD_RATE_WINDOW_SECONDS)
            .await
        {
            let _ = app_state.redis_conn.delete_key(key).await;
            crate::logger::warn!(error = ?err, "Failed to set forgot-password rate-limit TTL");
        }
    }
    count > max
}

#[axum::debug_handler]
pub async fn forgot_password(
    headers: HeaderMap,
    Json(payload): Json<ForgotPasswordRequest>,
) -> Result<Json<ForgotPasswordResponse>, error::ContainerError<UserAuthError>> {
    let app_state = get_tenant_app_state().await;
    let global_config = APP_STATE
        .get()
        .map(|s| s.global_config.clone())
        .ok_or(UserAuthError::StorageError)?;
    let email_client = APP_STATE
        .get()
        .map(|s| s.email_client.clone())
        .ok_or(UserAuthError::StorageError)?;

    // Every path below returns this same body — the response must never reveal whether
    // the address has an account.
    let generic_response = || {
        Json(ForgotPasswordResponse {
            message: "If an account exists for that email, a password reset link has been sent."
                .to_string(),
        })
    };

    let email = payload.email.trim().to_string();
    if email.is_empty()
        || email.len() > 254
        || !email.contains('@')
        || email.contains(char::is_whitespace)
    {
        return Ok(generic_response());
    }

    // Rate limit per email and per client IP. Counters advance for existing and
    // unknown emails alike so limiter behavior leaks nothing about account existence.
    let email_rate_key = format!(
        "{}email:{}",
        FORGOT_PASSWORD_RATE_PREFIX,
        auth::hash_api_key(&email.to_lowercase())
    );
    if is_forgot_password_rate_limited(&app_state, &email_rate_key, FORGOT_PASSWORD_RATE_MAX_PER_EMAIL)
        .await
    {
        return Ok(generic_response());
    }

    // Only the RIGHTMOST x-forwarded-for entry is trustworthy — it is the hop appended by
    // our own reverse proxy; everything left of it is client-supplied. Parsing as an IP
    // rejects forged free-text so arbitrary header bytes can't become Redis keys.
    let client_ip = headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.rsplit(',').next())
        .and_then(|value| value.trim().parse::<std::net::IpAddr>().ok())
        .or_else(|| {
            headers
                .get("x-real-ip")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.trim().parse::<std::net::IpAddr>().ok())
        });
    if let Some(ip) = client_ip {
        let ip_rate_key = format!("{}ip:{}", FORGOT_PASSWORD_RATE_PREFIX, ip);
        if is_forgot_password_rate_limited(&app_state, &ip_rate_key, FORGOT_PASSWORD_RATE_MAX_PER_IP)
            .await
        {
            return Ok(generic_response());
        }
    }

    let mut users = crate::generics::generic_find_all::<<User as HasTable>::Table, _, User>(
        &app_state.db,
        dsl::email.eq(email.clone()),
    )
    .await
    .change_error(UserAuthError::StorageError)?;

    let Some(user) = users.pop() else {
        return Ok(generic_response());
    };

    let is_active = {
        #[cfg(feature = "mysql")]
        {
            user.is_active != 0
        }
        #[cfg(feature = "postgres")]
        {
            user.is_active
        }
    };
    if !is_active {
        return Ok(generic_response());
    }
    // Unverified-but-active users DO get the email: completing a reset proves mailbox
    // control (it sets email_verified), so this is also the escape hatch for users who
    // lost their verification email.

    // Issue the code and send the email off the request path: the SES round trip and any
    // failure it produces must not be observable in the response, or they become an
    // account-existence oracle (latency, or a 500 only the user-exists branch can hit).
    let base_url = global_config.email.base_url.clone();
    let bg_state = app_state.clone();
    tokio::spawn(async move {
        let code = auth::generate_api_key();
        let code_key = format!("{}{}", PASSWORD_RESET_CODE_PREFIX, auth::hash_api_key(&code));
        // The value carries the issue time so redemption can reject codes issued before
        // the user's last password change/reset (see `reset_password`).
        let issued_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let code_value = format!("{}:{}", user.user_id, issued_at);
        if let Err(err) = bg_state
            .redis_conn
            .set_key_with_ttl(&code_key, &code_value, PASSWORD_RESET_CODE_TTL_SECONDS)
            .await
        {
            crate::logger::warn!(error = ?err, "Failed to store password reset code");
            return;
        }

        let reset_url = format!("{}/reset-password?token={}", base_url, code);
        if let Err(err) = email_client
            .send_email(
                crate::email::templates::PasswordResetTemplate {
                    user_email: user.email.clone(),
                    reset_url,
                }
                .into_message(),
            )
            .await
        {
            crate::logger::warn!(
                to = %user.email,
                error = ?err,
                "Failed to send password reset email"
            );
            let _ = bg_state.redis_conn.delete_key(&code_key).await;
        }
    });

    Ok(generic_response())
}

#[axum::debug_handler]
pub async fn reset_password(
    Json(payload): Json<ResetPasswordRequest>,
) -> Result<Json<ResetPasswordResponse>, error::ContainerError<UserAuthError>> {
    let app_state = get_tenant_app_state().await;
    let global_config = APP_STATE
        .get()
        .map(|s| s.global_config.clone())
        .ok_or(UserAuthError::StorageError)?;

    // Validate strength before touching the code — a weak password must not burn the
    // single-use link.
    auth::validate_password_strength(&payload.new_password)
        .change_context(UserAuthError::WeakPassword)?;

    let code_key = format!(
        "{}{}",
        PASSWORD_RESET_CODE_PREFIX,
        auth::hash_api_key(&payload.token)
    );
    // A missing/expired/unknown code reads as a failure — treat any failure to read it as
    // an invalid code, not a server error.
    let stored_value = match app_state.redis_conn.get_key_string(&code_key).await {
        Ok(value) if !value.is_empty() => value,
        _ => return Err(error::ContainerError::from(UserAuthError::InvalidResetToken)),
    };

    // Atomically claim the code by deleting it. `DEL` reports whether *this* call removed
    // the key, so with two concurrent redemptions exactly one sees KeyDeleted — the other
    // (already consumed, expired, or replayed) is rejected.
    let claimed = app_state
        .redis_conn
        .delete_key(&code_key)
        .await
        .change_context(UserAuthError::StorageError)?;
    if !matches!(claimed, redis_interface::types::DelReply::KeyDeleted) {
        return Err(error::ContainerError::from(UserAuthError::InvalidResetToken));
    }

    // Value format: "<user_id>:<issued_at_unix>" (user_ids are UUIDs, so the last colon
    // is unambiguous).
    let Some((user_id, issued_at)) = stored_value
        .rsplit_once(':')
        .and_then(|(id, ts)| ts.parse::<u64>().ok().map(|ts| (id.to_string(), ts)))
    else {
        return Err(error::ContainerError::from(UserAuthError::InvalidResetToken));
    };

    // Reject codes issued before the user's last password change/reset — once the
    // password changes, every previously emailed link must die with it. Strict `<`
    // mirrors the JWT iat check; fail-open on Redis read errors, like that check.
    let reset_at_key = format!("{}{}", PASSWORD_RESET_AT_PREFIX, user_id);
    if let Ok(val) = app_state.redis_conn.get_key_string(&reset_at_key).await {
        if let Ok(reset_at) = val.parse::<u64>() {
            if issued_at < reset_at {
                return Err(error::ContainerError::from(UserAuthError::InvalidResetToken));
            }
        }
    }

    // Re-check the account at redemption time — it may have been deactivated during the
    // code's 30-minute lifetime. Report inactive/missing identically to a bad code.
    let mut users = crate::generics::generic_find_all::<<User as HasTable>::Table, _, User>(
        &app_state.db,
        dsl::user_id.eq(user_id.clone()),
    )
    .await
    .change_error(UserAuthError::StorageError)?;
    let Some(user) = users.pop() else {
        return Err(error::ContainerError::from(UserAuthError::InvalidResetToken));
    };
    let is_active = {
        #[cfg(feature = "mysql")]
        {
            user.is_active != 0
        }
        #[cfg(feature = "postgres")]
        {
            user.is_active
        }
    };
    if !is_active {
        return Err(error::ContainerError::from(UserAuthError::InvalidResetToken));
    }

    // No same-as-old-password check here, deliberately: the requester has proven mailbox
    // control, and rejecting reuse after the atomic claim would burn the single-use code
    // on a recoverable error while confirming the current password to whoever holds the
    // link. Checking before the claim would be worse — a non-consuming password oracle.
    let new_hash = auth::hash_password(&payload.new_password)
        .change_context(UserAuthError::PasswordHashingFailed)?;

    let conn = app_state
        .db
        .get_conn()
        .await
        .change_error(UserAuthError::StorageError)?;

    // Completing a reset proves control of the mailbox, so mark the email verified in
    // the same statement — this also unsticks users who never received the signup
    // verification email.
    crate::generics::generic_update_if_present::<
        <User as HasTable>::Table,
        crate::storage::types::UserPasswordResetUpdate,
        _,
    >(
        &conn,
        dsl::user_id.eq(user_id.clone()),
        crate::storage::types::UserPasswordResetUpdate {
            password_hash: new_hash,
            #[cfg(feature = "mysql")]
            email_verified: 1,
            #[cfg(feature = "postgres")]
            email_verified: true,
        },
    )
    .await
    .change_context(UserAuthError::StorageError)?;

    record_password_reset_timestamp(&app_state, &user_id, &global_config).await?;

    Ok(Json(ResetPasswordResponse {
        message: "Password reset successfully. You can now sign in with your new password."
            .to_string(),
    }))
}

#[derive(Debug, Deserialize)]
pub struct InviteMemberRequest {
    pub email: String,
    pub role: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct InviteMemberResponse {
    pub email: String,
    pub is_new_user: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    pub role: String,
}

#[derive(Debug, Serialize)]
pub struct MemberInfo {
    pub user_id: String,
    pub email: String,
    pub role: String,
}

/// Signup response — either authenticated (no verification needed) or pending verification
#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum SignupResponse {
    Authenticated(AuthResponse),
    VerificationPending(SignupVerificationPendingResponse),
}

#[derive(Debug, Serialize)]
pub struct SignupVerificationPendingResponse {
    pub message: String,
    pub email_verification_required: bool,
}

#[derive(Debug, Serialize)]
pub struct VerifyEmailResponse {
    pub message: String,
}

#[axum::debug_handler]
pub async fn invite_member(
    headers: HeaderMap,
    Json(payload): Json<InviteMemberRequest>,
) -> Result<Json<InviteMemberResponse>, error::ContainerError<UserAuthError>> {
    let token = extract_bearer_token(&headers)?;
    let global_config = APP_STATE
        .get()
        .map(|s| s.global_config.clone())
        .ok_or(UserAuthError::StorageError)?;

    let claims = verify_jwt_not_revoked(token, &global_config.user_auth.jwt_secret).await?;

    if claims.role != "admin" {
        return Err(error::ContainerError::from(UserAuthError::Forbidden));
    }

    let role = match payload.role.as_deref().unwrap_or("member") {
        "admin" => "admin".to_string(),
        _ => "member".to_string(),
    };

    let app_state = get_tenant_app_state().await;

    #[cfg(feature = "mysql")]
    use crate::storage::schema::merchant_account::dsl as ma_dsl;
    #[cfg(feature = "postgres")]
    use crate::storage::schema_pg::merchant_account::dsl as ma_dsl;

    let merchant_name = crate::generics::generic_find_all::<
        <crate::storage::types::MerchantAccount as HasTable>::Table,
        _,
        crate::storage::types::MerchantAccount,
    >(
        &app_state.db,
        ma_dsl::merchant_id.eq(Some(claims.merchant_id.clone())),
    )
    .await
    .change_error(UserAuthError::StorageError)?
    .into_iter()
    .next()
    .and_then(|m| m.merchant_name)
    .unwrap_or_else(|| claims.merchant_id.clone());

    let existing_users = crate::generics::generic_find_all::<<User as HasTable>::Table, _, User>(
        &app_state.db,
        dsl::email.eq(payload.email.clone()),
    )
    .await
    .change_error(UserAuthError::StorageError)?;

    let now = date_time::now();

    if let Some(existing_user) = existing_users.into_iter().next() {
        // Check if already a member
        let existing_membership = crate::generics::generic_find_all::<
            <UserMerchant as HasTable>::Table,
            _,
            UserMerchant,
        >(
            &app_state.db,
            um_dsl::user_id
                .eq(existing_user.user_id.clone())
                .and(um_dsl::merchant_id.eq(claims.merchant_id.clone())),
        )
        .await
        .change_error(UserAuthError::StorageError)?;

        if !existing_membership.is_empty() {
            return Err(error::ContainerError::from(UserAuthError::AlreadyMember));
        }

        let new_user_merchant = NewUserMerchant {
            user_id: existing_user.user_id.clone(),
            merchant_id: claims.merchant_id.clone(),
            role: role.clone(),
            created_at: now,
        };

        crate::generics::generic_insert(&app_state.db, new_user_merchant)
            .await
            .change_context(UserAuthError::StorageError)?;

        let email_config = &global_config.email;
        if email_config.is_active() {
            let email_client = APP_STATE
                .get()
                .map(|s| s.email_client.clone())
                .ok_or(UserAuthError::StorageError)?;

            let email_msg = crate::email::templates::MemberAddedTemplate {
                user_email: existing_user.email.clone(),
                merchant_name: merchant_name.clone(),
                base_url: email_config.base_url.clone(),
            }
            .into_message();

            if let Err(err) = email_client.send_email(email_msg).await {
                crate::logger::warn!(
                    to = %existing_user.email,
                    error = ?err,
                    "Failed to send member-added notification email"
                );
            }
        }

        Ok(Json(InviteMemberResponse {
            email: existing_user.email,
            is_new_user: false,
            password: None,
            role,
        }))
    } else {
        // Create new user with generated password
        let generated_password = generate_random_password();

        let password_hash = auth::hash_password(&generated_password)
            .change_context(UserAuthError::PasswordHashingFailed)?;

        let user_id = uuid::Uuid::new_v4().to_string();

        let new_user = NewUser {
            user_id: user_id.clone(),
            email: payload.email.clone(),
            password_hash,
            merchant_id: None,
            role: role.clone(),
            #[cfg(feature = "mysql")]
            is_active: 1,
            #[cfg(feature = "postgres")]
            is_active: true,
            #[cfg(feature = "mysql")]
            email_verified: 1,
            #[cfg(feature = "postgres")]
            email_verified: true,
            created_at: now,
        };

        crate::generics::generic_insert(&app_state.db, new_user)
            .await
            .change_context(UserAuthError::StorageError)?;

        let new_user_merchant = NewUserMerchant {
            user_id: user_id.clone(),
            merchant_id: claims.merchant_id.clone(),
            role: role.clone(),
            created_at: now,
        };

        if crate::generics::generic_insert(&app_state.db, new_user_merchant)
            .await
            .is_err()
        {
            // Compensating delete: remove orphaned user if membership insert fails
            let conn = app_state.db.get_conn().await.ok();
            if let Some(conn) = conn {
                let _ = crate::generics::generic_delete::<
                    <User as diesel::associations::HasTable>::Table,
                    _,
                >(&conn, dsl::user_id.eq(user_id.clone()))
                .await;
            }
            return Err(error::ContainerError::from(UserAuthError::StorageError));
        }

        let email_config = &global_config.email;
        if email_config.is_active() {
            let email_client = APP_STATE
                .get()
                .map(|s| s.email_client.clone())
                .ok_or(UserAuthError::StorageError)?;

            let email_msg = crate::email::templates::InviteUserTemplate {
                user_email: payload.email.clone(),
                merchant_name: merchant_name.clone(),
                temporary_password: generated_password.clone(),
                base_url: email_config.base_url.clone(),
            }
            .into_message();

            if let Err(err) = email_client.send_email(email_msg).await {
                crate::logger::warn!(
                    to = %payload.email,
                    error = ?err,
                    "Failed to send invite email; invite still succeeded"
                );
            }
        }

        Ok(Json(InviteMemberResponse {
            email: payload.email,
            is_new_user: true,
            password: Some(generated_password),
            role,
        }))
    }
}

#[axum::debug_handler]
pub async fn list_members(
    headers: HeaderMap,
) -> Result<Json<Vec<MemberInfo>>, error::ContainerError<UserAuthError>> {
    let token = extract_bearer_token(&headers)?;
    let global_config = APP_STATE
        .get()
        .map(|s| s.global_config.clone())
        .ok_or(UserAuthError::StorageError)?;

    let claims = verify_jwt_not_revoked(token, &global_config.user_auth.jwt_secret).await?;
    let app_state = get_tenant_app_state().await;

    let memberships =
        crate::generics::generic_find_all::<<UserMerchant as HasTable>::Table, _, UserMerchant>(
            &app_state.db,
            um_dsl::merchant_id.eq(claims.merchant_id.clone()),
        )
        .await
        .change_error(UserAuthError::StorageError)?;

    let user_ids: Vec<String> = memberships.iter().map(|m| m.user_id.clone()).collect();

    let users = if user_ids.is_empty() {
        Vec::new()
    } else {
        crate::generics::generic_find_all::<<User as HasTable>::Table, _, User>(
            &app_state.db,
            dsl::user_id.eq_any(user_ids),
        )
        .await
        .change_error(UserAuthError::StorageError)?
    };

    let users_by_id: std::collections::HashMap<String, User> =
        users.into_iter().map(|u| (u.user_id.clone(), u)).collect();

    let members = memberships
        .into_iter()
        .filter_map(|membership| {
            users_by_id.get(&membership.user_id).map(|user| MemberInfo {
                user_id: user.user_id.clone(),
                email: user.email.clone(),
                role: membership.role,
            })
        })
        .collect();

    Ok(Json(members))
}

fn generate_random_password() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let uppercase = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let lowercase = b"abcdefghijklmnopqrstuvwxyz";
    let digits = b"0123456789";
    let special = b"!@#$%^&*";

    let mut password = vec![
        uppercase[rng.gen_range(0..uppercase.len())] as char,
        lowercase[rng.gen_range(0..lowercase.len())] as char,
        digits[rng.gen_range(0..digits.len())] as char,
        special[rng.gen_range(0..special.len())] as char,
    ];

    let all: Vec<u8> = [
        uppercase.as_ref(),
        lowercase.as_ref(),
        digits.as_ref(),
        special.as_ref(),
    ]
    .concat();
    for _ in 0..12 {
        password.push(all[rng.gen_range(0..all.len())] as char);
    }

    use rand::seq::SliceRandom;
    password.shuffle(&mut rng);
    password.into_iter().collect()
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
        .change_context(UserAuthError::InvalidToken)?;

    let app_state = get_tenant_app_state().await;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .change_error(UserAuthError::StorageError)?
        .as_secs();
    let remaining_ttl = claims.exp.saturating_sub(now) as i64;

    if remaining_ttl > 0 {
        let deny_key = format!("{}{}", JWT_DENYLIST_PREFIX, claims.jti);
        let _ = app_state
            .redis_conn
            .set_key_with_ttl(&deny_key, "1", remaining_ttl)
            .await;
    }

    Ok(Json(
        serde_json::json!({ "message": "Logged out successfully" }),
    ))
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

    let mut users = crate::generics::generic_find_all::<<User as HasTable>::Table, _, User>(
        &app_state.db,
        dsl::user_id.eq(claims.user_id.clone()),
    )
    .await
    .change_error(UserAuthError::StorageError)?;

    let user = users.pop().ok_or(UserAuthError::UserNotFound)?;
    let merchants = fetch_user_merchants(&app_state, &user.user_id).await?;

    Ok(Json(MeResponse {
        user_id: user.user_id,
        email: user.email,
        merchant_id: claims.merchant_id,
        role: user.role,
        #[cfg(feature = "mysql")]
        email_verified: user.email_verified != 0,
        #[cfg(feature = "postgres")]
        email_verified: user.email_verified,
        merchants,
    }))
}

async fn fetch_user_merchants(
    app_state: &crate::app::TenantAppState,
    user_id: &String,
) -> Result<Vec<MerchantInfo>, ContainerError<UserAuthError>> {
    #[cfg(feature = "mysql")]
    use crate::storage::schema::merchant_account::dsl as ma_dsl;
    #[cfg(feature = "postgres")]
    use crate::storage::schema_pg::merchant_account::dsl as ma_dsl;

    let user_merchant_rows = crate::generics::generic_find_all::<
        <UserMerchant as HasTable>::Table,
        _,
        UserMerchant,
    >(&app_state.db, um_dsl::user_id.eq(user_id.clone()))
    .await
    .change_error(UserAuthError::StorageError)?;

    let mut result = Vec::new();
    for um in user_merchant_rows {
        let mut accounts = crate::generics::generic_find_all::<
            <crate::storage::types::MerchantAccount as HasTable>::Table,
            _,
            crate::storage::types::MerchantAccount,
        >(
            &app_state.db,
            ma_dsl::merchant_id.eq(Some(um.merchant_id.clone())),
        )
        .await
        .change_error(UserAuthError::StorageError)?;

        let name = accounts
            .pop()
            .and_then(|a| a.merchant_name)
            .unwrap_or_else(|| um.merchant_id.clone());

        result.push(MerchantInfo {
            merchant_id: um.merchant_id,
            merchant_name: name,
            role: um.role,
        });
    }
    Ok(result)
}

#[derive(Debug, Deserialize)]
pub struct VerifyEmailQuery {
    pub token: String,
}

#[derive(Debug, Deserialize)]
pub struct VerifyEmailRequest {
    pub token: String,
}

/// POST /auth/verify-email — the SPA submits the token from the emailed link in the JSON
/// body, so the live token never appears in a backend URL (the request tracing span
/// records full URIs, query string included).
#[axum::debug_handler]
pub async fn verify_email(
    Json(payload): Json<VerifyEmailRequest>,
) -> Result<Json<VerifyEmailResponse>, error::ContainerError<UserAuthError>> {
    consume_verification_token(&payload.token).await
}

/// GET /auth/verify-email?token= — deprecated compatibility shim for SPA bundles cached
/// from before the POST variant existed; remove after one release. This path puts the
/// token in a logged URL, which is exactly what the POST variant exists to avoid.
#[axum::debug_handler]
pub async fn verify_email_get(
    Query(query): Query<VerifyEmailQuery>,
) -> Result<Json<VerifyEmailResponse>, error::ContainerError<UserAuthError>> {
    consume_verification_token(&query.token).await
}

async fn consume_verification_token(
    token: &str,
) -> Result<Json<VerifyEmailResponse>, error::ContainerError<UserAuthError>> {
    let app_state = get_tenant_app_state().await;

    let redis_key = format!("{}{}", EMAIL_VERIFICATION_PREFIX, token);

    // A missing/expired/unknown token reads as a failure — treat any failure to read it
    // as an invalid token, not a server error (same mapping as `reset_password`).
    let user_id = match app_state.redis_conn.get_key_string(&redis_key).await {
        Ok(user_id) if !user_id.is_empty() => user_id,
        _ => {
            return Err(error::ContainerError::from(
                UserAuthError::InvalidVerificationToken,
            ))
        }
    };

    // Atomically claim the token by deleting it BEFORE the DB write — with two concurrent
    // submissions exactly one sees KeyDeleted (same single-use pattern as
    // `reset_password`). If the DB update below then fails the token is burned; the user
    // is not stranded — completing a password reset also marks the email verified.
    let claimed = app_state
        .redis_conn
        .delete_key(&redis_key)
        .await
        .change_context(UserAuthError::StorageError)?;
    if !matches!(claimed, redis_interface::types::DelReply::KeyDeleted) {
        return Err(error::ContainerError::from(
            UserAuthError::InvalidVerificationToken,
        ));
    }

    let conn = app_state
        .db
        .get_conn()
        .await
        .change_error(UserAuthError::StorageError)?;

    let rows_updated = crate::generics::generic_update_if_present::<
        <User as HasTable>::Table,
        UserEmailVerifiedUpdate,
        _,
    >(
        &conn,
        dsl::user_id.eq(user_id.clone()),
        UserEmailVerifiedUpdate {
            #[cfg(feature = "mysql")]
            email_verified: 1,
            #[cfg(feature = "postgres")]
            email_verified: true,
        },
    )
    .await
    .change_context(UserAuthError::StorageError)?;

    if rows_updated == 0 {
        crate::logger::error!(user_id = %user_id, "Email verification update matched 0 rows — user_id not found in DB");
        return Err(error::ContainerError::from(
            UserAuthError::InvalidVerificationToken,
        ));
    }

    Ok(Json(VerifyEmailResponse {
        message: "Email verified successfully. You can now log in.".to_string(),
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
) -> Result<auth::JwtClaims, ContainerError<UserAuthError>> {
    let claims = auth::verify_jwt(token, secret).change_context(UserAuthError::InvalidToken)?;

    let app_state = get_tenant_app_state().await;
    let deny_key = format!("{}{}", JWT_DENYLIST_PREFIX, claims.jti);
    if let Ok(val) = app_state.redis_conn.get_key_string(&deny_key).await {
        if !val.is_empty() {
            return Err(ContainerError::from(UserAuthError::InvalidToken));
        }
    }

    // Reject tokens minted before the user's last password change/reset. Strict `<`
    // keeps a token minted in the same second as the change (e.g. the fresh token
    // change-password returns) valid. Fail-open on Redis errors, like the denylist.
    let reset_key = format!("{}{}", PASSWORD_RESET_AT_PREFIX, claims.user_id);
    if let Ok(val) = app_state.redis_conn.get_key_string(&reset_key).await {
        if let Ok(reset_at) = val.parse::<u64>() {
            if claims.iat < reset_at {
                return Err(ContainerError::from(UserAuthError::InvalidToken));
            }
        }
    }

    Ok(claims)
}
