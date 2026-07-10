//! Adyen `InvoiceSource` — the first invoice connector.
//!
//! Everything Adyen-specific about the invoice (the line-description vocabulary and how each maps
//! to a [`LineKind`], the column labels) is contained in this file, exactly as the settlement
//! parser (`connectors/adyen.rs`) contains everything report-specific.
//!
//! Accepts either format, auto-detected in [`AdyenInvoiceSource::parse_invoice`]:
//!  - the invoice **PDF** as downloaded from Adyen Customer Area ([`parse_pdf`]): text is extracted
//!    and the page-1 "Summary" buckets are read (each comes out as `<description> EUR <amount>`),
//!    with the transaction count / turnover enriched from the page-2 detail;
//!  - a **CSV** export ([`parse_csv`]): a table of `Description`, `Amount`, optional
//!    `Quantity`/`Currency` columns.
//!
//! Both normalize to the same [`ParsedInvoice`]; the classification ([`classify`]) and reduction are
//! format-independent. The vocabulary and both paths are validated against the real invoice
//! NL202510036645 (FootLocker-Eurasia, Oct 2025) in the tests. When onboarding a new merchant,
//! spot-check that their invoice uses the same section/line names — [`classify`] is the one place to
//! extend if a fee is named differently.

use super::source::InvoiceSource;
use super::types::{InvoiceLine, InvoiceSummary, LineKind, ParsedInvoice};
use crate::cost_ingestion::types::IngestError;

pub struct AdyenInvoiceSource;

impl AdyenInvoiceSource {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AdyenInvoiceSource {
    fn default() -> Self {
        Self::new()
    }
}

impl InvoiceSource for AdyenInvoiceSource {
    fn connector(&self) -> &'static str {
        "adyen"
    }

    /// Accepts either the invoice **PDF** (as downloaded from Adyen Customer Area) or a **CSV**
    /// export, auto-detected by the file's leading bytes. Both normalize to the same
    /// [`ParsedInvoice`]; the classification and reduction downstream are format-independent.
    fn parse_invoice(&self, bytes: &[u8]) -> Result<ParsedInvoice, IngestError> {
        if bytes.starts_with(b"%PDF") {
            parse_pdf(bytes)
        } else {
            parse_csv(bytes)
        }
    }
}

/// Parse the CSV export: a table of line items with `Description`, `Amount`, and optional
/// `Quantity`/`Currency` columns (see [`Cols`]).
fn parse_csv(bytes: &[u8]) -> Result<ParsedInvoice, IngestError> {
    // Invoices are tiny (a few hundred lines at most), so — unlike the multi-GB settlement
    // report — buffering and a simple typed CSV read is fine here.
    let mut rdr = csv::ReaderBuilder::new().flexible(true).from_reader(bytes);

    let header = rdr
        .byte_headers()
        .map_err(|e| IngestError::Parse(e.to_string()))?
        .clone();
    let cols = Cols::resolve(&header)?;

    let mut lines = Vec::new();
    let mut currency = String::new();
    let mut record = csv::ByteRecord::new();
    while rdr
        .read_byte_record(&mut record)
        .map_err(|e| IngestError::Parse(e.to_string()))?
    {
        let get = |i: usize| -> &str {
            record
                .get(i)
                .and_then(|b| std::str::from_utf8(b).ok())
                .unwrap_or("")
                .trim()
        };
        let description = get(cols.description).to_lowercase();
        if description.is_empty() {
            continue;
        }
        let amount = to_float(get(cols.amount));
        let quantity = cols
            .quantity
            .map(|i| get(i).parse::<u64>().unwrap_or(0))
            .unwrap_or(0);
        let line_ccy = cols
            .currency
            .map(|i| get(i).to_uppercase())
            .filter(|c| !c.is_empty());
        if currency.is_empty() {
            if let Some(c) = &line_ccy {
                currency = c.clone();
            }
        }
        lines.push(InvoiceLine {
            kind: classify(&description, amount),
            description,
            amount,
            quantity,
            currency: line_ccy.unwrap_or_else(|| currency.clone()),
        });
    }

    // Summary totals derived from the lines (a connector that states them explicitly on a header
    // row can override these in a richer parse; for the CSV export we derive):
    //  - subtotal  = sum of every fee-and-credit line (everything except pure turnover lines),
    //  - card_volume = sum of Volume lines (0 → None, so the reducer falls back to CH volume),
    //  - txn_count = total FlatPerTxn quantity (the per-txn fees are billed once per transaction).
    let subtotal_ex_tax: f64 = lines
        .iter()
        .filter(|l| l.kind != LineKind::Volume)
        .map(|l| l.amount)
        .sum();
    let card_volume: f64 = lines
        .iter()
        .filter(|l| l.kind == LineKind::Volume)
        .map(|l| l.amount)
        .sum();
    let txn_count: u64 = lines
        .iter()
        .filter(|l| l.kind == LineKind::FlatPerTxn)
        .map(|l| l.quantity)
        .max()
        .unwrap_or(0);

    Ok(ParsedInvoice {
        summary: InvoiceSummary {
            invoice_ref: String::new(),
            account: String::new(),
            card_volume: (card_volume > 0.0).then_some(card_volume),
            txn_count: (txn_count > 0).then_some(txn_count),
            subtotal_ex_tax: Some(subtotal_ex_tax),
            currency,
            period_start: None,
            period_end: None,
        },
        lines,
    })
}

