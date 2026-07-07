//! Canonical, connector-agnostic types for settlement-report ingestion.
//!
//! Every connector's native report is normalized into [`SettledFeeRow`]; once a report
//! reaches that shape, staging, the OLS fit, and serving are identical regardless of which
//! connector produced it (see `scratch/inhouse-cost-architecture.md` §7).

use chrono::NaiveDate;
use masking::Secret;

/// One normalized settled transaction — the atom the cost fit consumes.
///
/// Deliberately free of ingestion context: `(connector, merchant_id, report_date)` are
/// stamped by the worker from the queue row, so the parser stays reusable and testable.
#[derive(Debug, Clone, PartialEq)]
pub struct SettledFeeRow {
    /// Connector's unique transaction id (e.g. Adyen `Psp Reference`) — the staging dedup key.
    pub txn_ref: String,
    /// Card network, lowercased: `visa`, `mc`, …
    pub card_network: String,
    /// Payment-method variant (carries tier + funding), lowercased: `visastandarddebit`, …
    pub variant: String,
    /// Funding type derived from the variant: `debit` | `credit` | `""` when neither.
    pub funding: String,
    /// Issuer country (as the report states it): `FR`, `IT`, …
    pub issuer_country: String,
    /// Settlement currency: `EUR`, `AUD`, …
    pub currency: String,
    /// Interchange category from the report; `""` for flat-fee methods (iDEAL/Klarna/CB).
    pub ic_category: String,
    /// Transaction (booking) date, when the report carries one. Not staged into ClickHouse — used
    /// only to compute the ingested report's period (min/max) for the history record.
    pub txn_date: Option<NaiveDate>,
    /// Acceptance channel derived from the report: `pos` (a terminal id was present) vs `ecom`
    /// (none). Not a fit dimension — a *predictor* feature that resolves the POS/online category
    /// ambiguity at decide time (see §8/§9).
    pub channel: String,
    /// Gross settlement value (`payable + total_fee`) — the regression's `x`.
    pub gross: f64,
    /// Total fee charged (`interchange + scheme + markup + commission`) — the regression's `y`.
    pub total_fee: f64,
    /// Fee components, kept split so the shared-interchange vs per-connector-markup model
    /// (§3.3) is computable later without re-ingesting.
    pub interchange: f64,
    pub scheme_fee: f64,
    pub markup: f64,
    pub commission: f64,
}

impl SettledFeeRow {
    /// Map a variant string onto a funding bucket. Case-insensitive substring match, mirroring
    /// the Python `par_extract` behavior. Returns `""` for methods that are neither (iDEAL, …).
    pub fn funding_from_variant(variant: &str) -> String {
        let v = variant.to_lowercase();
        if v.contains("debit") {
            "debit".to_string()
        } else if v.contains("credit") {
            "credit".to_string()
        } else {
            String::new()
        }
    }
}

/// A "report is ready" event, extracted from a (verified) connector webhook.
#[derive(Debug, Clone)]
pub struct ReportNotification {
    /// Connector's unique notification/event id — the queue's replay-idempotency key.
    pub notification_id: String,
    /// Opaque handle used to fetch the report (a URL, a filename, …). Connector-specific.
    pub report_ref: String,
    /// The report's period, when the notification carries it.
    pub report_date: Option<NaiveDate>,
    /// Connector-side account the report belongs to (e.g. Adyen `merchantAccountCode`), used
    /// to resolve *our* merchant id. Distinct from our internal `merchant_id`.
    pub account: String,
}

/// Per-(merchant, connector) credentials, held in memory only long enough to use. Loaded and
/// decrypted from storage by the worker; never logged.
#[derive(Debug, Clone)]
pub struct ConnectorCreds {
    /// Secret used to verify inbound webhook signatures (e.g. Adyen HMAC key).
    pub webhook_secret: Secret<String>,
    /// Credential used to authenticate the report download (report-user basic auth / API key).
    pub download_auth: Secret<String>,
}

/// Failures across the ingestion path. Kept coarse — the worker logs and retries/parks based
/// on the variant, and none of these should ever surface to the connector's webhook caller
/// (which always gets a fast ACK).
#[derive(Debug, thiserror::Error)]
pub enum IngestError {
    #[error("no connector registered for '{0}'")]
    UnknownConnector(String),
    #[error("webhook signature verification failed")]
    SignatureMismatch,
    #[error("malformed notification: {0}")]
    MalformedNotification(String),
    #[error("report download failed: {0}")]
    Download(String),
    #[error("report parse failed: {0}")]
    Parse(String),
    #[error("credential encryption/decryption failed: {0}")]
    Crypto(String),
    #[error("credential storage failed: {0}")]
    Storage(String),
}
