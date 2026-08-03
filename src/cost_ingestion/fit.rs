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
use serde_json::json;

use crate::config::ClickHouseAnalyticsConfig;

use super::segment::{self, SuffStats};
use super::types::IngestError;

const FIT_TIMEOUT: Duration = Duration::from_secs(120);

/// Coverage of a freshly fit snapshot, used by the caller to decide whether to trust it.
///
/// Two scopes, and they answer different questions. The fit is a rolling-window RE-FIT, not a delta:
/// every ingest refits everything in `cost_daily_stats` inside the window and replaces the snapshot.
/// So the snapshot-wide counts describe the merchant's whole model, which for a second upload is
/// dominated by clusters the *previous* report built.
#[derive(Debug, Clone, Copy, Default)]
pub struct FitSummary {
    /// Every cluster in the resulting snapshot — the merchant's complete model after this fit.
    pub total_clusters: u64,
    pub good_clusters: u64,
    /// Only the clusters THIS ingestion contributed transactions to, resolved through
    /// `cost_daily_stats.ingestion_id`. This is what belongs on a per-upload history row: a 4.2k-row
    /// UAE report that produced 6 clusters read as "161/1572" before this existed, because that was
    /// the whole snapshot including 1,566 clusters from an earlier European report.
    ///
    /// "Contributed to", not "built alone" — a cluster this upload touched may still be fitted partly
    /// from earlier reports inside the window. That overlap is the design, not a rounding error.
    pub ingested_clusters: u64,
    pub ingested_good_clusters: u64,
}

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| super::ch_http::client(FIT_TIMEOUT))
}

/// The OLS fit as nested aggregation. `__DB__` is replaced with the configured database (the
/// `{name:Type}` placeholders are ClickHouse query parameters, bound over HTTP).
const FIT_SQL: &str = r#"
INSERT INTO __DB__.cost_fee_model
    (report_date, connector, account, merchant_id, card_network, variant, funding,
     issuer_country, currency, ic_category, card_product,
     pct_bps, fixed, n, bps_rmse, r2, gross_sum, verdict)
