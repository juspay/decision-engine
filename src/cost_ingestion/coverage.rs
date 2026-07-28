//! Cost-model coverage summary for the dashboard health card.
//!
//! Answers "is cost estimation actually working for this merchant?" by aggregating the latest
//! `cost_fee_model` snapshot: how many clusters are trustworthy (GOOD) and — more meaningfully —
//! what share of settled *volume* they cover. Everything else falls back to success-rate routing
//! (§10), so `good_volume_pct` is the headline number.

use std::sync::OnceLock;
use std::time::Duration;

use masking::PeekInterface;
use serde::Serialize;

use crate::config::ClickHouseAnalyticsConfig;

use super::types::IngestError;

const TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Serialize)]
pub struct CoverageSummary {
    // Cluster counts by class.
    pub total_clusters: u64,
    pub good_clusters: u64,
    /// One line can't price it, but its recovered amount tiers can — and the router serves them.
    pub tiered_clusters: u64,
    pub thin_clusters: u64,
    /// Neither a single line nor a clean set of tiers. Surfaced as "Poor fit".
    pub non_linear_clusters: u64,
    // Transaction counts by class (the thin-tail vs poor-fit split of the gap).
    pub total_txns: u64,
    pub good_txns: u64,
    pub tiered_txns: u64,
    pub thin_txns: u64,
    pub non_linear_txns: u64,
    /// Share of *transactions* with a trustworthy single-line model.
    pub good_txn_pct: f64,
    /// Share of *transactions* the router can actually price: GOOD **plus** TIERED.
    pub priced_txn_pct: f64,
    // Money-weighted coverage — the headline for a cost/EV system.
    pub total_gross: f64,
    pub good_gross: f64,
    pub tiered_gross: f64,
    pub thin_gross: f64,
    pub non_linear_gross: f64,
    /// Share of settled *volume* (money) with a trustworthy single-line model.
    pub good_gross_pct: f64,
    /// Share of settled *volume* the router can actually price: GOOD **plus** TIERED. This is the
    /// real coverage headline — `good_gross_pct` alone under-reports a merchant whose big-ticket
    /// segments are capped (exactly the UAE/Saudi debit case, which is 100% tiered).
    pub priced_gross_pct: f64,
    // Fit accuracy of the GOOD models (per-txn cost error, basis points).
    pub bps_rmse_p50: f64,
    pub bps_rmse_p90: f64,
    /// The snapshot these numbers are from (`YYYY-MM-DD`), for a freshness indicator.
    pub report_date: String,
}

// `rd` (the latest snapshot date) is bound once via WITH and reused both as a constant column and
// the row filter — selecting the date as a bare `any(report_date)` alongside a subquery filter on
// the same column trips ClickHouse's ILLEGAL_AGGREGATION check.
// A cluster is classified by whether the ROUTER CAN PRICE IT, not by the whole-cluster fit alone:
//
//   GOOD      one line prices it
//   TIERED    one line can't, but the amount tiers recovered from it can — `serving::lookup` serves
//             these through `MerchantModels::segmented`
//   THIN      too few transactions to trust any rate — safe fallback
//   POOR FIT  neither a line nor a clean set of tiers
//
// Without the TIERED split these clusters counted as "poor fit" while the router was in fact pricing
// them, which understated real coverage. A capped-interchange cluster is `NON_LINEAR` *by
// construction* — that verdict grades the single-line fit and is the very thing that triggers
// segmentation — so verdict alone can never answer "is this usable".
//
// The tiered set requires at least one GOOD piece, matching `serving::segments_from_tsv`, which
// refuses to serve a ladder whose every piece is untrusted. Both sides therefore agree on "usable".
const SUMMARY_SQL: &str = r#"
WITH (
    SELECT max(report_date) FROM __DB__.cost_fee_model WHERE merchant_id = {merchant_id:String}
) AS rd,
tiered AS (
    SELECT DISTINCT connector, account, card_network, variant, funding, issuer_country, currency,
        ic_category, card_product
    FROM __DB__.cost_fee_model_segment FINAL
    WHERE merchant_id = {merchant_id:String} AND report_date = rd AND verdict = 'GOOD'
)
SELECT
    count() AS total_clusters,
    countIf(is_good) AS good_clusters,
    countIf(is_tiered) AS tiered_clusters,
    countIf(is_thin) AS thin_clusters,
    countIf(is_poor) AS non_linear_clusters,
    sum(n) AS total_txns,
    sumIf(n, is_good) AS good_txns,
    sumIf(n, is_tiered) AS tiered_txns,
    sumIf(n, is_thin) AS thin_txns,
    sumIf(n, is_poor) AS non_linear_txns,
    sum(gross_sum) AS total_gross,
    sumIf(gross_sum, is_good) AS good_gross,
    sumIf(gross_sum, is_tiered) AS tiered_gross,
    sumIf(gross_sum, is_thin) AS thin_gross,
    sumIf(gross_sum, is_poor) AS non_linear_gross,
    quantileIf(0.5)(bps_rmse, is_good) AS bps_rmse_p50,
    quantileIf(0.9)(bps_rmse, is_good) AS bps_rmse_p90,
    toString(rd) AS report_date
