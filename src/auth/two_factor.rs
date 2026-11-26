//! Two-factor authentication primitives: TOTP codes, the sealed TOTP secret, and recovery codes.
//!
//! The parameters match Hyperswitch's dashboard (`totp-rs`, SHA-1, six digits, 30-second steps,
//! one step of clock skew either way), so an authenticator set up for one behaves the same in the
//! other. Nothing here touches storage or Redis; `routes::two_factor_auth` owns the flow.

use base64::Engine;
use rand::Rng;
use ring::{constant_time, hmac};
use totp_rs::{Algorithm, Secret as TotpSecret, TOTP};

use crate::crypto::encryption_manager::{
    encryption_interface::Encryption, managers::aes::GcmAes256,
};

pub const TOTP_DIGITS: usize = 6;
pub const TOTP_STEP_SECONDS: u64 = 30;
/// Steps accepted either side of the current one, to absorb clock drift and typing time.
pub const TOTP_SKEW: u8 = 1;

pub const RECOVERY_CODES_COUNT: usize = 8;
/// Characters per half of a `xxxx-xxxx` recovery code.
const RECOVERY_CODE_HALF_LENGTH: usize = 4;
/// Lowercase letters and digits, minus the easily confused ones (0/o, 1/l/i), since codes are read off
/// paper.
const RECOVERY_CODE_ALPHABET: &[u8] = b"abcdefghjkmnpqrstuvwxyz23456789";

/// Label under which the recovery-code digest key is derived from the configured key, so the same
/// bytes never serve as both an AES key and an HMAC key.
const RECOVERY_DIGEST_LABEL: &[u8] = b"decision-engine/two-factor/recovery-codes";

#[derive(Debug, thiserror::Error)]
pub enum TwoFactorError {
    #[error("TOTP secret is invalid")]
    InvalidSecret,
    #[error("failed to seal or open the TOTP secret")]
    Crypto,
}

/// The keys derived from `user_auth.two_factor_auth.secret_key`.
pub struct TwoFactorKeys {
    sealer: GcmAes256,
    recovery_digest: hmac::Key,
}

impl TwoFactorKeys {
    /// `None` unless `hex` decodes to exactly 32 bytes.
    pub fn from_hex(hex: &str) -> Option<Self> {
        let bytes = hex::decode(hex.trim()).ok()?;
        if bytes.len() != 32 {
            return None;
        }
        let derived = hmac::sign(
            &hmac::Key::new(hmac::HMAC_SHA256, &bytes),
            RECOVERY_DIGEST_LABEL,
        );
        Some(Self {
            recovery_digest: hmac::Key::new(hmac::HMAC_SHA256, derived.as_ref()),
            sealer: GcmAes256::new(bytes),
        })
    }

    /// Seals a base32 TOTP secret for storage.
    pub fn seal_secret(&self, secret_base32: &str) -> Result<String, TwoFactorError> {
        let sealed = self
            .sealer
            .encrypt(secret_base32.as_bytes().to_vec())
            .map_err(|_| TwoFactorError::Crypto)?;
        Ok(base64::engine::general_purpose::STANDARD.encode(sealed))
    }

    /// Opens a secret sealed by [`Self::seal_secret`].
    pub fn open_secret(&self, sealed: &str) -> Result<String, TwoFactorError> {
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(sealed)
            .map_err(|_| TwoFactorError::Crypto)?;
        let plain = self
            .sealer
            .decrypt(bytes)
            .map_err(|_| TwoFactorError::Crypto)?;
        String::from_utf8(plain).map_err(|_| TwoFactorError::Crypto)
    }

    /// The stored form of a recovery code. Keyed, so a copy of the database alone is not enough
    /// to search the code space offline.
    pub fn digest_recovery_code(&self, code: &str) -> String {
        hex::encode(hmac::sign(
            &self.recovery_digest,
            normalize_recovery_code(code).as_bytes(),
        ))
    }

    /// The position in `stored` of the digest `code` matches, if any.
    pub fn match_recovery_code(&self, code: &str, stored: &[String]) -> Option<usize> {
        let candidate = self.digest_recovery_code(code);
        stored.iter().position(|digest| {
            constant_time::verify_slices_are_equal(digest.as_bytes(), candidate.as_bytes()).is_ok()
        })
    }
}

/// A TOTP generator for `account` (the user's email). `None` as the secret generates a new one.
pub fn totp(
    secret_base32: Option<&str>,
    issuer: &str,
    account: &str,
) -> Result<TOTP, TwoFactorError> {
    let secret = match secret_base32 {
        Some(encoded) => TotpSecret::Encoded(encoded.to_string()),
        None => TotpSecret::generate_secret(),
    }
    .to_bytes()
    .map_err(|_| TwoFactorError::InvalidSecret)?;

    TOTP::new(
        Algorithm::SHA1,
        TOTP_DIGITS,
        TOTP_SKEW,
        TOTP_STEP_SECONDS,
        secret,
        Some(issuer.to_string()),
        account.to_string(),
    )
    .map_err(|_| TwoFactorError::InvalidSecret)
}

