pub mod access;
pub mod context;

use error_stack::{Report, ResultExt};
use josekit::jws::JwsHeader;
use josekit::jwt::{self, JwtPayload};
use ring::digest;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub use context::{AuthContext, AuthKind};

const KEY_PREFIX: &str = "DE_";

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("Failed to get current system time")]
    SystemTime,
    #[error("Failed to build JWT signer or verifier")]
    JwtKeyError,
    #[error("Failed to set JWT claim")]
    JwtClaimError,
    #[error("Failed to encode JWT")]
    JwtEncodeError,
    #[error("Failed to decode or verify JWT")]
    JwtDecodeError,
    #[error("JWT token has expired")]
    TokenExpired,
    #[error("JWT token is missing required claim: {0}")]
    MissingClaim(&'static str),
    #[error("Failed to hash password")]
    PasswordHashError,
    #[error("Failed to verify password hash")]
    PasswordVerifyError,
    #[error("Password does not meet strength requirements")]
    WeakPassword,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct JwtClaims {
    pub sub: String,
    pub user_id: String,
    pub email: String,
    pub merchant_id: String,
    pub role: String,
    pub token_type: String,
    pub jti: String,
    pub exp: u64,
    pub iat: u64,
    /// How far a handed-over session may move within the Hyperswitch tree. Absent on standard
    /// sessions, which are scoped by DE membership instead, and on handoff tokens minted before
    /// grants existed — both read as covering `merchant_id` alone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grant: Option<crate::types::merchant::hierarchy::ScopeGrant>,
    /// What this session may do. Absent on tokens minted before permissions existed and on callers
    /// that do not send them; [`JwtClaims::permissions`] decides what that means.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub perms: Option<Vec<Permission>>,
}

impl JwtClaims {
    /// Whether this session holds `permission`.
    pub fn allows(&self, permission: &Permission, require_explicit_permissions: bool) -> bool {
        self.permissions(require_explicit_permissions)
            .contains(permission)
    }

    /// The permissions this session effectively holds — what the middleware enforces, and what
    /// `/auth/me` reports, so the controls the dashboard offers and the requests it would be
    /// refused on cannot disagree.
    ///
    /// A session that named its own is taken at its word, unknown entries included: a dashboard
    /// newer than this build may understand one that this build does not.
    ///
    /// Naming none is resolved by who issued the token. Decision Engine mints its own sessions
    /// with an explicit list, so a standard or super-admin-view session without one predates that
    /// and belongs to a user whose authority comes from Decision Engine rather than Hyperswitch —
    /// it holds everything, and `require_explicit_permissions` does not apply. The flag governs
    /// handed-over sessions, which is where a caller can omit permissions: while it is off they
    /// hold everything, so a Decision Engine deployed ahead of a Hyperswitch that does not send
    /// them keeps working, and turning it on inverts that once omitting them is a bug rather than
    /// an old build.
    pub fn permissions(&self, require_explicit_permissions: bool) -> Vec<Permission> {
        match &self.perms {
            Some(held) => held.clone(),
            None if require_explicit_permissions && self.token_type == TOKEN_TYPE_HS_REDIRECT => {
                Vec::new()
            }
            None => KNOWN_PERMISSIONS.to_vec(),
        }
    }
}

/// One thing a session is allowed to do.
///
/// Orthogonal to `ScopeGrant`, which decides *which* scopes a session can reach: the grant comes
/// from the user's place in the tree, permissions from what their role lets them do there. A
/// session can hold read over a whole org, or read and write over a single profile.
///
/// Grows by adding a variant here and pointing the routes that need it at the new value in
/// [`access::required_permission`] — nothing in the token, the middleware, or the dashboard
/// contract changes shape when a new one appears.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Permission {
    RoutingRead,
    RoutingWrite,
    /// A permission this build has never heard of, kept verbatim.
    ///
    /// A newer Hyperswitch may grant permissions added after this Decision Engine was built. Failing
    /// the token over one would log the user out of a dashboard they are entitled to — over a
    /// permission they may not even need. Kept as it arrived, it matches none of the variants above,
    /// so it denies rather than grants, and it survives being re-signed when a session switches
    /// scope, instead of being quietly dropped on the way through.
    Unknown(String),
}

