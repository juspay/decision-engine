//! Checkout.com `SettlementReportSource`.
//!
//! Port of Checkout.com's "Financial Actions" report onto the canonical [`SettledFeeRow`].
//! Everything Checkout-specific — report column labels, the fee decomposition, and the multi-row
//! grouping — is contained in this file; the queue, staging, fit, and serving never see it.
//!
//! Structural difference from the other connectors. Adyen/Braintree/Chase emit one
//! [`SettledFeeRow`] per CSV row. Checkout instead **fans one payment across many rows**, keyed by
//! `Payment ID` and split by `Action Type` × `Breakdown Type`.
//!
//! So `parse_rows` **groups by `Payment ID`** (an in-memory accumulator, O(distinct payments) —
//! fine for a daily report) and emits one aggregated row per captured payment after the scan.
//!
//! Fee model. This report carries **no interchange line item** — Checkout merchants on
//! blended/premium pricing have the processor take bundled into `Premium*` / `Blended*` / auth fee
//! rows, mirroring Braintree's bundled model:
//!   * `scheme_fee`  ← `Scheme Fixed Fee` + `Scheme Variable Fee`
//!   * `commission`  ← every other fee breakdown (`Premium*`, `Blended*`, `Authorization Fixed Fee`,
//!     `Authentication Fixed Fee`, `Card Verification Fixed Fee`)
//!   * `interchange` ← 0.0, `markup` ← 0.0
//!
//! and `total_fee = scheme_fee + commission`.
//!
//! Gross and every fee are read from the **Holding Currency Amount** column. Checkout mixes
//! currencies within one payment (gross in the processing currency, fixed fees in the holding
//! currency), so only the holding-currency column is additive — mirroring Adyen's use of the
//! settlement-currency `(SC)` columns. Fee rows are negative in the report and are negated to a
//! positive cost magnitude; capture rows are already positive.
//!
//! Refund / Partial Refund / Void actions are skipped whole (a "captures only" filter, matching the
//! other connectors' sale-only filters). Auth-only / declined payments carry auth fees but no
//! capture, so they never reach a `gross` and are dropped — they can't feed the `fee ~ gross` fit.
//!
//! Webhook ingestion. Checkout pushes a [`report_generated`] event (configured via a
//! Dashboard/Workflow webhook) when a scheduled report is ready. `verify_and_parse_notification`
//! checks the `Cko-Signature` HMAC over the raw body, keeps only Financial Actions reports, and
//! extracts the `rpt_…` handle; `download_report` then fetches the report's CSV file(s) from the
//! Reports API (`GET /reports/{id}` → file ids → `GET /reports/{id}/files/{fileId}`) with the stored
//! secret key. Credentials are keyed by the report's Checkout **entity id** — `peek_account` reads
//! `data.account.entity_id` from the *unverified* body so the handler can load that account's key.
//!
//! [`report_generated`]: https://www.checkout.com/docs/developer-resources/event-notifications/event-types/report_generated

use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Duration;

use async_trait::async_trait;
use axum::http::HeaderMap;
use bytes::Bytes;
use chrono::NaiveDate;
use masking::{PeekInterface, Secret};
use ring::hmac;
use serde::Deserialize;
use serde_json::Value;

use crate::cost_ingestion::connectors::csv_reader;
use crate::cost_ingestion::source::SettlementReportSource;
use crate::cost_ingestion::types::{
    ConnectorCreds, IngestError, ReportNotification, SettledFeeRow,
};

/// The `type` of the report-ready webhook event this connector handles.
const REPORT_GENERATED_EVENT: &str = "report_generated";

/// Cap on a report download (details + file fetches), mirroring the other connectors.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(120);

fn http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(DOWNLOAD_TIMEOUT)
            .build()
            .expect("failed to build checkout report reqwest client")
    })
}

/// Production Reports API host; overridable via the credential blob for sandbox/regional hosts.
fn default_api_base() -> String {
    "https://api.checkout.com".to_string()
}

/// Report-download credentials, carried inside the opaque [`ConnectorCreds::download_auth`]. The
/// preferred form is a JSON blob `{"secret_key":"sk_…","api_base_url":"…"}`; a bare `sk_…` string is
/// also accepted and defaults `api_base_url` to production.
#[derive(Debug, Clone, Deserialize)]
struct CheckoutCreds {
    /// Checkout secret key (`sk_…`), sent as the `Authorization: Bearer` credential.
    secret_key: String,
    #[serde(default = "default_api_base")]
    api_base_url: String,
}

