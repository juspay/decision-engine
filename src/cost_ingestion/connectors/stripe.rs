//! Stripe "Payments Fee Report" `SettlementReportSource`.
//!
//! Normalizes Stripe's monthly fee report onto the canonical [`SettledFeeRow`]. Everything
//! Stripe-specific — the report's column labels and the aggregate row shape — is contained in this
//! file; the queue, staging, fit, and serving never see anything Stripe-specific.
//!
//! # IMPORTANT: this report is *aggregated*, not per-transaction
//!
//! Unlike the Adyen and Braintree settlement reports (one row = one settled transaction, with a
//! unique reference), the Stripe fee report is a **roll-up**: each row is a
//! `(card brand, shopper interaction, regionality, funding source, payment-method variant, fee
//! name, rate)` bucket carrying a `Count` (number of transactions), a `Gross Qty` (turnover the
//! fee applied to), and a `Cost Qty` (fee charged for that one line). A single transaction's total
//! cost is therefore spread across many `Fee Name` rows, and different fee lines within the same
//! cluster cover different transaction subsets (their `Count`s differ).
//!
//! Consequences the caller must know about:
//!   * There is no per-transaction id. `txn_ref` is a *synthetic* deterministic key built from the
//!     row's identifying fields so a re-upload of the same month replaces (not duplicates) its
//!     rows under the ReplacingMergeTree — it is NOT a real transaction reference.
//!   * `gross`/`total_fee` here are aggregate turnover/fee for a bucket, not one transaction. The
//!     per-transaction OLS in `fit.rs` (`total_fee = slope·gross + intercept`, `n ≥ 200`) assumes
//!     transaction-level scatter; fed these aggregate lines it will not produce meaningful `GOOD`
//!     clusters. Serving this connector needs an aggregate-fit path (compute `pct_bps` from
//!     `Variable Fee` and `fixed` from `Fixed Fee`, both of which this report already carries) —
//!     that lives outside this parser.
//!
//! Two report fields this parser cannot fill (the report simply does not carry them):
//!   * `issuer_country` — the report only has `Regionality` (DOMESTIC / INTER_REGIONAL), which is
//!     relative to the acquirer, not an ISO issuer country. Left empty.
//!   * `ic_category` — `Fee Name` is the fee *type* (many per cluster), not the interchange
//!     category. Left empty.
//!
//! Notification/download is NOT implemented — the intended path for this connector is the manual
//! dashboard upload (which only calls `parse_rows`). The webhook/download methods return a
//! descriptive error until Stripe's report-ready webhook and download auth are wired.

use std::sync::OnceLock;
use std::time::Duration;

use async_trait::async_trait;
use axum::http::HeaderMap;
use bytes::Bytes;
use masking::Secret;

use crate::cost_ingestion::source::SettlementReportSource;
use crate::cost_ingestion::types::{ConnectorCreds, IngestError, ReportNotification, SettledFeeRow};

/// `Refund Flag` value for forward (settled) fees. `Refund` rows are fee reversals whose amounts
/// would pollute the gross→fee relationship, so they're skipped (mirrors Adyen's record-type
/// filter and Braintree's sale-only filter).
const SETTLE_FLAG: &str = "settle";

#[allow(dead_code)] // used once download_report is implemented
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(120);

pub struct StripeReportSource;

impl StripeReportSource {
    pub fn new() -> Self {
        Self
    }
}

impl Default for StripeReportSource {
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
            .expect("failed to build stripe report reqwest client")
    })
}