/// Every permission this build understands. A session naming none is described with these.
pub static KNOWN_PERMISSIONS: &[Permission] = &[Permission::RoutingRead, Permission::RoutingWrite];

impl Permission {
    /// The wire spelling, which is what Hyperswitch sends and what the token carries. Namespaced by
    /// area so a later permission over something other than routing reads naturally.
    pub fn as_str(&self) -> &str {
        match self {
            Self::RoutingRead => "routing:read",
            Self::RoutingWrite => "routing:write",
            Self::Unknown(raw) => raw,
        }
    }
}

impl From<&str> for Permission {
    fn from(raw: &str) -> Self {
        match raw {
            "routing:read" => Self::RoutingRead,
            "routing:write" => Self::RoutingWrite,
            other => Self::Unknown(other.to_owned()),
        }
    }
}

// Written by hand rather than derived: the wire form is a flat string, and an unrecognised one has
// to land in `Unknown` instead of failing the token.
impl serde::Serialize for Permission {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> serde::Deserialize<'de> for Permission {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Ok(Self::from(String::deserialize(deserializer)?.as_str()))
    }
}

pub const TOKEN_TYPE_STANDARD: &str = "standard";
pub const TOKEN_TYPE_HS_REDIRECT: &str = "hs_redirect";
/// A platform super-admin viewing another merchant's dashboard. Carries the real admin's identity
/// (`user_id`/`email`) with `merchant_id` pointing at the target, so actions stay attributable. It
/// is a cross-merchant session, not a full account session — identity/account operations reject it.
pub const TOKEN_TYPE_SUPER_ADMIN_VIEW: &str = "super_admin_view";

pub fn generate_jwt(
    user_id: &str,
    email: &str,
    merchant_id: &str,
    role: &str,
    token_type: &str,
    secret: &str,
    expiry_seconds: u64,
) -> Result<String, Report<AuthError>> {
    generate_scoped_jwt(
        user_id,
        email,
        merchant_id,
        role,
        token_type,
        None,
        // Named rather than left absent, so enabling `require_explicit_permissions` — which is
        // about Hyperswitch callers — cannot strand a session Decision Engine issued itself.
        Some(KNOWN_PERMISSIONS),
        secret,
        expiry_seconds,
    )
}

/// As [`generate_jwt`], plus the Hyperswitch node a handed-over session may move within and what it
/// may do there.
#[allow(clippy::too_many_arguments)]
pub fn generate_scoped_jwt(
    user_id: &str,
    email: &str,
    merchant_id: &str,
    role: &str,
    token_type: &str,
    grant: Option<&crate::types::merchant::hierarchy::ScopeGrant>,
    perms: Option<&[Permission]>,
    secret: &str,
    expiry_seconds: u64,
) -> Result<String, Report<AuthError>> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .change_context(AuthError::SystemTime)?
        .as_secs();

    let jti = Uuid::new_v4().to_string();

    let mut payload = JwtPayload::new();
    payload.set_subject(user_id);
    payload
        .set_claim(
            "user_id",
            Some(serde_json::Value::String(user_id.to_string())),
        )
        .change_context(AuthError::JwtClaimError)?;
    payload
        .set_claim("email", Some(serde_json::Value::String(email.to_string())))
        .change_context(AuthError::JwtClaimError)?;
    payload
        .set_claim(
            "merchant_id",
            Some(serde_json::Value::String(merchant_id.to_string())),
        )
        .change_context(AuthError::JwtClaimError)?;
    payload
        .set_claim("role", Some(serde_json::Value::String(role.to_string())))
        .change_context(AuthError::JwtClaimError)?;
    payload
        .set_claim(
            "token_type",
            Some(serde_json::Value::String(token_type.to_string())),
        )
        .change_context(AuthError::JwtClaimError)?;
    payload
        .set_claim("jti", Some(serde_json::Value::String(jti)))
        .change_context(AuthError::JwtClaimError)?;
    if let Some(grant) = grant {
        payload
            .set_claim(
                "grant",
                Some(serde_json::to_value(grant).change_context(AuthError::JwtClaimError)?),
            )
            .change_context(AuthError::JwtClaimError)?;
    }
    if let Some(perms) = perms {
        payload
            .set_claim(
                "perms",
                Some(serde_json::to_value(perms).change_context(AuthError::JwtClaimError)?),
            )
            .change_context(AuthError::JwtClaimError)?;
    }
    payload
        .set_claim("iat", Some(serde_json::Value::Number(now.into())))
        .change_context(AuthError::JwtClaimError)?;
    payload
        .set_claim(
            "exp",
            Some(serde_json::Value::Number((now + expiry_seconds).into())),
        )
        .change_context(AuthError::JwtClaimError)?;

    let signer = josekit::jws::HS256
        .signer_from_bytes(secret.as_bytes())
        .change_context(AuthError::JwtKeyError)?;

    let header = JwsHeader::new();
    jwt::encode_with_signer(&payload, &header, &signer).change_context(AuthError::JwtEncodeError)
}

