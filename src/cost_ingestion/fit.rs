//! Fit per-cluster cost models from the daily sufficient-statistics rollup — the OLS of
//! `par_fit.py` expressed as a ClickHouse `GROUP BY`.
//!
//! `cost_daily_stats` already holds, per (cluster × day × band × channel), the additive sums an OLS
//! fit needs (`Σx, Σy, Σxx, Σxy, Σyy` and reciprocal terms). The fit sums those buckets over the
//! window — first collapsing band/channel, then across days — to reconstruct the exact per-cluster
//! sums it would get from raw transactions, and computes `pct_bps = slope·10⁴`, `fixed = intercept`,
//! and a per-transaction `bps_rmse`. Clusters are graded `GOOD` / `NON_LINEAR` / `THIN` by the §10
//! rule (`n ≥ 200 AND bps_rmse ≤ 15`). The €5 micro-amount floor was already applied at aggregation.
//!
//! Runs entirely in ClickHouse: one `INSERT … SELECT` writes the snapshot, then a summary query
//! reports coverage for the validation gate. See `scratch/inhouse-cost-architecture.md` §3, §7 and
//! `scratch/settlement-table-removal-worked-example.md`.

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
            -- Sum the per-day buckets that fall in each cluster's adaptive window into one set of
            -- per-cluster sufficient statistics (all sums are additive across days/bands/channels).
            SELECT
                card_network, variant, funding, issuer_country, currency, ic_category,
                sum(n) AS n,
                sum(sx) AS sx,
                sum(sy) AS sy,
                sum(sxx) AS sxx,
                sum(sxy) AS sxy,
                sum(syy) AS syy,
                sum(su) AS su,
                sum(suu) AS suu,
                sum(suy) AS suy,
                sum(suuy) AS suuy,
                sum(syyuu) AS syyuu
            FROM
            (
                -- Adaptive per-cluster window at day granularity: keep every day in the base window
                -- (recent, so high-volume clusters stay agile to price changes), and let thin
                -- clusters reach back over older days until the running txn count crosses
                -- MIN_SAMPLES (capped at the max window) so they can cross the GOOD sample gate
                -- instead of being stuck THIN. `cum_n_before` = txns on strictly-more-recent days.
                SELECT *
                FROM
                (
                    SELECT
                        card_network, variant, funding, issuer_country, currency, ic_category,
                        txn_date, n, sx, sy, sxx, sxy, syy, su, suu, suy, suuy, syyuu,
                        sum(n) OVER (
                            PARTITION BY card_network, variant, funding, issuer_country, currency, ic_category
                            ORDER BY txn_date DESC
                            ROWS BETWEEN UNBOUNDED PRECEDING AND 1 PRECEDING
                        ) AS cum_n_before
                    FROM
                    (
                        -- Collapse band/channel to per-(cluster, day) sums. FINAL dedups the
                        -- ReplacingMergeTree so a day re-delivered by a later report (overlapping
                        -- monthly+daily, a re-upload, webhook+manual) is counted once — its latest
                        -- authoritative bucket wins.
                        SELECT
                            card_network, variant, funding, issuer_country, currency, ic_category,
                            txn_date,
                            sum(n) AS n, sum(sx) AS sx, sum(sy) AS sy, sum(sxx) AS sxx,
                            sum(sxy) AS sxy, sum(syy) AS syy, sum(su) AS su, sum(suu) AS suu,
                            sum(suy) AS suy, sum(suuy) AS suuy, sum(syyuu) AS syyuu
                        FROM __DB__.cost_daily_stats FINAL
                        WHERE connector = {connector:String}
                          AND account = {account:String}
                          AND merchant_id = {merchant_id:String}
                          AND txn_date >= {max_window_start:Date}
                        GROUP BY card_network, variant, funding, issuer_country, currency,
                                 ic_category, txn_date
                    )
                )
                WHERE txn_date >= {base_window_start:Date}
                   OR coalesce(cum_n_before, 0) < {min_samples:UInt32}
            )
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

/// When a refit yields an empty snapshot — the data behind this `(connector, account)` is gone
/// (e.g. its last ingestion was deleted) — drop ALL of its prior `cost_fee_model` snapshots too.
/// Coverage and serving read the *latest* snapshot by `report_date`; without this, an empty refit
/// (which inserts no rows for today) leaves an older non-empty snapshot as the max, so the dashboard
/// and the router keep showing / routing on models that no longer have any supporting data.
const PURGE_MODEL_SQL: &str = r#"
DELETE FROM __DB__.cost_fee_model
WHERE connector = {connector:String} AND account = {account:String}
  AND merchant_id = {merchant_id:String}
