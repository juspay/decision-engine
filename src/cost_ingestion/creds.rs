//! Encrypted storage for per-settlement-source ingestion credentials.
//!
//! A settlement source is `(connector, account)` — e.g. one Adyen `merchantAccountCode`. A single
//! merchant may own several accounts (each with its own HMAC key, report-user auth, region, and
//! markup), so the account is the real unit, and it carries *our* `merchant_id`. Keying on
//! `(connector, account)` also resolves the webhook chicken-and-egg: the handler reads the
//! account from the unverified body, then loads that account's secret *and* merchant id to verify.
//!
//! Credentials must be *decryptable* (we use them to download reports), so they are encrypted at
//! rest with AES-256-GCM ([`GcmAes256`]) rather than hashed, and persisted as a base64 blob in the
//! generic `service_configuration` key-value store — no new table.

use std::collections::HashMap;

use base64::Engine;
use masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::crypto::encryption_manager::{
    encryption_interface::Encryption, managers::aes::GcmAes256,
};
use crate::types::service_configuration;

use super::types::{ConnectorCreds, IngestError};

/// A resolved settlement source: the credentials plus the merchant they belong to.
#[derive(Debug, Clone)]
pub struct ResolvedCreds {
    pub merchant_id: String,
    pub creds: ConnectorCreds,
}

/// A `(connector, account)` a merchant has configured — the non-secret half, safe to list.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SourceRef {
    pub connector: String,
    pub account: String,
}

/// A configured source plus *masked* previews of its stored credentials — just enough to recognize
/// which key is set (the last few characters) without disclosing it. Never carries a full secret.
#[derive(Debug, Clone, Serialize)]
pub struct MaskedSource {
    pub connector: String,
    pub account: String,
    /// e.g. `"••••a3f9"`, or `"—"` when no credential blob is stored / decryptable.
    pub webhook_secret_hint: String,
    /// Basic auth shows the user (`"reportuser:••••"`); an API key shows its tail (`"••••a3f9"`).
    pub download_auth_hint: String,
}

/// Mask a secret to a recognizable hint: bullets followed by the last 4 characters. Fully masked
/// when too short to reveal 4 without exposing most of it.
fn mask_secret(s: &str) -> String {
    let n = s.chars().count();
    if n == 0 {
        return "—".to_string();
    }
    if n <= 4 {
        return "••••".to_string();
    }
    let last4: String = s.chars().skip(n - 4).collect();
    format!("••••{last4}")
}

/// Report-download auth: Basic auth (`user:password`) shows the non-secret user and masks the
/// password; a bare API key shows its tail via [`mask_secret`].
fn mask_download_auth(s: &str) -> String {
    match s.split_once(':') {
        Some((user, _)) => format!("{user}:••••"),
        None => mask_secret(s),
    }
}

/// Per-merchant index name holding the (non-secret) list of configured sources. Lets the
/// dashboard show what's set up without scanning/decrypting every credential blob.
fn sources_index_name(merchant_id: &str) -> String {
    format!("cost_ingest_sources::{merchant_id}")
}

/// List the `(connector, account)` sources a merchant has configured (no secrets).
pub async fn list_sources(merchant_id: &str) -> Result<Vec<SourceRef>, IngestError> {
    let stored = service_configuration::find_config_by_name(sources_index_name(merchant_id))
        .await
        .map_err(|e| IngestError::Storage(e.to_string()))?;
    match stored.and_then(|c| c.value) {
        Some(v) => serde_json::from_str(&v).map_err(|e| IngestError::Storage(e.to_string())),
        None => Ok(Vec::new()),
    }
}