pub fn verify_jwt(token: &str, secret: &str) -> Result<JwtClaims, Report<AuthError>> {
    let verifier = josekit::jws::HS256
        .verifier_from_bytes(secret.as_bytes())
        .change_context(AuthError::JwtKeyError)?;

    let (payload, _) =
        jwt::decode_with_verifier(token, &verifier).change_context(AuthError::JwtDecodeError)?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .change_context(AuthError::SystemTime)?
        .as_secs();

    let exp = payload
        .claim("exp")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| Report::new(AuthError::MissingClaim("exp")))?;

    if now > exp {
        return Err(Report::new(AuthError::TokenExpired));
    }

    let user_id = payload
        .subject()
        .ok_or_else(|| Report::new(AuthError::MissingClaim("sub")))?
        .to_string();
    let email = payload
        .claim("email")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Report::new(AuthError::MissingClaim("email")))?
        .to_string();
    let merchant_id = payload
        .claim("merchant_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Report::new(AuthError::MissingClaim("merchant_id")))?
        .to_string();
    let role = payload
        .claim("role")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Report::new(AuthError::MissingClaim("role")))?
        .to_string();
    let jti = payload
        .claim("jti")
        .and_then(|v| v.as_str())
        .ok_or_else(|| Report::new(AuthError::MissingClaim("jti")))?
        .to_string();
    let iat = payload
        .claim("iat")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| Report::new(AuthError::MissingClaim("iat")))?;
    // Fall back to "standard" so existing tokens issued before this field was added remain valid.
    let token_type = payload
        .claim("token_type")
        .and_then(|v| v.as_str())
        .unwrap_or(TOKEN_TYPE_STANDARD)
        .to_string();
    // An unreadable grant reads as absent, which callers treat as covering `merchant_id` alone —
    // the narrowest access, and the same as a token minted before grants existed.
    let grant = payload
        .claim("grant")
        .and_then(|value| serde_json::from_value(value.clone()).ok());
    // Unreadable permissions read as absent, resolved the same way as a caller that never sent
    // any — so a malformed claim is never itself the reason a session gains something.
    let perms = payload
        .claim("perms")
        .and_then(|value| serde_json::from_value(value.clone()).ok());

    Ok(JwtClaims {
        sub: user_id.clone(),
        user_id,
        email,
        merchant_id,
        role,
        token_type,
        jti,
        exp,
        iat,
        grant,
        perms,
    })
}

pub fn hash_password(password: &str) -> Result<String, Report<AuthError>> {
    bcrypt::hash(password, bcrypt::DEFAULT_COST).change_context(AuthError::PasswordHashError)
}

