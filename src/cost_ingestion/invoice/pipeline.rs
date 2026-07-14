//! Invoice ingest glue: parse → reduce → store → refresh serving.
//!
//! The invoice equivalent of `cost_ingestion::pipeline` for settlement reports, but far smaller: an
//! invoice is a few hundred lines, so there is no streaming, batching, or ClickHouse staging — the
//! whole document is parsed in memory, reduced to a two-parameter [`CostAddon`], and written to the
//! key-value store. The one ClickHouse touch is *optional*: the settled-volume fallback for the
//! amortization denominators when the invoice does not state its own turnover / transaction count.

use masking::PeekInterface;

use crate::config::ClickHouseAnalyticsConfig;
use crate::cost_ingestion::types::IngestError;

use super::reduce::{reduce_to_addon, VolumeFallback};
use super::source::InvoiceRegistry;
use super::store::{self, StoredAddon};
use super::types::{CostAddon, LineKind, ParsedInvoice};

/// One invoice fee type, collapsed across duplicate lines — what the dashboard shows so the merchant
/// sees *which* missing fees we identified (and which we correctly ignored as already-modeled).
#[derive(Debug, Clone)]
pub struct InvoiceLineGroup {
    pub description: String,
    pub kind: LineKind,
    /// Total on the invoice for this fee type (signed: credits negative).
    pub amount: f64,
    /// Amortized per-transaction contribution to the add-on (`0` for already-modeled / volume lines).
    pub per_txn: f64,
}

/// Outcome of ingesting one invoice — the computed add-on plus the identified detail, for the
/// caller's response and the dashboard's "here's what we found" card.
#[derive(Debug, Clone)]
pub struct InvoiceOutcome {
    pub connector: String,
    pub account: String,
    pub addon: CostAddon,
    pub subtotal_ex_tax: Option<f64>,
    pub currency: String,
    pub lines: usize,
    /// Transaction count the flat fees were blended over (invoice-stated or settled fallback).
    pub txn_count: Option<u64>,
    /// Card turnover the periodic fees were amortized over.
    pub card_volume: Option<f64>,
    /// Headline for the merchant: total additional fee applied **per transaction** — the sum of the
    /// identified missing fees amortized over the transaction count.
    pub total_addon_per_txn: f64,
    /// Per-fee-type breakdown, missing fees first (largest per-txn), then already-modeled / volume.
    pub breakdown: Vec<InvoiceLineGroup>,
}

/// Parse `bytes` as `connector`'s invoice for `(merchant_id, account)`, reduce it to a cost add-on,
/// persist it, and refresh this merchant's served models so the correction takes effect immediately.
///
/// `account` (and `invoice_ref`, when the caller has it) override the invoice-derived summary fields
/// — the connector-side account is usually supplied by the upload request, mirroring the settlement
/// report upload's `account` query parameter.
pub async fn ingest_invoice_bytes(
    clickhouse: &ClickHouseAnalyticsConfig,
    connector: &str,
    account: &str,
    merchant_id: &str,
    invoice_ref: &str,
    bytes: &[u8],
) -> Result<InvoiceOutcome, IngestError> {
    let connector = connector.to_lowercase();
    let registry = InvoiceRegistry::with_builtins();
    let source = registry.get(&connector)?;

    let mut parsed = source.parse_invoice(bytes)?;
    // Stamp caller-supplied identity onto the summary.
    if !account.is_empty() {
        parsed.summary.account = account.to_string();
    }
    if !invoice_ref.is_empty() {
        parsed.summary.invoice_ref = invoice_ref.to_string();
    }

    // Only reach for ClickHouse when the invoice itself does not state both denominators.
    let need_fallback = parsed.summary.card_volume.is_none() || parsed.summary.txn_count.is_none();
    let fallback = if need_fallback {
        settled_volume(clickhouse, &connector, account, merchant_id)
            .await
            .unwrap_or_default()
    } else {
        VolumeFallback::default()
    };

    let addon = reduce_to_addon(&parsed, fallback);

    // The denominators actually used (invoice-stated, else the settled fallback) — reused for the
    // per-transaction display so it exactly matches what the reduction amortized over.
    let txn_count = parsed.summary.txn_count.unwrap_or(fallback.txn_count);
    let card_volume = parsed
        .summary
        .card_volume
        .filter(|v| *v > 0.0)
        .unwrap_or(fallback.card_volume);
    let (breakdown, total_addon_per_txn) = build_breakdown(&parsed, txn_count);

    let updated_at = crate::utils::date_time::now().to_string();
    let stored = StoredAddon::new(addon, &parsed.summary, updated_at);
    store::put(merchant_id, &connector, &stored).await?;

    // Serve the new add-on now rather than after the periodic tick (same as the report upload path).
    if let Err(e) = crate::cost_ingestion::serving::refresh_merchant(clickhouse, merchant_id).await
    {
        crate::logger::warn!(
            tag = "invoice_ingest",
            "serving refresh after invoice failed: {}",
            e
        );
    }

    Ok(InvoiceOutcome {
        connector,
        account: account.to_string(),
        addon,
        subtotal_ex_tax: parsed.summary.subtotal_ex_tax,
        currency: parsed.summary.currency,
        lines: parsed.lines.len(),
        txn_count: (txn_count > 0).then_some(txn_count),
        card_volume: (card_volume > 0.0).then_some(card_volume),
        total_addon_per_txn,
        breakdown,
    })
}