WITH
-- Deduped per-(cluster, day, LOG-amount-band) buckets over the max window. We keep `band` (a
-- base-10 log bucket, 10/decade) so the fit can later resolve each cluster's a* crossover; FINAL
-- dedups the ReplacingMergeTree so a re-delivered day is counted once.
day_band AS (
    SELECT card_network, variant, funding, issuer_country, currency, ic_category, card_product, band, txn_date,
        sum(n) AS n, sum(sx) AS sx, sum(sy) AS sy, sum(sxx) AS sxx, sum(sxy) AS sxy,
        sum(syy) AS syy, sum(su) AS su, sum(suu) AS suu, sum(suy) AS suy,
        sum(suuy) AS suuy, sum(syyuu) AS syyuu
    FROM __DB__.cost_daily_stats FINAL
    WHERE connector = {connector:String} AND account = {account:String}
      AND merchant_id = {merchant_id:String} AND txn_date >= {max_window_start:Date}
    GROUP BY card_network, variant, funding, issuer_country, currency, ic_category, card_product, band, txn_date
),
-- Adaptive per-cluster window at DAY granularity (band-independent): keep the base window plus older
-- days until the running txn count reaches MIN_SAMPLES, so thin clusters can cross the sample gate.
windowed_days AS (
    SELECT card_network, variant, funding, issuer_country, currency, ic_category, card_product, txn_date
    FROM (
        SELECT card_network, variant, funding, issuer_country, currency, ic_category, card_product, txn_date,
            sum(dn) OVER (
                PARTITION BY card_network, variant, funding, issuer_country, currency, ic_category, card_product
                ORDER BY txn_date DESC ROWS BETWEEN UNBOUNDED PRECEDING AND 1 PRECEDING
            ) AS cum_n_before
        FROM (
            SELECT card_network, variant, funding, issuer_country, currency, ic_category, card_product, txn_date,
                sum(n) AS dn
            FROM day_band
            GROUP BY card_network, variant, funding, issuer_country, currency, ic_category, card_product, txn_date
        )
    )
    WHERE txn_date >= {base_window_start:Date} OR coalesce(cum_n_before, 0) < {min_samples:UInt32}
),
-- Per-(cluster, band) sufficient stats over the in-window days, with each bucket's amount range.
per_band AS (
    SELECT card_network, variant, funding, issuer_country, currency, ic_category, card_product, band,
        sum(n) AS n, sum(sx) AS sx, sum(sy) AS sy, sum(sxx) AS sxx, sum(sxy) AS sxy,
        sum(syy) AS syy, sum(su) AS su, sum(suu) AS suu, sum(suy) AS suy,
        sum(suuy) AS suuy, sum(syyuu) AS syyuu,
        pow(10, (toFloat64(band) + 1) / 10) AS band_hi   -- bucket's upper amount bound
    FROM day_band
    INNER JOIN windowed_days USING (card_network, variant, funding, issuer_country, currency, ic_category, card_product, txn_date)
    GROUP BY card_network, variant, funding, issuer_country, currency, ic_category, card_product, band
),
-- Whole-cluster OLS fit → slope, intercept, and the a* = fixed/rate crossover (0 when there is no
-- positive fixed fee, which makes the a* grade fall back to the whole cluster).
whole AS (
    SELECT *,
        (n * sxx - sx * sx) AS denom,
        if(denom = 0, nan, (n * sxy - sx * sy) / denom) AS slope,
        if(denom = 0, nan, (sy - slope * sx) / n) AS intercept,
        if(denom = 0 OR (n * syy - sy * sy) = 0, nan,
           pow(n * sxy - sx * sy, 2) / (denom * (n * syy - sy * sy))) AS r2,
        if(slope > 0 AND intercept > 0, intercept / slope, 0.0) AS a_star,
        -- 95% CI half-width of the slope, in bps (L2 reliability gate). Based on the ABSOLUTE
        -- residual variance (SSE), which — unlike proportional bps_rmse — is not inflated by the
        -- low-amount dust, so a tight rate reads tight even for a thin cluster. 999999 when
        -- undegenerate (n<=2 or no amount spread) so it can never promote.
        if(n > 2 AND denom > 0,
           1.96 * sqrt(greatest(0.0, syy - intercept * sy - slope * sxy) / (n - 2) * n / denom) * 10000,
           999999.0) AS ci_bps
    FROM (
        SELECT card_network, variant, funding, issuer_country, currency, ic_category, card_product,
            sum(n) AS n, sum(sx) AS sx, sum(sy) AS sy, sum(sxx) AS sxx, sum(sxy) AS sxy, sum(syy) AS syy
        FROM per_band
        GROUP BY card_network, variant, funding, issuer_country, currency, ic_category, card_product
    )
),
-- Proportional-error sufficient stats over the buckets AT/ABOVE a* (upper bound past a*), i.e. the
-- region where cost is proportional. The fixed-fee-dominated buckets below a* are excluded from the
-- grade — no data deleted, no hardcoded threshold; a* comes from each cluster's own fit.
above AS (
    SELECT pb.card_network AS card_network, pb.variant AS variant, pb.funding AS funding,
        pb.issuer_country AS issuer_country, pb.currency AS currency, pb.ic_category AS ic_category,
        pb.card_product AS card_product,
        sum(pb.n) AS n_above, sum(pb.su) AS su, sum(pb.suu) AS suu, sum(pb.suy) AS suy,
        sum(pb.suuy) AS suuy, sum(pb.syyuu) AS syyuu
    FROM per_band AS pb
    INNER JOIN whole AS w USING (card_network, variant, funding, issuer_country, currency, ic_category, card_product)
    WHERE pb.band_hi > w.a_star
    GROUP BY pb.card_network, pb.variant, pb.funding, pb.issuer_country, pb.currency, pb.ic_category, pb.card_product
)
SELECT
    report_date, connector, account, merchant_id, card_network, variant, funding,
    issuer_country, currency, ic_category, card_product,
    pct_bps, fixed, n, bps_rmse, r2, gross_sum,
    -- Verdict with the L2 reliability gate. A poor fit is NON_LINEAR only with enough data to say so
    -- (else THIN). A good fit is GOOD with >=200 txns OR — the L2 promotion — with >=30 txns and a
    -- tight slope CI (the rate is well-pinned despite few samples). Otherwise THIN (safe fallback).
    multiIf(
        (n_above = 0 OR isNaN(bps_rmse) OR bps_rmse > 15) AND n >= 200, 'NON_LINEAR',
        n_above = 0 OR isNaN(bps_rmse) OR bps_rmse > 15, 'THIN',
        n >= 200, 'GOOD',
        n >= 30 AND ci_bps <= 15, 'GOOD',
        'THIN'
    ) AS verdict
