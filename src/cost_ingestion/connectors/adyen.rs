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

use crate::cost_ingestion::source::SettlementReportSource;
use crate::cost_ingestion::types::{ConnectorCreds, IngestError, ReportNotification, SettledFeeRow};

/// Adyen record types that actually carry settlement fees; everything else (Authorised,
/// Received, Refused, …) has empty fee columns and would pollute the fit.
const FEE_RECORD_TYPES: [&str; 2] = ["SentForSettle", "Settled"];

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
        let v: Value = serde_json::from_slice(raw_body)
            .map_err(|e| IngestError::MalformedNotification(e.to_string()))?;
        first_item(&v)
            .and_then(|item| item.get("merchantAccountCode").and_then(Value::as_str))
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
        let v: Value = serde_json::from_slice(raw_body)
            .map_err(|e| IngestError::MalformedNotification(e.to_string()))?;
        let item = first_item(&v).ok_or_else(|| {
            IngestError::MalformedNotification("no NotificationRequestItem".to_string())
        })?;

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
        let payload = hmac_payload(item);
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
        // Adyen report downloads use report-user Basic Auth. We store "user:password" in
        // `download_auth`; split once for the request.
        let auth = creds.download_auth.peek();
        let (user, pass) = auth
            .split_once(':')
            .ok_or_else(|| IngestError::Download("download_auth must be 'user:password'".into()))?;

        let resp = http_client()
            .get(&note.report_ref)
            .basic_auth(user, Some(pass))
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
        on_row: &mut dyn FnMut(SettledFeeRow) -> Result<(), IngestError>,
    ) -> Result<(), IngestError> {
        // `csv::Reader` wraps `reader` in its own buffered reader and pulls records lazily, so a
        // huge report is never fully resident — one parsed record at a time.
        let mut reader = csv::ReaderBuilder::new()
            .flexible(true)
            .from_reader(reader);

        // Resolve column indices by header label (order can drift between report versions).
        let headers = reader
            .headers()
            .map_err(|e| IngestError::Parse(e.to_string()))?
            .clone();
        let idx = |name: &str| headers.iter().position(|h| h == name);
        let col = |name: &str| -> Result<usize, IngestError> {
            idx(name).ok_or_else(|| IngestError::Parse(format!("missing column: {name}")))
        };

        let c_record = col("Record Type")?;
        let c_psp = col("Psp Reference")?;
        let c_variant = col("Payment Method Variant")?;
        let c_brand = col("Global Card Brand")?;
        let c_issuer = col("Issuer Country")?;
        let c_ccy = col("Settlement Currency")?;
        let c_payable = col("Payable (SC)")?;
        let c_commission = col("Commission (SC)")?;
        let c_markup = col("Markup (SC)")?;
        let c_scheme = col("Scheme Fees (SC)")?;
        let c_interchange = col("Interchange (SC)")?;
        let c_icsf = col("ICSF details")?;
        // Optional: used only for the ingested report's period; absent in older/test reports.
        let c_booking = idx("Booking Date");
        // Optional: a terminal id marks an in-person (POS) acceptance; its absence ⇒ online (ecom).
        // Drives the channel feature of the category predictor.
        let c_terminal = idx("Unique Terminal ID");

        for record in reader.records() {
            let record = record.map_err(|e| IngestError::Parse(e.to_string()))?;
            let get = |i: usize| record.get(i).unwrap_or("");

            if !FEE_RECORD_TYPES.contains(&get(c_record)) {
                continue;
            }

            let commission = to_float(get(c_commission));
            let markup = to_float(get(c_markup));
            let scheme_fee = to_float(get(c_scheme));
            let interchange = to_float(get(c_interchange));
            let total_fee = commission + markup + scheme_fee + interchange;
            let gross = to_float(get(c_payable)) + total_fee;

            let variant = get(c_variant).to_lowercase();
            let funding = SettledFeeRow::funding_from_variant(&variant);
            let txn_date = c_booking.and_then(|i| parse_booking_date(get(i)));
            // POS when a terminal id is present, else online. Absent column ⇒ unknown ⇒ ecom.
            let channel = match c_terminal {
                Some(i) if !get(i).trim().is_empty() => "pos",
                _ => "ecom",
            }
            .to_string();

            on_row(SettledFeeRow {
                txn_ref: get(c_psp).to_string(),
                card_network: get(c_brand).to_lowercase(),
                variant,
                funding,
                issuer_country: get(c_issuer).to_string(),
                currency: get(c_ccy).to_string(),
                ic_category: ic_category(get(c_icsf)),
                txn_date,
                channel,
                gross,
                total_fee,
                interchange,
                scheme_fee,
                markup,
                commission,
            })?;
        }
        Ok(())
    }
}

/// First `NotificationRequestItem` in an Adyen webhook envelope.
fn first_item(v: &Value) -> Option<&Value> {
    v.get("notificationItems")?
        .as_array()?
        .first()?
        .get("NotificationRequestItem")
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

/// Pull the interchange category from the `ICSF details` JSON array: the element with `t=="ic"`
/// carries the product category in `n`. `""` when absent (flat-fee methods) or unparseable.
fn ic_category(raw: &str) -> String {
    if raw.is_empty() {
        return String::new();
    }
    let Ok(Value::Array(arr)) = serde_json::from_str::<Value>(raw) else {
        return String::new();
    };
    arr.iter()
        .find(|e| e.get("t").and_then(Value::as_str) == Some("ic"))
        .and_then(|e| e.get("n").and_then(Value::as_str))
        .unwrap_or("")
        .trim()
        .to_string()
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
        let rows = AdyenReportSource::new().parse_report(csv.as_bytes()).unwrap();
        assert_eq!(rows.len(), 1, "only the Settled row is kept");
        let r = &rows[0];
        assert_eq!(r.txn_ref, "ref2");
        assert_eq!(r.funding, "debit");
        assert_eq!(r.ic_category, "Intra EEA Consumer EMV Debit");
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
        assert_eq!(ic_category(""), "");
        assert_eq!(ic_category("[{\"t\":\"scheme\",\"n\":\"x\"}]"), "");
        assert_eq!(ic_category("not json"), "");
    }

    #[test]
    fn funding_derivation() {
        assert_eq!(SettledFeeRow::funding_from_variant("visastandarddebit"), "debit");
        assert_eq!(SettledFeeRow::funding_from_variant("mcsuperpremiumcredit"), "credit");
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
