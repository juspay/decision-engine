//! Chase (J.P. Morgan Payments) `SettlementReportSource`.
//!
//! Port of J.P. Morgan's **Deposit Details** preset report onto the canonical [`SettledFeeRow`].
//! Deposit Details is the only preset report that carries per-transaction *split* interchange and
//! assessment fees together with every cluster dimension the fit keys on (payment method, card
//! usage type, issuer country, settlement currency, interchange qualification code, digital wallet),
//! so it is the one report we ingest for Chase. Everything Chase-specific — the report envelope, the
//! column labels, the fee decomposition, the fee-sign convention — is contained in this file.
//!
//! Fee model. Deposit Details is a **pass-through** report: it itemizes the interchange and
//! assessment (scheme) fees but not Chase's own processor discount/commission (that lives in the
//! Funded Transaction Fee Details report). So, mirroring Braintree's single-report shape:
//!   * `interchange` ← `Total Interchange Amount`
//!   * `scheme_fee`  ← `Total Assessment Amount`
//!   * `markup`      ← `Other Debit Passthrough Fees` (debit pass-through; 0 for credit)
//!   * `commission`  ← 0.0 (Chase's discount rate is not in this report)
//! and `total_fee = interchange + scheme_fee + markup + commission`. The served cost therefore
//! excludes Chase's discount rate — a roughly constant offset that does not affect PSP *ranking*.
//! Backfilling `commission` from Funded Transaction Fee Details (joined on `Merchant Order Number`)
//! is a deliberate follow-up.
//!
//! Fee sign. Chase reports settled fees as **negative** amounts (money deducted from the merchant);
//! the OLS fit regresses a *positive* `total_fee` on a positive `gross`, so every fee column is
//! negated here to a positive cost magnitude. `gross = Transaction Amount in Presentment Currency`
//! is already positive for sales (refunds are filtered out before any fee extraction).
//!
//! Report envelope. A preset report is framed: a UTF-8 BOM, a `BEGIN…` line, an
//! `EntityId=…,ReportTypeName=…` metadata line, then the CSV header + data rows, then an `END…`
//! line. [`EnvelopeReader`] strips that frame line-by-line so [`csv_reader::parse`] sees a plain CSV
//! and memory stays flat for large reports.
//!
//! Notification/download is NOT yet implemented — Chase's report-ready webhook shape, signature
//! scheme, and download transport (SFTP vs. API) still need to be confirmed. Those three methods
//! return a descriptive error until then; the parser (`parse_rows`) is complete and independently
//! testable.

use std::io::{BufReader, Read};
use std::sync::OnceLock;
use std::time::Duration;

use async_trait::async_trait;
use axum::http::HeaderMap;
use bytes::Bytes;
use masking::Secret;

use crate::cost_ingestion::connectors::csv_reader;
use crate::cost_ingestion::source::SettlementReportSource;
use crate::cost_ingestion::types::{ConnectorCreds, IngestError, ReportNotification, SettledFeeRow};

/// `Action Type Code Text` value that carries a settled processing fee. Only sales feed the fit;
/// `REFUND` rows are reversals whose signed amounts would distort the gross→fee regression, so they
/// are skipped (mirrors Adyen's record-type and Braintree's transaction-type filter).
const SALE_ACTION: &str = "SALE";

#[allow(dead_code)] // used once download_report is implemented
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(120);

pub struct ChaseReportSource;

impl ChaseReportSource {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ChaseReportSource {
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
            .expect("failed to build chase report reqwest client")
    })
}