/// Collapse the parsed lines to one group per (kind, description), amortize each added line over
/// `txn_count`, and order them missing-fees-first (largest per-txn) so the dashboard leads with what
/// we now apply. Returns the groups and the total added per transaction.
fn build_breakdown(parsed: &ParsedInvoice, txn_count: u64) -> (Vec<InvoiceLineGroup>, f64) {
    use std::collections::BTreeMap;
    // (kind tag, description) → summed amount. BTreeMap keeps it deterministic.
    let mut sums: BTreeMap<(&'static str, String), (LineKind, f64)> = BTreeMap::new();
    for l in &parsed.lines {
        let e = sums
            .entry((l.kind.as_str(), l.description.clone()))
            .or_insert((l.kind, 0.0));
        e.1 += l.amount;
    }

    let mut groups: Vec<InvoiceLineGroup> = sums
        .into_iter()
        .map(|((_, description), (kind, amount))| {
            let per_txn = if kind.is_added() && txn_count > 0 {
                amount / txn_count as f64
            } else {
                0.0
            };
            InvoiceLineGroup {
                description,
                kind,
                amount,
                per_txn,
            }
        })
        .collect();

    // Added (missing) fees first, then already-modeled, then volume; within a group by |per_txn|/|amount|.
    groups.sort_by(|a, b| {
        b.kind
            .is_added()
            .cmp(&a.kind.is_added())
            .then((a.kind == LineKind::Volume).cmp(&(b.kind == LineKind::Volume)))
            .then(
                b.per_txn
                    .abs()
                    .partial_cmp(&a.per_txn.abs())
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
            .then(
                b.amount
                    .abs()
                    .partial_cmp(&a.amount.abs())
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
    });

    let total_addon_per_txn = groups
        .iter()
        .filter(|g| g.kind.is_added())
        .map(|g| g.per_txn)
        .sum();
    (groups, total_addon_per_txn)
}

/// Settled volume + transaction count for `(merchant, connector, account)` from the fit window of
/// `cost_daily_stats` — the amortization denominators when the invoice is silent. Best-effort: any
/// error yields the zero fallback (which zeroes only the affected add-on term).
async fn settled_volume(
    cfg: &ClickHouseAnalyticsConfig,
    connector: &str,
    account: &str,
    merchant_id: &str,
) -> Result<VolumeFallback, IngestError> {
    // `sx` = Σ gross (turnover), `n` = transaction count. Account filter is optional: a blank
    // `account` sums across every account under the merchant/connector.
    let account_pred = if account.is_empty() {
        String::new()
    } else {
        " AND account = {account:String}".to_string()
    };
    let sql = format!(
        "SELECT sum(sx), sum(n) FROM {db}.cost_daily_stats FINAL \
         WHERE connector = {{connector:String}} AND merchant_id = {{merchant_id:String}}{account_pred} \
         FORMAT TSV",
        db = cfg.database,
    );
    let mut params = vec![
        ("connector", connector.to_string()),
        ("merchant_id", merchant_id.to_string()),
    ];
    if !account.is_empty() {
        params.push(("account", account.to_string()));
    }
    let out = exec(cfg, &sql, &params).await?;
    let mut cols = out.trim().split('\t');
    let card_volume = cols
        .next()
        .and_then(|s| s.trim().parse::<f64>().ok())
        .unwrap_or(0.0);
    let txn_count = cols
        .next()
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(0);
    Ok(VolumeFallback {
        card_volume,
        txn_count,
    })
}

/// POST a `{name:Type}`-parameterized query to ClickHouse (same transport as `fit`/`serving`).
pub(super) async fn exec(
    cfg: &ClickHouseAnalyticsConfig,
    query: &str,
    params: &[(&str, String)],
) -> Result<String, IngestError> {
    let q: Vec<(String, String)> = params
        .iter()
        .map(|(k, v)| (format!("param_{k}"), v.clone()))
        .collect();
    let mut req = reqwest::Client::new()
        .post(cfg.url.trim_end_matches('/'))
        .query(&q)
        .body(query.to_string());
    if !cfg.user.is_empty() {
        req = req.basic_auth(&cfg.user, cfg.password.as_ref().map(|p| p.peek().clone()));
    }
    let resp = req
        .send()
        .await
        .map_err(|e| IngestError::Storage(e.to_string()))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(IngestError::Storage(format!(
            "clickhouse invoice query failed ({status}): {text}"
        )));
    }
    resp.text()
        .await
        .map_err(|e| IngestError::Storage(e.to_string()))
}