#[async_trait]
impl SettlementReportSource for StripeReportSource {
    fn connector(&self) -> &'static str {
        "stripe"
    }

    fn peek_account(&self, _raw_body: &[u8]) -> Result<String, IngestError> {
        // TODO(stripe-webhook): extract the connected-account id from the unverified event body
        // once Stripe's report-ready webhook is wired. Manual upload passes the account explicitly.
        Err(not_implemented("peek_account"))
    }

    fn verify_and_parse_notification(
        &self,
        _headers: &HeaderMap,
        _raw_body: &[u8],
        _secret: &Secret<String>,
    ) -> Result<ReportNotification, IngestError> {
        // TODO(stripe-webhook): verify the `Stripe-Signature` header (HMAC-SHA256 over
        // `timestamp.payload`) and pull the report handle from the event.
        Err(not_implemented("verify_and_parse_notification"))
    }

    async fn download_report(
        &self,
        _creds: &ConnectorCreds,
        _note: &ReportNotification,
    ) -> Result<Bytes, IngestError> {
        // TODO(stripe-webhook): fetch the report file via the Files API using the merchant's key.
        Err(not_implemented("download_report"))
    }

    fn parse_rows(
        &self,
        reader: Box<dyn std::io::Read + Send>,
        on_row: &mut dyn FnMut(SettledFeeRow) -> Result<(), IngestError>,
    ) -> Result<(), IngestError> {

        let mut reader = csv::ReaderBuilder::new().flexible(true).from_reader(reader);

        let headers = reader
            .headers()
            .map_err(|e| IngestError::Parse(e.to_string()))?
            .clone();
        let idx = |name: &str| headers.iter().position(|h| h == name);
        let col = |name: &str| -> Result<usize, IngestError> {
            idx(name).ok_or_else(|| IngestError::Parse(format!("missing column: {name}")))
        };

        let c_brand = col("Card Brand")?;
        let c_interaction = col("Shopper Interaction")?;
        let c_funding = col("Funding Source")?;
        let c_variant = col("Payment Method Variant")?;
        let c_gross = col("Gross Qty")?;
        let c_cost = col("Cost Qty")?;
        let c_refund_flag = col("Refund Flag")?;
        // Currency: prefer the gross-side currency, fall back to the cost-side.
        let c_gross_ccy = col("Gross Ccy")?;
        let c_cost_ccy = idx("Cost Ccy");
        // Optional — used only for the ingested report's period, and to make `txn_ref` unique.
        let c_month = idx("Month");
        // Optional — carried into the synthetic key so different fee lines/rates in the same
        // cluster don't collapse onto one another under the ReplacingMergeTree dedup.
        let c_fee_name = idx("Fee Name");
        let c_variable_fee = idx("Variable Fee");
        let c_fixed_fee = idx("Fixed Fee");

        for record in reader.records() {
            let record = record.map_err(|e| IngestError::Parse(e.to_string()))?;
            let get = |i: usize| record.get(i).unwrap_or("");
            let get_opt = |i: Option<usize>| i.map(get).unwrap_or("");

            // Keep only forward (settled) fees; skip refund/reversal lines.
            if get(c_refund_flag).trim().to_lowercase() != SETTLE_FLAG {
                continue;
            }

            let gross = to_float(get(c_gross));
            let total_fee = to_float(get(c_cost));

            let card_network = normalize_network(get(c_brand));
            let variant = get(c_variant).trim().to_lowercase();
            let funding = funding_from_source(get(c_funding));
            let currency = {
                let g = get(c_gross_ccy).trim();
                if g.is_empty() { get_opt(c_cost_ccy).trim() } else { g }.to_string()
            };
            let channel = channel_from_interaction(get(c_interaction));
            let month = get_opt(c_month).trim();
            let txn_date = parse_month(month);

            on_row(SettledFeeRow {
                // Synthetic, deterministic key (see module docs): identifies this aggregate line so
                // a same-month re-upload replaces rather than duplicates. NOT a real txn reference.
                txn_ref: synth_ref(&[
                    &variant,
                    get(c_interaction).trim(),
                    get(c_funding).trim(),
                    get_opt(c_fee_name).trim(),
                    get_opt(c_variable_fee).trim(),
                    get_opt(c_fixed_fee).trim(),
                    &currency,
                    month,
                ]),
                card_network,
                variant,
                funding,
                issuer_country: String::new(),
                currency,
                ic_category: String::new(),
                txn_date,
                channel,
                gross,
                total_fee,
                // Stripe decomposes fee lines as fixed vs. variable (see `Fixed Fee` / `Variable
                // Fee`), not interchange/scheme/markup/commission, so the four split fields stay 0.
                // The fit only reads `gross` and `total_fee`, so this does not affect estimation.
                interchange: 0.0,
                scheme_fee: 0.0,
                markup: 0.0,
                commission: 0.0,
            })?;
        }
        Ok(())
    }
}

/// Placeholder error for the not-yet-wired notification/download path.
fn not_implemented(method: &str) -> IngestError {
    IngestError::MalformedNotification(format!(
        "stripe connector: {method} not yet implemented (webhook/download shape TBD)"
    ))
}

/// Build a deterministic key from a row's identifying parts, joined with a delimiter unlikely to
/// appear in the values. Stable across re-uploads ⇒ idempotent staging via the ReplacingMergeTree.
fn synth_ref(parts: &[&str]) -> String {
    format!("stripe|{}", parts.join("\u{1f}"))
}

/// Map Stripe's `Funding Source` (`DEBIT` / `CREDIT` / `PREPAID` / blank) onto the canonical
fn funding_from_source(source: &str) -> String {
    match source.trim().to_lowercase().as_str() {
        "debit" => "debit".to_string(),
        "credit" => "credit".to_string(),
        "prepaid" => "prepaid".to_string(),
        _ => String::new(),
    }
}

/// Canonicalize a Stripe card-brand label to the lowercased network ids the rest of the pipeline
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

/// Derive the acceptance channel from Stripe's `Shopper Interaction`. Card-present / POS maps to
/// `pos`; everything else (`Ecommerce`, `ContAuth`, MOTO, …) is treated as online `ecom`.
fn channel_from_interaction(interaction: &str) -> String {
    let i = interaction.trim().to_lowercase();
    if i.contains("pos") || i.contains("cardpresent") || i.contains("card present") {
        "pos".to_string()
    } else {
        "ecom".to_string()
    }
}