"#;

/// Base trailing window (days of transactions) every cluster fits over — recent enough that
/// high-volume clusters react quickly to a fee change.
pub const BASE_WINDOW_DAYS: i64 = 90;
/// Hard cap on how far a thin cluster may reach back to accumulate enough samples.
const MAX_WINDOW_DAYS: i64 = 365;
/// Minimum transactions for the GOOD sample gate; a thin cluster extends its window toward this.
/// Must match the `n < 200 -> THIN` verdict gate in `FIT_SQL`.
const MIN_SAMPLES: u32 = 200;

/// Whether a fit result should trigger the empty-refit purge ([`PURGE_MODEL_SQL`]). Extracted as a
/// pure function because it is the one decision coupled to a destructive `DELETE`, so it is locked
/// in by unit tests: purge fires ONLY on a *definitively-parsed* zero cluster count — never on a
/// parse failure (`None`), which would otherwise masquerade as "empty" and delete a healthy model —
/// and never with a blank identifier that could widen the delete's scope.
fn should_purge_empty(
    total_parsed: Option<u64>,
    connector: &str,
    account: &str,
    merchant_id: &str,
) -> bool {
    total_parsed == Some(0)
        && !connector.is_empty()
        && !account.is_empty()
        && !merchant_id.is_empty()
}

/// Fit `(connector, account, merchant_id)` from the last `BASE_WINDOW_DAYS` of `cost_daily_stats`
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
         FROM {}.cost_daily_stats FINAL \
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
    exec(
        cfg,
        &CLEAR_SNAPSHOT_SQL.replace("__DB__", &cfg.database),
        &params,
    )
    .await?;
    exec(cfg, &FIT_SQL.replace("__DB__", &cfg.database), &params).await?;
    let summary = exec(cfg, &SUMMARY_SQL.replace("__DB__", &cfg.database), &params).await?;

    let mut fields = summary.trim().split('\t');
    // Parse `total` strictly: a parse *failure* must NOT collapse to "0 clusters", because that
    // value drives the destructive purge below. `unwrap_or(0)` here would turn a malformed/empty
    // summary response into an accidental table-wide delete. Only a definitively-parsed 0 purges.
    let total_parsed = fields.next().and_then(|s| s.parse::<u64>().ok());
    let good = fields.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let total = total_parsed.unwrap_or(0);

    // A definitively-empty refit means this (connector, account) has no fittable data left in the
    // entire fit window — its rows were deleted, or it never had any. A single sparse report cannot
    // cause this, since the fit windows over ALL staged days, not just the one just ingested. Purge
    // its stale snapshots so coverage/serving don't fall back to an older non-empty one (see
    // PURGE_MODEL_SQL and `should_purge_empty`).
    if should_purge_empty(total_parsed, connector, account, merchant_id) {
        exec(
            cfg,
            &PURGE_MODEL_SQL.replace("__DB__", &cfg.database),
            &params,
        )
        .await?;
    }

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

#[cfg(test)]
mod tests {
    use super::should_purge_empty;

    // The purge is a table-scoped DELETE, so its trigger must be exact. These tests lock in that
    // it fires on — and only on — a definitively-parsed zero cluster count with a full identifier.

    #[test]
    fn purges_on_definitive_zero() {
        // The one case that should purge: the fit ran and produced zero clusters (data is gone).
        assert!(should_purge_empty(Some(0), "adyen", "acc", "m1"));
    }

    #[test]
    fn never_purges_when_clusters_exist() {
        assert!(!should_purge_empty(Some(1), "adyen", "acc", "m1"));
        assert!(!should_purge_empty(Some(8318), "adyen", "acc", "m1"));
    }

    #[test]
    fn never_purges_on_parse_failure() {
        // A malformed/empty summary response parses to None; it must NOT look like "0 clusters" and
        // delete a healthy model. This is the sharp edge the strict parse closed.
        assert!(!should_purge_empty(None, "adyen", "acc", "m1"));
    }

    #[test]
    fn never_purges_with_blank_identifier() {
        // A blank identifier must never widen the delete's scope, even on a genuine zero count.
        assert!(!should_purge_empty(Some(0), "", "acc", "m1"));
        assert!(!should_purge_empty(Some(0), "adyen", "", "m1"));
        assert!(!should_purge_empty(Some(0), "adyen", "acc", ""));
    }
}