FROM
(
    SELECT
        {report_date:Date} AS report_date, {connector:String} AS connector,
        {account:String} AS account, {merchant_id:String} AS merchant_id,
        w.card_network AS card_network, w.variant AS variant, w.funding AS funding,
        w.issuer_country AS issuer_country, w.currency AS currency, w.ic_category AS ic_category,
        w.card_product AS card_product,
        w.slope * 10000 AS pct_bps, w.intercept AS fixed, w.n AS n, w.r2 AS r2, w.sx AS gross_sum,
        w.ci_bps AS ci_bps,
        -- A cluster with no bucket above a* (fully fixed-fee-dominated) has no proportional region;
        -- coalesce keeps the row and grades it NON_LINEAR rather than emitting a null.
        coalesce(a.n_above, 0) AS n_above,
        if(n_above = 0, 999999.0,
           sqrt(greatest(0.0,
               w.intercept * w.intercept * coalesce(a.suu, 0.0) + n_above * w.slope * w.slope
               + coalesce(a.syyuu, 0.0) - 2 * w.intercept * coalesce(a.suuy, 0.0)
               - 2 * w.slope * coalesce(a.suy, 0.0) + 2 * w.intercept * w.slope * coalesce(a.su, 0.0)
           ) / n_above) * 10000
        ) AS bps_rmse
    FROM whole AS w
    LEFT JOIN above AS a USING (card_network, variant, funding, issuer_country, currency, ic_category, card_product)
)
"#;

const SUMMARY_SQL: &str = r#"
SELECT count() AS total, countIf(verdict = 'GOOD') AS good
FROM __DB__.cost_fee_model
WHERE connector = {connector:String} AND account = {account:String}
  AND merchant_id = {merchant_id:String} AND report_date = {report_date:Date}
FORMAT TSV
"#;

/// The same coverage, narrowed to the clusters ONE ingestion contributed to.
///
/// `cost_daily_stats.ingestion_id` records which ingest last wrote each bucket, so the distinct
/// cluster keys carrying this id are exactly the clusters this report touched; the snapshot then
/// supplies their grade. Without this narrowing a per-upload history row reports the whole rolling
/// window's model, which is not what the merchant just uploaded.
const INGESTED_SUMMARY_SQL: &str = r#"
SELECT count() AS total, countIf(verdict = 'GOOD') AS good
FROM __DB__.cost_fee_model
WHERE connector = {connector:String} AND account = {account:String}
  AND merchant_id = {merchant_id:String} AND report_date = {report_date:Date}
  AND (card_network, variant, funding, issuer_country, currency, ic_category, card_product) IN (
      SELECT DISTINCT card_network, variant, funding, issuer_country, currency, ic_category,
          card_product
      FROM __DB__.cost_daily_stats FINAL
      WHERE connector = {connector:String} AND account = {account:String}
        AND merchant_id = {merchant_id:String} AND ingestion_id = {ingestion_id:String}
  )
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

/// Per-(cluster, band) sufficient statistics over the base fit window — the input to the piecewise
/// (L1) segmenter. Summed over days/channel per (cluster, band); Rust sorts the bands, runs the DP,
/// and writes the resulting segments. The base window (not the adaptive one) is used deliberately:
/// segmentation is a display aid over a cluster's recent shape, not the thin-cluster sample gate.
const SEGMENT_STATS_SQL: &str = r#"
SELECT card_network, variant, funding, issuer_country, currency, ic_category, card_product, band,
    sum(n), sum(sx), sum(sy), sum(sxx), sum(sxy), sum(syy),
    sum(su), sum(suu), sum(suy), sum(suuy), sum(syyuu)
