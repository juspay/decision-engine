//! Adyen `SettlementReportSource` — the first connector.
//!
//! The report parser is a faithful port of `scratch/par_extract.py`: keep settled rows, project
//! the fee columns, pull the interchange category out of the `ICSF details` JSON, and derive
//! `gross`/`total_fee`. Everything Adyen-specific (report column names, the notification shape,
//! the HMAC scheme, the download auth) is contained in this file.

use std::sync::OnceLock;
use std::time::Duration;

use async_trait::async_trait;
use axum::http::HeaderMap;
use bytes::Bytes;
use masking::{PeekInterface, Secret};
use ring::hmac;
use serde_json::Value;

use crate::cost_ingestion::connectors::csv_reader;
use crate::cost_ingestion::source::SettlementReportSource;
use crate::cost_ingestion::types::{
    ConnectorCreds, IngestError, ReportNotification, SettledFeeRow,
};

/// Adyen record type that carries the final settled fee signal. PAR also includes
/// `SentForSettle`, but `cluster_explorer.py` keeps one settled leg only so the same transaction
/// does not affect the fee fit twice.
const FEE_RECORD_TYPES: [&str; 1] = ["Settled"];

const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(120);

pub struct AdyenReportSource;

impl AdyenReportSource {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AdyenReportSource {
    fn default() -> Self {
        Self::new()
    }
}

fn http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(DOWNLOAD_TIMEOUT)
            .build()
            .expect("failed to build adyen report reqwest client")
    })
}