/// Parse the invoice **PDF**: extract text (pdf-extract) and read the page-1 "Summary" buckets —
/// each of which comes out as a clean `<description> EUR <amount>` line — then enrich the flat-fee
/// denominator (transaction count) and turnover from the page-2 "Transaction fee" detail line. The
/// European number format (`42.547,47`) is handled by [`parse_eu_amount`]. Validated against the
/// real invoice NL202510036645 (see the test).
fn parse_pdf(bytes: &[u8]) -> Result<ParsedInvoice, IngestError> {
    let text = pdf_extract::extract_text_from_mem(bytes)
        .map_err(|e| IngestError::Parse(format!("pdf text extraction failed: {e}")))?;
    parse_summary_text(&text)
}

/// Parse Adyen invoice text into a [`ParsedInvoice`] from its "Summary" section. Split out from PDF
/// extraction so it is unit-testable on plain text without a PDF fixture.
fn parse_summary_text(text: &str) -> Result<ParsedInvoice, IngestError> {
    let mut lines: Vec<InvoiceLine> = Vec::new();
    let mut currency = String::new();
    let mut in_summary = false;
    let mut subtotals_seen = 0;

    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        if line.to_lowercase().starts_with("summary (overview") {
            in_summary = true;
            continue;
        }
        if !in_summary {
            continue;
        }
        let Some((desc, ccy, amount)) = parse_summary_line(line) else {
            continue;
        };
        let d = desc.to_lowercase();
        // The two "Invoice Subtotal excluding taxes" rows bracket the fee lines; the second is the
        // final subtotal — everything after it (Invoiced on Merchant / VAT / Total / Amount due) is
        // settlement-timing, not cost. Skip the subtotal rows; stop once the second is seen.
        if d.starts_with("invoice subtotal") {
            subtotals_seen += 1;
            if subtotals_seen >= 2 {
                break;
            }
            continue;
        }
        if d.starts_with("invoiced on merchant")
            || d.starts_with("invoice total")
            || d.starts_with("vat ")
            || d.starts_with("amount due")
        {
            break;
        }
        if currency.is_empty() {
            currency = ccy.clone();
        }
        lines.push(InvoiceLine {
            kind: classify(&d, amount),
            description: d,
            amount,
            quantity: 0,
            currency: ccy,
        });
    }

    if lines.is_empty() {
        return Err(IngestError::Parse(
            "no invoice summary lines found (unrecognized PDF layout)".to_string(),
        ));
    }

    // Enrich from the page-2 "<count> Transaction fee EUR <turnover>" detail line: the transaction
    // count blends the flat fees, the turnover amortizes the periodic fees. Best-effort — absence
    // just leaves the ClickHouse settled-volume fallback to supply the denominators.
    if let Some((count, turnover)) = transaction_fee_detail(text) {
        if count > 0 {
            if let Some(flat) = lines
                .iter_mut()
                .find(|l| l.kind == LineKind::FlatPerTxn && l.description.contains("processing"))
            {
                flat.quantity = count;
            }
        }
        if turnover > 0.0 {
            lines.push(InvoiceLine {
                description: "turnover".to_string(),
                kind: LineKind::Volume,
                amount: turnover,
                quantity: 0,
                currency: currency.clone(),
            });
        }
    }

    let subtotal_ex_tax: f64 = lines
        .iter()
        .filter(|l| l.kind != LineKind::Volume)
        .map(|l| l.amount)
        .sum();
    let card_volume: f64 = lines
        .iter()
        .filter(|l| l.kind == LineKind::Volume)
        .map(|l| l.amount)
        .sum();
    let txn_count: u64 = lines
        .iter()
        .filter(|l| l.kind == LineKind::FlatPerTxn)
        .map(|l| l.quantity)
        .max()
        .unwrap_or(0);

    Ok(ParsedInvoice {
        summary: InvoiceSummary {
            invoice_ref: String::new(),
            account: String::new(),
            card_volume: (card_volume > 0.0).then_some(card_volume),
            txn_count: (txn_count > 0).then_some(txn_count),
            subtotal_ex_tax: Some(subtotal_ex_tax),
            currency,
            period_start: None,
            period_end: None,
        },
        lines,
    })
}