/// Record a `(connector, account)` in the merchant's source index (idempotent).
async fn add_source(merchant_id: &str, connector: &str, account: &str) -> Result<(), IngestError> {
    let mut sources = list_sources(merchant_id).await?;
    let entry = SourceRef {
        connector: connector.to_string(),
        account: account.to_string(),
    };
    if sources.contains(&entry) {
        return Ok(());
    }
    sources.push(entry);
    let value = serde_json::to_string(&sources).map_err(|e| IngestError::Storage(e.to_string()))?;
    let name = sources_index_name(merchant_id);
    let exists = service_configuration::find_config_by_name(name.clone())
        .await
        .map_err(|e| IngestError::Storage(e.to_string()))?
        .is_some();
    if exists {
        service_configuration::update_config(name, Some(value)).await
    } else {
        service_configuration::insert_config(name, Some(value)).await
    }
    .map_err(|e| IngestError::Storage(e.to_string()))
}

/// Remove a `(connector, account)` from the merchant's source index (idempotent). When the last
/// source goes, the index row is dropped entirely rather than left as an empty `[]`.
async fn remove_source(
    merchant_id: &str,
    connector: &str,
    account: &str,
) -> Result<(), IngestError> {
    let mut sources = list_sources(merchant_id).await?;
    let before = sources.len();
    sources.retain(|s| !(s.connector == connector && s.account == account));
    if sources.len() == before {
        return Ok(()); // nothing was configured under this pair
    }
    let name = sources_index_name(merchant_id);
    if sources.is_empty() {
        return service_configuration::delete_config(name)
            .await
            .map_err(|e| IngestError::Storage(e.to_string()));
    }
    let value = serde_json::to_string(&sources).map_err(|e| IngestError::Storage(e.to_string()))?;
    service_configuration::update_config(name, Some(value))
        .await
        .map_err(|e| IngestError::Storage(e.to_string()))
}

/// Delete a settlement source: its encrypted credentials *and* its entry in the merchant's source
/// index, so it disappears from the configured list. Idempotent — deleting an absent source is not
/// an error. No keyring needed (we're removing, not decrypting), so this is a free function.
pub async fn delete_source(
    connector: &str,
    account: &str,
    merchant_id: &str,
) -> Result<(), IngestError> {
    service_configuration::delete_config(config_name(connector, account))
        .await
        .map_err(|e| IngestError::Storage(e.to_string()))?;
    remove_source(merchant_id, connector, account).await
}

/// `service_configuration.name` under which a `(connector, account)`'s encrypted creds live.
/// The account (e.g. Adyen `merchantAccountCode`) is unique within a connector, so this key is
/// stable even when one merchant owns several accounts.
fn config_name(connector: &str, account: &str) -> String {
    format!("cost_ingest_creds::{connector}::{account}")
}

/// Whether a connector is *pulled* (we poll its API for ready reports) rather than *pushed* (it
/// calls our webhook). Sourced from the connector's own [`SettlementReportSource::is_pull`], so no
/// connector id is hardcoded here. Pull connectors need a discoverable, cross-merchant list of their
/// sources (see [`list_poll_sources`]); push connectors are found via their webhook payload instead.
pub fn is_pull_connector(connector: &str) -> bool {
    super::ConnectorRegistry::with_builtins()
        .get(connector)
        .map(|s| s.is_pull())
        .unwrap_or(false)
}

/// Per-connector index name holding every `(connector, account)` the poller must sweep. The KV
/// store has no prefix/list-all query, so a pull connector's sources are enumerated from here
/// rather than by scanning `cost_ingest_creds::{connector}::*`.
fn poll_index_name(connector: &str) -> String {
    format!("cost_ingest_poll::{connector}")
}

/// List all `(connector, account)` sources the poller should sweep for `connector`, across every
/// merchant. Empty when none are configured.
pub async fn list_poll_sources(connector: &str) -> Result<Vec<SourceRef>, IngestError> {
    let stored = service_configuration::find_config_by_name(poll_index_name(connector))
        .await
        .map_err(|e| IngestError::Storage(e.to_string()))?;
    match stored.and_then(|c| c.value) {
        Some(v) => serde_json::from_str(&v).map_err(|e| IngestError::Storage(e.to_string())),
        None => Ok(Vec::new()),
    }
}

