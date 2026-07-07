//! Fit per-cluster cost models from staged settlement rows — the OLS of `par_fit.py` expressed
//! as a ClickHouse `GROUP BY`.
//!
//! For each cluster `total_fee = slope·gross + intercept` is fit from streaming sufficient
//! statistics (`Σx, Σy, Σxx, Σxy, …`), giving `pct_bps = slope·10⁴`, `fixed = intercept`, and a
//! per-transaction `bps_rmse`. Clusters are graded `GOOD` / `NON_LINEAR` / `THIN` by the §10 rule
//! (`n ≥ 200 AND bps_rmse ≤ 15`). The €5 micro-amount floor is a `WHERE gross >= 5`.
//!
//! Runs entirely in ClickHouse: one `INSERT … SELECT` writes the snapshot, then a summary query
//! reports coverage for the validation gate. See `scratch/inhouse-cost-architecture.md` §3, §7.

use std::sync::OnceLock;
use std::time::Duration;

use masking::PeekInterface;

use crate::config::ClickHouseAnalyticsConfig;

use super::types::IngestError;

const FIT_TIMEOUT: Duration = Duration::from_secs(120);

/// Coverage of a freshly fit snapshot, used by the caller to decide whether to trust it.
#[derive(Debug, Clone, Copy)]
pub struct FitSummary {
    pub total_clusters: u64,
    pub good_clusters: u64,
}

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(FIT_TIMEOUT)
            .build()
            .expect("failed to build clickhouse fit client")
    })
}

/// The OLS fit as nested aggregation. `__DB__` is replaced with the configured database (the
/// `{name:Type}` placeholders are ClickHouse query parameters, bound over HTTP).
const FIT_SQL: &str = r#"
INSERT INTO __DB__.cost_fee_model
    (report_date, connector, account, merchant_id, card_network, variant, funding,
     issuer_country, currency, ic_category, pct_bps, fixed, n, bps_rmse, r2, gross_sum, verdict)
SELECT
    report_date, connector, account, merchant_id, card_network, variant, funding,
    issuer_country, currency, ic_category, pct_bps, fixed, n, bps_rmse, r2, gross_sum,
    multiIf(n < 200, 'THIN', isNaN(bps_rmse) OR bps_rmse > 15, 'NON_LINEAR', 'GOOD') AS verdict
FROM
(
    SELECT
        report_date, connector, account, merchant_id, card_network, variant, funding,
        issuer_country, currency, ic_category, n, r2, gross_sum,
        slope * 10000 AS pct_bps,
        intercept AS fixed,
        sqrt(greatest(0.0, sum_sq) / n) * 10000 AS bps_rmse
    FROM
    (
        SELECT
            {report_date:Date} AS report_date,
            {connector:String} AS connector,
            {account:String} AS account,
            {merchant_id:String} AS merchant_id,
            card_network, variant, funding, issuer_country, currency, ic_category, n,
            sx AS gross_sum,
            (n * sxx - sx * sx) AS denom,
            if(denom = 0, nan, (n * sxy - sx * sy) / denom) AS slope,
            if(denom = 0, nan, (sy - slope * sx) / n) AS intercept,
            if(denom = 0 OR (n * syy - sy * sy) = 0, nan,
               pow(n * sxy - sx * sy, 2) / (denom * (n * syy - sy * sy))) AS r2,
            (intercept * intercept * suu + n * slope * slope + syyuu
             - 2 * intercept * suuy - 2 * slope * suy + 2 * intercept * slope * su) AS sum_sq
        FROM
        (
            SELECT
                card_network, variant, funding, issuer_country, currency, ic_category,
                count() AS n,
                sum(gross) AS sx,
                sum(total_fee) AS sy,
                sum(gross * gross) AS sxx,
                sum(gross * total_fee) AS sxy,
                sum(total_fee * total_fee) AS syy,
                sum(1 / gross) AS su,
                sum(1 / (gross * gross)) AS suu,
                sum(total_fee / gross) AS suy,
                sum(total_fee / (gross * gross)) AS suuy,
                sum(total_fee * total_fee / (gross * gross)) AS syyuu
            FROM
            (
                -- FINAL dedups the ReplacingMergeTree by txn_ref, so a transaction delivered in
                -- more than one report (overlapping monthly+daily, a re-upload, webhook+manual) is
                -- counted exactly once. Rank each cluster's txns by recency for the adaptive window.
                SELECT
                    card_network, variant, funding, issuer_country, currency, ic_category,
                    gross, total_fee, txn_date,
                    row_number() OVER (
                        PARTITION BY card_network, variant, funding, issuer_country, currency, ic_category
                        ORDER BY txn_date DESC
                    ) AS rn
                FROM __DB__.settlement_txn_fees FINAL
                WHERE connector = {connector:String}
                  AND account = {account:String}
                  AND merchant_id = {merchant_id:String}
                  AND gross >= 5
                  AND txn_date >= {max_window_start:Date}
            )
            -- Adaptive per-cluster window: keep every transaction in the base window (recent, so
            -- high-volume clusters stay agile to price changes), and let thin clusters reach back to
            -- their most-recent MIN_SAMPLES (capped at the max window) so they can cross the GOOD
            -- sample gate instead of being stuck THIN.
            WHERE txn_date >= {base_window_start:Date} OR rn <= {min_samples:UInt32}
            GROUP BY card_network, variant, funding, issuer_country, currency, ic_category
        )
    )
)
"#;