/// Match a summary line `"<description> <CCY> <amount>"` with exactly one trailing currency+amount
/// (European format). Returns `(description, currency, amount)`. Lines with two amounts (e.g. the VAT
/// row) still match on the *last* one, but the caller stops before reaching them.
fn parse_summary_line(line: &str) -> Option<(String, String, f64)> {
    // Find the last "<CCY> <number>" occurrence; the description is everything before it.
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(r"(?P<ccy>[A-Z]{3})\s+(?P<amt>-?[\d.]+,\d{2})\s*$").unwrap()
    });
    let caps = re.captures(line)?;
    let m = caps.get(0)?;
    let desc = line[..m.start()].trim();
    if desc.is_empty() {
        return None;
    }
    let amount = parse_eu_amount(&caps["amt"])?;
    Some((desc.to_string(), caps["ccy"].to_string(), amount))
}

/// Pull `(transaction_count, turnover)` from the page-2 `"<count> Transaction fee EUR <turnover>"`
/// line. The `EUR` must directly follow "Transaction fee" so the "Transaction fee Refunds" line
/// doesn't match. Returns the first (settlement-detail) occurrence.
fn transaction_fee_detail(text: &str) -> Option<(u64, f64)> {
    static RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(r"([\d.]+)\s+Transaction fee\s+[A-Z]{3}\s+([\d.]+,\d{2})").unwrap()
    });
    let caps = re.captures(text)?;
    let count = parse_eu_int(&caps[1])?;
    let turnover = parse_eu_amount(&caps[2])?;
    Some((count, turnover))
}

/// Parse a European-format money string (`42.547,47`, `-1.786,15`, `136.752.869,52`) to `f64`.
fn parse_eu_amount(s: &str) -> Option<f64> {
    s.trim()
        .replace('.', "")
        .replace(',', ".")
        .parse::<f64>()
        .ok()
}

/// Parse a European-format integer (`1.418.249`) to `u64`.
fn parse_eu_int(s: &str) -> Option<u64> {
    s.trim().replace('.', "").parse::<u64>().ok()
}

/// Resolved column indices (labels resolve by name so column order can drift between exports).
struct Cols {
    description: usize,
    amount: usize,
    quantity: Option<usize>,
    currency: Option<usize>,
}