/// Record a pull connector's `(connector, account)` in its poll index (idempotent).
async fn add_poll_source(connector: &str, account: &str) -> Result<(), IngestError> {
    let mut sources = list_poll_sources(connector).await?;
    let entry = SourceRef {
        connector: connector.to_string(),
        account: account.to_string(),
    };
    if sources.contains(&entry) {
        return Ok(());
    }
    sources.push(entry);
    let value = serde_json::to_string(&sources).map_err(|e| IngestError::Storage(e.to_string()))?;
    let name = poll_index_name(connector);
    let exists = service_configuration::find_config_by_name(name.clone())
        .await
        .map_err(|e| IngestError::Storage(e.to_string()))?
        .is_some();
    if exists {
        service_configuration::update_config(name, Some(value)).await
    } else {
        service_configuration::insert_config(name, Some(value)).await
    }
    .map_err(|e| IngestError::Storage(e.to_string()))
}

/// On-the-wire shape of the encrypted blob (before AES). Secrets are peeked only here, at the
/// encryption boundary.
#[derive(Serialize, Deserialize)]
struct StoredCreds {
    merchant_id: String,
    webhook_secret: String,
    download_auth: String,
}

/// Seals/opens [`ConnectorCreds`] with a versioned keyring and persists them.
///
/// Each stored blob is prefixed with the id of the key that encrypted it (`"{key_id}:{base64}"`).
/// New credentials use `current_id`; decryption uses whichever key the blob names, so rotating
/// the current key leaves older credentials readable as long as their key stays in the ring.
pub struct ConnectorCredsStore {
    current_id: String,
    ciphers: HashMap<String, GcmAes256>,
}

impl ConnectorCredsStore {
    /// Build a store from the configured keyring. Returns `None` (credential storage disabled)
    /// unless there is at least one key, every key is a valid 32-byte hex string, and
    /// `current_id` names one of them.
    pub fn from_keyring(
        current_id: &str,
        keys: &HashMap<String, Secret<String>>,
    ) -> Option<Self> {
        if current_id.is_empty() || keys.is_empty() {
            return None;
        }
        let mut ciphers = HashMap::with_capacity(keys.len());
        for (id, hex) in keys {
            let bytes = hex_decode(hex.peek())?;
            if bytes.len() != 32 {
                return None;
            }
            ciphers.insert(id.clone(), GcmAes256::new(bytes));
        }
        // `current` must be a real key, or we'd write blobs we can never open.
        ciphers.contains_key(current_id).then(|| Self {
            current_id: current_id.to_string(),
            ciphers,
        })
    }

    /// Encrypt creds into a `"{current_id}:{base64}"` string (no DB). Split out so it is
    /// unit-testable without a database.
    fn seal(&self, merchant_id: &str, creds: &ConnectorCreds) -> Result<String, IngestError> {
        let cipher = self
            .ciphers
            .get(&self.current_id)
            .ok_or_else(|| IngestError::Crypto("current key missing from keyring".to_string()))?;
        let blob = StoredCreds {
            merchant_id: merchant_id.to_string(),
            webhook_secret: creds.webhook_secret.peek().clone(),
            download_auth: creds.download_auth.peek().clone(),
        };
        let plaintext =
            serde_json::to_vec(&blob).map_err(|e| IngestError::Crypto(e.to_string()))?;
        let ciphertext = cipher
            .encrypt(plaintext)
            .map_err(|e| IngestError::Crypto(format!("{e:?}")))?;
        let encoded = base64::engine::general_purpose::STANDARD.encode(ciphertext);
        Ok(format!("{}:{}", self.current_id, encoded))
    }

