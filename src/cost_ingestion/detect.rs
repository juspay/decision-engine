//! Fee-regime-change detection: surface clusters whose fitted price moved between snapshots.
//!
//! Because we refit on every ingestion, a connector changing its pricing (e.g. Adyen 2.40% + €0.22
//! → 2.50% + €0.25) shows up as a step in a cluster's `pct_bps`/`fixed` from one `cost_fee_model`
//! snapshot to the next. This diffs each cluster's two most recent fits and reports the material
//! moves — a price-change timeline for the dashboard. The two-part model means we can say *which*
//! part moved: the percentage (`pct_bps`) or the flat fee (`fixed`). See §10.

use std::sync::OnceLock;
use std::time::Duration;

use masking::PeekInterface;
use serde::Serialize;

use crate::config::ClickHouseAnalyticsConfig;

use super::types::IngestError;

const TIMEOUT: Duration = Duration::from_secs(30);
/// Report a change only if the percentage moved more than this (basis points).
const TOL_BPS: f64 = 3.0;
/// …or the flat per-transaction fee moved more than this (settlement-currency units).
const TOL_FIXED: f64 = 0.01;

#[derive(Debug, Clone, Serialize)]
pub struct PriceChange {
    pub connector: String,
    pub account: String,
    pub card_network: String,
    pub variant: String,
    pub funding: String,
    pub issuer_country: String,
    pub currency: String,
    pub ic_category: String,
    pub old_pct_bps: f64,
    pub new_pct_bps: f64,
    pub old_fixed: f64,
    pub new_fixed: f64,
    /// The snapshot date the new price first appeared.
    pub changed_on: String,
}

const CHANGES_SQL: &str = r#"
WITH ranked AS (
    SELECT
        connector, account, card_network, variant, funding, issuer_country, currency, ic_category,
        report_date, pct_bps, fixed, verdict,
        row_number() OVER (
            PARTITION BY connector, account, card_network, variant, funding, issuer_country, currency, ic_category
            ORDER BY report_date DESC
        ) AS rn
    FROM __DB__.cost_fee_model FINAL
    WHERE merchant_id = {merchant_id:String}
)
SELECT
    cur.connector, cur.account, cur.card_network, cur.variant, cur.funding,
    cur.issuer_country, cur.currency, cur.ic_category,
    prev.pct_bps AS old_pct, cur.pct_bps AS new_pct,
    prev.fixed AS old_fixed, cur.fixed AS new_fixed,
    toString(cur.report_date) AS changed_on
FROM (SELECT * FROM ranked WHERE rn = 1) AS cur
INNER JOIN (SELECT * FROM ranked WHERE rn = 2) AS prev
    USING (connector, account, card_network, variant, funding, issuer_country, currency, ic_category)
WHERE cur.verdict = 'GOOD'
  AND (abs(cur.pct_bps - prev.pct_bps) > {tol_bps:Float64}
       OR abs(cur.fixed - prev.fixed) > {tol_fixed:Float64})
ORDER BY abs(cur.pct_bps - prev.pct_bps) + abs(cur.fixed - prev.fixed) * 10000 DESC
LIMIT 200
FORMAT TSV
"#;

/// Detected fee-regime changes for a merchant, most significant first.
pub async fn price_changes(
    cfg: &ClickHouseAnalyticsConfig,
    merchant_id: &str,
) -> Result<Vec<PriceChange>, IngestError> {
    let sql = CHANGES_SQL.replace("__DB__", &cfg.database);
    let mut req = client()
        .post(cfg.url.trim_end_matches('/'))
        .query(&[
            ("param_merchant_id", merchant_id),
            ("param_tol_bps", &TOL_BPS.to_string()),
            ("param_tol_fixed", &TOL_FIXED.to_string()),
        ])
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
            "clickhouse price-change query failed ({status}): {text}"
        )));
    }
    let text = resp
        .text()
        .await
        .map_err(|e| IngestError::Storage(e.to_string()))?;

    let mut out = Vec::new();
    for line in text.lines() {
        if line.is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        if f.len() < 13 {
            continue;
        }
        let g = |i: usize| f[i].trim().parse::<f64>().unwrap_or(0.0);
        out.push(PriceChange {
            connector: f[0].to_string(),
            account: f[1].to_string(),
            card_network: f[2].to_string(),
            variant: f[3].to_string(),
            funding: f[4].to_string(),
            issuer_country: f[5].to_string(),
            currency: f[6].to_string(),
            ic_category: f[7].to_string(),
            old_pct_bps: g(8),
            new_pct_bps: g(9),
            old_fixed: g(10),
            new_fixed: g(11),
            changed_on: f[12].trim().to_string(),
        });
    }
    Ok(out)
}

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| super::ch_http::client(TIMEOUT))
}
