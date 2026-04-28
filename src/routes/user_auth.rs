use crate::app::{get_tenant_app_state, APP_STATE};
use crate::auth;
use crate::error::{self, ContainerError, ResultContainerExt, UserAuthError};
use crate::storage::types::{
    MerchantAccountNew, NewUser, NewUserMerchant, User, UserMerchant, UserMerchantIdUpdate,
};
use crate::utils::date_time;
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

#[axum::debug_handler]
pub async fn signup(
    Json(payload): Json<SignupRequest>,
) -> Result<Json<AuthResponse>, error::ContainerError<UserAuthError>> {
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
        email_verified: if global_config.user_auth.email_verification_enabled {
            0
        } else {
            1
        },
        #[cfg(feature = "postgres")]
        email_verified: !global_config.user_auth.email_verification_enabled,
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

    if global_config.user_auth.email_verification_enabled {
        return Err(error::ContainerError::from(UserAuthError::EmailNotVerified));
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

    Ok(Json(AuthResponse {
        token,
        user_id: user_id.clone(),
        email: payload.email,
        merchant_id: requested_merchant_id.unwrap_or_default(),
        role: "admin".to_string(),
        merchants: fetch_user_merchants(&app_state, &user_id).await?,
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