FROM __DB__.cost_daily_stats FINAL
WHERE connector = {connector:String} AND account = {account:String}
  AND merchant_id = {merchant_id:String} AND txn_date >= {base_window_start:Date}
GROUP BY card_network, variant, funding, issuer_country, currency, ic_category, card_product, band
FORMAT TSV
"#;

/// Clear this snapshot's segments before re-inserting, so a refit is a clean REPLACE (mirrors
/// `CLEAR_SNAPSHOT_SQL` for the model). Segmentation is best-effort, so the clear is too.
const CLEAR_SEGMENTS_SQL: &str = r#"
DELETE FROM __DB__.cost_fee_model_segment
WHERE connector = {connector:String} AND account = {account:String}
  AND merchant_id = {merchant_id:String} AND report_date = {report_date:Date}
"#;

/// The six cluster-key fields identifying a fitted cluster (matches `cost_fee_model` / overrides).
#[derive(Clone, PartialEq, Eq, Hash, Default, Debug)]
struct SegKey {
    card_network: String,
    variant: String,
    funding: String,
    issuer_country: String,
    currency: String,
    ic_category: String,
    card_product: String,
}

/// Parse the band-stats TSV and return, for each cluster the L1 segmenter splits into MORE THAN ONE
/// piece, its ordered segments. GOOD/linear clusters (a single piece) are omitted — their
/// whole-cluster `cost_fee_model` row already prices them. Pure over its input, so it is unit-tested
/// offline without ClickHouse.
fn segment_clusters_from_tsv(tsv: &str) -> Vec<(SegKey, Vec<segment::Segment>)> {
    use std::collections::HashMap;
    let mut by_cluster: HashMap<SegKey, Vec<(i64, SuffStats)>> = HashMap::new();
    for line in tsv.lines() {
        if line.is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        // 7 key fields (…, ic_category, card_product) + band + 11 sums.
        if f.len() < 19 {
            continue;
        }
        let Ok(band) = f[7].parse::<i64>() else {
            continue;
        };
        let p = |i: usize| f[i].parse::<f64>().unwrap_or(0.0);
        let key = SegKey {
            card_network: f[0].to_string(),
            variant: f[1].to_string(),
            funding: f[2].to_string(),
            issuer_country: f[3].to_string(),
            currency: f[4].to_string(),
            ic_category: f[5].to_string(),
            card_product: f[6].to_string(),
        };
        let st = SuffStats {
            n: p(8),
            sx: p(9),
            sy: p(10),
            sxx: p(11),
            sxy: p(12),
            syy: p(13),
            su: p(14),
            suu: p(15),
            suy: p(16),
            suuy: p(17),
            syyuu: p(18),
        };
        by_cluster.entry(key).or_default().push((band, st));
    }

    let mut out = Vec::new();
    for (key, mut bands) in by_cluster {
        bands.sort_by_key(|(b, _)| *b);
        // Only clusters a single line can't fit benefit from a piecewise model; skip GOOD ones.
        if segment::whole_cluster_verdict(&bands) == segment::Verdict::Good {
            continue;
        }
        // Only surface a GENUINE multi-tier recovery. A fan (overlapping interchange rates under one
        // key) or otherwise-unrecoverable cluster produces mostly-NON_LINEAR pieces; showing those as
        // clean "tiers" is misleading, so skip them (the cluster falls back to the coarse blend).
        let segs = segment::segment_cluster(&bands);
        if segment::is_clean_recovery(&segs) {
            out.push((key, segs));
        }
    }
    out
}

