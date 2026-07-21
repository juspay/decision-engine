//! Braintree `SettlementReportSource`.
//!
//! Port of the Braintree "Merchant Financial (PAR)" settlement report onto the canonical
//! [`SettledFeeRow`]. Everything Braintree-specific — report column labels, the fee decomposition,
//! and (eventually) the notification/download shape — is contained in this file. The queue,
//! staging, fit, and serving never see anything Braintree-specific.
//!
//! Fee model. Braintree's report already splits the pass-through vs. processor take, so the
//! decomposition mirrors Adyen's:
//!   * `interchange` ← `Interchange Total Amount`
//!   * `scheme_fee`  ← `Total Scheme Fees`
//!   * `commission`  ← `Braintree Total Amount` (Braintree's own per-txn + discount take)
//!   * `markup`      ← 0.0 (Braintree bundles its take into one amount; no separate markup line)
//!
//! and `total_fee = interchange + scheme_fee + markup + commission`, which the report's
//! `Total Fee Amount` column equals.
//!
//! `gross = Settlement Amount` directly. Unlike Adyen's `Payable (SC)` (net-of-fees, so Adyen has
//! to add the fee back), Braintree's `Settlement Amount` is already the gross transaction value —
//! per Braintree's fee-report reference it is "the transaction amount that serves as the basis for
//! calculating fees" (`Braintree Total Amount = Discount × Settlement Amount + Per Transaction
//! Fee`). Adding `total_fee` here would double-count the fee.
//!
//! Notification/download is NOT yet implemented — Braintree's report-ready webhook shape,
//! signature scheme, and download auth still need to be confirmed against Braintree docs (see
//! architecture doc §7.7). Those three methods return a descriptive error until then; the parser
//! (`parse_rows`) is complete and independently testable.

use std::sync::OnceLock;
use std::time::Duration;

use async_trait::async_trait;
use axum::http::HeaderMap;
use bytes::Bytes;
use masking::Secret;

use crate::cost_ingestion::connectors::csv_reader;
use crate::cost_ingestion::source::SettlementReportSource;
use crate::cost_ingestion::types::{
    ConnectorCreds, IngestError, ReportNotification, SettledFeeRow,
};

/// Braintree `Transaction Type` values that carry a settled processing fee. Sales are the fit's
/// signal; `credit` / `dispute debit` / `dispute credit` rows are reversals whose signed amounts
/// would distort the gross→fee regression, so they're skipped (mirrors Adyen's record-type filter).
const FEE_TRANSACTION_TYPES: [&str; 1] = ["sale"];

#[allow(dead_code)] // used once download_report is implemented
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(120);

pub struct BraintreeReportSource;

impl BraintreeReportSource {
    pub fn new() -> Self {
        Self
    }
}

impl Default for BraintreeReportSource {
    fn default() -> Self {
        Self::new()
    }
}

#[allow(dead_code)] // used once download_report is implemented
fn http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(DOWNLOAD_TIMEOUT)
            .build()
            .expect("failed to build braintree report reqwest client")
    })
}