const SUMMARY_SQL: &str = r#"
SELECT count() AS total, countIf(verdict = 'GOOD') AS good
FROM __DB__.cost_fee_model
WHERE connector = {connector:String} AND account = {account:String}
  AND merchant_id = {merchant_id:String} AND report_date = {report_date:Date}
FORMAT TSV
"#;

// Clear this (connector, account, report_date) snapshot before the fit re-inserts it, so a refit is
// a clean REPLACE, not an append. Without this, a refit after a delete (or a same-day re-ingest)
// would leave stale clusters the new fit no longer produces — including the whole snapshot when the
// new fit is empty (an INSERT of nothing can't overwrite the old rows).
const CLEAR_SNAPSHOT_SQL: &str = r#"
DELETE FROM __DB__.cost_fee_model
WHERE connector = {connector:String} AND account = {account:String}
  AND merchant_id = {merchant_id:String} AND report_date = {report_date:Date}
"#;

/// Base trailing window (days of transactions) every cluster fits over — recent enough that
/// high-volume clusters react quickly to a fee change.
pub const BASE_WINDOW_DAYS: i64 = 90;
/// Hard cap on how far a thin cluster may reach back to accumulate enough samples.
const MAX_WINDOW_DAYS: i64 = 365;
/// Minimum transactions for the GOOD sample gate; a thin cluster extends its window toward this.
/// Must match the `n < 200 -> THIN` verdict gate in `FIT_SQL`.
const MIN_SAMPLES: u32 = 200;

/// Fit `(connector, account, merchant_id)` from the last `FIT_WINDOW_DAYS` of `settlement_txn_fees`
/// (by transaction date) into a `cost_fee_model` snapshot stamped `report_date` (the fit-run date),
/// and return the resulting coverage.
pub async fn fit_snapshot(
    cfg: &ClickHouseAnalyticsConfig,
    connector: &str,
    account: &str,
    merchant_id: &str,
    report_date: &str,
) -> Result<FitSummary, IngestError> {
    let base_params = [
        ("connector", connector.to_string()),
        ("account", account.to_string()),
        ("merchant_id", merchant_id.to_string()),
    ];

    // Window bounds are relative to the *latest transaction in the data*, not the wall clock — so a
    // backfill of older reports (or any ingestion cadence) fits correctly regardless of upload time.
    let bounds_sql = format!(
        "SELECT toString(max(txn_date) - toIntervalDay({BASE_WINDOW_DAYS})), \
                toString(max(txn_date) - toIntervalDay({MAX_WINDOW_DAYS})) \
         FROM {}.settlement_txn_fees FINAL \
         WHERE connector = {{connector:String}} AND account = {{account:String}} \
         AND merchant_id = {{merchant_id:String}} FORMAT TSV",
        cfg.database
    );
    let bounds = exec(cfg, &bounds_sql, &base_params).await?;
    let mut cols = bounds.trim().split('\t');
    // Empty staging → NULL dates; a sentinel keeps the query valid and yields 0 clusters.
    let clean = |s: &str| -> String {
        let s = s.trim();
        if s.is_empty() || s == "\\N" {
            "1970-01-01".to_string()
        } else {
            s.to_string()
        }
    };
    let base_window_start = clean(cols.next().unwrap_or(""));
    let max_window_start = clean(cols.next().unwrap_or(""));

    let params = [
        ("connector", connector.to_string()),
        ("account", account.to_string()),
        ("merchant_id", merchant_id.to_string()),
        ("report_date", report_date.to_string()),
        ("base_window_start", base_window_start),
        ("max_window_start", max_window_start),
        ("min_samples", MIN_SAMPLES.to_string()),
    ];

    // Idempotent snapshot: clear then re-insert, so the result exactly reflects current staging.
    exec(cfg, &CLEAR_SNAPSHOT_SQL.replace("__DB__", &cfg.database), &params).await?;
    exec(cfg, &FIT_SQL.replace("__DB__", &cfg.database), &params).await?;
    let summary = exec(cfg, &SUMMARY_SQL.replace("__DB__", &cfg.database), &params).await?;

    let mut fields = summary.trim().split('\t');
    let total = fields.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let good = fields.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    Ok(FitSummary {
        total_clusters: total,
        good_clusters: good,
    })
}

/// POST a query to ClickHouse with `{name:Type}` parameters bound as `param_<name>`.
async fn exec(
    cfg: &ClickHouseAnalyticsConfig,
    query: &str,
    params: &[(&str, String)],
) -> Result<String, IngestError> {
    // The SQL goes in the request body (guarantees a Content-Length; ClickHouse rejects a
    // body-less POST with 411). Only the `{name:Type}` bindings ride in the query string.
    let q: Vec<(String, String)> = params
        .iter()
        .map(|(k, v)| (format!("param_{k}"), v.clone()))
        .collect();
    let mut req = client()
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
            "clickhouse fit failed ({status}): {text}"
        )));
    }
    resp.text()
        .await
        .map_err(|e| IngestError::Storage(e.to_string()))
}
