//! Cryptographic verification of short-lived merchant service tokens.
//!
//! This foundation is deliberately not connected to HTTP authentication. Its result is not an
//! `AuthContext` or an `AuthorizedMerchant`: runtime integration must also check the live shared
//! key, tenant binding, merchant grant, authorization versions and required operation. See
//! `docs/merchant-service-tokens.md` for that boundary and the proposed token contract.

use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

use josekit::jwk::Jwk;
use josekit::jws::{alg::ecdsa::EcdsaJwsVerifier, JwsVerifier, ES256};
use josekit::jwt;
use serde::Deserialize;

pub const MERCHANT_SERVICE_TOKEN_TYPE: &str = "merchant_service";
pub const MAX_TOKEN_LIFETIME_SECONDS: u64 = 300;
const MAX_TOKEN_BYTES: usize = 8 * 1024;

/// Service permissions are independent of the dashboard permission/role contract. Unknown strings
/// are rejected, never replaced by defaults. Defining a permission here does not enable an endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
pub enum MerchantServicePermission {
    #[serde(rename = "routing:execute")]
    RoutingExecute,
    #[serde(rename = "routing:read")]
    RoutingRead,
    #[serde(rename = "routing:write")]
    RoutingWrite,
    #[serde(rename = "feedback:write")]
    FeedbackWrite,
    #[serde(rename = "analytics:read")]
    AnalyticsRead,
}

/// Server-owned policy; never build this from request headers or token claims. There is no
/// clock leeway: the configured lifetime must be in 1..=300 seconds.
pub struct MerchantServiceTokenPolicy {
    pub issuer: String,
    pub audience: String,
    pub max_lifetime_seconds: u64,
}

/// The key and target scope expected by the caller. Runtime integration must derive `key_id` and
/// `tenant_id` from authenticated server state and authorize the requested merchant independently.
pub struct MerchantServiceTokenBinding<'a> {
    pub key_id: &'a str,
    pub tenant_id: &'a str,
    pub merchant_id: &'a str,
}

#[derive(Debug, Deserialize)]
pub struct MerchantServiceClaims {
    pub iss: String,
    /// A single exact audience, not a list of services the token may be replayed against.
    pub aud: String,
    pub token_type: String,
    /// The shared API key's identifier, not the signing key's `kid`.
    pub key_id: String,
    pub tenant_id: String,
    pub merchant_id: String,
    pub permissions: Vec<MerchantServicePermission>,
    pub grant_version: u64,
    pub key_auth_version: u64,
    pub jti: String,
    pub iat: u64,
    pub exp: u64,
    pub nbf: Option<u64>,
}

/// A verified signature, contract, time window and expected scope. This does not prove that the key
/// or grant is still active, or that its authorization versions match the current database rows.
#[derive(Debug)]
pub struct VerifiedMerchantServiceToken {
    claims: MerchantServiceClaims,
}