/// The time step `code` is valid for at `now` (Unix seconds), within the allowed skew, or `None`
/// when it matches none of them. The step is what a caller records to refuse the same code twice.
pub fn matching_step(totp: &TOTP, code: &str, now: u64) -> Option<u64> {
    let code = code.trim();
    if code.len() != TOTP_DIGITS || !code.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let current = now / TOTP_STEP_SECONDS;
    let skew = u64::from(TOTP_SKEW);
    (current.saturating_sub(skew)..=current + skew).find(|step| {
        let expected = totp.generate(step * TOTP_STEP_SECONDS);
        constant_time::verify_slices_are_equal(expected.as_bytes(), code.as_bytes()).is_ok()
    })
}

/// A fresh set of recovery codes, as shown to the user once.
pub fn generate_recovery_codes() -> Vec<String> {
    let mut rng = rand::thread_rng();
    let mut half = || -> String {
        (0..RECOVERY_CODE_HALF_LENGTH)
            .map(|_| {
                char::from(RECOVERY_CODE_ALPHABET[rng.gen_range(0..RECOVERY_CODE_ALPHABET.len())])
            })
            .collect()
    };
    (0..RECOVERY_CODES_COUNT)
        .map(|_| format!("{}-{}", half(), half()))
        .collect()
}

/// Case, spaces and the hyphen are presentation; `ABCD EFGH` is `abcd-efgh`.
fn normalize_recovery_code(code: &str) -> String {
    code.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";

    fn keys() -> TwoFactorKeys {
        TwoFactorKeys::from_hex(KEY).expect("valid key")
    }

    #[test]
    fn only_a_32_byte_hex_key_is_accepted() {
        assert!(TwoFactorKeys::from_hex(KEY).is_some());
        assert!(TwoFactorKeys::from_hex("").is_none());
        assert!(TwoFactorKeys::from_hex("abcd").is_none());
        assert!(TwoFactorKeys::from_hex(&"zz".repeat(32)).is_none());
    }

    #[test]
    fn a_sealed_secret_opens_to_itself_and_nothing_else_opens_it() {
        let sealed = keys().seal_secret("JBSWY3DPEHPK3PXP").expect("seals");
        assert_ne!(sealed, "JBSWY3DPEHPK3PXP");
        assert_eq!(
            keys().open_secret(&sealed).expect("opens"),
            "JBSWY3DPEHPK3PXP"
        );

        let other = TwoFactorKeys::from_hex(&"ab".repeat(32)).expect("valid key");
        assert!(other.open_secret(&sealed).is_err());
    }

    #[test]
    fn a_code_matches_its_own_step_and_the_skew_either_side_only() {
        let totp = totp(None, "Decision Engine", "user@example.com").expect("totp");
        let now = 1_700_000_000;
        let step = now / TOTP_STEP_SECONDS;

        let current = totp.generate(now);
        assert_eq!(matching_step(&totp, &current, now), Some(step));

        let previous = totp.generate(now - TOTP_STEP_SECONDS);
        assert_eq!(matching_step(&totp, &previous, now), Some(step - 1));

        let stale = totp.generate(now - 3 * TOTP_STEP_SECONDS);
        assert_eq!(matching_step(&totp, &stale, now), None);

        assert_eq!(matching_step(&totp, "12345", now), None);
        assert_eq!(matching_step(&totp, "abcdef", now), None);
    }

    #[test]
    fn a_stored_secret_reproduces_the_same_codes() {
        let first = totp(None, "Decision Engine", "user@example.com").expect("totp");
        let encoded = first.get_secret_base32();
        let again = totp(Some(&encoded), "Decision Engine", "user@example.com").expect("totp");
        assert_eq!(first.generate(1_700_000_000), again.generate(1_700_000_000));
    }

    #[test]
    fn recovery_codes_are_distinct_and_shaped_for_reading() {
        let codes = generate_recovery_codes();
        assert_eq!(codes.len(), RECOVERY_CODES_COUNT);
        let unique: std::collections::HashSet<_> = codes.iter().collect();
        assert_eq!(unique.len(), RECOVERY_CODES_COUNT);
        for code in &codes {
            assert_eq!(code.len(), 2 * RECOVERY_CODE_HALF_LENGTH + 1);
            assert_eq!(code.as_bytes()[RECOVERY_CODE_HALF_LENGTH], b'-');
        }
    }

    #[test]
    fn a_recovery_code_matches_however_it_is_typed() {
        let keys = keys();
        let stored = vec![
            keys.digest_recovery_code("abcd-efgh"),
            keys.digest_recovery_code("jkmn-pqrs"),
        ];
        assert_eq!(keys.match_recovery_code("jkmn-pqrs", &stored), Some(1));
        assert_eq!(keys.match_recovery_code(" JKMN PQRS ", &stored), Some(1));
        assert_eq!(keys.match_recovery_code("jkmnpqrs", &stored), Some(1));
        assert_eq!(keys.match_recovery_code("zzzz-zzzz", &stored), None);
    }
}
