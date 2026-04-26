pub mod context;

use josekit::jws::JwsHeader;
use josekit::jwt::{self, JwtPayload};
use ring::digest;
use std::time::{SystemTime, UNIX_EPOCH};
use uuid::Uuid;

pub use context::{AuthContext, AuthKind};

const KEY_PREFIX: &str = "DE_";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct JwtClaims {
    pub sub: String,
    pub user_id: String,
    pub email: String,
    pub merchant_id: String,
    pub role: String,
    pub jti: String,
    pub exp: u64,
    pub iat: u64,
}

pub fn generate_jwt(
    user_id: &str,
    email: &str,
    merchant_id: &str,
    role: &str,
    secret: &str,
    expiry_seconds: u64,
) -> Result<String, String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();

    let jti = Uuid::new_v4().to_string();

    let mut payload = JwtPayload::new();
    payload.set_subject(user_id);
    payload
        .set_claim(
            "user_id",
            Some(serde_json::Value::String(user_id.to_string())),
        )
        .map_err(|e| e.to_string())?;
    payload
        .set_claim("email", Some(serde_json::Value::String(email.to_string())))
        .map_err(|e| e.to_string())?;
    payload
        .set_claim(
            "merchant_id",
            Some(serde_json::Value::String(merchant_id.to_string())),
        )
        .map_err(|e| e.to_string())?;
    payload
        .set_claim("role", Some(serde_json::Value::String(role.to_string())))
        .map_err(|e| e.to_string())?;
    payload
        .set_claim("jti", Some(serde_json::Value::String(jti)))
        .map_err(|e| e.to_string())?;
    payload
        .set_claim("iat", Some(serde_json::Value::Number(now.into())))
        .map_err(|e| e.to_string())?;
    payload
        .set_claim(
            "exp",
            Some(serde_json::Value::Number((now + expiry_seconds).into())),
        )
        .map_err(|e| e.to_string())?;

    let signer = josekit::jws::HS256
        .signer_from_bytes(secret.as_bytes())
        .map_err(|e| e.to_string())?;

    let header = JwsHeader::new();
    jwt::encode_with_signer(&payload, &header, &signer).map_err(|e| e.to_string())
}

pub fn verify_jwt(token: &str, secret: &str) -> Result<JwtClaims, String> {
    let verifier = josekit::jws::HS256
        .verifier_from_bytes(secret.as_bytes())
        .map_err(|e| e.to_string())?;

    let (payload, _) = jwt::decode_with_verifier(token, &verifier).map_err(|e| e.to_string())?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs();

    let exp = payload
        .claim("exp")
        .and_then(|v| v.as_u64())
        .ok_or("missing exp")?;

    if now > exp {
        return Err("token expired".to_string());
    }

    let user_id = payload.subject().ok_or("missing sub")?.to_string();
    let email = payload
        .claim("email")
        .and_then(|v| v.as_str())
        .ok_or("missing email")?
        .to_string();
    let merchant_id = payload
        .claim("merchant_id")
        .and_then(|v| v.as_str())
        .ok_or("missing merchant_id")?
        .to_string();
    let role = payload
        .claim("role")
        .and_then(|v| v.as_str())
        .ok_or("missing role")?
        .to_string();
    let jti = payload
        .claim("jti")
        .and_then(|v| v.as_str())
        .ok_or("missing jti")?
        .to_string();
    let iat = payload
        .claim("iat")
        .and_then(|v| v.as_u64())
        .ok_or("missing iat")?;

    Ok(JwtClaims {
        sub: user_id.clone(),
        user_id,
        email,
        merchant_id,
        role,
        jti,
        exp,
        iat,
    })
}

pub fn hash_password(password: &str) -> Result<String, String> {
    bcrypt::hash(password, bcrypt::DEFAULT_COST).map_err(|e| e.to_string())
}

pub fn verify_password(password: &str, hash: &str) -> Result<bool, String> {
    bcrypt::verify(password, hash).map_err(|e| e.to_string())
}

pub fn validate_password_strength(password: &str) -> Result<(), String> {
    if password.chars().count() < 10 {
        return Err("Password must be at least 10 characters long".to_string());
    }

    let has_uppercase = password.chars().any(|c| c.is_ascii_uppercase());
    let has_lowercase = password.chars().any(|c| c.is_ascii_lowercase());
    let has_digit = password.chars().any(|c| c.is_ascii_digit());
    let has_special = password.chars().any(|c| !c.is_ascii_alphanumeric());

    if !(has_uppercase && has_lowercase && has_digit && has_special) {
        return Err(
            "Password must include an uppercase letter, a lowercase letter, a number, and a special character"
                .to_string(),
        );
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
        // Simulate what the middleware does: hash the incoming key and compare
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