impl CheckoutCreds {
    fn parse(creds: &ConnectorCreds) -> Result<Self, IngestError> {
        let raw = creds.download_auth.peek();
        // JSON blob first; fall back to treating the whole value as a bare secret key.
        if let Ok(c) = serde_json::from_str::<Self>(raw) {
            if !c.secret_key.trim().is_empty() {
                return Ok(c);
            }
        }
        let key = raw.trim();
        if key.is_empty() {
            return Err(IngestError::MalformedNotification(
                "checkout download_auth: missing secret key".to_string(),
            ));
        }
        Ok(Self {
            secret_key: key.to_string(),
            api_base_url: default_api_base(),
        })
    }

    /// The API host without a trailing slash, so `format!("{base}/reports/…")` never double-slashes.
    fn base(&self) -> &str {
        self.api_base_url.trim_end_matches('/')
    }
}

/// The `Get report details` response — only the file list is read.
#[derive(Deserialize)]
struct ReportDetailsResponse {
    #[serde(default)]
    files: Vec<ReportFileRef>,
}

#[derive(Deserialize)]
struct ReportFileRef {
    #[serde(default)]
    id: String,
}

pub struct CheckoutReportSource;

impl CheckoutReportSource {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CheckoutReportSource {
    fn default() -> Self {
        Self::new()
    }
}

/// Running aggregate for one `Payment ID` while scanning the report. Metadata is stamped from the
/// payment's capture row (authoritative and always present when we emit); fee components accumulate
/// across the payment's many rows.
#[derive(Default)]
struct PaymentAcc {
    txn_ref: String,
    card_network: String,
    funding: String,
    variant: String,
    issuer_country: String,
    currency: String,
    txn_date: Option<NaiveDate>,
    gross: f64,
    scheme_fee: f64,
    commission: f64,
    has_capture: bool,
}

impl PaymentAcc {
    /// Materialize the aggregate. Payments without a capture (auth-only/declined) have no gross and
    /// are dropped — they can't anchor the `fee ~ gross` regression.
    fn into_row(self) -> Option<SettledFeeRow> {
        if !self.has_capture {
            return None;
        }
        // Blended pricing: no interchange line, no separate markup. `total_fee` is the report's own
        // sum of the scheme + processor-take rows for this payment.
        let total_fee = self.scheme_fee + self.commission;
        Some(SettledFeeRow {
            txn_ref: self.txn_ref,
            card_network: self.card_network,
            variant: self.variant,
            funding: self.funding,
            issuer_country: self.issuer_country,
            currency: self.currency,
            // No single interchange category exists for blended pricing (`Fee Detail` is per-line
            // and noisy), so leave it empty — the rollup buckets all blended volume together.
            ic_category: String::new(),
            txn_date: self.txn_date,
            // Checkout is card-not-present online acceptance; no terminal id in the report.
            channel: "ecom".to_string(),
            gross: self.gross,
            total_fee,
            interchange: 0.0,
            scheme_fee: self.scheme_fee,
            markup: 0.0,
            commission: self.commission,
            // Checkout's blended report carries no PAN, so no BIN observation.
            bin: String::new(),
        })
    }
}

#[async_trait]
impl SettlementReportSource for CheckoutReportSource {
    fn connector(&self) -> &'static str {
        "checkout"
    }

    fn peek_account(&self, raw_body: &[u8]) -> Result<String, IngestError> {
        let event = parse_event(raw_body)?;
        account_id(&event).ok_or_else(|| {
            IngestError::MalformedNotification("missing data.account.entity_id".to_string())
        })
    }

    fn verify_and_parse_notification(
        &self,
        headers: &HeaderMap,
        raw_body: &[u8],
        secret: &Secret<String>,
    ) -> Result<ReportNotification, IngestError> {
        // Checkout signs the raw body with HMAC-SHA256 under the webhook signature key and sends the
        // hex-encoded digest in `Cko-Signature`. Verify before trusting any field of the payload.
        let provided = headers
            .get("cko-signature")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| {
                IngestError::MalformedNotification("missing Cko-Signature header".to_string())
            })?;
        if !verify_signature(raw_body, provided, secret.peek()) {
            return Err(IngestError::SignatureMismatch);
        }

        let event = parse_event(raw_body)?;