#[async_trait]
impl SettlementReportSource for AdyenReportSource {
    fn connector(&self) -> &'static str {
        "adyen"
    }

    fn peek_account(&self, raw_body: &[u8]) -> Result<String, IngestError> {
        let item = notification_item(raw_body)?;
        item.get("merchantAccountCode")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| {
                IngestError::MalformedNotification("missing merchantAccountCode".to_string())
            })
    }

    fn verify_and_parse_notification(
        &self,
        _headers: &HeaderMap,
        raw_body: &[u8],
        secret: &Secret<String>,
    ) -> Result<ReportNotification, IngestError> {
        let item = notification_item(raw_body)?;

        // Adyen carries the HMAC in the body (additionalData.hmacSignature), not a header.
        let provided_sig = item
            .get("additionalData")
            .and_then(|a| a.get("hmacSignature"))
            .and_then(Value::as_str)
            .ok_or_else(|| {
                IngestError::MalformedNotification("missing hmacSignature".to_string())
            })?;

        // NOTE: field set/ordering below follows Adyen's *standard* notification signing scheme.
        // The exact payload for REPORT_AVAILABLE must be confirmed against Adyen docs before
        // production (see architecture doc §7.7) — the HMAC *mechanism* here is correct.
        let payload = hmac_payload(&item);
        if !verify_hmac(&payload, provided_sig, secret.peek()) {
            return Err(IngestError::SignatureMismatch);
        }

        let account = item
            .get("merchantAccountCode")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        // For REPORT_AVAILABLE the download URL is carried in `reason`; `pspReference` is unique.
        let report_ref = item
            .get("reason")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let notification_id = item
            .get("pspReference")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .unwrap_or(&report_ref)
            .to_string();
        let report_date = item
            .get("eventDate")
            .and_then(Value::as_str)
            .and_then(|d| d.get(0..10))
            .and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok());

        if report_ref.is_empty() {
            return Err(IngestError::MalformedNotification(
                "no report reference in notification".to_string(),
            ));
        }

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
        // Adyen accepts two ways to authenticate a report download (see
        // https://docs.adyen.com/reporting/automatically-get-reports): report-user Basic auth, or a
        // Report Service API key sent as `X-API-Key`. We store one string in `download_auth` and
        // disambiguate by the ':' — Basic auth is always "user:password"; an Adyen API key never
        // contains a colon, so a colon-less value is treated as the API key.
        let auth = creds.download_auth.peek();
        let request = http_client().get(&note.report_ref);
        let request = match auth.split_once(':') {
            Some((user, pass)) => request.basic_auth(user, Some(pass)),
            None => request.header("X-API-Key", auth),
        };

        let resp = request
            .send()
            .await
            .map_err(|e| IngestError::Download(e.to_string()))?;

        if !resp.status().is_success() {
            return Err(IngestError::Download(format!("status {}", resp.status())));
        }
        resp.bytes()
            .await
            .map_err(|e| IngestError::Download(e.to_string()))
    }

    fn parse_rows(
        &self,
        reader: Box<dyn std::io::Read + Send>,
        mapping: &crate::cost_ingestion::mapping::ColumnMapping,
        on_row: &mut dyn FnMut(SettledFeeRow) -> Result<(), IngestError>,
    ) -> Result<(), IngestError> {
        // Resolved column indices for one report (order can drift between report versions).
        struct Cols {
            record: usize,
            psp: usize,
            variant: usize,
            brand: usize,
            issuer: usize,
            ccy: usize,
            payable: usize,
            commission: usize,
            markup: usize,
            scheme: usize,
            interchange: usize,
            icsf: usize,
            merchant_account: Option<usize>,
            booking: Option<usize>,
            terminal: Option<usize>,
        }

        csv_reader::parse(
            reader,
            mapping,
            |h| {
                Ok(Cols {
                    record: h.require("Record Type")?,
                    psp: h.require("Psp Reference")?,
                    variant: h.require("Payment Method Variant")?,
                    brand: h.require("Global Card Brand")?,
                    issuer: h.require("Issuer Country")?,
                    ccy: h.require("Settlement Currency")?,
                    payable: h.require("Payable (SC)")?,
                    commission: h.require("Commission (SC)")?,
                    markup: h.require("Markup (SC)")?,
                    scheme: h.require("Scheme Fees (SC)")?,
                    interchange: h.require("Interchange (SC)")?,
                    icsf: h.require("ICSF details")?,
                    // Optional in tests/older fixtures; present in Adyen accounting reports and
                    // needed to match cluster_explorer.py's per-merchant-account split.
                    merchant_account: h.index("Merchant Account"),
                    // Optional: used only for the ingested report's period; absent in older/test reports.
                    booking: h.index("Booking Date"),
                    // Optional: a terminal id marks in-person (POS) acceptance; absence ⇒ online (ecom).
                    // Drives the channel feature of the category predictor.
                    terminal: h.index("Unique Terminal ID"),
                })
            },
            |c, row| {
                // Skip non-fee rows before any field extraction — this is the ~90% majority.
                if !FEE_RECORD_TYPES.contains(&row.get(c.record)) {
                    return Ok(None);
                }

                let commission = to_float(row.get(c.commission));
                let markup = to_float(row.get(c.markup));
                let scheme_fee = to_float(row.get(c.scheme));
                let interchange = to_float(row.get(c.interchange));
                let total_fee = commission + markup + scheme_fee + interchange;
                let gross = to_float(row.get(c.payable)) + total_fee;

                let variant = row.get(c.variant).to_lowercase();
                let funding = SettledFeeRow::funding_from_variant(&variant);
                let txn_date = c.booking.and_then(|i| parse_booking_date(row.get(i)));
                // POS when a terminal id is present, else online. Absent column ⇒ unknown ⇒ ecom.
                let channel = match c.terminal {
                    Some(i) if !row.get(i).trim().is_empty() => "pos",
                    _ => "ecom",
                }
                .to_string();

                let (ic_category, interchange_bps) = ic_details(row.get(c.icsf));

                Ok(Some(SettledFeeRow {
                    txn_ref: row.get(c.psp).to_string(),
                    report_account: c
                        .merchant_account
                        .map(|i| row.get(i).trim().to_string())
                        .unwrap_or_default(),
                    card_network: row.get(c.brand).to_lowercase(),
                    variant,
                    funding,
                    issuer_country: row.get(c.issuer).to_string(),
                    currency: row.get(c.ccy).to_string(),
                    ic_category,
                    interchange_bps,
                    txn_date,
                    channel,
                    gross,
                    total_fee,
                    interchange,
                    scheme_fee,
                    markup,
                    commission,
                }))
            },
            on_row,
        )
    }
}

/// Normalize an Adyen webhook body into a single `NotificationRequestItem`-shaped object,
/// accepting both formats Adyen can post: the JSON envelope
/// (`{"notificationItems":[{"NotificationRequestItem":{…}}]}`) and the legacy / "Test
/// configuration" `application/x-www-form-urlencoded` post (flat `key=value` pairs, with the
/// signature under `additionalData.hmacSignature` and amount split across `value`/`currency`).
/// Everything downstream (`hmac_payload`, field extraction) reads this one shape.
fn notification_item(raw_body: &[u8]) -> Result<Value, IngestError> {
    // JSON envelope first: a form-urlencoded body isn't valid JSON, so this falls through.
    if let Ok(v) = serde_json::from_slice::<Value>(raw_body) {
        if let Some(item) = first_item(&v) {
            return Ok(item.clone());
        }
        // Tolerate a bare NotificationRequestItem posted without the envelope.
        if v.get("merchantAccountCode").is_some() {
            return Ok(v);
        }
        return Err(IngestError::MalformedNotification(
            "no NotificationRequestItem".to_string(),
        ));
    }
    Ok(form_to_item(raw_body))
}

