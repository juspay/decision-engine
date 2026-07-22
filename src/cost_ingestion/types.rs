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
    /// Connector's per-transaction id when the report carries one (e.g. Adyen `Psp Reference`),
    /// else a synthetic best-effort key (Stripe's aggregated report). Provenance only: the rollup
    /// aggregates by the cluster fields below (read straight off this struct) and nothing
    /// downstream reads `txn_ref`, so it is NOT a dedup key.
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
    /// Issuer BIN (leading PAN digits) when the report carries the card number, else `""`. Seeds the
    /// global `cost_bin_product` map that resolves the card product for co-badged schemes whose
    /// `variant` leaves `funding` blank (Open Risk #4). Never itself a fit dimension.
    pub bin: String,
}

/// Base-10 **log** amount bucket ([`BUCKETS_PER_DECADE`] per decade), as a string. This replaces the
/// old fixed-unit bands (`20/50/100/250`), which were currency-BLIND and too coarse for the fit's
/// `a*` crossover: a €2 `a*` sat inside one band, and a HUF cluster put all real volume in one band.
/// A log bucket is a fixed *ratio*, so `a*` resolves at the same relative precision in any currency.
/// Shared by the rollup (stamps each txn) and the decide-time predictor lookup in `serving.rs`, so
/// the two can never drift. Bucket `k` spans `[10^(k/10), 10^((k+1)/10))`; the fit recovers a
/// bucket's lower amount as `pow(10, k/10)` to decide which buckets lie above `a*`.
pub const BUCKETS_PER_DECADE: f64 = 10.0;
pub fn amount_band(amount: f64) -> String {
    if amount <= 0.0 {
        return "0".to_string();
    }
    ((amount.log10() * BUCKETS_PER_DECADE).floor() as i64).to_string()
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

    /// Resolve funding for the cluster key: the variant when it encodes it (`mc*`/`visa*`), else —
    /// for co-badged schemes (`cartebancaire`) whose variant is silent — infer from the stated
    /// interchange rate. The report exposes no product field for these cards; the rate is the only
    /// signal, and it is not arbitrary — the observed values cluster on the EU Interchange Fee
    /// Regulation caps: 0.20% (20 bps) consumer debit, 0.30% (30 bps) consumer credit, with
    /// commercial cards exempt from the caps and running higher (~90 bps). We map each band to its
    /// product: `<= 25` debit, `<= 60` credit, above commercial. A heuristic bootstrap; when
    /// `cards_info` (BIN → funding) is fed it supersedes this.
    /// `None`/absent rate ⇒ unresolved (`""`) ⇒ the cluster abstains, exactly as today.
    pub fn resolve_funding(variant: &str, ic_bps: Option<f64>) -> String {
        let f = Self::funding_from_variant(variant);
        if !f.is_empty() {
            return f;
        }
        match ic_bps {
            Some(b) if b > 0.0 && b <= 25.0 => "debit".to_string(),
            Some(b) if b > 25.0 && b <= 60.0 => "credit".to_string(),
            Some(b) if b > 60.0 => "commercial".to_string(),
            _ => String::new(),
        }
    }

    /// Extract the issuer BIN from a PAN as the report states it — reports mask the middle
    /// (`489678****4354`), so we take the leading run of digits, capped at 8 (the modern BIN
    /// length). Returns `""` when the field is empty or non-numeric (trimmed/tokenized reports),
    /// which simply means this row contributes no BIN observation.
    pub fn bin_from_pan(pan: &str) -> String {
        pan.trim()
            .chars()
            .take_while(|c| c.is_ascii_digit())
            .take(8)
            .collect()
    }
}

/// A report instance a *pull* connector has found ready to ingest (from listing its reporting API).
/// The connector-agnostic report poller enqueues one job per [`ReadyReport`]; the download → parse →
/// fit path is then identical to a webhook-delivered report.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadyReport {
    /// Connector's unique report id — the queue's idempotency key (`notification_id`).
    pub report_id: String,
    /// Opaque handle used to fetch the report (a URL, a filename, …). Connector-specific; stored as
    /// the job's `report_ref` and passed back to `download_report`.
    pub report_ref: String,
    pub period_start: Option<NaiveDate>,
    pub period_end: Option<NaiveDate>,
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
    /// The header row resolved, but one or more required columns were absent. Distinct from
    /// [`Self::Parse`] because it is *actionable by the merchant* — it names every missing column at
    /// once (not just the first) alongside the labels this connector expects and the ones the file
    /// actually carried, which is what the upload preflight renders. Also produced deliberately by
    /// probing a connector with an empty header row, where `missing == required` — that is how
    /// `preflight` enumerates a connector's schema without duplicating any column list.
    #[error("report is missing required column(s): {}", .missing.join(", "))]
    MissingColumns {
        /// Required labels absent from the file, in the order the connector resolves them.
        missing: Vec<String>,
        /// Every label this connector requires.
        required: Vec<String>,
        /// Every label this connector reads if present but tolerates the absence of.
        optional: Vec<String>,
        /// The header labels the uploaded file actually carried.
        found: Vec<String>,
    },
    #[error("credential encryption/decryption failed: {0}")]
    Crypto(String),
    #[error("credential storage failed: {0}")]
    Storage(String),
}