        let event_type = event
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if event_type != REPORT_GENERATED_EVENT {
            return Err(IngestError::MalformedNotification(format!(
                "unexpected event type '{event_type}', expected {REPORT_GENERATED_EVENT}"
            )));
        }

        let data = event
            .get("data")
            .ok_or_else(|| IngestError::MalformedNotification("missing data".to_string()))?;

        // Only the Financial Actions report matches this connector's parser; any other scheduled
        // report shares the webhook but not the columns, so reject it rather than enqueue a job the
        // worker could only fail to parse.
        let report_type = data
            .get("report_type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if !is_financial_actions_report(report_type) {
            return Err(IngestError::MalformedNotification(format!(
                "unsupported report_type '{report_type}' (checkout connector ingests Financial Actions only)"
            )));
        }

        // `report_id` (`rpt_…`) is the download handle passed back to `download_report`.
        let report_ref = data
            .get("report_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        if report_ref.is_empty() {
            return Err(IngestError::MalformedNotification(
                "missing data.report_id".to_string(),
            ));
        }

        // The event id (`evt_…`) is the queue's replay-idempotency key; fall back to the report id.
        let notification_id = event
            .get("id")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .unwrap_or(&report_ref)
            .to_string();

        let report_date = event
            .get("created_on")
            .and_then(Value::as_str)
            .and_then(|d| d.get(0..10))
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());

        let account = account_id(&event).unwrap_or_default();

        Ok(ReportNotification {
            notification_id,
            report_ref,
            report_date,
            account,
        })
    }

    async fn download_report(
        &self,
        creds: &ConnectorCreds,
        note: &ReportNotification,
    ) -> Result<Bytes, IngestError> {
        let cko = CheckoutCreds::parse(creds)?;
        let report_id = note.report_ref.trim();
        if report_id.is_empty() {
            return Err(IngestError::Download(
                "checkout: empty report id".to_string(),
            ));
        }

        // 1. Get report details → the file ids that make up this report.
        let details_url = format!("{}/reports/{}", cko.base(), report_id);
        let resp = http_client()
            .get(&details_url)
            .bearer_auth(&cko.secret_key)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| IngestError::Download(format!("checkout report details: {e}")))?;
        if !resp.status().is_success() {
            return Err(IngestError::Download(format!(
                "checkout report details status {}",
                resp.status()
            )));
        }
        let body = resp
            .bytes()
            .await
            .map_err(|e| IngestError::Download(e.to_string()))?;
        let details: ReportDetailsResponse = serde_json::from_slice(&body)
            .map_err(|e| IngestError::Download(format!("checkout report details parse: {e}")))?;
        let file_ids: Vec<&str> = details
            .files
            .iter()
            .map(|f| f.id.trim())
            .filter(|id| !id.is_empty())
            .collect();
        if file_ids.is_empty() {
            return Err(IngestError::Download(
                "checkout report has no files".to_string(),
            ));
        }

        // 2. Download each CSV file. A report over 1M rows is split across files, each carrying its
        // own header; concatenate them into one CSV, dropping the repeated header of every file
        // after the first so the shared CSV parser sees a single header + all rows.
        let mut out: Vec<u8> = Vec::new();
        for (i, file_id) in file_ids.iter().enumerate() {
            let file_url = format!("{}/reports/{}/files/{}", cko.base(), report_id, file_id);
            let resp = http_client()
                .get(&file_url)
                .bearer_auth(&cko.secret_key)
                .header("Accept", "text/csv")
                .send()
                .await
                .map_err(|e| IngestError::Download(format!("checkout report file: {e}")))?;
            if !resp.status().is_success() {
                return Err(IngestError::Download(format!(
                    "checkout report file status {}",
                    resp.status()
                )));
            }
            let bytes = resp
                .bytes()
                .await
                .map_err(|e| IngestError::Download(e.to_string()))?;
            if i == 0 {
                out.extend_from_slice(&bytes);
            } else {
                if !out.is_empty() && !out.ends_with(b"\n") {
                    out.push(b'\n');
                }
                out.extend_from_slice(strip_header(&bytes));
            }
        }
        Ok(Bytes::from(out))
    }

    /// One payment fans across a capture line plus its fee lines, which are accumulated and flushed
    /// at EOF — so a partially-read report ends with an incomplete payment.
    fn groups_rows(&self) -> bool {
        true
    }

    fn parse_rows(
        &self,
        reader: Box<dyn std::io::Read + Send>,
        mapping: &crate::cost_ingestion::mapping::ColumnMapping,
        on_row: &mut dyn FnMut(SettledFeeRow) -> Result<(), IngestError>,
    ) -> Result<(), IngestError> {
        // Resolved column indices for one report. Field order can drift between report versions, so
        // never index positionally.
        struct Cols {
            payment_id: usize,
            action_type: usize,
            breakdown_type: usize,
            holding_currency: usize,
            holding_amount: usize,
            payment_method: usize,
            card_type: usize,
            issuer_country: usize,
            processed_on: usize,
        }

        // One payment fans across many rows, so we accumulate rather than emit per row. Bounded by
        // the distinct-payment count (hundreds/thousands in a daily report), not the row count.
        let mut acc: HashMap<String, PaymentAcc> = HashMap::new();

        // `parse` emits per row; we never emit mid-scan (map_row always returns `Ok(None)`) and
        // instead fold into `acc`, so its `on_row` is a discard. The real `on_row` runs after.
        let mut discard = |_row: SettledFeeRow| -> Result<(), IngestError> { Ok(()) };
        csv_reader::parse(
            reader,
            mapping,
            |h| {
                Ok(Cols {
                    payment_id: h.require("Payment ID")?,
                    action_type: h.require("Action Type")?,
                    breakdown_type: h.require("Breakdown Type")?,
                    holding_currency: h.require("Holding Currency")?,
                    holding_amount: h.require("Holding Currency Amount")?,
                    payment_method: h.require("Payment Method")?,
                    card_type: h.require("Card Type")?,
                    issuer_country: h.require("Issuer Country")?,
                    processed_on: h.require("Processed On")?,
                })
            },
            |c, row| {
                // Skip the refund/void side entirely — their signed amounts would distort the
                // gross→fee regression (mirrors the other connectors' sale-only filter).
                let action = row.get(c.action_type).trim().to_lowercase();
                if matches!(action.as_str(), "refund" | "partial refund" | "void") {
                    return Ok(None);
                }
                let pid = row.get(c.payment_id).trim();
                if pid.is_empty() {
                    return Ok(None);
                }

                let breakdown = row.get(c.breakdown_type).trim().to_lowercase();
                let amount = to_float(row.get(c.holding_amount));
                let entry = acc.entry(pid.to_string()).or_default();

                if matches!(breakdown.as_str(), "capture" | "partial capture") {
                    // The money-movement line: gross (positive). Sum handles partial captures.
                    entry.gross += amount;
                    entry.has_capture = true;
                    // Stamp metadata from the capture row (authoritative, always present here).
                    entry.txn_ref = pid.to_string();
                    let network = normalize_network(row.get(c.payment_method));
                    let funding = funding_from_card_type(row.get(c.card_type));
                    entry.variant = build_variant(&network, &funding);
                    entry.card_network = network;
                    entry.funding = funding;
                    entry.issuer_country = row.get(c.issuer_country).trim().to_string();
                    entry.currency = row.get(c.holding_currency).trim().to_string();
                    entry.txn_date = parse_date(row.get(c.processed_on));
                } else if breakdown.starts_with("scheme ") {
                    // `Scheme Fixed Fee` / `Scheme Variable Fee` — the card-scheme pass-through.
                    entry.scheme_fee += -amount;
                } else {
                    // Every other fee breakdown (Premium/Blended, auth, authentication, card
                    // verification) is Checkout's bundled processor take.
                    entry.commission += -amount;
                }
                Ok(None)
            },
            &mut discard,
        )?;

        // Flush: one aggregated row per captured payment.
        for (_pid, payment) in acc {
            if let Some(row) = payment.into_row() {
                on_row(row)?;
            }
        }
        Ok(())
    }
}

