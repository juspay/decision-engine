use crate::app::{get_tenant_app_state, APP_STATE};
use crate::auth::{self, TOKEN_TYPE_HS_REDIRECT, TOKEN_TYPE_STANDARD, TOKEN_TYPE_SUPER_ADMIN_VIEW};
use crate::error::{self, ContainerError, ResultContainerExt, UserAuthError};
use crate::storage::types::{
    MerchantAccountNew, NewUser, NewUserMerchant, User, UserEmailVerifiedUpdate, UserMerchant,
    UserMerchantIdUpdate,
};
use crate::types::merchant::merchant_account::load_merchant_by_merchant_id;
use crate::utils::date_time;
use axum::extract::Query;
use axum::http::HeaderMap;
use axum::Json;
use diesel::associations::HasTable;
use diesel::{BoolExpressionMethods, ExpressionMethods};
use error_stack::ResultExt;
use masking::PeekInterface;
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

/// One-time SSO handoff codes (HS → DE merchant redirect). The code — never a session token —
/// is what travels in the redirect URL; it is single-use and short-lived.
const HS_SSO_CODE_PREFIX: &str = "hs_sso_code:";
const HS_SSO_CODE_TTL_SECONDS: i64 = 60;

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
    /// This user's email is on the platform super-admin roster, so the dashboard should offer the
    /// "enter any merchant" control. UX hint only — the backend re-authorizes on every action.
    pub is_super_admin: bool,
    /// The current session is a super-admin viewing another merchant (drives the banner + Exit).
    pub is_super_admin_view: bool,
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
const PASSWORD_RESET_PREFIX: &str = "password_reset:";
const PASSWORD_RESET_TTL_SECONDS: i64 = 3600; // 1 hour