impl Cols {
    fn resolve(header: &csv::ByteRecord) -> Result<Self, IngestError> {
        let idx = |name: &str| -> Option<usize> {
            let n = name.as_bytes();
            header.iter().position(|h| h.eq_ignore_ascii_case(n))
        };
        // Either "Description" or "Type" carries the line label depending on the export variant.
        let description = idx("Description")
            .or_else(|| idx("Type"))
            .ok_or_else(|| IngestError::Parse("invoice missing Description/Type column".into()))?;
        let amount = idx("Amount")
            .or_else(|| idx("Booked Amount"))
            .ok_or_else(|| IngestError::Parse("invoice missing Amount column".into()))?;
        Ok(Self {
            description,
            amount,
            quantity: idx("Quantity"),
            currency: idx("Currency"),
        })
    }
}

/// Map an Adyen invoice line description to its [`LineKind`]. **This is the crux of the feature** —
/// the shape decides the math. Substring match on the lowercased description, most-specific first.
///
/// Unknown positive lines default to [`LineKind::Periodic`] (counted toward the add-on): on an Adyen
/// invoice the non-acquiring lines are overwhelmingly genuine periodic fees, and any misclassification
/// surfaces as a non-zero residual in reconciliation ([`super::reconcile`]) rather than silently
/// biasing routing. The double-count risks — the acquiring lines the OLS fit already prices — are the
/// ones caught *explicitly* below (`AlreadyModeled`).
fn classify(description: &str, amount: f64) -> LineKind {
    let has = |needle: &str| description.contains(needle);

    // 1. Credits first (reduce net cost): refunds, DCC (Dynamic Currency Conversion is a rebate),
    //    scheme-fee *corrections*/overcharges, and any negative-amount line. Checked before
    //    everything so a negative "Scheme Fee Correction" isn't swept into `AlreadyModeled`, and the
    //    "Transaction fee Refunds" line isn't mistaken for the flat "Transaction fee".
    if has("refund")
        || has("dcc")
        || has("dynamic currency conversion")
        || has("correction")
        || has("rebate")
        || amount < 0.0
    {
        return LineKind::Credit;
    }

    // 2. Already priced by the settlement-report fit — the entire Adyen "Payment Method Fees" bucket
    //    (interchange + scheme + markup + commission, incl. the authorisation scheme fee). Excluded
    //    from the add-on so we never double-count. Matching the bucket name AND the individual
    //    acquiring line names covers both a summary-level and a line-level invoice CSV.
    if has("payment method fees")
        || has("interchange")
        || has("commission")
        || has("markup")
        || has("acquiring")
        || has("authorisation scheme fee")
    {
        return LineKind::AlreadyModeled;
    }

    // 3. Non-Transactional Scheme Fees (3DS, Token, Acquirer Access, System Integrity) are a
    //    *periodic* fee, distinct from the acquiring "Scheme Fees" above — but the substring
    //    "scheme fee" would otherwise sweep them into `AlreadyModeled`. Match them before that.
    if has("non-transactional") || has("non transactional") {
        return LineKind::Periodic;
    }
    if has("scheme fee") {
        return LineKind::AlreadyModeled;
    }

    // 4. Flat per-transaction fees — the ~90% of the gap. Adyen's flat processing fee is billed as
    //    the "Transaction fee" line under the "Processing Fees" bucket; RevenueProtect is the flat
    //    risk fee. Do not vary with amount ⇒ fixed term.
    if has("processing fee")
        || has("transaction fee")
        || has("revenue protect")
        || has("revenueprotect")
        || has("risk fee")
    {
        return LineKind::FlatPerTxn;
    }

    // 5. Turnover / volume lines: not a fee, only the amortization denominator.
    if has("turnover") || has("volume") || has("sales") {
        return LineKind::Volume;
    }

    // 6. Everything else is periodic: Management, Management & Reconciliation, Chargeback, Managed
    //    Risk, and the individual NTSF detail lines (3D Secure Fee, Acquirer Access Fee, …).
    LineKind::Periodic
}