#[async_trait]
impl SettlementReportSource for ChaseReportSource {
    fn connector(&self) -> &'static str {
        "chase"
    }

    fn peek_account(&self, _raw_body: &[u8]) -> Result<String, IngestError> {
        // TODO(chase-webhook): extract the Chase merchant/entity identifier from the unverified
        // notification body once the report-ready webhook shape is confirmed. The report frames it
        // as `EntityId=…` in the BEGIN/metadata lines; the webhook envelope is TBD.
        Err(not_implemented("peek_account"))
    }

    fn verify_and_parse_notification(
        &self,
        _headers: &HeaderMap,
        _raw_body: &[u8],
        _secret: &Secret<String>,
    ) -> Result<ReportNotification, IngestError> {
        // TODO(chase-webhook): verify Chase's report-available webhook signature and extract the
        // report handle. Signature scheme and notification shape need confirmation before wiring.
        Err(not_implemented("verify_and_parse_notification"))
    }

    async fn download_report(
        &self,
        _creds: &ConnectorCreds,
        _note: &ReportNotification,
    ) -> Result<Bytes, IngestError> {
        // TODO(chase-webhook): fetch the report using the merchant's stored credentials. Chase
        // delivers preset reports over SFTP/API — confirm the transport before wiring `http_client`.
        Err(not_implemented("download_report"))
    }

    fn parse_rows(
        &self,
        reader: Box<dyn Read + Send>,
        on_row: &mut dyn FnMut(SettledFeeRow) -> Result<(), IngestError>,
    ) -> Result<(), IngestError> {
        // Resolved column indices for one report. Preset-report column order can drift between
        // report versions, so never index positionally — resolve every column by its exact label.
        struct Cols {
            order: usize,
            action: usize,
            method: usize,
            usage: usize,
            issuer: usize,
            currency: usize,
            ic_code: usize,
            amount: usize,
            interchange: usize,
            assessment: usize,
            // Optional columns — absent in some report profiles. Missing ⇒ blank/0.
            passthrough: Option<usize>,
            wallet: Option<usize>,
            submission_date: Option<usize>,
            // A terminal id marks in-person (POS) acceptance; absent ⇒ online (ecom). Drives the
            // channel feature of the interchange-category predictor (as Adyen's terminal id does).
            terminal: Option<usize>,
        }

        // Strip the preset-report BEGIN/metadata/END envelope so the shared CSV driver sees a plain
        // header + data stream.
        let reader: Box<dyn Read + Send> = Box::new(EnvelopeReader::new(reader));

        csv_reader::parse(
            reader,
            |h| {
                Ok(Cols {
                    order: h.require("Merchant Order Number")?,
                    action: h.require("Action Type Code Text")?,
                    method: h.require("Payment Method Code")?,
                    usage: h.require("Card Usage Type")?,
                    issuer: h.require("Country of Issuance")?,
                    currency: h.require("Settlement Currency Code")?,
                    ic_code: h.require("Interchange Qualification Code")?,
                    amount: h.require("Transaction Amount in Presentment Currency")?,
                    interchange: h.require("Total Interchange Amount")?,
                    assessment: h.require("Total Assessment Amount")?,
                    passthrough: h.index("Other Debit Passthrough Fees"),
                    wallet: h.index("Digital Wallet Brand"),
                    submission_date: h.index("Submission Date"),
                    terminal: h.index("Terminal Identifier"),
                })
            },
            |c, row| {
                // Keep only settled sales; skip refunds (and any stray envelope remnant, whose
                // action field is out of range ⇒ "" ⇒ not a sale) before any field extraction.
                if !row.get(c.action).trim().eq_ignore_ascii_case(SALE_ACTION) {
                    return Ok(None);
                }

                // Chase reports settled fees as negative deductions; negate to the positive cost
                // magnitudes the OLS fit expects.
                let interchange = -to_float(row.get(c.interchange));
                let scheme_fee = -to_float(row.get(c.assessment));
                let markup = -to_float(row.get_opt(c.passthrough));
                // Deposit Details carries no processor discount/commission; that fee lives in the
                // Funded Transaction Fee Details report. Left 0 until a join backfills it.
                let commission = 0.0;
                let total_fee = interchange + scheme_fee + markup + commission;
                // `Transaction Amount in Presentment Currency` is the sale value (the fee's
                // calculation base) and is already positive for sales.
                let gross = to_float(row.get(c.amount));

                let network = normalize_network(row.get(c.method));
                // Reconcile funding: the interchange qualification code is authoritative about the
                // fee tier, so it corrects a `Card Usage Type` that some exports stamp uniformly
                // (every row `1`), which would otherwise price credit txns as debit.
                let funding = reconcile_funding(row.get(c.usage), row.get(c.ic_code));
                let wallet = row.get_opt(c.wallet);
                let variant = build_variant(&network, &funding, wallet);
                let txn_date = c.submission_date.and_then(|i| parse_date(row.get(i)));
                // POS when a terminal id is present, else online. Absent column ⇒ unknown ⇒ ecom.
                let channel = match c.terminal {
                    Some(i) if !row.get(i).trim().is_empty() => "pos",
                    _ => "ecom",
                }
                .to_string();

                Ok(Some(SettledFeeRow {
                    txn_ref: row.get(c.order).trim().to_string(),
                    card_network: network,
                    variant,
                    funding,
                    issuer_country: row.get(c.issuer).trim().to_string(),
                    currency: row.get(c.currency).trim().to_string(),
                    ic_category: row.get(c.ic_code).trim().to_string(),
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

/// Placeholder error for the not-yet-wired notification/download path.
fn not_implemented(method: &str) -> IngestError {
    IngestError::MalformedNotification(format!(
        "chase connector: {method} not yet implemented (webhook/download shape TBD)"
    ))
}

/// Map a Chase `Payment Method Code` (MOP) onto the lowercased network ids the rest of the pipeline
/// uses (matching Adyen/Braintree: `visa`, `mc`, `amex`, …). Unknown codes pass through lowercased
/// so they still form their own consistent cluster rather than being dropped.
fn normalize_network(code: &str) -> String {
    match code.trim().to_uppercase().as_str() {
        "" => String::new(),
        "VI" => "visa".to_string(),
        "MC" => "mc".to_string(),
        "AX" => "amex".to_string(),
        "DI" => "discover".to_string(),
        "JC" => "jcb".to_string(),
        "DC" => "diners".to_string(),
        other => other.to_lowercase(),
    }
}

/// Map Chase's `Card Usage Type` onto the canonical funding bucket: `1` = signature debit and
/// `2` = PIN debit both fold to `debit`; `3` = credit; anything else is unknown (`""`).
fn funding_from_usage_type(code: &str) -> String {
    match code.trim() {
        "1" | "2" => "debit".to_string(),
        "3" => "credit".to_string(),
        _ => String::new(),
    }
}

/// Funding implied by a known **interchange qualification code**, or `None` when the code isn't
/// classified. The interchange program *is* the fee tier, so it is authoritative about funding —
/// a credit program is a credit transaction whatever the `Card Usage Type` column says. This is a
/// curated lookup of the Visa/Mastercard/Amex/Discover/JCB programs seen in Chase Deposit Details;
/// extend it as new programs appear. Unlisted codes defer to `Card Usage Type`.
fn funding_from_ic_code(ic: &str) -> Option<&'static str> {
    match ic.trim().to_uppercase().as_str() {
        // ── credit programs ──
        // Visa (US consumer/rewards/commercial + EU-capped + cross-border consumer credit)
        "V148" | "V105" | "VBL2" | "VPDM" | "V987" | "VINT" | "V5FI"
        // Mastercard credit
        | "MCRW" | "MX29" | "MM1" | "MIP" | "MIC" | "MHB2" | "MHB4"
        // Amex (OptBlue), Discover, JCB — credit networks
        | "AXSP" | "DIRW" | "D460" | "D361" | "DDEM" | "DCEC" | "DCED" | "DPEC" => Some("credit"),
        // ── debit programs ──
        // Visa regulated/signature/EU debit, Mastercard EU debit
        "VSDD" | "VCEB" | "V2DB" | "MDIN" => Some("debit"),
        _ => None,
    }
}

/// Resolve funding, preferring the authoritative interchange program over `Card Usage Type`. Some
/// Chase exports stamp every row `Card Usage Type = 1` (signature debit), which would otherwise
/// price credit interchange (e.g. Visa `V148` at ~3.3%) as debit; keying off the interchange code
/// corrects that. Falls back to `Card Usage Type` for any code we don't classify.
fn reconcile_funding(usage_code: &str, ic: &str) -> String {
    match funding_from_ic_code(ic) {
        Some(f) => f.to_string(),
        None => funding_from_usage_type(usage_code),
    }
}

/// Synthesize a `variant` cluster key. Chase's Deposit Details carries no scheme *tier* on the card
/// row (the interchange qualification is captured separately as `ic_category`), so for cards we use
/// `{network}{funding}` (e.g. `visacredit`) and for wallets the report's own wallet form
/// (`{network}_applepay`). This matches how serving reconstructs the variant at decide time.
fn build_variant(network: &str, funding: &str, wallet: &str) -> String {
    let w = wallet.trim().to_lowercase();
    if w.contains("apple") {
        return format!("{network}_applepay");
    }
    if w.contains("google") {
        return format!("{network}_googlepay");
    }
    let v = format!("{network}{funding}");
    if v.is_empty() {
        network.to_string()
    } else {
        v
    }
}

/// Parse a money cell; blanks/garbage become `0.0` (mirrors Adyen's/Braintree's `to_float`).
fn to_float(s: &str) -> f64 {
    s.trim().parse::<f64>().unwrap_or(0.0)
}

/// Parse a Chase date cell (`Submission Date`), formatted US `M/D/YYYY` (e.g. `3/3/2025`). A
/// 2-digit-year fallback is kept for safety. Blank/odd values yield `None`.
fn parse_date(s: &str) -> Option<chrono::NaiveDate> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    // Choose the year format by the last field's width so `%Y` doesn't read `25` as year 0025.
    let fmt = match s.rsplit('/').next().map(str::len) {
        Some(4) => "%m/%d/%Y",
        Some(2) => "%m/%d/%y",
        _ => return None,
    };
    chrono::NaiveDate::parse_from_str(s, fmt).ok()
}

/// Streams a J.P. Morgan preset report with its envelope stripped: a leading UTF-8 BOM, the
/// `BEGIN…` and `EntityId=…,ReportTypeName=…` preamble lines, and the trailing `END…` line are all
/// dropped, leaving a plain `header + data` CSV for [`csv_reader::parse`]. Line-buffered, so a
/// multi-GB report stays flat in memory.
struct EnvelopeReader<R> {
    inner: BufReader<R>,
    line: Vec<u8>, // current kept line (incl. its trailing '\n'), being handed out
    pos: usize,    // bytes of `line` already emitted
    first: bool,   // strip a UTF-8 BOM on the first physical line
    eof: bool,
}

impl<R: Read> EnvelopeReader<R> {
    fn new(inner: R) -> Self {
        Self {
            inner: BufReader::new(inner),
            line: Vec::new(),
            pos: 0,
            first: true,
            eof: false,
        }
    }
}

/// A framing line the CSV parser must not see: the `BEGIN`/`END` markers and the `EntityId=…`
/// metadata preamble. Data rows begin with a date and the header with `Merchant Order Number`/etc.,
/// so none of them collide with these prefixes.
fn is_envelope_line(line: &[u8]) -> bool {
    let s = line
        .iter()
        .position(|b| !b.is_ascii_whitespace())
        .map(|i| &line[i..])
        .unwrap_or(line);
    s.starts_with(b"BEGIN,") || s.starts_with(b"END,") || s.starts_with(b"EntityId=")
}

impl<R: Read> Read for EnvelopeReader<R> {
    fn read(&mut self, out: &mut [u8]) -> std::io::Result<usize> {
        use std::io::BufRead;
        loop {
            // Drain the current kept line first.
            if self.pos < self.line.len() {
                let n = (self.line.len() - self.pos).min(out.len());
                out[..n].copy_from_slice(&self.line[self.pos..self.pos + n]);
                self.pos += n;
                return Ok(n);
            }
            if self.eof {
                return Ok(0);
            }
            // Pull the next physical line.
            self.line.clear();
            self.pos = 0;
            if self.inner.read_until(b'\n', &mut self.line)? == 0 {
                self.eof = true;
                return Ok(0);
            }
            if self.first {
                if self.line.starts_with(&[0xEF, 0xBB, 0xBF]) {
                    self.line.drain(0..3);
                }
                self.first = false;
            }
            if is_envelope_line(&self.line) {
                // Discard the frame line so the drain check at the loop top doesn't emit it, then
                // pull the next physical line.
                self.line.clear();
                continue;
            }
            // Kept line — loop back and emit it.
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal Deposit Details report: BOM + BEGIN + metadata preamble, header, a sale, a refund,
    /// then the END trailer. Column order is intentionally the real report's order.
    const REPORT: &[u8] = b"\xef\xbb\xbfBEGIN,EntityId=418553,Frequency=adhoc\n\
EntityId=418553,ReportTypeName=SubmissionDetails,Frequency=adhoc,FromDate=2025-03-03,ToDate=2025-03-04\n\
Submission Date,Presentment Currency Code,Settlement Currency Code,Transaction Division External Identifier,Payment Method Code,Merchant Order Number,Account Number,Credit Card Expiration Year Month Number,Transaction Amount in Presentment Currency,Merchant Action Code,Action Type Code Text,Authorization Date,Country of Issuance,Transaction Type Code,Transaction Type Text,Merchant Category Code,Digital Token Method Code,Digital Wallet Brand,Interchange Qualification Code,Interchange Unit Fee,Fee Rate,Total Interchange Amount,Total Assessment Amount,Other Debit Passthrough Fees,Merchant Information Description,Surcharge Amount,Bank Sort Code,Card Usage Type\n\
3/3/2025,USD,USD,418554,VI,SYN0002500,401068XXXXXX9249,26-Jan,60,DP,SALE,3/3/2025,IN,7,ECI Indicator - Channel Encrypted Transaction,4722,0,,VINT,0.20,0.0315,-2.09,-0.084,-0.01,N,,,3\n\
3/3/2025,USD,USD,418554,VI,SYN0002501,476134XXXXXX0050,25-May,-20,RF,REFUND,3/3/2025,US,7,ECI Indicator - Channel Encrypted Transaction,5968,0,-0.0205,0.41,-0.022,0,N,,,1\n\
END,EntityId=418553,Frequency=adhoc\n";

    #[test]
    fn parses_sales_and_skips_refunds_and_envelope() {
        let rows = ChaseReportSource::new().parse_report(REPORT).unwrap();
        assert_eq!(rows.len(), 1, "only the sale row survives (refund + envelope skipped)");
        let r = &rows[0];
        assert_eq!(r.txn_ref, "SYN0002500");
        assert_eq!(r.card_network, "visa", "VI -> visa");
        assert_eq!(r.funding, "credit", "Card Usage Type 3 -> credit");
        assert_eq!(r.variant, "visacredit");
        assert_eq!(r.issuer_country, "IN");
        assert_eq!(r.currency, "USD");
        assert_eq!(r.ic_category, "VINT");
        assert_eq!(r.channel, "ecom", "no Terminal Identifier column -> ecom");
        // Fees are negated to positive cost magnitudes.
        assert!((r.interchange - 2.09).abs() < 1e-9);
        assert!((r.scheme_fee - 0.084).abs() < 1e-9);
        assert!((r.markup - 0.01).abs() < 1e-9, "Other Debit Passthrough Fees negated");
        assert_eq!(r.commission, 0.0);
        assert!((r.total_fee - (2.09 + 0.084 + 0.01)).abs() < 1e-9);
        assert!((r.gross - 60.0).abs() < 1e-9, "Transaction Amount is gross as-is");
        assert_eq!(r.txn_date, chrono::NaiveDate::from_ymd_opt(2025, 3, 3));
    }

    #[test]
    fn debit_and_wallet_variants() {
        // signature debit (usage 1) with an Apple Pay wallet.
        let csv = b"BEGIN,EntityId=1,Frequency=adhoc\n\
Merchant Order Number,Payment Method Code,Card Usage Type,Country of Issuance,Settlement Currency Code,Interchange Qualification Code,Action Type Code Text,Transaction Amount in Presentment Currency,Total Interchange Amount,Total Assessment Amount,Digital Wallet Brand\n\
o1,MC,1,US,USD,MCEB,SALE,40,-0.30,-0.05,APPLE\n\
END,EntityId=1,Frequency=adhoc\n";
        let rows = ChaseReportSource::new().parse_report(csv).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].card_network, "mc");
        assert_eq!(rows[0].funding, "debit", "usage 1 = signature debit");
        assert_eq!(rows[0].variant, "mc_applepay", "wallet takes precedence");
        assert_eq!(rows[0].markup, 0.0, "absent Other Debit Passthrough Fees -> 0");
    }

    #[test]
    fn funding_reconciled_from_interchange_code() {
        // A Visa credit interchange program (V148, ~3.3%) mislabeled `Card Usage Type = 1` (signature
        // debit) — the export contamination. The interchange code wins, so it prices as credit.
        let csv = b"Merchant Order Number,Payment Method Code,Card Usage Type,Country of Issuance,Settlement Currency Code,Interchange Qualification Code,Action Type Code Text,Transaction Amount in Presentment Currency,Total Interchange Amount,Total Assessment Amount\n\
o1,VI,1,US,USD,V148,SALE,100,-3.15,-0.14\n\
o2,VI,1,US,USD,VSDD,SALE,100,-0.05,-0.13\n";
        let rows = ChaseReportSource::new().parse_report(csv).unwrap();
        assert_eq!(rows.len(), 2);
        // V148 is a credit program → corrected to credit despite usage=1.
        assert_eq!(rows[0].funding, "credit", "V148 credit program overrides usage=1");
        assert_eq!(rows[0].variant, "visacredit");
        // VSDD is a genuine (regulated) debit program → stays debit, consistent with usage=1.
        assert_eq!(rows[1].funding, "debit", "VSDD debit program");
        assert_eq!(rows[1].variant, "visadebit");
    }

    #[test]
    fn unknown_method_code_forms_its_own_cluster() {
        let csv = b"Merchant Order Number,Payment Method Code,Card Usage Type,Country of Issuance,Settlement Currency Code,Interchange Qualification Code,Action Type Code Text,Transaction Amount in Presentment Currency,Total Interchange Amount,Total Assessment Amount\n\
o2,ED,3,GB,GBP,EDBT,SALE,100,-1.00,-0.20\n";
        let rows = ChaseReportSource::new().parse_report(csv).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].card_network, "ed", "unknown MOP code lowercased");
        assert_eq!(rows[0].variant, "edcredit");
    }

    #[test]
    fn missing_required_column_errors() {
        let csv = b"Merchant Order Number,Action Type Code Text\no1,SALE\n";
        let err = ChaseReportSource::new().parse_report(csv).unwrap_err();
        assert!(matches!(err, IngestError::Parse(_)));
    }

    #[test]
    fn helpers() {
        assert_eq!(normalize_network("VI"), "visa");
        assert_eq!(normalize_network("mc"), "mc");
        assert_eq!(normalize_network("AX"), "amex");
        assert_eq!(normalize_network(""), "");
        assert_eq!(funding_from_usage_type("1"), "debit");
        assert_eq!(funding_from_usage_type("2"), "debit");
        assert_eq!(funding_from_usage_type("3"), "credit");
        assert_eq!(funding_from_usage_type(""), "");
        // Reconciliation: interchange code wins over usage type when it's a known program.
        assert_eq!(reconcile_funding("1", "V148"), "credit", "credit program beats usage=1");
        assert_eq!(reconcile_funding("3", "VSDD"), "debit", "debit program beats usage=3");
        assert_eq!(reconcile_funding("1", "MCEB"), "debit", "unlisted code -> usage type");
        assert_eq!(reconcile_funding("3", "UNKNOWN"), "credit", "unlisted code -> usage type");
        assert_eq!(reconcile_funding("", ""), "", "no signal -> empty");
        assert_eq!(parse_date("3/3/2025"), chrono::NaiveDate::from_ymd_opt(2025, 3, 3));
        assert_eq!(parse_date("12/21/2024"), chrono::NaiveDate::from_ymd_opt(2024, 12, 21));
        assert_eq!(parse_date(""), None);
    }
}