    /// Inverse of [`seal`](Self::seal): reads the key-id prefix, decrypts with that key.
    fn open(&self, stored: &str) -> Result<ResolvedCreds, IngestError> {
        // Base64 (standard alphabet) never contains ':', so the first ':' cleanly splits the id.
        let (key_id, encoded) = stored
            .split_once(':')
            .ok_or_else(|| IngestError::Crypto("stored credential missing key id".to_string()))?;
        let cipher = self.ciphers.get(key_id).ok_or_else(|| {
            IngestError::Crypto(format!("key '{key_id}' not in keyring (retired?)"))
        })?;
        let ciphertext = base64::engine::general_purpose::STANDARD
            .decode(encoded)
            .map_err(|e| IngestError::Crypto(e.to_string()))?;
        let plaintext = cipher
            .decrypt(ciphertext)
            .map_err(|e| IngestError::Crypto(format!("{e:?}")))?;
        let blob: StoredCreds =
            serde_json::from_slice(&plaintext).map_err(|e| IngestError::Crypto(e.to_string()))?;
        Ok(ResolvedCreds {
            merchant_id: blob.merchant_id,
            creds: ConnectorCreds {
                webhook_secret: Secret::new(blob.webhook_secret),
                download_auth: Secret::new(blob.download_auth),
            },
        })
    }

    /// Upsert a settlement source's credentials (encrypted), tagged with its owning merchant.
    pub async fn put(
        &self,
        connector: &str,
        account: &str,
        merchant_id: &str,
        creds: &ConnectorCreds,
    ) -> Result<(), IngestError> {
        let name = config_name(connector, account);
        let value = self.seal(merchant_id, creds)?;
        let exists = service_configuration::find_config_by_name(name.clone())
            .await
            .map_err(|e| IngestError::Storage(e.to_string()))?
            .is_some();
        if exists {
            service_configuration::update_config(name, Some(value)).await
        } else {
            service_configuration::insert_config(name, Some(value)).await
        }
        .map_err(|e| IngestError::Storage(e.to_string()))?;

        // Record the source in the merchant's index so the dashboard can list it.
        add_source(merchant_id, connector, account).await?;

        // Pull connectors also go in a per-connector poll index so the background poller can find
        // every source to sweep, across merchants, without a prefix scan.
        if is_pull_connector(connector) {
            add_poll_source(connector, account).await?;
        }
        Ok(())
    }

    /// List a merchant's configured sources with masked credential previews. Decrypts each blob to
    /// derive the hint, but only ever returns the masked form — never a full secret.
    pub async fn list_masked(&self, merchant_id: &str) -> Result<Vec<MaskedSource>, IngestError> {
        let sources = list_sources(merchant_id).await?;
        let mut out = Vec::with_capacity(sources.len());
        for s in sources {
            // A missing/undecryptable blob still lists the source, just without hints.
            let (webhook_secret_hint, download_auth_hint) = match self.get(&s.connector, &s.account).await {
                Ok(Some(r)) => (
                    mask_secret(r.creds.webhook_secret.peek()),
                    mask_download_auth(r.creds.download_auth.peek()),
                ),
                _ => ("—".to_string(), "—".to_string()),
            };
            out.push(MaskedSource {
                connector: s.connector,
                account: s.account,
                webhook_secret_hint,
                download_auth_hint,
            });
        }
        Ok(out)
    }

    /// Load and decrypt a settlement source's credentials, or `None` if none are stored.
    pub async fn get(
        &self,
        connector: &str,
        account: &str,
    ) -> Result<Option<ResolvedCreds>, IngestError> {
        let name = config_name(connector, account);
        let stored = service_configuration::find_config_by_name(name)
            .await
            .map_err(|e| IngestError::Storage(e.to_string()))?;
        match stored.and_then(|c| c.value) {
            Some(value) => Ok(Some(self.open(&value)?)),
            None => Ok(None),
        }
    }
}