/// First `NotificationRequestItem` in an Adyen JSON webhook envelope.
fn first_item(v: &Value) -> Option<&Value> {
    v.get("notificationItems")?
        .as_array()?
        .first()?
        .get("NotificationRequestItem")
}

/// Rebuild the `NotificationRequestItem` shape from Adyen's flat form-urlencoded post: `value` and
/// `currency` fold into a nested `amount`, and dotted keys (`additionalData.hmacSignature`) become
/// nested objects. All values stay strings — `hmac_payload`/`value_to_string` handle that.
fn form_to_item(raw_body: &[u8]) -> Value {
    let mut item = serde_json::Map::new();
    let mut amount = serde_json::Map::new();
    let mut additional = serde_json::Map::new();
    for (k, v) in form_urlencoded::parse(raw_body) {
        let value = Value::String(v.into_owned());
        match k.as_ref() {
            "value" => {
                amount.insert("value".to_string(), value);
            }
            "currency" => {
                amount.insert("currency".to_string(), value);
            }
            key => match key.strip_prefix("additionalData.") {
                Some(sub) => {
                    additional.insert(sub.to_string(), value);
                }
                None => {
                    item.insert(key.to_string(), value);
                }
            },
        }
    }
    if !amount.is_empty() {
        item.insert("amount".to_string(), Value::Object(amount));
    }
    if !additional.is_empty() {
        item.insert("additionalData".to_string(), Value::Object(additional));
    }
    Value::Object(item)
}

/// Adyen's colon-joined, backslash-escaped signing payload (standard-notification field order).
fn hmac_payload(item: &Value) -> String {
    let f = |key: &str| -> String {
        item.get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .replace('\\', "\\\\")
            .replace(':', "\\:")
    };
    [
        f("pspReference"),
        f("originalReference"),
        f("merchantAccountCode"),
        f("merchantReference"),
        item.pointer("/amount/value")
            .map(value_to_string)
            .unwrap_or_default(),
        item.pointer("/amount/currency")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        f("eventCode"),
        f("success"),
    ]
    .join(":")
}

fn value_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// HMAC-SHA256 over `payload` with a hex-encoded Adyen key, base64-compared in constant time.
fn verify_hmac(payload: &str, provided_b64: &str, hex_key: &str) -> bool {
    let Some(key_bytes) = hex_decode(hex_key) else {
        return false;
    };
    let key = hmac::Key::new(hmac::HMAC_SHA256, &key_bytes);
    let tag = hmac::sign(&key, payload.as_bytes());
    let expected_b64 = base64_encode(tag.as_ref());
    // Constant-time comparison to avoid a signature-timing side channel.
    ring::constant_time::verify_slices_are_equal(expected_b64.as_bytes(), provided_b64.as_bytes())
        .is_ok()
}

fn hex_decode(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&s[i..i + 2], 16).ok())
        .collect()
}