impl VerifiedMerchantServiceToken {
    pub fn claims(&self) -> &MerchantServiceClaims {
        &self.claims
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum MerchantServiceTokenError {
    #[error("Invalid merchant service token policy")]
    InvalidPolicy,
    #[error("Invalid merchant service token public signing key")]
    InvalidSigningKey,
    #[error("Invalid merchant service token signature or encoding")]
    InvalidToken,
    #[error("Missing or malformed merchant service token claims")]
    InvalidClaims,
    #[error("Merchant service token issuer does not match")]
    IssuerMismatch,
    #[error("Merchant service token audience does not match")]
    AudienceMismatch,
    #[error("Invalid merchant service token type")]
    TokenTypeMismatch,
    #[error("Merchant service token has expired")]
    Expired,
    #[error("Merchant service token is not yet valid")]
    NotYetValid,
    #[error("Invalid merchant service token lifetime")]
    InvalidLifetime,
    #[error("Merchant service token key or scope does not match")]
    ScopeMismatch,
    #[error("Unable to read current time")]
    SystemTime,
}

/// Trust is supplied by server configuration as public P-256 JWKs. `kid` selects only among those
/// keys. The verifier never fetches `jku`/`x5u` or trusts an embedded `jwk`, and pins ES256 itself.
pub struct MerchantServiceTokenVerifier {
    policy: MerchantServiceTokenPolicy,
    signing_keys: HashMap<String, EcdsaJwsVerifier>,
}

impl MerchantServiceTokenVerifier {
    pub fn new(
        policy: MerchantServiceTokenPolicy,
        public_signing_keys: &[Jwk],
    ) -> Result<Self, MerchantServiceTokenError> {
        use MerchantServiceTokenError::{InvalidPolicy, InvalidSigningKey};

        if !is_nonempty(&policy.issuer)
            || !is_nonempty(&policy.audience)
            || !(1..=MAX_TOKEN_LIFETIME_SECONDS).contains(&policy.max_lifetime_seconds)
        {
            return Err(InvalidPolicy);
        }
        let mut signing_keys = HashMap::new();
        for key in public_signing_keys {
            let kid = key
                .key_id()
                .filter(|id| is_nonempty(id))
                .ok_or(InvalidSigningKey)?;
            // Do not put private key material in a verification-only configuration. ES256's JWK
            // importer also checks kty=EC, crv=P-256, alg (if present), use and key_ops.
            if key.parameter("d").is_some() || signing_keys.contains_key(kid) {
                return Err(InvalidSigningKey);
            }
            let verifier = ES256
                .verifier_from_jwk(key)
                .map_err(|_| InvalidSigningKey)?;
            signing_keys.insert(kid.to_string(), verifier);
        }
        if signing_keys.is_empty() {
            return Err(InvalidSigningKey);
        }
        Ok(Self {
            policy,
            signing_keys,
        })
    }

    pub fn verify(
        &self,
        token: &str,
        expected: &MerchantServiceTokenBinding<'_>,
    ) -> Result<VerifiedMerchantServiceToken, MerchantServiceTokenError> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| MerchantServiceTokenError::SystemTime)?
            .as_secs();
        self.verify_at(token, expected, now)
    }

    // Kept private so production callers cannot accidentally supply a stale or caller-owned clock.
    fn verify_at(
        &self,
        token: &str,
        expected: &MerchantServiceTokenBinding<'_>,
        now: u64,
    ) -> Result<VerifiedMerchantServiceToken, MerchantServiceTokenError> {
        use MerchantServiceTokenError::*;

        if token.len() > MAX_TOKEN_BYTES {
            return Err(InvalidToken);
        }
        let (payload, _) = jwt::decode_with_verifier_selector(token, |header| {
            Ok(header.key_id().and_then(|kid| {
                self.signing_keys
                    .get(kid)
                    .map(|key| key as &dyn JwsVerifier)
            }))
        })
        .map_err(|_| InvalidToken)?;
        // Option<u64> would otherwise accept an explicit null as an absent nbf.
        if payload
            .claim("nbf")
            .is_some_and(|value| value.as_u64().is_none())
        {
            return Err(InvalidClaims);
        }
        let claims: MerchantServiceClaims =
            serde_json::from_value(serde_json::Value::Object(payload.claims_set().clone()))
                .map_err(|_| InvalidClaims)?;
        if [
            claims.iss.as_str(),
            claims.aud.as_str(),
            claims.token_type.as_str(),
            claims.key_id.as_str(),
            claims.tenant_id.as_str(),
            claims.merchant_id.as_str(),
            claims.jti.as_str(),
        ]
        .iter()
        .any(|value| !is_nonempty(value))
            || claims.permissions.is_empty()
            || claims.permissions.iter().collect::<HashSet<_>>().len() != claims.permissions.len()
            || claims.grant_version == 0
            || claims.key_auth_version == 0
        {
            return Err(InvalidClaims);
        }
        if claims.iss != self.policy.issuer {
            return Err(IssuerMismatch);
        }
        if claims.aud != self.policy.audience {
            return Err(AudienceMismatch);
        }
        if claims.token_type != MERCHANT_SERVICE_TOKEN_TYPE {
            return Err(TokenTypeMismatch);
        }
        if claims.exp <= claims.iat
            || claims.exp - claims.iat > self.policy.max_lifetime_seconds
            || claims.nbf.is_some_and(|nbf| nbf >= claims.exp)
        {
            return Err(InvalidLifetime);
        }
        if claims.exp <= now {
            return Err(Expired);
        }
        if claims.iat > now || claims.nbf.is_some_and(|nbf| nbf > now) {
            return Err(NotYetValid);
        }
        if claims.key_id != expected.key_id
            || claims.tenant_id != expected.tenant_id
            || claims.merchant_id != expected.merchant_id
        {
            return Err(ScopeMismatch);
        }
        Ok(VerifiedMerchantServiceToken { claims })
    }
}