pub fn verify_password(password: &str, hash: &str) -> Result<bool, Report<AuthError>> {
    bcrypt::verify(password, hash).change_context(AuthError::PasswordVerifyError)
}

pub fn validate_password_strength(password: &str) -> Result<(), Report<AuthError>> {
    if password.chars().count() < 10 {
        return Err(Report::new(AuthError::WeakPassword)
            .attach_printable("Password must be at least 10 characters long"));
    }

    let has_uppercase = password.chars().any(|c| c.is_ascii_uppercase());
    let has_lowercase = password.chars().any(|c| c.is_ascii_lowercase());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    let has_special = password.chars().any(|c| !c.is_ascii_alphanumeric());

    if !(has_uppercase && has_lowercase && has_digit && has_special) {
        return Err(Report::new(AuthError::WeakPassword).attach_printable(
            "Password must include an uppercase letter, a lowercase letter, a number, and a special character",
        ));
    }

    Ok(())
}

pub fn generate_api_key() -> String {
    let random_bytes = Uuid::new_v4().as_bytes().to_vec();
    let second_bytes = Uuid::new_v4().as_bytes().to_vec();
    let combined: Vec<u8> = random_bytes.into_iter().chain(second_bytes).collect();
    format!("{}{}", KEY_PREFIX, hex::encode(combined))
}

pub fn hash_api_key(key: &str) -> String {
    let digest = digest::digest(&digest::SHA256, key.as_bytes());
    hex::encode(digest.as_ref())
}