fn base64_encode(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

/// Pull the interchange category and rate from the `ICSF details` JSON array: the element with
/// `t=="ic"` carries the product category in `n` and, when present, the card-product rate in `bps`.
/// Empty strings mean absent (flat-fee methods) or unparsable.
fn ic_details(raw: &str) -> (String, String) {
    if raw.is_empty() {
        return (String::new(), String::new());
    }
    let Ok(Value::Array(arr)) = serde_json::from_str::<Value>(raw) else {
        return (String::new(), String::new());
    };
    let Some(ic) = arr
        .iter()
        .find(|e| e.get("t").and_then(Value::as_str) == Some("ic"))
    else {
        return (String::new(), String::new());
    };
    let category = ic
        .get("n")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .to_string();
    let bps = ic.get("bps").map(normalize_bps).unwrap_or_default();
    (category, bps)
}

fn normalize_bps(v: &Value) -> String {
    let raw = match v {
        Value::Number(n) => n.to_string(),
        Value::String(s) => s.trim().to_string(),
        _ => String::new(),
    };
    let Ok(n) = raw.parse::<f64>() else {
        return raw;
    };
    if !n.is_finite() {
        return String::new();
    }
    let mut s = format!("{n:.6}");
    while s.contains('.') && s.ends_with('0') {
        s.pop();
    }
    if s.ends_with('.') {
        s.pop();
    }
    s
}

/// Parse a money cell; blanks/garbage become `0.0` (mirrors `par_extract.to_float`).
fn to_float(s: &str) -> f64 {
    s.trim().parse::<f64>().unwrap_or(0.0)
}

/// Parse Adyen's `Booking Date` (`"2026-01-16 00:00:53"`, optional timezone column) to a date.
/// Only the date part matters for the report period; a blank/odd value yields `None`.
fn parse_booking_date(s: &str) -> Option<chrono::NaiveDate> {
    let date_part = s.trim().get(0..10)?;
    chrono::NaiveDate::parse_from_str(date_part, "%Y-%m-%d").ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_settled_rows_and_skips_non_fee_records() {
        // Header order intentionally not the code's order — indices resolve by label.
        let csv = "\
Psp Reference,Record Type,Payment Method Variant,Global Card Brand,Issuer Country,Settlement Currency,Payable (SC),Commission (SC),Markup (SC),Scheme Fees (SC),Interchange (SC),ICSF details\n\
ref1,Authorised,visastandarddebit,visa,FR,EUR,,,,,,\n\
ref2,Settled,visastandarddebit,visa,FR,EUR,100.00,0.05,0.00,0.02,0.20,\"[{\"\"t\"\":\"\"ic\"\",\"\"n\"\":\"\"Intra EEA Consumer EMV Debit\"\"}]\"\n";
        let rows = AdyenReportSource::new()
            .parse_report(csv.as_bytes())
            .unwrap();
        assert_eq!(rows.len(), 1, "only the Settled row is kept");
        let r = &rows[0];
        assert_eq!(r.txn_ref, "ref2");
        assert_eq!(r.funding, "debit");
        assert_eq!(r.ic_category, "Intra EEA Consumer EMV Debit");
        assert_eq!(r.interchange_bps, "");
        assert!((r.total_fee - 0.27).abs() < 1e-9, "0.05+0.00+0.02+0.20");
        assert!((r.gross - 100.27).abs() < 1e-9, "payable + total_fee");
        assert!(r.txn_date.is_none(), "no Booking Date column -> None");
    }

    #[test]
    fn booking_date_parses_date_part() {
        assert_eq!(
            parse_booking_date("2026-01-16 00:00:53"),
            chrono::NaiveDate::from_ymd_opt(2026, 1, 16),
        );
        assert_eq!(parse_booking_date(""), None);
        assert_eq!(parse_booking_date("garbage"), None);
    }

    #[test]
    fn ic_category_absent_yields_empty() {
        assert_eq!(ic_details(""), (String::new(), String::new()));
        assert_eq!(
            ic_details("[{\"t\":\"scheme\",\"n\":\"x\"}]"),
            (String::new(), String::new())
        );
        assert_eq!(ic_details("not json"), (String::new(), String::new()));
    }

    #[test]
    fn ic_details_extracts_rate_bps() {
        assert_eq!(
            ic_details("[{\"t\":\"ic\",\"n\":\"Consumer Debit\",\"bps\":0.2}]"),
            ("Consumer Debit".to_string(), "0.2".to_string())
        );
        assert_eq!(
            ic_details("[{\"t\":\"ic\",\"n\":\"Commercial\",\"bps\":\"142.5000\"}]"),
            ("Commercial".to_string(), "142.5".to_string())
        );
    }

    #[test]
    fn funding_derivation() {
        assert_eq!(
            SettledFeeRow::funding_from_variant("visastandarddebit"),
            "debit"
        );
        assert_eq!(
            SettledFeeRow::funding_from_variant("mcsuperpremiumcredit"),
            "credit"
        );
        assert_eq!(SettledFeeRow::funding_from_variant("ideal"), "");
    }

    #[test]
    fn hmac_verifies_known_vector() {
        // key = hex("test") bytes; verify a self-computed signature round-trips and a wrong one fails.
        let hex_key = "74657374"; // "test"
        let payload = "a:b:c";
        let key = hmac::Key::new(hmac::HMAC_SHA256, b"test");
        let good = base64_encode(hmac::sign(&key, payload.as_bytes()).as_ref());
        assert!(verify_hmac(payload, &good, hex_key));
        assert!(!verify_hmac(payload, "AAAA", hex_key));
    }
}