/// Parse the webhook body as JSON, or a typed error.
fn parse_event(raw_body: &[u8]) -> Result<Value, IngestError> {
    serde_json::from_slice(raw_body).map_err(|e| {
        IngestError::MalformedNotification(format!("checkout webhook body is not JSON: {e}"))
    })
}

/// The Checkout account the report relates to: the entity id (`ent_…`), falling back to the client
/// id (`cli_…`). This is the value stored as the credential `account`, so the webhook handler can
/// load the right signing key and merchant from the *unverified* body.
fn account_id(event: &Value) -> Option<String> {
    let account = event.get("data")?.get("account")?;
    let id = account
        .get("entity_id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())
        .or_else(|| {
            account
                .get("client_id")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
        })?;
    Some(id.to_string())
}

/// Whether `report_type` is one of the Financial Actions reports this connector's parser handles
/// (by date range or by payout id — same columns). Compared case-insensitively for safety.
fn is_financial_actions_report(report_type: &str) -> bool {
    let t = report_type.trim();
    t.eq_ignore_ascii_case("FinancialActions") || t.eq_ignore_ascii_case("FinancialActionsByPayout")
}

/// Verify Checkout's `Cko-Signature`: hex-encoded HMAC-SHA256 of the raw body, keyed by the webhook
/// signature key (used as raw UTF-8 bytes). Compared in constant time; the provided value is
/// lowercased first because Checkout emits lowercase hex.
fn verify_signature(raw_body: &[u8], provided_hex: &str, key: &str) -> bool {
    let mac = hmac::Key::new(hmac::HMAC_SHA256, key.as_bytes());
    let tag = hmac::sign(&mac, raw_body);
    let expected = hex_encode(tag.as_ref());
    let provided = provided_hex.trim().to_ascii_lowercase();
    ring::constant_time::verify_slices_are_equal(expected.as_bytes(), provided.as_bytes()).is_ok()
}

/// Lowercase hex encoding (Base16) of a byte slice.
fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(char::from_digit((b >> 4) as u32, 16).unwrap());
        s.push(char::from_digit((b & 0x0f) as u32, 16).unwrap());
    }
    s
}