/// Compute and store the piecewise (L1) segments for this snapshot. Best-effort: any failure is
/// logged and swallowed — segmentation is a display enrichment and must never fail the fit or the
/// ingestion. Always clears the snapshot's prior segments first (so an empty result also cleans up).
async fn refresh_segments(
    cfg: &ClickHouseAnalyticsConfig,
    connector: &str,
    account: &str,
    merchant_id: &str,
    report_date: &str,
    base_window_start: &str,
) {
    let seg_params = [
        ("connector", connector.to_string()),
        ("account", account.to_string()),
        ("merchant_id", merchant_id.to_string()),
        ("report_date", report_date.to_string()),
        ("base_window_start", base_window_start.to_string()),
    ];

    // Clear first so a refit that now yields no segments doesn't leave stale ones.
    if let Err(e) = exec(
        cfg,
        &CLEAR_SEGMENTS_SQL.replace("__DB__", &cfg.database),
        &seg_params,
    )
    .await
    {
        crate::logger::warn!(tag = "cost_fit", "segment clear failed: {e}");
        return;
    }

    let tsv = match exec(
        cfg,
        &SEGMENT_STATS_SQL.replace("__DB__", &cfg.database),
        &seg_params,
    )
    .await
    {
        Ok(t) => t,
        Err(e) => {
            crate::logger::warn!(tag = "cost_fit", "segment stats fetch failed: {e}");
            return;
        }
    };

    let clusters = segment_clusters_from_tsv(&tsv);
    if clusters.is_empty() {
        return;
    }
    if let Err(e) =
        insert_segments(cfg, connector, account, merchant_id, report_date, &clusters).await
    {
        crate::logger::warn!(tag = "cost_fit", "segment insert failed: {e}");
    }
}

