use ring::digest;
use uuid::Uuid;

const KEY_PREFIX: &str = "DE_";

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
}