/// Drop the leading header line (up to and including the first newline) of a split report file, so
/// only the first file's header survives concatenation. An input with no newline is all-header (no
/// data rows) and yields an empty slice.
fn strip_header(bytes: &[u8]) -> &[u8] {
    match bytes.iter().position(|&b| b == b'\n') {
        Some(i) => &bytes[i + 1..],
        None => &[],
    }
}

/// Map Checkout's `Card Type` (`Credit` / `Debit` / blank) onto the canonical funding bucket.
fn funding_from_card_type(card_type: &str) -> String {
    match card_type.trim().to_lowercase().as_str() {
        "debit" => "debit".to_string(),
        "credit" => "credit".to_string(),
        _ => String::new(),
    }
}

/// Canonicalize Checkout's `Payment Method` label (`VISA`, `MASTERCARD`, `AMEX`, …) to the
/// lowercased network ids the rest of the pipeline uses (`visa`, `mc`, `amex`, …).
fn normalize_network(raw: &str) -> String {
    match raw.trim().to_lowercase().as_str() {
        "" => String::new(),
        "mastercard" | "master card" => "mc".to_string(),
        "visa" => "visa".to_string(),
        "american express" | "amex" => "amex".to_string(),
        "discover" => "discover".to_string(),
        "diners" | "diners club" => "diners".to_string(),
        "jcb" => "jcb".to_string(),
        other => other.replace(' ', ""),
    }
}

/// Synthesize a `variant` cluster key. Checkout's report carries no scheme tier, so use
/// `{network}{funding}` (e.g. `visacredit`); mirrors Braintree's card path.
fn build_variant(network: &str, funding: &str) -> String {
    format!("{network}{funding}")
}

/// Parse a money cell; blanks/garbage become `0.0` (mirrors the other connectors' `to_float`).
fn to_float(s: &str) -> f64 {
    s.trim().parse::<f64>().unwrap_or(0.0)
}