FROM (
    SELECT n, gross_sum, bps_rmse,
        verdict = 'GOOD' AS is_good,
        -- Tiered is checked before THIN: a thin-but-recovered cluster is served, so "usable" wins.
        NOT is_good AND (connector, account, card_network, variant, funding, issuer_country,
            currency, ic_category, card_product) IN (SELECT * FROM tiered) AS is_tiered,
        NOT is_good AND NOT is_tiered AND verdict = 'THIN' AS is_thin,
        NOT is_good AND NOT is_tiered AND NOT is_thin AS is_poor
    FROM __DB__.cost_fee_model
    WHERE merchant_id = {merchant_id:String} AND report_date = rd
)
FORMAT TSV
"#;

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| super::ch_http::client(TIMEOUT))
}

/// Coverage of a merchant's latest fitted snapshot across all its connectors/accounts.
pub async fn for_merchant(
    cfg: &ClickHouseAnalyticsConfig,
    merchant_id: &str,
) -> Result<CoverageSummary, IngestError> {
    let sql = SUMMARY_SQL.replace("__DB__", &cfg.database);
    // The SQL goes in the request body (guarantees a Content-Length; ClickHouse rejects a
    // body-less POST with 411). Only the `{name:Type}` bindings ride in the query string.
    let mut req = client()
        .post(cfg.url.trim_end_matches('/'))
        .query(&[("param_merchant_id", merchant_id)])
        .body(sql);
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
            "clickhouse coverage query failed ({status}): {text}"
        )));
    }
    let text = resp
        .text()
        .await
        .map_err(|e| IngestError::Storage(e.to_string()))?;

    let f: Vec<&str> = text.trim().split('\t').collect();
    let u = |i: usize| -> u64 { f.get(i).and_then(|s| s.trim().parse().ok()).unwrap_or(0) };
    let g = |i: usize| -> f64 { f.get(i).and_then(|s| s.trim().parse().ok()).unwrap_or(0.0) };
    let total_clusters = u(0);
    let good_clusters = u(1);
    let tiered_clusters = u(2);
    let thin_clusters = u(3);
    let non_linear_clusters = u(4);
    let total_txns = u(5);
    let good_txns = u(6);
    let tiered_txns = u(7);
    let thin_txns = u(8);
    let non_linear_txns = u(9);
    let total_gross = g(10);
    let good_gross = g(11);
    let tiered_gross = g(12);
    let thin_gross = g(13);
    let non_linear_gross = g(14);
    // quantiles come back as `nan` when there are no GOOD clusters; treat as 0.
    let bps_rmse_p50 = g(15);
    let bps_rmse_p90 = g(16);
    let report_date = f.get(17).unwrap_or(&"").trim().to_string();

    let pct = |part: f64, whole: f64| {
        if whole > 0.0 {
            part / whole * 100.0
        } else {
            0.0
        }
    };
    let good_txn_pct = pct(good_txns as f64, total_txns as f64);
    let priced_txn_pct = pct((good_txns + tiered_txns) as f64, total_txns as f64);
    let good_gross_pct = pct(good_gross, total_gross);
    let priced_gross_pct = pct(good_gross + tiered_gross, total_gross);
    Ok(CoverageSummary {
        total_clusters,
        good_clusters,
        tiered_clusters,
        thin_clusters,
        non_linear_clusters,
        total_txns,
        good_txns,
        tiered_txns,
        thin_txns,
        non_linear_txns,
        good_txn_pct,
        priced_txn_pct,
        total_gross,
        good_gross,
        tiered_gross,
        thin_gross,
        non_linear_gross,
        good_gross_pct,
        priced_gross_pct,
        bps_rmse_p50: if bps_rmse_p50.is_nan() {
            0.0
        } else {
            bps_rmse_p50
        },
        bps_rmse_p90: if bps_rmse_p90.is_nan() {
            0.0
        } else {
            bps_rmse_p90
        },
        report_date,
    })
}
