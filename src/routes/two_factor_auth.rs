//! Two-factor authentication (TOTP) for dashboard password logins.
//!
//! A password login that needs a second factor gets no session. It gets a *flow token* instead: an
//! opaque, Redis-held value bound to one user and one [`TwoFactorStep`]. It is deliberately not a
//! JWT — the middleware and the `/auth/*` handlers accept any JWT signed with the session secret —
//! so holding one never authorizes anything but the step it names.
//!
//! The steps:
//! - `verify_totp`: an enrolled user enters a code from their app, or a recovery code.
//! - `verify_email`: a user who has not enrolled is sent a link. Opening it is the only way to reach
//!   setup, and setup continues in the browser that opened it — so enrolling takes the password
//!   (to start the flow) and the mailbox (to continue it). This is the re-verification every
//!   existing user goes through when two-factor authentication becomes required.
//! - `setup_totp`: the user scans a new secret, confirms one code, and receives recovery codes.
//!
//! Signed-in users manage their enrolment from the account page through the `/auth/2fa/*` routes
//! that take a session; enrolling from there starts at `verify_email` too.

use axum::http::HeaderMap;
use diesel::{BoolExpressionMethods, ExpressionMethods};
use error_stack::ResultExt;
use masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::app::{get_tenant_app_state, TenantAppState, APP_STATE};
use crate::auth::{self, two_factor, TOKEN_TYPE_STANDARD};
use crate::config::{GlobalConfig, TwoFactorMode};
use crate::custom_extractors::Json;
use crate::error::{ContainerError, ResultContainerExt, UserAuthError};
use crate::generics::Repository;
use crate::routes::user_auth::{
    extract_bearer_token, issue_session, verify_jwt_not_revoked, AuthResponse, MessageResponse,
};
use crate::storage::backend::schema::users::dsl;
use crate::storage::types::{User, UserEmailVerifiedUpdate, UserTwoFactorUpdate};

type Result<T> = std::result::Result<T, ContainerError<UserAuthError>>;

const FLOW_PREFIX: &str = "two_factor_flow:";
/// How long each step of a sign-in flow stays open, the emailed link included.
const FLOW_TTL_SECONDS: i64 = 900;
const EMAIL_LINK_PREFIX: &str = "two_factor_email_link:";
const EMAIL_SENT_PREFIX: &str = "two_factor_email_sent:";
const EMAIL_RESEND_INTERVAL_SECONDS: i64 = 60;
/// Code attempts per user — app and recovery codes together — within the window.
const ATTEMPTS_PREFIX: &str = "two_factor_attempts:";
const MAX_ATTEMPTS: i64 = 5;
const ATTEMPTS_WINDOW_SECONDS: i64 = 300;
/// Time steps already used per user, so an observed code cannot be replayed while still valid.
const USED_STEP_PREFIX: &str = "two_factor_used_step:";
const USED_STEP_TTL_SECONDS: i64 =
    ((2 * two_factor::TOTP_SKEW as u64 + 2) * two_factor::TOTP_STEP_SECONDS) as i64;

/// What a flow token lets its holder do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TwoFactorStep {
    VerifyEmail,
    SetupTotp,
    VerifyTotp,
}

#[derive(Debug, Serialize, Deserialize)]
struct FlowEntry {
    user_id: String,
    step: TwoFactorStep,
    /// The secret shown at `setup_totp`, kept until a code from it is confirmed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pending_secret: Option<Secret<String>>,
}