/// Parse a money cell; strips thousands separators, blanks/garbage → `0.0`.
fn to_float(s: &str) -> f64 {
    s.trim().replace(',', "").parse::<f64>().unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_the_real_invoice_vocabulary() {
        // Exact line/bucket names from Adyen invoice NL202510036645 (FootLocker-Eurasia, Oct 2025).
        // Flat per-transaction fees:
        assert_eq!(classify("processing fees", 42547.47), LineKind::FlatPerTxn);
        assert_eq!(classify("transaction fee", 42547.47), LineKind::FlatPerTxn);
        assert_eq!(
            classify("revenue protect service", 17524.20),
            LineKind::FlatPerTxn
        );
        assert_eq!(
            classify("risk fee (revenueprotect)", 17524.20),
            LineKind::FlatPerTxn
        );
        // Periodic:
        assert_eq!(classify("management service", 115.31), LineKind::Periodic);
        assert_eq!(
            classify("management & reconciliation service", 1236.10),
            LineKind::Periodic
        );
        assert_eq!(classify("chargeback service", 1504.72), LineKind::Periodic);
        assert_eq!(classify("managed risk service", 5500.0), LineKind::Periodic);
        assert_eq!(
            classify("non-transactional scheme fees", 4480.47),
            LineKind::Periodic
        );
        assert_eq!(
            classify("mastercard 3d secure fee", 1911.98),
            LineKind::Periodic
        );
        assert_eq!(
            classify("visa acquirer access fee", 771.90),
            LineKind::Periodic
        );
        // Credits:
        assert_eq!(classify("refund fees", -1786.15), LineKind::Credit);
        assert_eq!(
            classify("dynamic currency conversion", -12182.51),
            LineKind::Credit
        );
        assert_eq!(
            classify(
                "scheme fee correction (overrcharge) - mastercard eu 3ds2",
                -1783.73
            ),
            LineKind::Credit
        );
        assert_eq!(classify("transaction fee refunds", -0.03), LineKind::Credit);
        // Already in the PAR — MUST NOT be added (the €549k bucket + the acquiring lines):
        assert_eq!(
            classify("payment method fees", 549256.67),
            LineKind::AlreadyModeled
        );
        assert_eq!(
            classify("interchange issuing banks (adyen eu acquiring)", 114546.51),
            LineKind::AlreadyModeled
        );
        assert_eq!(
            classify("commission markup (adyen eu acquiring)", 47015.33),
            LineKind::AlreadyModeled
        );
        assert_eq!(
            classify(
                "scheme fee visa & mastercard (adyen eu acquiring)",
                69464.04
            ),
            LineKind::AlreadyModeled
        );
        assert_eq!(
            classify("authorisation scheme fee authorised", 228.99),
            LineKind::AlreadyModeled
        );
        // Turnover:
        assert_eq!(classify("turnover", 136752869.52), LineKind::Volume);
    }

    /// The PDF path, over the **exact text `pdf-extract` produces** for invoice NL202510036645
    /// (page-1 summary lines + the page-2 Transaction fee detail). Locks in that the European number
    /// format, the two-subtotal stop, and the count/turnover enrichment all work on the real output.
    #[test]
    fn parses_real_pdf_extracted_text() {
        use super::super::reduce::{reduce_to_addon, VolumeFallback};
        // Verbatim from `pdf-extract` on the real invoice (summary block + the detail line).
        let text = "\
Summary (overview of final calculation)\n\
Processing Fees EUR 42.547,47\n\
Payment Method Fees EUR 549.256,67\n\
Management Service EUR 115,31\n\
Management & Reconciliation Service EUR 1.236,10\n\
Refund Fees EUR -1.786,15\n\
Refund Fees EUR 0,00\n\
Chargeback Service EUR 1.504,72\n\
Revenue Protect Service EUR 17.524,20\n\
Dynamic Currency Conversion EUR -12.182,51\n\
Non-Transactional Scheme Fees EUR 4.480,47\n\
Invoice Subtotal excluding taxes EUR 602.696,28\n\
Scheme Fee Correction (overrcharge) - Mastercard EU 3DS2 (2025-07) (Jul-25) EUR -1.783,73\n\
Managed Risk Service EUR 5.500,00\n\
Invoice Subtotal excluding taxes EUR 606.412,55\n\
Invoiced on Merchant Account(s) EUR -600.913,24\n\
VAT 21.0% over EUR 5.499,31 EUR 1.154,86\n\
Amount due EUR 6.654,17\n\
1.418.249 Transaction fee EUR 136.752.869,52 EUR 0,0300 EUR 42.547,47 EUR 42.547,47\n";
        let parsed = parse_summary_text(text).unwrap();

        // The count + turnover were enriched from the detail line.
        assert_eq!(parsed.summary.txn_count, Some(1_418_249));
        assert_eq!(parsed.summary.card_volume, Some(136_752_869.52));
        // The €549k Payment Method Fees line parsed and is excluded from the add-on.
        assert_eq!(
            parsed
                .lines
                .iter()
                .find(|l| l.description.contains("payment method"))
                .unwrap()
                .kind,
            LineKind::AlreadyModeled
        );
        // Everything after the 2nd subtotal (Invoiced on Merchant / VAT / Amount due) was ignored.
        assert!(parsed.lines.iter().all(|l| !l.description.contains("vat")
            && !l.description.contains("amount due")
            && !l.description.contains("invoiced on merchant")));
        // Subtotal ties to the invoice's €606.412,55 (sum of every non-turnover fee line).
        let sub = parsed.summary.subtotal_ex_tax.unwrap();
        assert!((sub - 606_412.55).abs() < 0.01, "subtotal={sub}");

        let addon = reduce_to_addon(&parsed, VolumeFallback::default());
        assert!(
            (addon.fixed_addon - 0.042356).abs() < 1e-5,
            "fixed={}",
            addon.fixed_addon
        );
    }

    /// End-to-end over the real invoice's summary buckets: the parser + reduction reproduce the
    /// ~€0.0424/txn flat add-on and correctly EXCLUDE the €549k Payment Method Fees from it.
    #[test]
    fn reduces_the_real_invoice_summary() {
        use super::super::reduce::{reduce_to_addon, VolumeFallback};
        // The exact figures from invoice NL202510036645, page-1 summary (blank Quantity = no count).
        let csv = "\
Description,Amount,Quantity,Currency\n\
Processing Fees,42547.47,1418249,EUR\n\
Revenue Protect Service,17524.20,,EUR\n\
Payment Method Fees,549256.67,,EUR\n\
Management Service,115.31,,EUR\n\
Management & Reconciliation Service,1236.10,,EUR\n\
Chargeback Service,1504.72,,EUR\n\
Managed Risk Service,5500.00,,EUR\n\
Non-Transactional Scheme Fees,4480.47,,EUR\n\
Refund Fees,-1786.15,,EUR\n\
Dynamic Currency Conversion,-12182.51,,EUR\n\
Scheme Fee Correction (overcharge) - Mastercard EU 3DS2,-1783.73,,EUR\n\
Turnover,136752869.52,,EUR\n";
        let parsed = AdyenInvoiceSource::new()
            .parse_invoice(csv.as_bytes())
            .unwrap();
        assert_eq!(parsed.summary.currency, "EUR");
        assert_eq!(parsed.summary.txn_count, Some(1_418_249));

        // The €549k Payment Method Fees line is present but classified AlreadyModeled (not added).
        let pmf = parsed
            .lines
            .iter()
            .find(|l| l.description.contains("payment method"))
            .unwrap();
        assert_eq!(pmf.kind, LineKind::AlreadyModeled);

        let addon = reduce_to_addon(&parsed, VolumeFallback::default());
        // Flat: (42547.47 + 17524.20) / 1_418_249 ≈ €0.04235/txn — matches the coverage analysis.
        assert!(
            (addon.fixed_addon - 0.042356).abs() < 1e-5,
            "fixed={}",
            addon.fixed_addon
        );
        // pct: (periodic − credits) / turnover · 1e4 ≈ −0.21 bps (credits net the periodics down).
        let periodic = 115.31 + 1236.10 + 1504.72 + 5500.0 + 4480.47;
        let credits = -1786.15 - 12182.51 - 1783.73;
        let expected = (periodic + credits) / 136_752_869.52 * 10_000.0;
        assert!(
            (addon.pct_addon_bps - expected).abs() < 1e-6,
            "pct={}",
            addon.pct_addon_bps
        );
    }
}