fn is_nonempty(value: &str) -> bool {
    !value.is_empty() && value.trim() == value
}

#[cfg(test)]
mod tests {
    use super::*;
    use josekit::jwk::{alg::ec::EcKeyPair, KeyPair};
    use josekit::jws::{self, JwsHeader, JwsSigner};
    use serde_json::{json, Value};

    const NOW: u64 = 1_700_000_000;

    fn policy() -> MerchantServiceTokenPolicy {
        MerchantServiceTokenPolicy {
            issuer: "decision-engine".to_string(),
            audience: "decision-engine-api".to_string(),
            max_lifetime_seconds: 300,
        }
    }

    fn binding() -> MerchantServiceTokenBinding<'static> {
        MerchantServiceTokenBinding {
            key_id: "key_shared_1",
            tenant_id: "public",
            merchant_id: "merchant_1",
        }
    }

    fn signing_key(kid: &str) -> EcKeyPair {
        let mut key = ES256.generate_key_pair().expect("generate test key");
        key.set_key_id(Some(kid));
        key
    }

    fn claims() -> Value {
        json!({
            "iss": "decision-engine",
            "aud": "decision-engine-api",
            "token_type": "merchant_service",
            "key_id": "key_shared_1",
            "tenant_id": "public",
            "merchant_id": "merchant_1",
            "permissions": ["routing:execute", "feedback:write"],
            "grant_version": 2,
            "key_auth_version": 3,
            "jti": "token_1",
            "iat": NOW,
            "exp": NOW + 300,
        })
    }

    fn signed_with(claims: &Value, kid: &str, signer: &dyn JwsSigner) -> String {
        let mut header = JwsHeader::new();
        header.set_token_type("JWT");
        header.set_key_id(kid);
        jws::serialize_compact(
            &serde_json::to_vec(claims).expect("serialize claims"),
            &header,
            signer,
        )
        .expect("sign test token")
    }

    fn signed(claims: &Value, key: &EcKeyPair) -> String {
        let signer = ES256
            .signer_from_jwk(&key.to_jwk_key_pair())
            .expect("test signer");
        signed_with(claims, key.key_id().expect("kid"), &signer)
    }

    fn verifier(key: &EcKeyPair) -> MerchantServiceTokenVerifier {
        MerchantServiceTokenVerifier::new(policy(), &[key.to_jwk_public_key()])
            .expect("public test verifier")
    }

    #[test]
    fn verifies_signed_claims_and_preserves_authorization_versions() {
        let key = signing_key("signing_1");
        let result = verifier(&key)
            .verify_at(&signed(&claims(), &key), &binding(), NOW)
            .expect("valid service token");
        assert_eq!(result.claims().key_id, "key_shared_1");
        assert_eq!(result.claims().grant_version, 2);
        assert_eq!(result.claims().key_auth_version, 3);
        assert_eq!(
            result.claims().permissions,
            [
                MerchantServicePermission::RoutingExecute,
                MerchantServicePermission::FeedbackWrite
            ]
        );
    }

    #[test]
    fn public_entrypoint_uses_the_system_clock() {
        let key = signing_key("signing_1");
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_secs();
        let mut claims = claims();
        claims["iat"] = json!(now);
        claims["exp"] = json!(now + 300);
        assert!(verifier(&key)
            .verify(&signed(&claims, &key), &binding())
            .is_ok());
    }

    #[test]
    fn trust_is_server_supplied_and_supports_signing_key_rotation() {
        let first = signing_key("signing_1");
        let second = signing_key("signing_2");
        let verifier = MerchantServiceTokenVerifier::new(
            policy(),
            &[first.to_jwk_public_key(), second.to_jwk_public_key()],
        )
        .expect("two trusted public keys");
        for key in [&first, &second] {
            assert!(verifier
                .verify_at(&signed(&claims(), key), &binding(), NOW)
                .is_ok());
        }
        let attacker = signing_key("signing_1");
        assert_eq!(
            verifier
                .verify_at(&signed(&claims(), &attacker), &binding(), NOW)
                .unwrap_err(),
            MerchantServiceTokenError::InvalidToken
        );
        let unknown = signing_key("untrusted");
        assert!(verifier
            .verify_at(&signed(&claims(), &unknown), &binding(), NOW)
            .is_err());
    }

    #[test]
    fn rejects_symmetric_unsigned_and_malformed_tokens() {
        use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};

        let key = signing_key("signing_1");
        let verifier = verifier(&key);
        let signer = jws::HS256
            .signer_from_bytes(b"test-secret-padded-past-32-bytes!!")
            .expect("HMAC test signer");
        let hmac = signed_with(&claims(), "signing_1", &signer);
        let unsigned = format!(
            "{}.{}.",
            URL_SAFE_NO_PAD.encode(br#"{"alg":"none","kid":"signing_1"}"#),
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims()).expect("serialize"))
        );
        for token in [
            &hmac,
            &unsigned,
            "invalid",
            "",
            &"x".repeat(MAX_TOKEN_BYTES + 1),
        ] {
            assert!(verifier.verify_at(token, &binding(), NOW).is_err());
        }
    }

    #[test]
    fn requires_known_signing_kid_and_ignores_embedded_attacker_key() {
        let key = signing_key("signing_1");
        let attacker = signing_key("attacker");
        let mut signer = ES256
            .signer_from_jwk(&attacker.to_jwk_key_pair())
            .expect("attacker signer");
        signer.remove_key_id();
        let mut header = JwsHeader::new();
        header.set_key_id("signing_1");
        header
            .set_claim(
                "jwk",
                Some(serde_json::to_value(attacker.to_jwk_public_key()).expect("jwk")),
            )
            .expect("embedded key");
        let token = jws::serialize_compact(
            &serde_json::to_vec(&claims()).expect("claims"),
            &header,
            &signer,
        )
        .expect("signed with embedded key");
        assert!(verifier(&key).verify_at(&token, &binding(), NOW).is_err());

        let mut signer = ES256
            .signer_from_jwk(&key.to_jwk_key_pair())
            .expect("signer");
        signer.remove_key_id();
        let token = jws::serialize_compact(
            &serde_json::to_vec(&claims()).expect("claims"),
            &JwsHeader::new(),
            &signer,
        )
        .expect("signed without kid");
        assert!(verifier(&key).verify_at(&token, &binding(), NOW).is_err());
    }

    #[test]
    fn rejects_wrong_issuer_audience_type_and_scope_binding() {
        let key = signing_key("signing_1");
        let verifier = verifier(&key);
        for (field, replacement, expected) in [
            (
                "iss",
                "other-issuer",
                MerchantServiceTokenError::IssuerMismatch,
            ),
            (
                "aud",
                "dashboard",
                MerchantServiceTokenError::AudienceMismatch,
            ),
            (
                "token_type",
                "standard",
                MerchantServiceTokenError::TokenTypeMismatch,
            ),
            (
                "token_type",
                "hs_redirect",
                MerchantServiceTokenError::TokenTypeMismatch,
            ),
            (
                "key_id",
                "key_other",
                MerchantServiceTokenError::ScopeMismatch,
            ),
            (
                "tenant_id",
                "tenant_other",
                MerchantServiceTokenError::ScopeMismatch,
            ),
            (
                "merchant_id",
                "merchant_other",
                MerchantServiceTokenError::ScopeMismatch,
            ),
        ] {
            let mut claims = claims();
            claims[field] = json!(replacement);
            assert_eq!(
                verifier
                    .verify_at(&signed(&claims, &key), &binding(), NOW)
                    .unwrap_err(),
                expected,
                "{field}"
            );
        }
    }

    #[test]
    fn every_mandatory_claim_must_be_present() {
        let key = signing_key("signing_1");
        let verifier = verifier(&key);
        for field in claims().as_object().expect("claims object").keys() {
            let mut claims = claims();
            claims.as_object_mut().expect("object").remove(field);
            assert!(
                verifier
                    .verify_at(&signed(&claims, &key), &binding(), NOW)
                    .is_err(),
                "{field}"
            );
        }
    }

    #[test]
    fn malformed_claims_and_permission_lists_never_gain_defaults() {
        let key = signing_key("signing_1");
        let verifier = verifier(&key);
        for (field, replacement) in [
            ("iss", json!("")),
            ("aud", json!(["decision-engine-api", "other-api"])),
            ("key_id", json!(" ")),
            ("tenant_id", json!(" public")),
            ("merchant_id", json!("")),
            ("jti", json!("")),
            ("permissions", json!([])),
            ("permissions", json!(null)),
            ("permissions", json!("routing:write")),
            ("permissions", json!(["unknown:admin"])),
            ("permissions", json!(["routing:read", 1])),
            ("permissions", json!(["routing:read", "routing:read"])),
            ("grant_version", json!(0)),
            ("grant_version", json!(-1)),
            ("grant_version", json!(1.5)),
            ("key_auth_version", json!(0)),
            ("key_auth_version", json!("3")),
            ("key_auth_version", json!(null)),
            ("iat", json!("1700000000")),
            ("exp", json!(null)),
            ("nbf", json!(null)),
            ("nbf", json!("tomorrow")),
        ] {
            let mut claims = claims();
            claims[field] = replacement;
            assert!(
                verifier
                    .verify_at(&signed(&claims, &key), &binding(), NOW)
                    .is_err(),
                "{field}"
            );
        }
    }

    #[test]
    fn enforces_expiry_issuance_time_max_lifetime_and_nbf() {
        let key = signing_key("signing_1");
        let verifier = verifier(&key);
        let token = signed(&claims(), &key);
        assert!(verifier.verify_at(&token, &binding(), NOW + 299).is_ok());
        assert_eq!(
            verifier
                .verify_at(&token, &binding(), NOW + 300)
                .unwrap_err(),
            MerchantServiceTokenError::Expired
        );
        assert_eq!(
            verifier.verify_at(&token, &binding(), NOW - 1).unwrap_err(),
            MerchantServiceTokenError::NotYetValid
        );
        for expiry in [NOW, NOW - 1, NOW + 301] {
            let mut claims = claims();
            claims["exp"] = json!(expiry);
            assert_eq!(
                verifier
                    .verify_at(&signed(&claims, &key), &binding(), NOW)
                    .unwrap_err(),
                MerchantServiceTokenError::InvalidLifetime
            );
        }
        let mut claims = claims();
        claims["nbf"] = json!(NOW + 10);
        let token = signed(&claims, &key);
        assert_eq!(
            verifier.verify_at(&token, &binding(), NOW).unwrap_err(),
            MerchantServiceTokenError::NotYetValid
        );
        assert!(verifier.verify_at(&token, &binding(), NOW + 10).is_ok());
        claims["nbf"] = json!(NOW + 300);
        assert_eq!(
            verifier
                .verify_at(&signed(&claims, &key), &binding(), NOW)
                .unwrap_err(),
            MerchantServiceTokenError::InvalidLifetime
        );
    }

    #[test]
    fn rejects_invalid_policy_and_nonpublic_or_ambiguous_trust() {
        let key = signing_key("signing_1");
        for lifetime in [0, 301, u64::MAX] {
            let mut policy = policy();
            policy.max_lifetime_seconds = lifetime;
            assert!(matches!(
                MerchantServiceTokenVerifier::new(policy, &[key.to_jwk_public_key()]),
                Err(MerchantServiceTokenError::InvalidPolicy)
            ));
        }
        for (issuer, audience) in [("", "api"), ("issuer", " ")] {
            let mut policy = policy();
            policy.issuer = issuer.to_string();
            policy.audience = audience.to_string();
            assert!(MerchantServiceTokenVerifier::new(policy, &[key.to_jwk_public_key()]).is_err());
        }
        let wrong_curve = jws::ES384.generate_key_pair().expect("P-384 test key");
        let mut wrong_curve = wrong_curve.to_jwk_public_key();
        wrong_curve.set_key_id("signing_1");
        wrong_curve.set_algorithm("ES256");
        let mut symmetric = Jwk::new("oct");
        symmetric.set_key_id("signing_1");
        let mut no_kid = ES256.generate_key_pair().expect("test key");
        no_kid.set_key_id(None::<&str>);
        for keys in [
            vec![],
            vec![key.to_jwk_key_pair()],
            vec![key.to_jwk_public_key(), key.to_jwk_public_key()],
            vec![wrong_curve],
            vec![symmetric],
            vec![no_kid.to_jwk_public_key()],
        ] {
            assert!(matches!(
                MerchantServiceTokenVerifier::new(policy(), &keys),
                Err(MerchantServiceTokenError::InvalidSigningKey)
            ));
        }
    }
}