#[axum::debug_handler]
pub async fn signup(
    headers: HeaderMap,
    Json(payload): Json<SignupRequest>,
) -> Result<Json<SignupResponse>, error::ContainerError<UserAuthError>> {
    let app_state = get_tenant_app_state().await;
    let global_config = APP_STATE
        .get()
        .map(|s| s.global_config.clone())
        .ok_or(UserAuthError::StorageError)?;

    if global_config.user_auth.signup_requires_admin_secret {
        let provided_admin_secret = headers
            .get("x-admin-secret")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if provided_admin_secret != global_config.admin_secret.secret.peek().as_str() {
            return Err(error::ContainerError::from(UserAuthError::Unauthorized));
        }
    }

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
        TOKEN_TYPE_STANDARD,
        global_config.user_auth.jwt_secret.peek(),
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
        TOKEN_TYPE_STANDARD,
        global_config.user_auth.jwt_secret.peek(),
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

    let claims = verify_jwt_not_revoked(token, global_config.user_auth.jwt_secret.peek()).await?;

    // Only a full standard account session may create a merchant. HS-redirect sessions have no
    // real user row (would attach to a phantom user); super-admin-view sessions are scoped to
    // viewing someone else's merchant and must not mint merchants under the admin's own account.
    if claims.token_type != TOKEN_TYPE_STANDARD {
        return Err(error::ContainerError::from(
            UserAuthError::UnsupportedOperation,
        ));
    }

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
        TOKEN_TYPE_STANDARD,
        global_config.user_auth.jwt_secret.peek(),
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

    let claims = verify_jwt_not_revoked(token, global_config.user_auth.jwt_secret.peek()).await?;
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

    let claims = verify_jwt_not_revoked(token, global_config.user_auth.jwt_secret.peek()).await?;

    // switch-merchant is membership-scoped and only valid for a full standard session. A
    // super-admin-view session must Exit back to its own session before switching, so its
    // single-level return token isn't overwritten.
    if claims.token_type != TOKEN_TYPE_STANDARD {
        return Err(error::ContainerError::from(
            UserAuthError::UnsupportedOperation,
        ));
    }

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
        TOKEN_TYPE_STANDARD,
        global_config.user_auth.jwt_secret.peek(),
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

    let claims = verify_jwt_not_revoked(token, global_config.user_auth.jwt_secret.peek()).await?;

    // Account management requires a full standard session. A super-admin-view session acts on
    // another merchant's dashboard and must never touch the admin's own credentials from there.
    if claims.token_type != TOKEN_TYPE_STANDARD {
        return Err(error::ContainerError::from(
            UserAuthError::UnsupportedOperation,
        ));
    }

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

    Ok(Json(ChangePasswordResponse {
        message: "Password updated successfully.".to_string(),
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

    let claims = verify_jwt_not_revoked(token, global_config.user_auth.jwt_secret.peek()).await?;

    // Only a full standard session may manage identity. HS-redirect tokens could otherwise mint
    // persistent real accounts from a leaked short-lived token; a super-admin-view session must not
    // add members to a merchant it is merely inspecting.
    if claims.token_type != TOKEN_TYPE_STANDARD {
        return Err(error::ContainerError::from(
            UserAuthError::UnsupportedOperation,
        ));
    }

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

    let claims = verify_jwt_not_revoked(token, global_config.user_auth.jwt_secret.peek()).await?;
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

    let claims = auth::verify_jwt(token, global_config.user_auth.jwt_secret.peek())
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

    let claims = verify_jwt_not_revoked(token, global_config.user_auth.jwt_secret.peek()).await?;

    if claims.token_type == TOKEN_TYPE_HS_REDIRECT {
        return Ok(Json(MeResponse {
            user_id: claims.user_id,
            email: claims.email,
            merchant_id: claims.merchant_id,
            role: claims.role,
            email_verified: true,
            merchants: vec![],
            is_super_admin: false,
            is_super_admin_view: false,
        }));
    }

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
        // Roster is keyed off the signed `email` claim, not the DB row, so the check matches the
        // identity the rest of this session authorizes against.
        is_super_admin: is_super_admin(&global_config, &claims.email),
        is_super_admin_view: claims.token_type == TOKEN_TYPE_SUPER_ADMIN_VIEW,
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

/// Whether `email` is on the platform super-admin roster. Compared case-insensitively (emails are
/// not case-sensitive) and empty emails never match — so HS-redirect sessions, which carry an empty
/// email, can never be mistaken for a super admin.
fn is_super_admin(global_config: &crate::config::GlobalConfig, email: &str) -> bool {
    !email.is_empty()
        && global_config
            .user_auth
            .super_admin_emails
            .iter()
            .any(|allowed| allowed.eq_ignore_ascii_case(email))
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
pub struct ForgotPasswordRequest {
    pub email: String,
}

#[derive(Debug, Deserialize)]
pub struct ResetPasswordRequest {
    pub token: String,
    pub new_password: String,
}

#[derive(Debug, Serialize)]
pub struct MessageResponse {
    pub message: String,
}

/// Generic message returned by `forgot_password` regardless of whether the email
/// exists — this prevents attackers from using the endpoint to enumerate accounts.
const FORGOT_PASSWORD_GENERIC_MESSAGE: &str =
    "If an account exists for that email, a password reset link has been sent.";

#[axum::debug_handler]
pub async fn forgot_password(
    Json(payload): Json<ForgotPasswordRequest>,
) -> Result<Json<MessageResponse>, error::ContainerError<UserAuthError>> {
    let generic_ok = || {
        Ok(Json(MessageResponse {
            message: FORGOT_PASSWORD_GENERIC_MESSAGE.to_string(),
        }))
    };

    let global_config = APP_STATE
        .get()
        .map(|s| s.global_config.clone())
        .ok_or(UserAuthError::StorageError)?;

    // Without a configured email backend there is no way to deliver the reset link,
    // so short-circuit — but still return the generic response to avoid leaking that.
    if !global_config.email.is_active() {
        crate::logger::warn!(
            "forgot_password requested but no email backend is configured; skipping send"
        );
        return generic_ok();
    }

    let app_state = get_tenant_app_state().await;

    let mut users = crate::generics::generic_find_all::<<User as HasTable>::Table, _, User>(
        &app_state.db,
        dsl::email.eq(payload.email.clone()),
    )
    .await
    .change_error(UserAuthError::StorageError)?;

    let user = match users.pop() {
        Some(user) => user,
        // Unknown email — respond as if it succeeded so callers can't probe for accounts.
        None => return generic_ok(),
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
        return generic_ok();
    }

    let token = uuid::Uuid::new_v4().to_string();
    let redis_key = format!("{}{}", PASSWORD_RESET_PREFIX, token);
    let reset_url = format!(
        "{}/reset-password?token={}",
        global_config.email.base_url, token
    );

    app_state
        .redis_conn
        .set_key_with_ttl(&redis_key, &user.user_id, PASSWORD_RESET_TTL_SECONDS)
        .await
        .change_context(UserAuthError::StorageError)?;

    let email_client = APP_STATE
        .get()
        .map(|s| s.email_client.clone())
        .ok_or(UserAuthError::StorageError)?;

    let send_result = email_client
        .send_email(
            crate::email::templates::PasswordResetTemplate {
                user_email: user.email.clone(),
                reset_url,
            }
            .into_message(),
        )
        .await;

    if send_result.is_err() {
        // Drop the token so a failed send doesn't leave a dangling reset entry.
        let _ = app_state.redis_conn.delete_key(&redis_key).await;
        send_result.change_context(UserAuthError::EmailSendFailed)?;
    }

    generic_ok()
}

#[axum::debug_handler]
pub async fn reset_password(
    Json(payload): Json<ResetPasswordRequest>,
) -> Result<Json<MessageResponse>, error::ContainerError<UserAuthError>> {
    let app_state = get_tenant_app_state().await;

    let redis_key = format!("{}{}", PASSWORD_RESET_PREFIX, payload.token);

    // Validate and hash before touching the token so a weak password doesn't consume it.
    auth::validate_password_strength(&payload.new_password)
        .change_context(UserAuthError::WeakPassword)?;

    let new_hash = auth::hash_password(&payload.new_password)
        .change_context(UserAuthError::PasswordHashingFailed)?;

    // Single-use token: GETDEL consumes it atomically, so concurrent requests carrying the
    // same token can't both read it and reset the password twice.
    let user_id = app_state
        .redis_conn
        .get_and_delete_key_string(&redis_key)
        .await
        .change_context(UserAuthError::StorageError)?
        .filter(|user_id| !user_id.is_empty())
        .ok_or(UserAuthError::InvalidResetToken)?;

    let conn = app_state
        .db
        .get_conn()
        .await
        .change_error(UserAuthError::StorageError)?;

    let rows_updated = crate::generics::generic_update_if_present::<
        <User as HasTable>::Table,
        crate::storage::types::UserPasswordUpdate,
        _,
    >(
        &conn,
        dsl::user_id.eq(user_id.clone()),
        crate::storage::types::UserPasswordUpdate {
            password_hash: new_hash,
        },
    )
    .await
    .change_context(UserAuthError::StorageError)?;

    if rows_updated == 0 {
        crate::logger::error!(user_id = %user_id, "Password reset matched 0 rows — user_id not found in DB");
        return Err(error::ContainerError::from(
            UserAuthError::InvalidResetToken,
        ));
    }

    Ok(Json(MessageResponse {
        message: "Password reset successfully. You can now sign in with your new password."
            .to_string(),
    }))
}

#[derive(Debug, Deserialize)]
pub struct VerifyEmailQuery {
    pub token: String,
}

#[axum::debug_handler]
pub async fn verify_email(
    Query(query): Query<VerifyEmailQuery>,
) -> Result<Json<VerifyEmailResponse>, error::ContainerError<UserAuthError>> {
    let app_state = get_tenant_app_state().await;

    let redis_key = format!("{}{}", EMAIL_VERIFICATION_PREFIX, query.token);

    let user_id = app_state
        .redis_conn
        .get_key_string(&redis_key)
        .await
        .change_context(UserAuthError::StorageError)?;

    if user_id.is_empty() {
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

    let _ = app_state.redis_conn.delete_key(&redis_key).await;

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

    Ok(claims)
}

#[derive(Debug, Deserialize)]
pub struct AdminMerchantTokenRequest {
    pub merchant_id: String,
}

#[derive(Debug, Serialize)]
pub struct AdminMerchantCodeResponse {
    pub code: String,
    pub expires_in: i64,
}

/// Server-to-server: HS calls this with the shared admin secret to obtain a short-lived,
/// single-use handoff *code* for a merchant. The code — never a session token — is what
/// travels in the redirect URL; the browser redeems it for the JWT via `exchange_merchant_token`.
/// Keeping the token out of the URL prevents it leaking to access logs, proxies, browser
/// history, and the `Referer` header.
#[axum::debug_handler]
pub async fn admin_merchant_token(
    headers: HeaderMap,
    Json(payload): Json<AdminMerchantTokenRequest>,
) -> Result<Json<AdminMerchantCodeResponse>, error::ContainerError<UserAuthError>> {
    let global_config = APP_STATE
        .get()
        .map(|s| s.global_config.clone())
        .ok_or(UserAuthError::StorageError)?;

    let provided = headers
        .get("x-admin-secret")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if provided != global_config.admin_secret.secret.peek().as_str() {
        return Err(error::ContainerError::from(UserAuthError::InvalidToken));
    }

    // Verify the merchant actually exists in DE before issuing a code.
    load_merchant_by_merchant_id(payload.merchant_id.clone())
        .await
        .ok_or_else(|| error::ContainerError::from(UserAuthError::MerchantNotFound))?;

    let app_state = get_tenant_app_state().await;

    // Opaque, single-use code stored against the merchant. The JWT is *not* minted here — it is
    // minted only on redemption, so no session token is ever placed in a URL.
    let code = auth::generate_api_key();
    let code_key = format!("{}{}", HS_SSO_CODE_PREFIX, code);
    app_state
        .redis_conn
        .set_key_with_ttl(&code_key, &payload.merchant_id, HS_SSO_CODE_TTL_SECONDS)
        .await
        .change_context(UserAuthError::StorageError)?;

    Ok(Json(AdminMerchantCodeResponse {
        code,
        expires_in: HS_SSO_CODE_TTL_SECONDS,
    }))
}

#[derive(Debug, Deserialize)]
pub struct ExchangeMerchantTokenRequest {
    pub code: String,
}

/// Called by the DE SPA with the one-time code carried in the redirect URL. Atomically consumes
/// the code (single-use) and mints a short-lived `hs_redirect` session JWT scoped to the merchant.
#[axum::debug_handler]
pub async fn exchange_merchant_token(
    Json(payload): Json<ExchangeMerchantTokenRequest>,
) -> Result<Json<AuthResponse>, error::ContainerError<UserAuthError>> {
    let global_config = APP_STATE
        .get()
        .map(|s| s.global_config.clone())
        .ok_or(UserAuthError::StorageError)?;

    let app_state = get_tenant_app_state().await;
    let code_key = format!("{}{}", HS_SSO_CODE_PREFIX, payload.code);

    // Read the merchant behind the code. A missing/expired/unknown code reads as NotFound —
    // treat any failure to read it as an invalid code (401), not a server error (500).
    let merchant_id = match app_state.redis_conn.get_key_string(&code_key).await {
        Ok(merchant_id) if !merchant_id.is_empty() => merchant_id,
        _ => return Err(error::ContainerError::from(UserAuthError::InvalidToken)),
    };

    // Atomically claim the code by deleting it. `DEL` is atomic and reports whether *this* call
    // removed the key, so with two concurrent redemptions exactly one sees KeyDeleted — the
    // other (already consumed, expired, or replayed) is rejected. This revision's DelReply is a
    // bare enum with no helper methods, so match the variant directly.
    let claimed = app_state
        .redis_conn
        .delete_key(&code_key)
        .await
        .change_context(UserAuthError::StorageError)?;

    if !matches!(claimed, redis_interface::types::DelReply::KeyDeleted) {
        return Err(error::ContainerError::from(UserAuthError::InvalidToken));
    }

    let synthetic_user_id = format!("hs_{}", merchant_id);

    let token = auth::generate_jwt(
        &synthetic_user_id,
        "",
        &merchant_id,
        "admin",
        TOKEN_TYPE_HS_REDIRECT,
        global_config.user_auth.jwt_secret.peek().as_str(),
        global_config.user_auth.hs_redirect_jwt_expiry_seconds,
    )
    .change_context(UserAuthError::TokenGenerationFailed)?;

    Ok(Json(AuthResponse {
        token,
        user_id: synthetic_user_id,
        email: String::new(),
        merchant_id,
        role: "admin".to_string(),
        merchants: vec![],
    }))
}

#[derive(Debug, Deserialize)]
pub struct EnterMerchantRequest {
    pub merchant_id: String,
}

/// Platform super-admin → view any merchant's dashboard by pasting its ID. The caller must present a
/// full standard session whose email is on the config roster. Mints a `super_admin_view` token
/// scoped to the target merchant while keeping the admin's own identity, so actions stay
/// attributable and the roster check on later actions still matches this admin.
#[axum::debug_handler]
pub async fn enter_merchant(
    headers: HeaderMap,
    Json(payload): Json<EnterMerchantRequest>,
) -> Result<Json<AuthResponse>, error::ContainerError<UserAuthError>> {
    let token = extract_bearer_token(&headers)?;
    let global_config = APP_STATE
        .get()
        .map(|s| s.global_config.clone())
        .ok_or(UserAuthError::StorageError)?;

    let claims = verify_jwt_not_revoked(token, global_config.user_auth.jwt_secret.peek()).await?;

    // Entry is only from a full standard session. A view session must Exit first — re-entering from
    // one would strand the admin with no way back to their own session.
    if claims.token_type != TOKEN_TYPE_STANDARD {
        return Err(error::ContainerError::from(
            UserAuthError::UnsupportedOperation,
        ));
    }

    // Authorize live against the config roster (the source of truth), by email.
    if !is_super_admin(&global_config, &claims.email) {
        return Err(error::ContainerError::from(UserAuthError::Forbidden));
    }

    // A typo'd or unknown ID is a clean 404, not a token scoped to a merchant that isn't there.
    load_merchant_by_merchant_id(payload.merchant_id.clone())
        .await
        .ok_or_else(|| error::ContainerError::from(UserAuthError::MerchantNotFound))?;

    let new_token = auth::generate_jwt(
        &claims.user_id,
        &claims.email,
        &payload.merchant_id,
        "admin",
        TOKEN_TYPE_SUPER_ADMIN_VIEW,
        global_config.user_auth.jwt_secret.peek(),
        global_config.user_auth.jwt_expiry_seconds,
    )
    .change_context(UserAuthError::TokenGenerationFailed)?;

    Ok(Json(AuthResponse {
        token: new_token,
        user_id: claims.user_id,
        email: claims.email,
        merchant_id: payload.merchant_id,
        role: "admin".to_string(),
        merchants: vec![],
    }))
}

/// Ends a super-admin-view session and returns the admin to their own standard session. Obtainable
/// only from a valid `super_admin_view` token — itself minted from a rostered standard session — and
/// the roster is re-checked so a since-revoked admin can't mint a fresh standard session from it.
/// This grants nothing the admin couldn't get by logging in again; it just avoids a re-login.
#[axum::debug_handler]
pub async fn exit_merchant(
    headers: HeaderMap,
) -> Result<Json<AuthResponse>, error::ContainerError<UserAuthError>> {
    let token = extract_bearer_token(&headers)?;
    let global_config = APP_STATE
        .get()
        .map(|s| s.global_config.clone())
        .ok_or(UserAuthError::StorageError)?;

    let claims = verify_jwt_not_revoked(token, global_config.user_auth.jwt_secret.peek()).await?;

    if claims.token_type != TOKEN_TYPE_SUPER_ADMIN_VIEW {
        return Err(error::ContainerError::from(
            UserAuthError::UnsupportedOperation,
        ));
    }
    if !is_super_admin(&global_config, &claims.email) {
        return Err(error::ContainerError::from(UserAuthError::Forbidden));
    }

    let app_state = get_tenant_app_state().await;

    let mut users = crate::generics::generic_find_all::<<User as HasTable>::Table, _, User>(
        &app_state.db,
        dsl::user_id.eq(claims.user_id.clone()),
    )
    .await
    .change_error(UserAuthError::StorageError)?;
    let user = users.pop().ok_or(UserAuthError::UserNotFound)?;

    let merchants = fetch_user_merchants(&app_state, &user.user_id).await?;
    let home_merchant_id = user.merchant_id.clone().unwrap_or_else(|| {
        merchants
            .first()
            .map(|m| m.merchant_id.clone())
            .unwrap_or_default()
    });

    let new_token = auth::generate_jwt(
        &user.user_id,
        &user.email,
        &home_merchant_id,
        &user.role,
        TOKEN_TYPE_STANDARD,
        global_config.user_auth.jwt_secret.peek(),
        global_config.user_auth.jwt_expiry_seconds,
    )
    .change_context(UserAuthError::TokenGenerationFailed)?;

    Ok(Json(AuthResponse {
        token: new_token,
        user_id: user.user_id,
        email: user.email,
        merchant_id: home_merchant_id,
        role: user.role,
        merchants,
    }))
}

#[derive(Debug, Deserialize)]
pub struct LookupRequest {
    pub query: String,
}

#[derive(Debug, Serialize)]
pub struct MerchantMember {
    pub email: String,
    pub role: String,
}

#[derive(Debug, Serialize)]
pub struct MerchantLookupResult {
    pub merchant_id: String,
    pub merchant_name: String,
    pub members: Vec<MerchantMember>,
}

/// Cap on lookup results — this is a "find the ID to enter" helper, not a full directory dump.
const MERCHANT_LOOKUP_LIMIT: usize = 25;

/// Escape the LIKE/ILIKE wildcards so a user's `%` or `_` is matched literally rather than acting as
/// a wildcard. The default escape character is `\` on both MySQL and Postgres.
fn escape_like(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// Super-admin merchant lookup: one query string matched (case-insensitively, substring) against
/// both user email and merchant name, plus an exact merchant-id match. Returns the matching
/// merchants with their members so a super-admin can find the ID to enter from a person or company
/// name. Same gate as `enter_merchant` — a rostered standard session.
#[axum::debug_handler]
pub async fn lookup_merchants(
    headers: HeaderMap,
    Json(payload): Json<LookupRequest>,
) -> Result<Json<Vec<MerchantLookupResult>>, error::ContainerError<UserAuthError>> {
    let token = extract_bearer_token(&headers)?;
    let global_config = APP_STATE
        .get()
        .map(|s| s.global_config.clone())
        .ok_or(UserAuthError::StorageError)?;

    let claims = verify_jwt_not_revoked(token, global_config.user_auth.jwt_secret.peek()).await?;

    if claims.token_type != TOKEN_TYPE_STANDARD {
        return Err(error::ContainerError::from(
            UserAuthError::UnsupportedOperation,
        ));
    }
    if !is_super_admin(&global_config, &claims.email) {
        return Err(error::ContainerError::from(UserAuthError::Forbidden));
    }

    let query = payload.query.trim().to_string();
    if query.is_empty() {
        return Ok(Json(vec![]));
    }

    #[cfg(feature = "mysql")]
    use crate::storage::schema::merchant_account::dsl as ma_dsl;
    #[cfg(feature = "postgres")]
    use crate::storage::schema_pg::merchant_account::dsl as ma_dsl;

    // `.like`/`.ilike` live on different traits and only one backend has ILIKE, so the substring
    // predicates below are cfg-split; everything else is shared.
    #[cfg(feature = "postgres")]
    use diesel::PgTextExpressionMethods;
    #[cfg(feature = "mysql")]
    use diesel::TextExpressionMethods;

    use crate::storage::types::MerchantAccount;

    let app_state = get_tenant_app_state().await;
    let pattern = format!("%{}%", escape_like(&query));

    // 1. Users whose email matches → the merchants they belong to.
    #[cfg(feature = "mysql")]
    let matched_users = crate::generics::generic_find_all::<<User as HasTable>::Table, _, User>(
        &app_state.db,
        dsl::email.like(pattern.clone()),
    )
    .await
    .change_error(UserAuthError::StorageError)?;
    #[cfg(feature = "postgres")]
    let matched_users = crate::generics::generic_find_all::<<User as HasTable>::Table, _, User>(
        &app_state.db,
        dsl::email.ilike(pattern.clone()),
    )
    .await
    .change_error(UserAuthError::StorageError)?;

    let matched_user_ids: Vec<String> = matched_users.into_iter().map(|u| u.user_id).collect();

    let people_memberships = if matched_user_ids.is_empty() {
        Vec::new()
    } else {
        crate::generics::generic_find_all::<<UserMerchant as HasTable>::Table, _, UserMerchant>(
            &app_state.db,
            um_dsl::user_id.eq_any(matched_user_ids),
        )
        .await
        .change_error(UserAuthError::StorageError)?
    };

    // 2. Merchants whose name matches, or whose ID is exactly the query.
    #[cfg(feature = "mysql")]
    let matched_merchants = crate::generics::generic_find_all::<
        <MerchantAccount as HasTable>::Table,
        _,
        MerchantAccount,
    >(
        &app_state.db,
        ma_dsl::merchant_name
            .like(pattern.clone())
            .or(ma_dsl::merchant_id.eq(Some(query.clone()))),
    )
    .await
    .change_error(UserAuthError::StorageError)?;
    #[cfg(feature = "postgres")]
    let matched_merchants = crate::generics::generic_find_all::<
        <MerchantAccount as HasTable>::Table,
        _,
        MerchantAccount,
    >(
        &app_state.db,
        ma_dsl::merchant_name
            .ilike(pattern.clone())
            .or(ma_dsl::merchant_id.eq(Some(query.clone()))),
    )
    .await
    .change_error(UserAuthError::StorageError)?;

    // Merge into an order-preserving, deduped id list: direct merchant matches first, then merchants
    // reached via a matching person. Capped so a broad query can't return an unbounded set.
    let mut ordered_ids: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for merchant_id in matched_merchants
        .into_iter()
        .filter_map(|m| m.merchant_id)
        .chain(people_memberships.iter().map(|m| m.merchant_id.clone()))
    {
        if seen.insert(merchant_id.clone()) {
            ordered_ids.push(merchant_id);
            if ordered_ids.len() >= MERCHANT_LOOKUP_LIMIT {
                break;
            }
        }
    }

    if ordered_ids.is_empty() {
        return Ok(Json(vec![]));
    }

    // 3. Names for the result merchants, and the full membership of each (not just the matched
    // person) so the super-admin can confirm the target.
    let name_rows = crate::generics::generic_find_all::<
        <MerchantAccount as HasTable>::Table,
        _,
        MerchantAccount,
    >(
        &app_state.db,
        ma_dsl::merchant_id.eq_any(ordered_ids.iter().cloned().map(Some).collect::<Vec<_>>()),
    )
    .await
    .change_error(UserAuthError::StorageError)?;
    let name_by_id: std::collections::HashMap<String, String> = name_rows
        .into_iter()
        .filter_map(|m| {
            m.merchant_id
                .clone()
                .map(|id| (id, m.merchant_name.unwrap_or_default()))
        })
        .collect();

    let memberships =
        crate::generics::generic_find_all::<<UserMerchant as HasTable>::Table, _, UserMerchant>(
            &app_state.db,
            um_dsl::merchant_id.eq_any(ordered_ids.clone()),
        )
        .await
        .change_error(UserAuthError::StorageError)?;

    let member_user_ids: Vec<String> = memberships
        .iter()
        .map(|m| m.user_id.clone())
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    let member_users = if member_user_ids.is_empty() {
        Vec::new()
    } else {
        crate::generics::generic_find_all::<<User as HasTable>::Table, _, User>(
            &app_state.db,
            dsl::user_id.eq_any(member_user_ids),
        )
        .await
        .change_error(UserAuthError::StorageError)?
    };
    let email_by_user_id: std::collections::HashMap<String, String> = member_users
        .into_iter()
        .map(|u| (u.user_id, u.email))
        .collect();

    let results = ordered_ids
        .into_iter()
        .map(|merchant_id| {
            let members = memberships
                .iter()
                .filter(|m| m.merchant_id == merchant_id)
                .filter_map(|m| {
                    email_by_user_id
                        .get(&m.user_id)
                        .map(|email| MerchantMember {
                            email: email.clone(),
                            role: m.role.clone(),
                        })
                })
                .collect();
            let merchant_name = name_by_id
                .get(&merchant_id)
                .cloned()
                .filter(|n| !n.is_empty())
                .unwrap_or_else(|| merchant_id.clone());
            MerchantLookupResult {
                merchant_id,
                merchant_name,
                members,
            }
        })
        .collect();

    Ok(Json(results))
}