/// Decode an even-length hex string to bytes; `None` on any non-hex character or odd length.
fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a keyring from `(id, hex)` pairs.
    fn ring(pairs: &[(&str, String)]) -> HashMap<String, Secret<String>> {
        pairs
            .iter()
            .map(|(id, hex)| (id.to_string(), Secret::new(hex.clone())))
            .collect()
    }

    fn store() -> ConnectorCredsStore {
        ConnectorCredsStore::from_keyring("v1", &ring(&[("v1", "01".repeat(32))])).expect("valid")
    }

    fn sample() -> ConnectorCreds {
        ConnectorCreds {
            webhook_secret: Secret::new("hmac-key-hex".to_string()),
            download_auth: Secret::new("reportuser:pass".to_string()),
        }
    }

    #[test]
    fn seal_open_roundtrips_with_merchant() {
        let s = store();
        let creds = sample();
        let sealed = s.seal("merchant_A", &creds).unwrap();
        assert!(sealed.starts_with("v1:"), "blob is tagged with the key id");
        let opened = s.open(&sealed).unwrap();
        assert_eq!(opened.merchant_id, "merchant_A");
        assert_eq!(opened.creds.webhook_secret.peek(), creds.webhook_secret.peek());
        assert_eq!(opened.creds.download_auth.peek(), creds.download_auth.peek());
    }

    #[test]
    fn only_pull_connectors_are_polled() {
        assert!(is_pull_connector("chase"));
        assert!(!is_pull_connector("adyen"));
        assert!(!is_pull_connector("braintree"));
        assert_eq!(poll_index_name("chase"), "cost_ingest_poll::chase");
    }

    #[test]
    fn two_accounts_key_independently() {
        // Same merchant, two Adyen accounts -> distinct config keys, no collision.
        assert_eq!(
            config_name("adyen", "AcmeEU"),
            "cost_ingest_creds::adyen::AcmeEU"
        );
        assert_ne!(config_name("adyen", "AcmeEU"), config_name("adyen", "AcmeUS"));
    }

    #[test]
    fn ciphertext_is_not_plaintext_and_is_nonce_randomized() {
        let s = store();
        let creds = sample();
        let a = s.seal("m", &creds).unwrap();
        let b = s.seal("m", &creds).unwrap();
        assert!(!a.contains("hmac-key-hex"), "plaintext must not leak into the blob");
        assert_ne!(a, b, "GCM nonce should randomize each ciphertext");
    }

    #[test]
    fn rotation_keeps_old_blobs_readable() {
        // Store under v1.
        let v1 = store();
        let sealed_v1 = v1.seal("m", &sample()).unwrap();

        // Rotate: current is now v2, but v1 stays in the ring.
        let rotated = ConnectorCredsStore::from_keyring(
            "v2",
            &ring(&[("v1", "01".repeat(32)), ("v2", "02".repeat(32))]),
        )
        .expect("valid");

        // Old blob still opens (uses its tagged v1 key)...
        assert_eq!(rotated.open(&sealed_v1).unwrap().merchant_id, "m");
        // ...and new writes are tagged with the new current key.
        let sealed_v2 = rotated.seal("m", &sample()).unwrap();
        assert!(sealed_v2.starts_with("v2:"));
        assert!(rotated.open(&sealed_v2).is_ok());
    }

    #[test]
    fn retiring_a_key_makes_its_blobs_fail_clearly() {
        let sealed_v1 = store().seal("m", &sample()).unwrap(); // "v1:…"
        // A ring without v1 can't open a v1 blob — and says so, rather than returning garbage.
        let without_v1 =
            ConnectorCredsStore::from_keyring("v2", &ring(&[("v2", "02".repeat(32))])).unwrap();
        let err = without_v1.open(&sealed_v1).unwrap_err();
        assert!(matches!(err, IngestError::Crypto(_)));
    }

    #[test]
    fn rejects_invalid_keyrings() {
        // Empty ring / empty current.
        assert!(ConnectorCredsStore::from_keyring("v1", &ring(&[])).is_none());
        assert!(ConnectorCredsStore::from_keyring("", &ring(&[("v1", "01".repeat(32))])).is_none());
        // `current` names a key that isn't in the ring.
        assert!(
            ConnectorCredsStore::from_keyring("v9", &ring(&[("v1", "01".repeat(32))])).is_none()
        );
        // Bad key material.
        assert!(ConnectorCredsStore::from_keyring("v1", &ring(&[("v1", "zz".to_string())])).is_none());
        assert!(
            ConnectorCredsStore::from_keyring("v1", &ring(&[("v1", "01".repeat(16))])).is_none(),
            "16-byte key is not AES-256"
        );
    }
}