/// Parse Stripe's `Month` (`"2025/01"`) to the first day of that month — used only for the
/// ingested report's period. Blank/odd values yield `None`.
fn parse_month(s: &str) -> Option<chrono::NaiveDate> {
    let s = s.trim();
    let (year, month) = s.split_once('/')?;
    let year: i32 = year.trim().parse().ok()?;
    let month: u32 = month.trim().parse().ok()?;
    chrono::NaiveDate::from_ymd_opt(year, month, 1)
}

/// Parse a money cell; blanks/garbage become `0.0` (mirrors the Adyen/Braintree parsers).
fn to_float(s: &str) -> f64 {
    s.trim().parse::<f64>().unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_settled_lines_and_skips_refunds() {
        // Header order intentionally not the code's order — indices resolve by label.
        let csv = "\
Company,Merchant,Card Brand,Shopper Interaction,Regionality,Funding Source,Payment Method Variant,Fee Name,Fee,Count,Gross Ccy,Gross Qty,Cost Ccy,Cost Qty,Month,Lookup String,Refund Flag,Fixed Fee Ccy,Fixed Fee,Fixed Fee in USD,Variable Fee,USD Amount,USD Fee,Fixed Amount,Variable Amount\n\
Merchant-A,Merchant-A-Stripe-US,mc,Ecommerce,DOMESTIC,DEBIT,maestro,Clearing Sender Connectivity Fee,1.65% + USD 0.1500,23155,USD,453160.54,USD,10950.39891,2025/01,GGH-ADYEN-USmcUSD,Settle,USD,0.15,0.15,0.0165,453160.54,10950.39891,3473.25,7477.14891\n\
Merchant-A,Merchant-A-Stripe-US,mc,Ecommerce,DOMESTIC,CREDIT,mccommercialcredit,Clearing Sender Connectivity Fee,2.16%,9,USD,497828.26,USD,10753.09042,2025/01,GGH-ADYEN-USmcUSD,Refund,,0.0,0.0,0.0216,497828.26,10753.09042,0.0,10753.09042\n";
        let rows = StripeReportSource::new().parse_report(csv.as_bytes()).unwrap();
        assert_eq!(rows.len(), 1, "only the Settle row is kept");
        let r = &rows[0];
        assert_eq!(r.card_network, "mc");
        assert_eq!(r.variant, "maestro");
        assert_eq!(r.funding, "debit");
        assert_eq!(r.currency, "USD");
        assert_eq!(r.channel, "ecom");
        assert_eq!(r.issuer_country, "", "not present in the Stripe report");
        assert_eq!(r.ic_category, "", "Fee Name is not the interchange category");
        assert!((r.gross - 453160.54).abs() < 1e-6, "Gross Qty");
        assert!((r.total_fee - 10950.39891).abs() < 1e-6, "Cost Qty");
        assert_eq!(r.txn_date, chrono::NaiveDate::from_ymd_opt(2025, 1, 1));
        assert!(r.txn_ref.starts_with("stripe|"), "synthetic key");
    }

    #[test]
    fn synthetic_ref_is_stable_and_distinguishes_fee_lines() {
        // Same cluster, two different fee lines ⇒ two distinct keys (so neither dedups the other).
        let csv = "\
Card Brand,Shopper Interaction,Regionality,Funding Source,Payment Method Variant,Fee Name,Gross Qty,Cost Qty,Gross Ccy,Cost Ccy,Month,Refund Flag,Fixed Fee,Variable Fee\n\
mc,Ecommerce,DOMESTIC,CREDIT,mccommercialcredit,Interchange,100000,2100,USD,USD,2025/01,Settle,0.10,0.019\n\
mc,Ecommerce,DOMESTIC,CREDIT,mccommercialcredit,Scheme Fee,100000,150,USD,USD,2025/01,Settle,0.00,0.0015\n";
        let rows = StripeReportSource::new().parse_report(csv.as_bytes()).unwrap();
        assert_eq!(rows.len(), 2);
        assert_ne!(rows[0].txn_ref, rows[1].txn_ref, "distinct fee lines keep distinct keys");
    }

    #[test]
    fn missing_required_column_errors() {
        let csv = "Card Brand,Shopper Interaction\nmc,Ecommerce\n";
        let err = StripeReportSource::new().parse_report(csv.as_bytes()).unwrap_err();
        assert!(matches!(err, IngestError::Parse(_)));
    }

    #[test]
    fn helpers() {
        assert_eq!(funding_from_source("DEBIT"), "debit");
        assert_eq!(funding_from_source("CREDIT"), "credit");
        assert_eq!(funding_from_source("PREPAID"), "prepaid");
        assert_eq!(funding_from_source(""), "");
        assert_eq!(normalize_network("mc"), "mc");
        assert_eq!(normalize_network("MasterCard"), "mc");
        assert_eq!(normalize_network("Visa"), "visa");
        assert_eq!(channel_from_interaction("Ecommerce"), "ecom");
        assert_eq!(channel_from_interaction("ContAuth"), "ecom");
        assert_eq!(channel_from_interaction("CardPresent"), "pos");
        assert_eq!(parse_month("2025/01"), chrono::NaiveDate::from_ymd_opt(2025, 1, 1));
        assert_eq!(parse_month(""), None);
    }
}