/// Bulk-insert the recovered segments as `FORMAT JSONEachRow` (mirrors `sink.rs`). One row per
/// segment; `pct_bps`/`fixed`/`bps_rmse` are Nullable so an unfittable piece serializes as null.
async fn insert_segments(
    cfg: &ClickHouseAnalyticsConfig,
    connector: &str,
    account: &str,
    merchant_id: &str,
    report_date: &str,
    clusters: &[(SegKey, Vec<segment::Segment>)],
) -> Result<(), IngestError> {
    let mut body = String::new();
    for (key, segs) in clusters {
        for (idx, s) in segs.iter().enumerate() {
            let obj = json!({
                "report_date": report_date,
                "connector": connector,
                "account": account,
                "merchant_id": merchant_id,
                "card_network": key.card_network,
                "variant": key.variant,
                "funding": key.funding,
                "issuer_country": key.issuer_country,
                "currency": key.currency,
                "ic_category": key.ic_category,
                "card_product": key.card_product,
                "seg_idx": idx as u8,
                "lo": s.lo,
                "hi": s.hi,
                "pct_bps": s.pct_bps,
                "fixed": s.fixed,
                "bps_rmse": s.bps_rmse,
                "n": s.n,
                "gross_sum": s.vol,
                "verdict": s.verdict.as_str(),
            });
            body.push_str(
                &serde_json::to_string(&obj).map_err(|e| IngestError::Storage(e.to_string()))?,
            );
            body.push('\n');
        }
    }

    let query = format!(
        "INSERT INTO {}.cost_fee_model_segment FORMAT JSONEachRow",
        cfg.database
    );
    let mut req = client()
        .post(cfg.url.trim_end_matches('/'))
        .query(&[("query", query.as_str())])
        .body(body);
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
            "clickhouse segment insert failed ({status}): {text}"
        )));
    }
    Ok(())
}

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
    ingestion_id: &str,
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
        ("base_window_start", base_window_start.clone()),
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

    // Piecewise (L1) segments for capped/tiered clusters — a best-effort display enrichment layered
    // after the model snapshot. Any failure inside is logged and swallowed, never failing the fit.
    refresh_segments(
        cfg,
        connector,
        account,
        merchant_id,
        report_date,
        &base_window_start,
    )
    .await;

    // Narrow the same coverage to what THIS upload touched, for the per-ingestion history row.
    // Best-effort and non-fatal: it is a reporting refinement, and losing it must never fail an
    // ingest that has already written its snapshot. With no ingestion id to attribute rows to
    // (nothing staged them under one), fall back to the snapshot-wide figures rather than report a
    // misleading zero.
    let (ingested, ingested_good) = if ingestion_id.is_empty() {
        (total, good)
    } else {
        let mut p = params.to_vec();
        p.push(("ingestion_id", ingestion_id.to_string()));
        match exec(
            cfg,
            &INGESTED_SUMMARY_SQL.replace("__DB__", &cfg.database),
            &p,
        )
        .await
        {
            Ok(text) => {
                let mut f = text.trim().split('\t');
                let t = f.next().and_then(|s| s.parse().ok()).unwrap_or(0);
                let g = f.next().and_then(|s| s.parse().ok()).unwrap_or(0);
                (t, g)
            }
            Err(e) => {
                crate::logger::warn!(
                    tag = "cost_ingestion",
                    "per-ingestion cluster summary failed for {}: {:?}",
                    ingestion_id,
                    e
                );
                (total, good)
            }
        }
    };

    Ok(FitSummary {
        total_clusters: total,
        good_clusters: good,
        ingested_clusters: ingested,
        ingested_good_clusters: ingested_good,
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

    // ── Piecewise segmentation wiring (parse band-stats TSV → segments) ──

    #[test]
    fn segment_clusters_from_tsv_splits_capped_and_omits_linear() {
        use super::{segment, segment_clusters_from_tsv, SuffStats};
        use std::collections::BTreeMap;

        // Deterministic LCG (no dep) mirroring the segment module's synthetic clouds.
        struct Lcg(u64);
        impl Lcg {
            fn f(&mut self) -> f64 {
                self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1);
                ((self.0 >> 11) as f64) / ((1u64 << 53) as f64)
            }
            fn logu(&mut self, lo: f64, hi: f64) -> f64 {
                10f64.powf(lo.log10() + (hi.log10() - lo.log10()) * self.f())
            }
        }

        // Two clusters worth of band stats: a UAE debit with a 1.00% cap at AED 50 (kinked), and a
        // plain single-rate cluster (linear).
        let mut rng = Lcg(9);
        let mut cap: BTreeMap<i64, SuffStats> = BTreeMap::new();
        let mut lin: BTreeMap<i64, SuffStats> = BTreeMap::new();
        for _ in 0..900 {
            let g = rng.logu(20.0, 20000.0);
            let capped = (0.01 * g).min(50.0) + 0.001 * g + 0.006 * g + 0.42;
            let linear = 0.017 * g + 0.42;
            cap.entry(segment::band_of(g)).or_default().add(g, capped);
            lin.entry(segment::band_of(g)).or_default().add(g, linear);
        }

        // Serialize both to the 19-column TSV the fetch query returns: network, variant, funding,
        // issuer, currency, ic_category(""), card_product(""), band, then the 11 sums.
        fn emit(tsv: &mut String, var: &str, m: &BTreeMap<i64, SuffStats>) {
            for (b, s) in m {
                tsv.push_str(&format!(
                    "visa\t{var}\tdebit\tAE\tAED\t\t\t{b}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                    s.n, s.sx, s.sy, s.sxx, s.sxy, s.syy, s.su, s.suu, s.suy, s.suuy, s.syyuu
                ));
            }
        }
        let mut tsv = String::new();
        emit(&mut tsv, "visadebit_capped", &cap);
        emit(&mut tsv, "visacredit_flat", &lin);

        let out = segment_clusters_from_tsv(&tsv);
        // Only the capped cluster is returned (linear one is omitted), split into 2 GOOD segments.
        assert_eq!(out.len(), 1, "only the kinked cluster yields segments");
        let (key, segs) = &out[0];
        assert_eq!(key.variant, "visadebit_capped");
        assert_eq!(segs.len(), 2, "one cap knot -> two segments");
        assert!(segs.iter().all(|s| s.verdict == segment::Verdict::Good));
        assert!(
            segs[0].hi > 4000.0 && segs[0].hi < 6000.0,
            "knot near AED 5,000"
        );
    }

    #[test]
    fn segment_clusters_from_tsv_ignores_malformed_lines() {
        use super::segment_clusters_from_tsv;
        // Too-few columns and a non-numeric band (f[7]) are skipped without panicking.
        let tsv = "visa\tvisadebit\tdebit\tAE\tAED\n\
                   visa\tvisadebit\tdebit\tAE\tAED\t\t135\tNOTNUM\t1\t2\t3\t4\t5\t6\t7\t8\t9\t10\t11\n";
        assert!(segment_clusters_from_tsv(tsv).is_empty());
    }

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