/// Parse a Checkout date cell (`Processed On`), an ISO-8601 local timestamp
/// (`2026-07-09T18:07:06.844`), down to its date. Falls back to a plain `YYYY-MM-DD`; blank/odd
/// values yield `None`.
fn parse_date(s: &str) -> Option<NaiveDate> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if let Ok(dt) = chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f") {
        return Some(dt.date());
    }
    NaiveDate::parse_from_str(s, "%Y-%m-%d").ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One report exercising: scheme+premium fees, an auth fee folded into a captured payment, a
    /// blended payment whose partial refund is skipped, an auth-only payment (dropped), and a
    /// multi-partial-capture payment. Header order intentionally differs from `Cols` — indices
    /// resolve by label. Amounts are all in the holding currency (GBP) for a clean assertion.
    const REPORT: &str = "\
Payment ID,Action Type,Breakdown Type,Payment Method,Card Type,Issuer Country,Holding Currency,Holding Currency Amount,Processed On\n\
pay_1,Authorization,Authorization Fixed Fee,VISA,Credit,GB,GBP,-0.01,2026-07-09T18:07:06.844\n\
pay_1,Authorization,Scheme Variable Fee,VISA,Credit,GB,GBP,-0.05,2026-07-09T18:07:06.844\n\
pay_1,Capture,Capture,VISA,Credit,GB,GBP,50.00,2026-07-09T18:07:06.844\n\
pay_1,Capture,Premium Fixed Fee,VISA,Credit,GB,GBP,-0.02,2026-07-09T18:07:06.844\n\
pay_1,Capture,Premium Variable Fee,VISA,Credit,GB,GBP,-0.03,2026-07-09T18:07:06.844\n\
pay_1,Capture,Scheme Fixed Fee,VISA,Credit,GB,GBP,-0.04,2026-07-09T18:07:06.844\n\
pay_2,Capture,Capture,MASTERCARD,Debit,US,GBP,20.00,2026-07-09T10:00:00.000\n\
pay_2,Capture,Blended Fixed Fee,MASTERCARD,Debit,US,GBP,-0.01,2026-07-09T10:00:00.000\n\
pay_2,Capture,Blended Variable Fee,MASTERCARD,Debit,US,GBP,-0.02,2026-07-09T10:00:00.000\n\
pay_2,Partial Refund,Refund,MASTERCARD,Debit,US,GBP,-5.00,2026-07-09T10:00:00.000\n\
pay_2,Partial Refund,Refund Fixed Fee,MASTERCARD,Debit,US,GBP,-0.01,2026-07-09T10:00:00.000\n\
pay_3,Authorization,Authorization Fixed Fee,VISA,Credit,GB,GBP,-0.01,2026-07-09T11:00:00.000\n\
pay_3,Authorization,Scheme Fixed Fee,VISA,Credit,GB,GBP,-0.003,2026-07-09T11:00:00.000\n\
pay_4,Partial Capture,Partial Capture,AMEX,Credit,GB,GBP,30.00,2026-07-09T12:00:00.000\n\
pay_4,Partial Capture,Partial Capture,AMEX,Credit,GB,GBP,20.00,2026-07-09T12:00:00.000\n";

    fn parse() -> Vec<SettledFeeRow> {
        CheckoutReportSource::new()
            .parse_report(REPORT.as_bytes())
            .unwrap()
    }

    fn by_ref<'a>(rows: &'a [SettledFeeRow], txn_ref: &str) -> &'a SettledFeeRow {
        rows.iter()
            .find(|r| r.txn_ref == txn_ref)
            .unwrap_or_else(|| panic!("no row for {txn_ref}"))
    }

    #[test]
    fn groups_payment_and_decomposes_fees() {
        let rows = parse();
        // pay_1 (captured) and pay_2 (captured) and pay_4 (partial-captured) are emitted; pay_3
        // (auth-only) is not.
        assert_eq!(
            rows.len(),
            3,
            "one row per captured payment; auth-only dropped"
        );

        let p1 = by_ref(&rows, "pay_1");
        assert_eq!(p1.card_network, "visa");
        assert_eq!(p1.funding, "credit");
        assert_eq!(p1.variant, "visacredit");
        assert_eq!(p1.issuer_country, "GB");
        assert_eq!(p1.currency, "GBP");
        assert_eq!(p1.ic_category, "");
        assert_eq!(p1.channel, "ecom");
        assert_eq!(p1.interchange, 0.0);
        assert_eq!(p1.markup, 0.0);
        assert!((p1.gross - 50.00).abs() < 1e-9);
        // scheme = 0.05 (auth) + 0.04 (capture); commission = 0.01 (auth) + 0.02 + 0.03.
        assert!((p1.scheme_fee - 0.09).abs() < 1e-9);
        assert!((p1.commission - 0.06).abs() < 1e-9);
        assert!((p1.total_fee - 0.15).abs() < 1e-9);
        assert_eq!(p1.txn_date, NaiveDate::from_ymd_opt(2026, 7, 9));
    }

    #[test]
    fn refund_rows_are_skipped_not_netted() {
        let p2 = parse();
        let p2 = by_ref(&p2, "pay_2");
        assert_eq!(p2.card_network, "mc", "MASTERCARD -> mc");
        assert_eq!(p2.funding, "debit");
        assert_eq!(p2.variant, "mcdebit");
        assert!(
            (p2.gross - 20.00).abs() < 1e-9,
            "refund not subtracted from gross"
        );
        assert_eq!(p2.scheme_fee, 0.0);
        assert!(
            (p2.commission - 0.03).abs() < 1e-9,
            "blended fees only; refund fee skipped"
        );
        assert!((p2.total_fee - 0.03).abs() < 1e-9);
    }

    #[test]
    fn auth_only_payment_is_dropped() {
        assert!(
            parse().iter().all(|r| r.txn_ref != "pay_3"),
            "a payment with no capture yields no row"
        );
    }

    #[test]
    fn partial_captures_sum_into_gross() {
        let p4 = parse();
        let p4 = by_ref(&p4, "pay_4");
        assert_eq!(p4.card_network, "amex");
        assert_eq!(p4.variant, "amexcredit");
        assert!((p4.gross - 50.00).abs() < 1e-9, "30 + 20 partial captures");
    }

    #[test]
    fn missing_required_column_errors() {
        let csv = "Payment ID,Action Type\npay_x,Capture\n";
        let err = CheckoutReportSource::new()
            .parse_report(csv.as_bytes())
            .unwrap_err();
        let IngestError::MissingColumns {
            missing, required, ..
        } = err
        else {
            panic!("expected MissingColumns, got {err:?}");
        };
        // Every miss is reported at once, not just the first one resolved.
        assert_eq!(missing.len(), required.len() - 2, "all but the two present");
        assert!(missing.contains(&"Holding Currency Amount".to_string()));
        assert!(!missing.contains(&"Payment ID".to_string()));
    }

    #[test]
    fn helpers() {
        assert_eq!(funding_from_card_type("Debit"), "debit");
        assert_eq!(funding_from_card_type("Credit"), "credit");
        assert_eq!(funding_from_card_type(""), "");
        assert_eq!(normalize_network("VISA"), "visa");
        assert_eq!(normalize_network("MASTERCARD"), "mc");
        assert_eq!(normalize_network("AMEX"), "amex");
        assert_eq!(normalize_network(""), "");
        assert_eq!(build_variant("visa", "credit"), "visacredit");
        assert_eq!(
            parse_date("2026-07-09T18:07:06.844"),
            NaiveDate::from_ymd_opt(2026, 7, 9)
        );
        assert_eq!(
            parse_date("2026-07-09"),
            NaiveDate::from_ymd_opt(2026, 7, 9)
        );
        assert_eq!(parse_date(""), None);
        assert_eq!(parse_date("garbage"), None);
    }

    // ── Webhook (report_generated) tests ──

    /// A `report_generated` event, shaped after Checkout's documented example.
    const EVENT: &str = r#"{
      "id": "evt_jaudu78q7duakmcakady18djass",
      "type": "report_generated",
      "version": "1.1.0",
      "created_on": "2026-07-10T15:24:13.8431084Z",
      "data": {
        "report_id": "rpt_bzhlovfpy32e3fbveixijllwfe",
        "report_type": "FinancialActions",
        "account": {
          "client_id": "cli_urq524lg2lzevf6qapnfwrvyee",
          "entity_id": "ent_r7nge7vl53crsa3ozjxzoiykj4"
        }
      },
      "_links": { "self": { "href": "https://api.checkout.com/workflows/events/evt_x" } }
    }"#;

    const KEY: &str = "sig_key_secret";

    /// Compute Checkout's hex HMAC-SHA256 signature the way the connector verifies it.
    fn sign(body: &[u8], key: &str) -> String {
        let mac = hmac::Key::new(hmac::HMAC_SHA256, key.as_bytes());
        hex_encode(hmac::sign(&mac, body).as_ref())
    }

    fn headers_with_sig(sig: &str) -> HeaderMap {
        let mut h = HeaderMap::new();
        h.insert("cko-signature", sig.parse().unwrap());
        h
    }

    #[test]
    fn peek_account_prefers_entity_then_client() {
        let src = CheckoutReportSource::new();
        assert_eq!(
            src.peek_account(EVENT.as_bytes()).unwrap(),
            "ent_r7nge7vl53crsa3ozjxzoiykj4"
        );
        // No entity_id -> fall back to client_id.
        let body = r#"{"data":{"account":{"client_id":"cli_x"}}}"#;
        assert_eq!(src.peek_account(body.as_bytes()).unwrap(), "cli_x");
        // No account at all -> a clear error.
        let bad = r#"{"data":{}}"#;
        assert!(matches!(
            src.peek_account(bad.as_bytes()),
            Err(IngestError::MalformedNotification(_))
        ));
    }

    #[test]
    fn verifies_signature_and_extracts_handle() {
        let src = CheckoutReportSource::new();
        let secret = Secret::new(KEY.to_string());
        let headers = headers_with_sig(&sign(EVENT.as_bytes(), KEY));
        let note = src
            .verify_and_parse_notification(&headers, EVENT.as_bytes(), &secret)
            .unwrap();
        assert_eq!(note.notification_id, "evt_jaudu78q7duakmcakady18djass");
        assert_eq!(note.report_ref, "rpt_bzhlovfpy32e3fbveixijllwfe");
        assert_eq!(note.account, "ent_r7nge7vl53crsa3ozjxzoiykj4");
        assert_eq!(note.report_date, NaiveDate::from_ymd_opt(2026, 7, 10));
    }

    #[test]
    fn wrong_signature_is_rejected() {
        let src = CheckoutReportSource::new();
        let secret = Secret::new(KEY.to_string());
        // Signed with a different key.
        let headers = headers_with_sig(&sign(EVENT.as_bytes(), "other_key"));
        assert!(matches!(
            src.verify_and_parse_notification(&headers, EVENT.as_bytes(), &secret),
            Err(IngestError::SignatureMismatch)
        ));
        // Missing header entirely.
        assert!(matches!(
            src.verify_and_parse_notification(&HeaderMap::new(), EVENT.as_bytes(), &secret),
            Err(IngestError::MalformedNotification(_))
        ));
    }

    #[test]
    fn non_report_or_unsupported_type_rejected() {
        let src = CheckoutReportSource::new();
        let secret = Secret::new(KEY.to_string());

        // A correctly-signed but wrong event type.
        let other = r#"{"type":"payment_approved","data":{"report_id":"rpt_x","report_type":"FinancialActions","account":{"entity_id":"ent_x"}}}"#;
        let h = headers_with_sig(&sign(other.as_bytes(), KEY));
        assert!(matches!(
            src.verify_and_parse_notification(&h, other.as_bytes(), &secret),
            Err(IngestError::MalformedNotification(_))
        ));

        // A report_generated for a report type this connector can't parse.
        let payments = r#"{"type":"report_generated","data":{"report_id":"rpt_x","report_type":"Payments","account":{"entity_id":"ent_x"}}}"#;
        let h = headers_with_sig(&sign(payments.as_bytes(), KEY));
        assert!(matches!(
            src.verify_and_parse_notification(&h, payments.as_bytes(), &secret),
            Err(IngestError::MalformedNotification(_))
        ));
    }

    #[test]
    fn webhook_helpers() {
        assert!(is_financial_actions_report("FinancialActions"));
        assert!(is_financial_actions_report("FinancialActionsByPayout"));
        assert!(is_financial_actions_report(" financialactions "));
        assert!(!is_financial_actions_report("Payments"));
        assert!(!is_financial_actions_report(""));

        assert_eq!(hex_encode(&[0x00, 0x0f, 0xa9, 0xff]), "000fa9ff");

        // strip_header drops the first line; first-file bytes are kept verbatim by the caller.
        assert_eq!(strip_header(b"h1,h2\na,b\nc,d\n"), b"a,b\nc,d\n");
        assert_eq!(strip_header(b"only-header-no-newline"), b"");

        // download_auth as a JSON blob, and as a bare secret key.
        let json = ConnectorCreds {
            webhook_secret: Secret::new(String::new()),
            download_auth: Secret::new(
                r#"{"secret_key":"sk_123","api_base_url":"https://api.sandbox.checkout.com/"}"#
                    .to_string(),
            ),
        };
        let c = CheckoutCreds::parse(&json).unwrap();
        assert_eq!(c.secret_key, "sk_123");
        assert_eq!(c.base(), "https://api.sandbox.checkout.com");

        let bare = ConnectorCreds {
            webhook_secret: Secret::new(String::new()),
            download_auth: Secret::new("sk_bare".to_string()),
        };
        let c = CheckoutCreds::parse(&bare).unwrap();
        assert_eq!(c.secret_key, "sk_bare");
        assert_eq!(c.base(), "https://api.checkout.com");

        let empty = ConnectorCreds {
            webhook_secret: Secret::new(String::new()),
            download_auth: Secret::new("   ".to_string()),
        };
        assert!(matches!(
            CheckoutCreds::parse(&empty),
            Err(IngestError::MalformedNotification(_))
        ));
    }
}