#[async_trait]
impl SettlementReportSource for BraintreeReportSource {
    fn connector(&self) -> &'static str {
        "braintree"
    }

    fn peek_account(&self, _raw_body: &[u8]) -> Result<String, IngestError> {
        // TODO(braintree-webhook): extract the merchant account token from the unverified
        // notification body once Braintree's report-ready webhook shape is confirmed. The report
        // itself carries it as `Merchant Account ID` (field 1); the webhook envelope is TBD.
        Err(not_implemented("peek_account"))
    }

    fn verify_and_parse_notification(
        &self,
        _headers: &HeaderMap,
        _raw_body: &[u8],
        _secret: &Secret<String>,
    ) -> Result<ReportNotification, IngestError> {
        // TODO(braintree-webhook): verify Braintree's webhook signature and extract the report
        // handle. Braintree signs webhooks with an `bt_signature`/`bt_payload` pair (public-key
        // scheme), unlike Adyen's body HMAC — confirm the report-notification variant before wiring.
        Err(not_implemented("verify_and_parse_notification"))
    }

    async fn download_report(
        &self,
        _creds: &ConnectorCreds,
        _note: &ReportNotification,
    ) -> Result<Bytes, IngestError> {
        // TODO(braintree-webhook): fetch the report using the merchant's stored download auth.
        // Reuse `http_client()` once the download URL/auth (API key vs. SFTP) is confirmed.
        Err(not_implemented("download_report"))
    }

    fn parse_rows(
        &self,
        reader: Box<dyn std::io::Read + Send>,
        mapping: &crate::cost_ingestion::mapping::ColumnMapping,
        on_row: &mut dyn FnMut(SettledFeeRow) -> Result<(), IngestError>,
    ) -> Result<(), IngestError> {
        // Resolved column indices for one report. Braintree's field order can drift between report
        // versions (the spec even flags planned column additions), so never index positionally.
        struct Cols {
            txn: usize,
            r#type: usize,
            ccy: usize,
            amount: usize,
            brand: usize,
            card_type: usize,
            instrument: usize,
            interchange: usize,
            braintree: usize,
            // Optional columns — nullable per spec or absent in older/test reports. Missing ⇒ blank/0.
            scheme: Option<usize>,
            ic_desc: Option<usize>,
            issuer: Option<usize>,
            settle_date: Option<usize>,
            // For pinless-debit rows the network is carried here rather than in `Card Brand`.
            network: Option<usize>,
        }

        csv_reader::parse(
            reader,
            mapping,
            |h| {
                Ok(Cols {
                    txn: h.require("Transaction ID")?,
                    r#type: h.require("Transaction Type")?,
                    ccy: h.require("Settlement Currency")?,
                    amount: h.require("Settlement Amount")?,
                    brand: h.require("Card Brand")?,
                    card_type: h.require("Card Type")?,
                    instrument: h.require("Payment Instrument")?,
                    interchange: h.require("Interchange Total Amount")?,
                    braintree: h.require("Braintree Total Amount")?,
                    scheme: h.index("Total Scheme Fees"),
                    ic_desc: h.index("Interchange Description"),
                    issuer: h.index("Card Issuing Country"),
                    settle_date: h.index("Settlement Date"),
                    network: h.index("Payment Network"),
                })
            },
            |c, row| {
                // Keep only settled sales; skip credits/disputes whose signed amounts would pollute
                // the gross→fee regression — done before any field extraction.
                if !FEE_TRANSACTION_TYPES
                    .contains(&row.get(c.r#type).trim().to_lowercase().as_str())
                {
                    return Ok(None);
                }

                let interchange = to_float(row.get(c.interchange));
                let scheme_fee = to_float(row.get_opt(c.scheme));
                // Braintree reports its own take as a single bundled amount; there is no separate
                // markup line, so `markup` stays 0 and `commission` carries the full processor take.
                let commission = to_float(row.get(c.braintree));
                let markup = 0.0;
                let total_fee = interchange + scheme_fee + markup + commission;
                // `Settlement Amount` is already the gross transaction value (the fee's calculation
                // base), so it maps to `gross` as-is — do NOT add `total_fee` (that's the Adyen path,
                // where `Payable (SC)` is net-of-fees).
                let gross = to_float(row.get(c.amount));

                // Prefer the card brand; fall back to the debit network (pinless), then the instrument.
                let network = {
                    let brand = normalize_network(row.get(c.brand));
                    if !brand.is_empty() {
                        brand
                    } else {
                        let net = normalize_network(row.get_opt(c.network));
                        if !net.is_empty() {
                            net
                        } else {
                            row.get(c.instrument).trim().to_lowercase()
                        }
                    }
                };
                let funding = funding_from_card_type(row.get(c.card_type));
                let variant = build_variant(row.get(c.instrument), &network, &funding);
                let txn_date = c.settle_date.and_then(|i| parse_date(row.get(i)));

                Ok(Some(SettledFeeRow {
                    txn_ref: row.get(c.txn).to_string(),
                    card_network: network,
                    variant,
                    funding,
                    issuer_country: row.get_opt(c.issuer).trim().to_string(),
                    currency: row.get(c.ccy).trim().to_string(),
                    ic_category: row.get_opt(c.ic_desc).trim().to_string(),
                    txn_date,
                    // Braintree's PAR carries no terminal/POS indicator, so every row is treated as
                    // online. Revisit if in-person / pinless acceptance data becomes distinguishable.
                    channel: "ecom".to_string(),
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

/// Placeholder error for the not-yet-wired notification/download path.
fn not_implemented(method: &str) -> IngestError {
    IngestError::MalformedNotification(format!(
        "braintree connector: {method} not yet implemented (webhook/download shape TBD)"
    ))
}

/// Map Braintree's `Card Type` (`Credit` / `Debit` / `Unknown` / blank) onto the canonical funding
/// bucket. Unlike Adyen (which sniffs the variant string) Braintree states funding explicitly.
fn funding_from_card_type(card_type: &str) -> String {
    match card_type.trim().to_lowercase().as_str() {
        "debit" => "debit".to_string(),
        "credit" => "credit".to_string(),
        _ => String::new(),
    }
}

/// Canonicalize a Braintree card-brand / payment-network label to the lowercased network ids the
/// rest of the pipeline uses (matching Adyen: `mc`, `visa`, `amex`, …). Empty in ⇒ empty out.
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

/// Synthesize a `variant` cluster key. Braintree carries no scheme *tier* (Adyen's
/// `visastandarddebit` packs one), so for cards we use `{network}{funding}` (e.g. `mcdebit`) and
/// for alternative instruments we use the instrument itself (e.g. `venmoaccount`).
fn build_variant(instrument: &str, network: &str, funding: &str) -> String {
    let inst = instrument.trim().to_lowercase();
    if inst.contains("card") {
        let v = format!("{network}{funding}");
        if v.is_empty() {
            inst.replace('_', "")
        } else {
            v
        }
    } else {
        inst.replace('_', "")
    }
}

/// Parse a money cell; blanks/garbage become `0.0` (mirrors Adyen's `to_float`).
fn to_float(s: &str) -> f64 {
    s.trim().parse::<f64>().unwrap_or(0.0)
}

/// Parse a Braintree date cell (`Settlement Date`). The report uses US `M/D/YYYY`, occasionally
/// with a 2-digit year (`2/2/20`). Blank/odd values yield `None`.
fn parse_date(s: &str) -> Option<chrono::NaiveDate> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    // Choose the year format by the last field's width — `%Y` would otherwise greedily read a
    // 2-digit `20` as year 0020.
    let fmt = match s.rsplit('/').next().map(str::len) {
        Some(4) => "%m/%d/%Y",
        Some(2) => "%m/%d/%y",
        _ => return None,
    };
    chrono::NaiveDate::parse_from_str(s, fmt).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sales_and_skips_reversals() {
        // Header order intentionally not the code's order — indices resolve by label.
        let csv = "\
Transaction ID,Transaction Type,Settlement Currency,Settlement Amount,Card Brand,Card Type,Payment Instrument,Interchange Description,Interchange Total Amount,Total Scheme Fees,Braintree Total Amount,Total Fee Amount,Card Issuing Country,Settlement Date,Payment Network\n\
9celsl42,sale,USD,72.5,MasterCard,debit,credit_card,MC-REGULATED COMM (DB),0.26,0.0395,0.08,0.3795,USA,2/2/20,\n\
r2,credit,USD,-10.00,Visa,credit,credit_card,MC-REGULATED,0.01,0.00,0.01,0.02,USA,2/3/2020,\n";
        let rows = BraintreeReportSource::new()
            .parse_report(csv.as_bytes())
            .unwrap();
        assert_eq!(rows.len(), 1, "only the sale row is kept");
        let r = &rows[0];
        assert_eq!(r.txn_ref, "9celsl42");
        assert_eq!(r.card_network, "mc", "MasterCard -> mc");
        assert_eq!(r.funding, "debit");
        assert_eq!(r.variant, "mcdebit");
        assert_eq!(r.issuer_country, "USA");
        assert_eq!(r.currency, "USD");
        assert_eq!(r.ic_category, "MC-REGULATED COMM (DB)");
        assert_eq!(r.channel, "ecom");
        assert!((r.interchange - 0.26).abs() < 1e-9);
        assert!((r.scheme_fee - 0.0395).abs() < 1e-9);
        assert!((r.commission - 0.08).abs() < 1e-9);
        assert_eq!(r.markup, 0.0);
        assert!(
            (r.total_fee - 0.3795).abs() < 1e-9,
            "0.26+0.0395+0.08 = Total Fee Amount"
        );
        assert!(
            (r.gross - 72.5).abs() < 1e-9,
            "Settlement Amount is already gross (not +fee)"
        );
        assert_eq!(r.txn_date, chrono::NaiveDate::from_ymd_opt(2020, 2, 2));
    }

    #[test]
    fn pinless_debit_falls_back_to_payment_network() {
        // Pinless-debit rows leave `Card Brand` empty and carry the network in `Payment Network`.
        let csv = "\
Transaction ID,Transaction Type,Settlement Currency,Settlement Amount,Card Brand,Card Type,Payment Instrument,Interchange Total Amount,Braintree Total Amount,Payment Network\n\
p1,sale,USD,50.00,,debit,credit_card,0.10,0.05,STAR\n";
        let rows = BraintreeReportSource::new()
            .parse_report(csv.as_bytes())
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].card_network, "star",
            "empty brand -> Payment Network"
        );
        assert_eq!(rows[0].scheme_fee, 0.0, "absent Total Scheme Fees -> 0");
    }

    #[test]
    fn non_card_instrument_uses_instrument_as_variant() {
        let csv = "\
Transaction ID,Transaction Type,Settlement Currency,Settlement Amount,Card Brand,Card Type,Payment Instrument,Interchange Total Amount,Braintree Total Amount\n\
v1,sale,USD,25.00,,,venmo_account,0.00,0.15\n";
        let rows = BraintreeReportSource::new()
            .parse_report(csv.as_bytes())
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].card_network, "venmo_account",
            "no brand/network -> instrument"
        );
        assert_eq!(rows[0].variant, "venmoaccount");
        assert_eq!(rows[0].funding, "");
    }

    #[test]
    fn missing_required_column_errors() {
        let csv = "Transaction ID,Transaction Type\nx,sale\n";
        let err = BraintreeReportSource::new()
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
        assert!(missing.contains(&"Settlement Amount".to_string()));
        assert!(!missing.contains(&"Transaction ID".to_string()));
    }

    #[test]
    fn funding_and_network_helpers() {
        assert_eq!(funding_from_card_type("Debit"), "debit");
        assert_eq!(funding_from_card_type("Credit"), "credit");
        assert_eq!(funding_from_card_type("Unknown"), "");
        assert_eq!(funding_from_card_type(""), "");
        assert_eq!(normalize_network("MasterCard"), "mc");
        assert_eq!(normalize_network("American Express"), "amex");
        assert_eq!(normalize_network("Visa"), "visa");
        assert_eq!(normalize_network(""), "");
    }

    #[test]
    fn date_parses_two_and_four_digit_years() {
        assert_eq!(
            parse_date("2/23/2022"),
            chrono::NaiveDate::from_ymd_opt(2022, 2, 23)
        );
        assert_eq!(
            parse_date("2/2/20"),
            chrono::NaiveDate::from_ymd_opt(2020, 2, 2)
        );
        assert_eq!(parse_date(""), None);
        assert_eq!(parse_date("garbage"), None);
    }
}