/// Returned in place of a session while a second factor is outstanding.
#[derive(Debug, Serialize)]
pub struct TwoFactorChallenge {
    /// Always true; tells this response apart from a session.
    pub two_factor_required: bool,
    pub flow_token: Secret<String>,
    pub next_step: TwoFactorStep,
    pub email: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FlowTokenRequest {
    pub flow_token: Secret<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfirmEmailRequest {
    pub token: Secret<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FlowCodeRequest {
    pub flow_token: Secret<String>,
    pub totp: Secret<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FlowRecoveryCodeRequest {
    pub flow_token: Secret<String>,
    pub recovery_code: Secret<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CodeRequest {
    pub totp: Secret<String>,
}

#[derive(Debug, Serialize)]
pub struct TotpSetupResponse {
    pub secret: Secret<String>,
    /// `otpauth://` URI for the authenticator app; the dashboard renders it as a QR code.
    pub otpauth_url: Secret<String>,
}

#[derive(Debug, Serialize)]
pub struct RecoveryCodesResponse {
    /// Shown once. Only keyed digests are stored.
    pub recovery_codes: Vec<Secret<String>>,
}

#[derive(Debug, Serialize)]
pub struct TotpEnabledResponse {
    pub recovery_codes: Vec<Secret<String>>,
    pub session: AuthResponse,
}

#[derive(Debug, Serialize)]
pub struct TwoFactorStatusResponse {
    pub mode: TwoFactorMode,
    pub enrolled: bool,
    pub recovery_codes_remaining: usize,
}

// ---------------------------------------------------------------------------------------------
// Entry points used by login and signup
// ---------------------------------------------------------------------------------------------

/// The challenge a password login must answer before it gets a session, or `None` when the
/// password alone is enough. Called only after the password has been checked.
pub(crate) async fn login_challenge(
    app_state: &TenantAppState,
    global_config: &GlobalConfig,
    user: &User,
) -> Result<Option<TwoFactorChallenge>> {
    let Some(step) = login_step(
        global_config.user_auth.two_factor_auth.mode,
        user.totp_secret.is_some(),
    ) else {
        return Ok(None);
    };

    let challenge = start_flow(app_state, user, step, None).await?;
    if step == TwoFactorStep::VerifyEmail {
        // The challenge stands without the email: the user can ask for it again from the next
        // screen, which is also where a recent send is waited out.
        if let Err(error) =
            send_verification_email(app_state, global_config, user, challenge.flow_token.peek())
                .await
        {
            crate::logger::warn!(
                ?error,
                "Two-factor verification email was not sent at login"
            );
        }
    }
    Ok(Some(challenge))
}

/// The first step a password login is held at, or `None` when the password is enough. Off means
/// off even for an enrolled user; otherwise an enrolled user is always asked for a code.
fn login_step(mode: TwoFactorMode, enrolled: bool) -> Option<TwoFactorStep> {
    match (mode, enrolled) {
        (TwoFactorMode::Off, _) | (TwoFactorMode::Optional, false) => None,
        (_, true) => Some(TwoFactorStep::VerifyTotp),
        (TwoFactorMode::Required, false) => Some(TwoFactorStep::VerifyEmail),
    }
}

// ---------------------------------------------------------------------------------------------
// Sign-in flow (authenticated by the flow token)
// ---------------------------------------------------------------------------------------------

/// Sends the verification link again for a flow waiting on it.
#[axum::debug_handler]
pub async fn resend_email(Json(payload): Json<FlowTokenRequest>) -> Result<Json<MessageResponse>> {
    let global_config = global_config()?;
    enabled_keys(&global_config)?;
    let app_state = get_tenant_app_state().await;

    let flow = load_flow(
        &app_state,
        payload.flow_token.peek(),
        TwoFactorStep::VerifyEmail,
    )
    .await?;
    let user = load_user(&app_state, &flow.user_id).await?;
    send_verification_email(&app_state, &global_config, &user, payload.flow_token.peek()).await?;

    Ok(Json(MessageResponse {
        message: format!("We sent a verification link to {}.", user.email),
    }))
}

/// Redeems the emailed link. The flow that sent it ends here, and a new one at `setup_totp` is
/// handed to whoever opened the link.
#[axum::debug_handler]
pub async fn confirm_email(
    Json(payload): Json<ConfirmEmailRequest>,
) -> Result<Json<TwoFactorChallenge>> {
    let global_config = global_config()?;
    enabled_keys(&global_config)?;
    let app_state = get_tenant_app_state().await;

    let link_key = format!("{EMAIL_LINK_PREFIX}{}", payload.token.peek());
    let flow_token = app_state
        .redis_conn
        .get_and_delete_key_string(&link_key)
        .await
        .change_context(UserAuthError::StorageError)?
        .filter(|token| !token.is_empty())
        .ok_or(UserAuthError::InvalidTwoFactorFlow)?;

    let flow = load_flow(&app_state, &flow_token, TwoFactorStep::VerifyEmail).await?;
    claim_flow(&app_state, &flow_token).await?;

    let user = load_user(&app_state, &flow.user_id).await?;
    if user.totp_secret.is_some() {
        return Err(UserAuthError::TwoFactorAlreadyEnrolled.into());
    }

    // Opening the link proves the mailbox, the same proof signup verification asks for.
    let conn = app_state
        .db
        .get_conn()
        .await
        .change_error(UserAuthError::StorageError)?;
    crate::generics::generic_update_if_present::<
        <User as diesel::associations::HasTable>::Table,
        UserEmailVerifiedUpdate,
        _,
    >(
        &conn,
        dsl::user_id.eq(user.user_id.clone()),
        UserEmailVerifiedUpdate {
            #[cfg(feature = "mysql")]
            email_verified: 1,
            #[cfg(feature = "postgres")]
            email_verified: true,
        },
    )
    .await
    .change_context(UserAuthError::StorageError)?;

    let challenge = start_flow(&app_state, &user, TwoFactorStep::SetupTotp, None).await?;
    Ok(Json(challenge))
}

/// Generates the secret to scan. Calling it again replaces the secret, so a user who lost the
/// first QR code can start over within the same flow.
#[axum::debug_handler]
pub async fn setup_totp(Json(payload): Json<FlowTokenRequest>) -> Result<Json<TotpSetupResponse>> {
    let global_config = global_config()?;
    enabled_keys(&global_config)?;
    let app_state = get_tenant_app_state().await;

    let flow = load_flow(
        &app_state,
        payload.flow_token.peek(),
        TwoFactorStep::SetupTotp,
    )
    .await?;
    let user = load_user(&app_state, &flow.user_id).await?;
    if user.totp_secret.is_some() {
        return Err(UserAuthError::TwoFactorAlreadyEnrolled.into());
    }

    let totp = two_factor::totp(None, issuer(&global_config), &user.email)
        .change_context(UserAuthError::StorageError)?;
    let secret = totp.get_secret_base32();

    write_flow(
        &app_state,
        payload.flow_token.peek(),
        &FlowEntry {
            user_id: user.user_id,
            step: TwoFactorStep::SetupTotp,
            pending_secret: Some(Secret::new(secret.clone())),
        },
    )
    .await?;

    Ok(Json(TotpSetupResponse {
        secret: Secret::new(secret),
        otpauth_url: Secret::new(totp.get_url()),
    }))
}

/// Confirms a code from the new secret, stores it, and finishes sign-in with recovery codes.
#[axum::debug_handler]
pub async fn enable_totp(
    Json(payload): Json<FlowCodeRequest>,
) -> Result<Json<TotpEnabledResponse>> {
    let global_config = global_config()?;
    let keys = enabled_keys(&global_config)?;
    let app_state = get_tenant_app_state().await;

    let flow = load_flow(
        &app_state,
        payload.flow_token.peek(),
        TwoFactorStep::SetupTotp,
    )
    .await?;
    let pending_secret = flow
        .pending_secret
        .ok_or(UserAuthError::TwoFactorSetupNotStarted)?;
    let user = load_user(&app_state, &flow.user_id).await?;

    take_attempt(&app_state, &user.user_id).await?;
    let totp = two_factor::totp(
        Some(pending_secret.peek()),
        issuer(&global_config),
        &user.email,
    )
    .change_context(UserAuthError::StorageError)?;
    let step = two_factor::matching_step(&totp, payload.totp.peek(), unix_now())
        .ok_or(UserAuthError::InvalidTwoFactorCode)?;

    claim_flow(&app_state, payload.flow_token.peek()).await?;

    let sealed_secret = keys
        .seal_secret(pending_secret.peek())
        .change_context(UserAuthError::StorageError)?;
    let (recovery_codes, digests) = new_recovery_codes(&keys)?;

    // Only an account with no secret is enrolled, so two flows racing to the end cannot leave the
    // user holding recovery codes that the other one overwrote.
    let conn = app_state
        .db
        .get_conn()
        .await
        .change_error(UserAuthError::StorageError)?;
    let rows = crate::generics::generic_update_if_present::<
        <User as diesel::associations::HasTable>::Table,
        UserTwoFactorUpdate,
        _,
    >(
        &conn,
        dsl::user_id
            .eq(user.user_id.clone())
            .and(dsl::totp_secret.is_null()),
        UserTwoFactorUpdate {
            totp_secret: Some(Some(sealed_secret)),
            totp_recovery_codes: Some(Some(digests)),
        },
    )
    .await
    .change_context(UserAuthError::StorageError)?;
    if rows == 0 {
        return Err(UserAuthError::TwoFactorAlreadyEnrolled.into());
    }

    mark_step_used(&app_state, &user.user_id, step).await;
    clear_attempts(&app_state, &user.user_id).await;

    let session = issue_session(&app_state, &global_config, user).await?;
    Ok(Json(TotpEnabledResponse {
        recovery_codes,
        session,
    }))
}

/// Finishes sign-in with a code from the authenticator app.
#[axum::debug_handler]
pub async fn verify_totp(Json(payload): Json<FlowCodeRequest>) -> Result<Json<AuthResponse>> {
    let global_config = global_config()?;
    let keys = enabled_keys(&global_config)?;
    let app_state = get_tenant_app_state().await;

    let flow = load_flow(
        &app_state,
        payload.flow_token.peek(),
        TwoFactorStep::VerifyTotp,
    )
    .await?;
    let user = load_user(&app_state, &flow.user_id).await?;

    check_totp(
        &app_state,
        &global_config,
        &keys,
        &user,
        payload.totp.peek(),
    )
    .await?;
    claim_flow(&app_state, payload.flow_token.peek()).await?;

    Ok(Json(issue_session(&app_state, &global_config, user).await?))
}

/// Finishes sign-in with a recovery code, which is used up.
#[axum::debug_handler]
pub async fn verify_recovery_code(
    Json(payload): Json<FlowRecoveryCodeRequest>,
) -> Result<Json<AuthResponse>> {
    let global_config = global_config()?;
    let keys = enabled_keys(&global_config)?;
    let app_state = get_tenant_app_state().await;

    let flow = load_flow(
        &app_state,
        payload.flow_token.peek(),
        TwoFactorStep::VerifyTotp,
    )
    .await?;
    let user = load_user(&app_state, &flow.user_id).await?;

    take_attempt(&app_state, &user.user_id).await?;
    let stored_json = user
        .totp_recovery_codes
        .clone()
        .ok_or(UserAuthError::TwoFactorNotEnrolled)?;
    let mut stored: Vec<String> =
        serde_json::from_str(&stored_json).change_context(UserAuthError::StorageError)?;
    let index = keys
        .match_recovery_code(payload.recovery_code.peek(), &stored)
        .ok_or(UserAuthError::InvalidTwoFactorCode)?;
    stored.remove(index);
    let remaining = serde_json::to_string(&stored).change_context(UserAuthError::StorageError)?;

    // Written only over the list just read, so a code cannot be spent twice by racing requests.
    let conn = app_state
        .db
        .get_conn()
        .await
        .change_error(UserAuthError::StorageError)?;
    let rows = crate::generics::generic_update_if_present::<
        <User as diesel::associations::HasTable>::Table,
        UserTwoFactorUpdate,
        _,
    >(
        &conn,
        dsl::user_id
            .eq(user.user_id.clone())
            .and(dsl::totp_recovery_codes.eq(stored_json)),
        UserTwoFactorUpdate {
            totp_secret: None,
            totp_recovery_codes: Some(Some(remaining)),
        },
    )
    .await
    .change_context(UserAuthError::StorageError)?;
    if rows == 0 {
        return Err(UserAuthError::InvalidTwoFactorCode.into());
    }

    claim_flow(&app_state, payload.flow_token.peek()).await?;
    clear_attempts(&app_state, &user.user_id).await;

    Ok(Json(issue_session(&app_state, &global_config, user).await?))
}

// ---------------------------------------------------------------------------------------------
// Account management (authenticated by a standard session)
// ---------------------------------------------------------------------------------------------

#[axum::debug_handler]
pub async fn status(headers: HeaderMap) -> Result<Json<TwoFactorStatusResponse>> {
    let global_config = global_config()?;
    let app_state = get_tenant_app_state().await;
    let user = session_user(&app_state, &global_config, &headers).await?;

    let recovery_codes_remaining = match user.totp_recovery_codes.as_deref() {
        Some(json) => serde_json::from_str::<Vec<String>>(json)
            .change_context(UserAuthError::StorageError)?
            .len(),
        None => 0,
    };

    Ok(Json(TwoFactorStatusResponse {
        mode: global_config.user_auth.two_factor_auth.mode,
        enrolled: user.totp_secret.is_some(),
        recovery_codes_remaining,
    }))
}

/// Starts enrolment for a signed-in user: the verification link goes out, and setup continues
/// from it exactly as it does during sign-in.
#[axum::debug_handler]
pub async fn enroll(headers: HeaderMap) -> Result<Json<TwoFactorChallenge>> {
    let global_config = global_config()?;
    enabled_keys(&global_config)?;
    let app_state = get_tenant_app_state().await;
    let user = session_user(&app_state, &global_config, &headers).await?;
    if user.totp_secret.is_some() {
        return Err(UserAuthError::TwoFactorAlreadyEnrolled.into());
    }

    let challenge = start_flow(&app_state, &user, TwoFactorStep::VerifyEmail, None).await?;
    send_verification_email(
        &app_state,
        &global_config,
        &user,
        challenge.flow_token.peek(),
    )
    .await?;
    Ok(Json(challenge))
}

/// Replaces every recovery code. Takes a current app code, so a session alone cannot do it.
#[axum::debug_handler]
pub async fn regenerate_recovery_codes(
    headers: HeaderMap,
    Json(payload): Json<CodeRequest>,
) -> Result<Json<RecoveryCodesResponse>> {
    let global_config = global_config()?;
    let keys = enabled_keys(&global_config)?;
    let app_state = get_tenant_app_state().await;
    let user = session_user(&app_state, &global_config, &headers).await?;

    check_totp(
        &app_state,
        &global_config,
        &keys,
        &user,
        payload.totp.peek(),
    )
    .await?;

    let (recovery_codes, digests) = new_recovery_codes(&keys)?;
    update_two_factor(
        &app_state,
        &user.user_id,
        UserTwoFactorUpdate {
            totp_secret: None,
            totp_recovery_codes: Some(Some(digests)),
        },
    )
    .await?;

    Ok(Json(RecoveryCodesResponse { recovery_codes }))
}

/// Removes the authenticator and recovery codes. Refused while two-factor authentication is
/// required, since the next login would only enrol the user again.
#[axum::debug_handler]
pub async fn disable(
    headers: HeaderMap,
    Json(payload): Json<CodeRequest>,
) -> Result<Json<MessageResponse>> {
    let global_config = global_config()?;
    let keys = enabled_keys(&global_config)?;
    if global_config.user_auth.two_factor_auth.mode == TwoFactorMode::Required {
        return Err(UserAuthError::TwoFactorMandatory.into());
    }
    let app_state = get_tenant_app_state().await;
    let user = session_user(&app_state, &global_config, &headers).await?;

    check_totp(
        &app_state,
        &global_config,
        &keys,
        &user,
        payload.totp.peek(),
    )
    .await?;

    update_two_factor(
        &app_state,
        &user.user_id,
        UserTwoFactorUpdate {
            totp_secret: Some(None),
            totp_recovery_codes: Some(None),
        },
    )
    .await?;

    Ok(Json(MessageResponse {
        message: "Two-factor authentication turned off.".to_string(),
    }))
}

// ---------------------------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------------------------

fn global_config() -> Result<GlobalConfig> {
    APP_STATE
        .get()
        .map(|state| state.global_config.clone())
        .ok_or_else(|| UserAuthError::StorageError.into())
}

/// The configured keys, or `TwoFactorDisabled` while the feature is off. Startup has already
/// refused a bad key whenever the feature is on, so the second failure is not expected.
fn enabled_keys(global_config: &GlobalConfig) -> Result<two_factor::TwoFactorKeys> {
    let config = &global_config.user_auth.two_factor_auth;
    if config.mode == TwoFactorMode::Off {
        return Err(UserAuthError::TwoFactorDisabled.into());
    }
    two_factor::TwoFactorKeys::from_hex(config.secret_key.peek()).ok_or_else(|| {
        crate::logger::error!("user_auth.two_factor_auth.secret_key is not a 32-byte hex key");
        UserAuthError::StorageError.into()
    })
}

fn issuer(global_config: &GlobalConfig) -> &str {
    &global_config.user_auth.two_factor_auth.issuer_name
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .unwrap_or(0)
}

async fn load_user(app_state: &TenantAppState, user_id: &str) -> Result<User> {
    let user = User::find_all(&app_state.db, dsl::user_id.eq(user_id.to_owned()))
        .await
        .change_error(UserAuthError::StorageError)?
        .pop()
        .ok_or(UserAuthError::UserNotFound)?;
    if !user.is_active() {
        return Err(UserAuthError::AccountInactive.into());
    }
    Ok(user)
}

/// The user behind a standard session. Account security is managed from the user's own session
/// only: a handed-over session has no Decision Engine user, and a super-admin view is someone
/// else's merchant.
async fn session_user(
    app_state: &TenantAppState,
    global_config: &GlobalConfig,
    headers: &HeaderMap,
) -> Result<User> {
    let token = extract_bearer_token(headers)?;
    let claims = verify_jwt_not_revoked(token, global_config.user_auth.jwt_secret.peek()).await?;
    if claims.token_type != TOKEN_TYPE_STANDARD {
        return Err(UserAuthError::UnsupportedOperation.into());
    }
    load_user(app_state, &claims.user_id).await
}

async fn start_flow(
    app_state: &TenantAppState,
    user: &User,
    step: TwoFactorStep,
    pending_secret: Option<Secret<String>>,
) -> Result<TwoFactorChallenge> {
    let flow_token = auth::generate_api_key();
    write_flow(
        app_state,
        &flow_token,
        &FlowEntry {
            user_id: user.user_id.clone(),
            step,
            pending_secret,
        },
    )
    .await?;
    Ok(TwoFactorChallenge {
        two_factor_required: true,
        flow_token: Secret::new(flow_token),
        next_step: step,
        email: user.email.clone(),
    })
}

async fn write_flow(app_state: &TenantAppState, flow_token: &str, entry: &FlowEntry) -> Result<()> {
    app_state
        .redis_conn
        .set_key_with_ttl(
            &format!("{FLOW_PREFIX}{flow_token}"),
            entry,
            FLOW_TTL_SECONDS,
        )
        .await
        .change_context(UserAuthError::StorageError)
        .map_err(Into::into)
}

/// The flow behind `flow_token`, provided it is at `expected`. A missing, expired or unreadable
/// entry is an invalid flow rather than a server error.
async fn load_flow(
    app_state: &TenantAppState,
    flow_token: &str,
    expected: TwoFactorStep,
) -> Result<FlowEntry> {
    let entry = app_state
        .redis_conn
        .get_key::<FlowEntry>(&format!("{FLOW_PREFIX}{flow_token}"), "TwoFactorFlow")
        .await
        .map_err(|_| UserAuthError::InvalidTwoFactorFlow)?;
    if entry.step != expected {
        return Err(UserAuthError::InvalidTwoFactorFlow.into());
    }
    Ok(entry)
}

/// Ends a flow. `DEL` reports whether this call removed the key, so of two requests finishing the
/// same flow at once, exactly one goes on to be issued a session.
async fn claim_flow(app_state: &TenantAppState, flow_token: &str) -> Result<()> {
    let deleted = app_state
        .redis_conn
        .delete_key(&format!("{FLOW_PREFIX}{flow_token}"))
        .await
        .change_context(UserAuthError::StorageError)?;
    if !matches!(deleted, redis_interface::types::DelReply::KeyDeleted) {
        return Err(UserAuthError::InvalidTwoFactorFlow.into());
    }
    Ok(())
}

async fn send_verification_email(
    app_state: &TenantAppState,
    global_config: &GlobalConfig,
    user: &User,
    flow_token: &str,
) -> Result<()> {
    let sent_key = format!("{EMAIL_SENT_PREFIX}{}", user.user_id);
    let may_send = app_state
        .redis_conn
        .set_key_if_not_exists(&sent_key, "1", EMAIL_RESEND_INTERVAL_SECONDS)
        .await
        .change_context(UserAuthError::StorageError)?;
    if !may_send {
        return Err(UserAuthError::EmailRecentlySent.into());
    }

    let link_token = auth::generate_api_key();
    let link_key = format!("{EMAIL_LINK_PREFIX}{link_token}");
    app_state
        .redis_conn
        .set_key_with_ttl(&link_key, flow_token, FLOW_TTL_SECONDS)
        .await
        .change_context(UserAuthError::StorageError)?;

    let email_client = APP_STATE
        .get()
        .map(|state| state.email_client.clone())
        .ok_or(UserAuthError::StorageError)?;
    let sent = email_client
        .send_email(
            crate::email::templates::TwoFactorEmailVerificationTemplate {
                user_email: user.email.clone(),
                verification_url: format!(
                    "{}/two-factor/verify-email?token={link_token}",
                    global_config.email.base_url
                ),
                expires_in_minutes: FLOW_TTL_SECONDS / 60,
            }
            .into_message(),
        )
        .await;
    if sent.is_err() {
        // Neither the dead link nor the resend interval should outlive a send that failed.
        let _ = app_state.redis_conn.delete_key(&link_key).await;
        let _ = app_state.redis_conn.delete_key(&sent_key).await;
    }
    sent.change_context(UserAuthError::EmailSendFailed)?;
    Ok(())
}

/// Counts one code attempt against the user, refusing it once the window's allowance is spent.
/// Counted before the code is checked, so concurrent guesses cannot all slip in under the limit.
async fn take_attempt(app_state: &TenantAppState, user_id: &str) -> Result<()> {
    let key = format!("{ATTEMPTS_PREFIX}{user_id}");
    let attempts = app_state
        .redis_conn
        .increment_key(&key)
        .await
        .change_context(UserAuthError::StorageError)?;
    if attempts == 1 {
        app_state
            .redis_conn
            .expire_key(&key, ATTEMPTS_WINDOW_SECONDS)
            .await
            .change_context(UserAuthError::StorageError)?;
    }
    if attempts > MAX_ATTEMPTS {
        return Err(UserAuthError::TooManyTwoFactorAttempts.into());
    }
    Ok(())
}

async fn clear_attempts(app_state: &TenantAppState, user_id: &str) {
    let _ = app_state
        .redis_conn
        .delete_key(&format!("{ATTEMPTS_PREFIX}{user_id}"))
        .await;
}

/// Records `step` as used. Best effort after enrolment, where the flow token is already spent.
async fn mark_step_used(app_state: &TenantAppState, user_id: &str, step: u64) -> bool {
    app_state
        .redis_conn
        .set_key_if_not_exists(
            &format!("{USED_STEP_PREFIX}{user_id}:{step}"),
            "1",
            USED_STEP_TTL_SECONDS,
        )
        .await
        .unwrap_or(false)
}

/// Checks a code from the user's enrolled authenticator, refusing one already used.
async fn check_totp(
    app_state: &TenantAppState,
    global_config: &GlobalConfig,
    keys: &two_factor::TwoFactorKeys,
    user: &User,
    code: &str,
) -> Result<()> {
    let sealed = user
        .totp_secret
        .as_deref()
        .ok_or(UserAuthError::TwoFactorNotEnrolled)?;
    take_attempt(app_state, &user.user_id).await?;

    let secret = keys
        .open_secret(sealed)
        .change_context(UserAuthError::StorageError)?;
    let totp = two_factor::totp(Some(&secret), issuer(global_config), &user.email)
        .change_context(UserAuthError::StorageError)?;
    let step = two_factor::matching_step(&totp, code, unix_now())
        .ok_or(UserAuthError::InvalidTwoFactorCode)?;
    if !mark_step_used(app_state, &user.user_id, step).await {
        return Err(UserAuthError::InvalidTwoFactorCode.into());
    }

    clear_attempts(app_state, &user.user_id).await;
    Ok(())
}

/// Fresh recovery codes for the user, and their digests as stored.
fn new_recovery_codes(keys: &two_factor::TwoFactorKeys) -> Result<(Vec<Secret<String>>, String)> {
    let codes = two_factor::generate_recovery_codes();
    let digests: Vec<String> = codes
        .iter()
        .map(|code| keys.digest_recovery_code(code))
        .collect();
    let stored = serde_json::to_string(&digests).change_context(UserAuthError::StorageError)?;
    Ok((codes.into_iter().map(Secret::new).collect(), stored))
}

async fn update_two_factor(
    app_state: &TenantAppState,
    user_id: &str,
    update: UserTwoFactorUpdate,
) -> Result<()> {
    let conn = app_state
        .db
        .get_conn()
        .await
        .change_error(UserAuthError::StorageError)?;
    crate::generics::generic_update_if_present::<
        <User as diesel::associations::HasTable>::Table,
        UserTwoFactorUpdate,
        _,
    >(&conn, dsl::user_id.eq(user_id.to_owned()), update)
    .await
    .change_context(UserAuthError::StorageError)?;
    Ok(())
}

// These requests name no payment.
crate::impl_api_event_payload!(
    CodeRequest,
    ConfirmEmailRequest,
    FlowCodeRequest,
    FlowRecoveryCodeRequest,
    FlowTokenRequest
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_login_is_held_only_where_the_mode_asks_for_it() {
        assert_eq!(login_step(TwoFactorMode::Off, false), None);
        assert_eq!(login_step(TwoFactorMode::Off, true), None);
        assert_eq!(login_step(TwoFactorMode::Optional, false), None);
        assert_eq!(
            login_step(TwoFactorMode::Optional, true),
            Some(TwoFactorStep::VerifyTotp)
        );
        assert_eq!(
            login_step(TwoFactorMode::Required, true),
            Some(TwoFactorStep::VerifyTotp)
        );
    }

    #[test]
    fn a_user_without_a_factor_reverifies_their_email_when_it_is_required() {
        assert_eq!(
            login_step(TwoFactorMode::Required, false),
            Some(TwoFactorStep::VerifyEmail)
        );
    }

    #[test]
    fn a_flow_entry_keeps_its_pending_secret_through_redis() {
        let entry = FlowEntry {
            user_id: "u1".to_string(),
            step: TwoFactorStep::SetupTotp,
            pending_secret: Some(Secret::new("JBSWY3DPEHPK3PXP".to_string())),
        };
        let encoded = serde_json::to_string(&entry).expect("serializes");
        let decoded: FlowEntry = serde_json::from_str(&encoded).expect("deserializes");
        assert_eq!(decoded.step, TwoFactorStep::SetupTotp);
        assert_eq!(
            decoded.pending_secret.as_ref().map(|s| s.peek().as_str()),
            Some("JBSWY3DPEHPK3PXP")
        );
        // Debug output is what reaches logs, and must not carry the secret.
        assert!(!format!("{entry:?}").contains("JBSWY3DPEHPK3PXP"));
    }
}