pub fn extract_key_prefix(key: &str) -> String {
    let hex_part = key.strip_prefix(KEY_PREFIX).unwrap_or(key);
    format!("{}{}", KEY_PREFIX, &hex_part[..8.min(hex_part.len())])
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::types::merchant::hierarchy::{GrantLevel, ScopeGrant};

    /// HS256 rejects keys under 32 bytes, so this is padded past that.
    const TEST_SECRET: &str = "test-jwt-secret-padded-to-32-bytes-min";

    #[test]
    fn a_permission_this_build_does_not_know_survives_the_token() {
        // The reason `Unknown` exists: a newer Hyperswitch grants something added after this build.
        // Failing the token would log the user out over a permission they may not even need.
        let perms = vec![Permission::RoutingRead, Permission::from("analytics:read")];
        let token = generate_scoped_jwt(
            "hs_pro_1",
            "",
            "pro_1",
            "admin",
            TOKEN_TYPE_HS_REDIRECT,
            None,
            Some(&perms),
            TEST_SECRET,
            60,
        )
        .expect("signs");

        let claims = verify_jwt(&token, TEST_SECRET).expect("verifies");
        assert_eq!(claims.perms, Some(perms));

        // It grants nothing here, and does not become a write.
        assert!(claims.allows(&Permission::RoutingRead, true));
        assert!(!claims.allows(&Permission::RoutingWrite, true));
    }

    #[test]
    fn a_read_only_session_does_not_hold_write() {
        let perms = vec![Permission::RoutingRead];
        let token = generate_scoped_jwt(
            "hs_pro_1",
            "",
            "pro_1",
            "admin",
            TOKEN_TYPE_HS_REDIRECT,
            None,
            Some(&perms),
            TEST_SECRET,
            60,
        )
        .expect("signs");

        let claims = verify_jwt(&token, TEST_SECRET).expect("verifies");
        // Independent of the rollout flag: an explicit list is honoured either way.
        for require_explicit in [false, true] {
            assert!(claims.allows(&Permission::RoutingRead, require_explicit));
            assert!(!claims.allows(&Permission::RoutingWrite, require_explicit));
        }
    }

    #[test]
    fn a_handed_over_session_naming_no_permissions_follows_the_rollout_flag() {
        let token = generate_scoped_jwt(
            "hs_pro_1",
            "",
            "pro_1",
            "admin",
            TOKEN_TYPE_HS_REDIRECT,
            None,
            None,
            TEST_SECRET,
            60,
        )
        .expect("signs");
        let claims = verify_jwt(&token, TEST_SECRET).expect("verifies");

        // Phase one: a Decision Engine ahead of Hyperswitch keeps working.
        assert!(claims.allows(&Permission::RoutingWrite, false));
        assert_eq!(claims.permissions(false), KNOWN_PERMISSIONS.to_vec());

        // Phase two: once every Hyperswitch sends them, silence grants nothing.
        assert!(!claims.allows(&Permission::RoutingWrite, true));
        assert!(claims.permissions(true).is_empty());
    }

    #[test]
    fn the_rollout_flag_never_strands_a_session_decision_engine_issued() {
        // The flag is about Hyperswitch callers. A dashboard user signing in to Decision Engine
        // itself has no Hyperswitch in the picture, so turning it on must not lock them out —
        // which is what would happen if these tokens were minted without a list.
        for token_type in [TOKEN_TYPE_STANDARD, TOKEN_TYPE_SUPER_ADMIN_VIEW] {
            let token = generate_jwt(
                "user_1",
                "a@b.com",
                "merc_1",
                "admin",
                token_type,
                TEST_SECRET,
                60,
            )
            .expect("signs");
            let claims = verify_jwt(&token, TEST_SECRET).expect("verifies");

            // Named on the token, so the reading never depends on the flag at all.
            assert_eq!(claims.perms.as_deref(), Some(KNOWN_PERMISSIONS));
            for require_explicit in [false, true] {
                assert!(
                    claims.allows(&Permission::RoutingWrite, require_explicit),
                    "{token_type} must keep write with require_explicit={require_explicit}"
                );
            }
        }
    }

    #[test]
    fn a_decision_engine_session_predating_permissions_still_works() {
        // Tokens already in a user's browser when this ships carry no list, and stay valid for the
        // rest of their lifetime. Turning the flag on must not log them out mid-session.
        let claims = JwtClaims {
            sub: "user_1".to_string(),
            user_id: "user_1".to_string(),
            email: "a@b.com".to_string(),
            merchant_id: "merc_1".to_string(),
            role: "admin".to_string(),
            token_type: TOKEN_TYPE_STANDARD.to_string(),
            jti: "jti".to_string(),
            exp: 0,
            iat: 0,
            grant: None,
            perms: None,
        };

        for require_explicit in [false, true] {
            assert!(claims.allows(&Permission::RoutingWrite, require_explicit));
        }
    }

    #[test]
    fn permissions_travel_as_plain_strings() {
        // Hyperswitch writes this list, so the wire form is part of the contract.
        assert_eq!(
            serde_json::to_string(&vec![Permission::RoutingRead, Permission::RoutingWrite])
                .expect("serializes"),
            r#"["routing:read","routing:write"]"#
        );
    }

    #[test]
    fn grant_round_trips_through_the_token() {
        let grant = ScopeGrant {
            level: GrantLevel::Org,
            id: "org_abc".to_string(),
        };
        let token = generate_scoped_jwt(
            "hs_pro_1",
            "",
            "pro_1",
            "admin",
            TOKEN_TYPE_HS_REDIRECT,
            Some(&grant),
            None,
            TEST_SECRET,
            60,
        )
        .expect("signs");

        let claims = verify_jwt(&token, TEST_SECRET).expect("verifies");
        assert_eq!(claims.grant, Some(grant));
    }

    #[test]
    fn a_token_without_a_grant_still_verifies() {
        // Every standard session, and every handoff minted before grants existed.
        let token = generate_jwt(
            "user_1",
            "a@b.com",
            "merc_1",
            "admin",
            TOKEN_TYPE_STANDARD,
            TEST_SECRET,
            60,
        )
        .expect("signs");

        let claims = verify_jwt(&token, TEST_SECRET).expect("verifies");
        assert_eq!(claims.grant, None);
    }

    #[test]
    fn an_unreadable_grant_reads_as_absent() {
        // Degrading to "no grant" narrows access to the token's own scope; failing the whole
        // token would instead lock a live session out of the dashboard.
        let mut payload = JwtPayload::new();
        payload.set_subject("hs_pro_1");
        for (claim, value) in [
            ("user_id", "hs_pro_1"),
            ("email", ""),
            ("merchant_id", "pro_1"),
            ("role", "admin"),
            ("token_type", TOKEN_TYPE_HS_REDIRECT),
            ("jti", "some-jti"),
        ] {
            payload
                .set_claim(claim, Some(serde_json::Value::String(value.to_string())))
                .expect("sets claim");
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_secs();
        payload
            .set_claim("iat", Some(serde_json::Value::Number(now.into())))
            .expect("sets iat");
        payload
            .set_claim("exp", Some(serde_json::Value::Number((now + 60).into())))
            .expect("sets exp");
        payload
            .set_claim("grant", Some(serde_json::json!({"level": "galaxy"})))
            .expect("sets grant");

        let signer = josekit::jws::HS256
            .signer_from_bytes(TEST_SECRET.as_bytes())
            .expect("signer");
        let token = jwt::encode_with_signer(&payload, &JwsHeader::new(), &signer).expect("signs");

        let claims = verify_jwt(&token, TEST_SECRET).expect("verifies");
        assert_eq!(claims.grant, None);
    }

    #[test]
    fn generated_key_has_correct_prefix() {
        let key = generate_api_key();
        assert!(key.starts_with("DE_"), "key should start with DE_: {}", key);
    }

    #[test]
    fn generated_key_has_correct_length() {
        let key = generate_api_key();
        // "DE_" (3) + hex of 32 bytes (64) = 67
        assert_eq!(
            key.len(),
            67,
            "expected length 67, got {}: {}",
            key.len(),
            key
        );
    }

    #[test]
    fn generated_keys_are_unique() {
        let a = generate_api_key();
        let b = generate_api_key();
        assert_ne!(a, b, "two generated keys must be different");
    }

    #[test]
    fn hash_is_deterministic() {
        let key = "DE_abc123";
        assert_eq!(hash_api_key(key), hash_api_key(key));
    }

    #[test]
    fn hash_is_64_hex_chars() {
        let key = generate_api_key();
        let hash = hash_api_key(&key);
        assert_eq!(hash.len(), 64, "SHA-256 hex should be 64 chars");
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn different_keys_produce_different_hashes() {
        let hash_a = hash_api_key("DE_aaaa");
        let hash_b = hash_api_key("DE_bbbb");
        assert_ne!(hash_a, hash_b);
    }

    #[test]
    fn hash_matches_on_verification() {
        let raw_key = generate_api_key();
        let stored_hash = hash_api_key(&raw_key);
        let incoming_hash = hash_api_key(&raw_key);
        assert_eq!(
            stored_hash, incoming_hash,
            "stored hash must match recomputed hash"
        );
    }

    #[test]
    fn wrong_key_does_not_match_hash() {
        let raw_key = generate_api_key();
        let stored_hash = hash_api_key(&raw_key);
        let wrong_key = generate_api_key();
        let wrong_hash = hash_api_key(&wrong_key);
        assert_ne!(
            stored_hash, wrong_hash,
            "wrong key must not match stored hash"
        );
    }

    #[test]
    fn key_prefix_format() {
        let key = generate_api_key();
        let prefix = extract_key_prefix(&key);
        assert!(prefix.starts_with("DE_"), "prefix should start with DE_");
        // "DE_" + 8 hex chars = 11 chars
        assert_eq!(prefix.len(), 11, "prefix should be 11 chars: {}", prefix);
    }

    #[test]
    fn key_prefix_is_consistent() {
        let key = generate_api_key();
        assert_eq!(extract_key_prefix(&key), extract_key_prefix(&key));
    }

    #[test]
    fn weak_password_is_rejected() {
        assert!(validate_password_strength("1234").is_err());
        assert!(validate_password_strength("abcdefghij").is_err());
    }

    #[test]
    fn strong_password_is_accepted() {
        assert!(validate_password_strength("StrongPass#1").is_ok());
    }
}
