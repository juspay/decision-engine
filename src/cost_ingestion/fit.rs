//! Fit per-cluster cost models from the daily sufficient-statistics rollup.
//!
//! `cost_daily_stats` holds additive OLS sums per `(cluster × day × fit_bucket × channel)`, plus a
//! bounded sample used only for fan detection. The fitter loads one connector/account/merchant
//! snapshot, reconstructs the same sufficient statistics the raw rows would provide, and ports the
//! richer `scratch/cluster_explorer.py` grading logic into production:
//!
//! - no currency-blind micro-amount floor;
//! - fixed/proportional decomposition at `a* = fixed / rate`;
//! - L2 confidence promotion for reliable lower-volume fits;
//! - L1 amount-range segmentation for non-linear/thin clusters;
//! - fan detection for minority sub-populations that RMSE can average away.

use std::collections::BTreeMap;
use std::sync::OnceLock;
use std::time::Duration;

use masking::PeekInterface;
use serde::Deserialize;
use serde_json::json;

use crate::config::ClickHouseAnalyticsConfig;

use super::types::IngestError;

const FIT_TIMEOUT: Duration = Duration::from_secs(120);

const MIN_N: u64 = 200;
const MAX_BPS: f64 = 15.0;
const MAX_SEGMENTS: usize = 5;
const SEG_FLOOR: u64 = 25;
const L2_MIN_N: u64 = 30;
const L2_MAX_PCT_BPS_CI: f64 = 15.0;
const FIX_VOL_TOL: f64 = 0.02;
const FAN_FRAC: f64 = 0.01;
const FAN_BPS: f64 = 30.0;
const FAN_MONEY_SEVERE: f64 = MAX_BPS;
const BUCKETS_PER_DECADE: f64 = 10.0;

/// Base trailing window (days of transactions) every cluster fits over — recent enough that
/// high-volume clusters react quickly to a fee change.
pub const BASE_WINDOW_DAYS: i64 = 90;
/// Hard cap on how far a thin cluster may reach back to accumulate enough samples.
const MAX_WINDOW_DAYS: i64 = 365;
/// Minimum transactions for the GOOD sample gate; a thin cluster extends its window toward this.
const MIN_SAMPLES: u32 = 200;

/// Coverage of a freshly fit snapshot, used by the caller to decide whether to trust it.
#[derive(Debug, Clone, Copy)]
pub struct FitSummary {
    pub total_clusters: u64,
    pub good_clusters: u64,
}

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| super::ch_http::client(FIT_TIMEOUT))
}

/// Load per-cluster/per-fit-bucket stats after applying the same adaptive day window as the old
/// ClickHouse-only fitter: always keep the recent base window, and let thin clusters reach farther
/// back until the running count crosses `MIN_SAMPLES` or the max window is exhausted.
const LOAD_ROLLUP_SQL: &str = r#"
SELECT
    s.report_account, s.card_network, s.variant, s.funding, s.issuer_country, s.currency, s.ic_category,
    s.interchange_bps, s.fit_bucket,
    sum(s.n) AS n,
    sum(s.sx) AS sx,
    sum(s.sy) AS sy,
    sum(s.sxx) AS sxx,
    sum(s.sxy) AS sxy,
    sum(s.syy) AS syy,
    sum(s.su) AS su,
    sum(s.suu) AS suu,
    sum(s.suy) AS suy,
    sum(s.suuy) AS suuy,
    sum(s.syyuu) AS syyuu,
    arrayFlatten(groupArray(s.sample_x)) AS sample_x,
    arrayFlatten(groupArray(s.sample_y)) AS sample_y
FROM __DB__.cost_daily_stats AS s FINAL
INNER JOIN
(
    SELECT report_account, card_network, variant, funding, issuer_country, currency, ic_category,
           interchange_bps, txn_date
    FROM
    (
        SELECT
            report_account, card_network, variant, funding, issuer_country, currency, ic_category,
            interchange_bps, txn_date, n,
            sum(n) OVER (
                PARTITION BY report_account, card_network, variant, funding, issuer_country, currency,
                             ic_category, interchange_bps
                ORDER BY txn_date DESC
                ROWS BETWEEN UNBOUNDED PRECEDING AND 1 PRECEDING
            ) AS cum_n_before
        FROM
        (
            SELECT
                report_account, card_network, variant, funding, issuer_country, currency, ic_category,
                interchange_bps, txn_date, sum(n) AS n
            FROM __DB__.cost_daily_stats FINAL
            WHERE connector = {connector:String}
              AND account = {account:String}
              AND merchant_id = {merchant_id:String}
              AND txn_date >= {max_window_start:Date}
            GROUP BY report_account, card_network, variant, funding, issuer_country, currency, ic_category,
                     interchange_bps, txn_date
        )
    )
    WHERE txn_date >= {base_window_start:Date}
       OR coalesce(cum_n_before, 0) < {min_samples:UInt32}
) AS d
USING (report_account, card_network, variant, funding, issuer_country, currency, ic_category,
       interchange_bps, txn_date)
WHERE s.connector = {connector:String}
  AND s.account = {account:String}
  AND s.merchant_id = {merchant_id:String}
  AND s.txn_date >= {max_window_start:Date}
GROUP BY s.report_account, s.card_network, s.variant, s.funding, s.issuer_country, s.currency, s.ic_category,
         s.interchange_bps, s.fit_bucket
ORDER BY s.report_account, s.card_network, s.variant, s.funding, s.issuer_country, s.currency, s.ic_category,
         s.interchange_bps, s.fit_bucket
FORMAT JSONEachRow
"#;

const CLEAR_SNAPSHOT_SQL: &str = r#"
DELETE FROM __DB__.cost_fee_model
WHERE connector = {connector:String} AND account = {account:String}
  AND merchant_id = {merchant_id:String} AND report_date = {report_date:Date}
"#;

const PURGE_MODEL_SQL: &str = r#"
DELETE FROM __DB__.cost_fee_model
WHERE connector = {connector:String} AND account = {account:String}
  AND merchant_id = {merchant_id:String}
"#;

const INSERT_COLUMNS: &str = "\
report_date,connector,account,report_account,merchant_id,card_network,variant,funding,issuer_country,currency,\
ic_category,interchange_bps,segment_idx,amount_lo,amount_hi,pct_bps,fixed,n,gross_sum,bps_rmse,\
grade_bps,pct_ci95_bps,crossover_amount,prop_bps,fix_abs,fix_bps,below_gross_frac,fan_frac,fan_money_bps,\
r2,verdict";

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ClusterKey {
    report_account: String,
    card_network: String,
    variant: String,
    funding: String,
    issuer_country: String,
    currency: String,
    ic_category: String,
    interchange_bps: String,
}

#[derive(Debug, Clone, Copy, Default)]
struct FitStats {
    n: u64,
    sx: f64,
    sy: f64,
    sxx: f64,
    sxy: f64,
    syy: f64,
    su: f64,
    suu: f64,
    suy: f64,
    suuy: f64,
    syyuu: f64,
}

impl FitStats {
    fn merge(&mut self, other: &Self) {
        self.n += other.n;
        self.sx += other.sx;
        self.sy += other.sy;
        self.sxx += other.sxx;
        self.sxy += other.sxy;
        self.syy += other.syy;
        self.su += other.su;
        self.suu += other.suu;
        self.suy += other.suy;
        self.suuy += other.suuy;
        self.syyuu += other.syyuu;
    }

    fn minus(self, other: Self) -> Self {
        Self {
            n: self.n.saturating_sub(other.n),
            sx: self.sx - other.sx,
            sy: self.sy - other.sy,
            sxx: self.sxx - other.sxx,
            sxy: self.sxy - other.sxy,
            syy: self.syy - other.syy,
            su: self.su - other.su,
            suu: self.suu - other.suu,
            suy: self.suy - other.suy,
            suuy: self.suuy - other.suuy,
            syyuu: self.syyuu - other.syyuu,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Sample {
    x: f64,
    y: f64,
}

#[derive(Debug, Clone)]
struct Bucket {
    fit_bucket: i32,
    stats: FitStats,
    samples: Vec<Sample>,
}

#[derive(Debug, Clone, Deserialize)]
struct RollupRow {
    #[serde(default)]
    report_account: String,
    card_network: String,
    variant: String,
    funding: String,
    issuer_country: String,
    currency: String,
    ic_category: String,
    #[serde(default)]
    interchange_bps: String,
    fit_bucket: i32,
    n: u64,
    sx: f64,
    sy: f64,
    sxx: f64,
    sxy: f64,
    syy: f64,
    su: f64,
    suu: f64,
    suy: f64,
    suuy: f64,
    syyuu: f64,
    #[serde(default)]
    sample_x: Vec<f64>,
    #[serde(default)]
    sample_y: Vec<f64>,
}

#[derive(Debug, Clone, Copy)]
struct OlsFit {
    slope: f64,
    intercept: f64,
    bps_rmse: f64,
    se_pct_bps: f64,
    r2: f64,
}

#[derive(Debug, Clone, Copy)]
struct Decomp {
    crossover_amount: f64,
    prop_bps: f64,
    fix_abs: f64,
    fix_bps: f64,
    below_n: u64,
    above_n: u64,
    below_frac: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Verdict {
    Good,
    NonLinear,
    Thin,
    Fan,
}

impl Verdict {
    fn as_str(self) -> &'static str {
        match self {
            Self::Good => "GOOD",
            Self::NonLinear => "NON_LINEAR",
            Self::Thin => "THIN",
            Self::Fan => "FAN",
        }
    }
}

#[derive(Debug, Clone)]
struct ModelRow {
    key: ClusterKey,
    segment_idx: u16,
    amount_lo: f64,
    amount_hi: f64,
    pct_bps: f64,
    fixed: f64,
    n: u64,
    gross_sum: f64,
    bps_rmse: f64,
    grade_bps: f64,
    pct_ci95_bps: f64,
    crossover_amount: f64,
    prop_bps: f64,
    fix_abs: f64,
    fix_bps: f64,
    below_gross_frac: f64,
    fan_frac: f64,
    fan_money_bps: f64,
    r2: f64,
    verdict: Verdict,
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

    let rollup = exec(
        cfg,
        &LOAD_ROLLUP_SQL.replace("__DB__", &cfg.database),
        &params,
    )
    .await?;
    let clusters = parse_rollup(&rollup)?;
    let rows = fit_clusters(&clusters);
    let total = u64::try_from(rows.len()).unwrap_or(u64::MAX);
    let good =
        u64::try_from(rows.iter().filter(|r| r.verdict == Verdict::Good).count()).unwrap_or(0);

    exec(
        cfg,
        &CLEAR_SNAPSHOT_SQL.replace("__DB__", &cfg.database),
        &params,
    )
    .await?;
    insert_models(cfg, connector, account, merchant_id, report_date, &rows).await?;

    if should_purge_empty(Some(total), connector, account, merchant_id) {
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

fn parse_rollup(text: &str) -> Result<BTreeMap<ClusterKey, Vec<Bucket>>, IngestError> {
    let mut out: BTreeMap<ClusterKey, Vec<Bucket>> = BTreeMap::new();
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let row: RollupRow =
            serde_json::from_str(line).map_err(|e| IngestError::Storage(e.to_string()))?;
        let key = ClusterKey {
            report_account: row.report_account,
            card_network: row.card_network,
            variant: row.variant,
            funding: row.funding,
            issuer_country: row.issuer_country,
            currency: row.currency,
            ic_category: row.ic_category,
            interchange_bps: row.interchange_bps,
        };
        let stats = FitStats {
            n: row.n,
            sx: row.sx,
            sy: row.sy,
            sxx: row.sxx,
            sxy: row.sxy,
            syy: row.syy,
            su: row.su,
            suu: row.suu,
            suy: row.suy,
            suuy: row.suuy,
            syyuu: row.syyuu,
        };
        let samples = row
            .sample_x
            .into_iter()
            .zip(row.sample_y.into_iter())
            .filter(|(x, y)| x.is_finite() && *x > 0.0 && y.is_finite())
            .map(|(x, y)| Sample { x, y })
            .collect();
        out.entry(key).or_default().push(Bucket {
            fit_bucket: row.fit_bucket,
            stats,
            samples,
        });
    }
    for buckets in out.values_mut() {
        buckets.sort_by_key(|b| b.fit_bucket);
    }
    Ok(out)
}

fn fit_clusters(clusters: &BTreeMap<ClusterKey, Vec<Bucket>>) -> Vec<ModelRow> {
    let mut rows = Vec::new();
    for (key, buckets) in clusters {
        rows.extend(fit_one_cluster(key, buckets));
    }
    rows
}

fn fit_one_cluster(key: &ClusterKey, buckets: &[Bucket]) -> Vec<ModelRow> {
    let whole = merge_bucket_stats(buckets);
    let all_samples = samples_for_range(buckets, f64::NEG_INFINITY, f64::INFINITY);
    let mut whole_row = build_row(key, 0, 0.0, 0.0, whole, buckets, &all_samples, None);

    if whole_row.verdict == Verdict::Thin
        && whole.n >= L2_MIN_N
        && whole_row.grade_bps.is_finite()
        && whole_row.grade_bps <= MAX_BPS
        && whole_row.pct_ci95_bps <= L2_MAX_PCT_BPS_CI
    {
        whole_row.verdict = Verdict::Good;
    }
    if whole_row.verdict == Verdict::Good
        && whole_row.fan_frac > FAN_FRAC
        && whole_row.fan_money_bps > FAN_MONEY_SEVERE
    {
        whole_row.verdict = Verdict::Fan;
    }

    if whole_row.verdict == Verdict::Good {
        return vec![whole_row];
    }

    let parts = segment_partitions(buckets);
    if parts.len() <= 1 {
        return vec![whole_row];
    }

    let mut seg_rows = Vec::new();
    for (idx, (p, q)) in parts.into_iter().enumerate() {
        let seg_buckets = &buckets[p..q];
        let stats = merge_bucket_stats(seg_buckets);
        let (lo, _) = bucket_range(seg_buckets[0].fit_bucket);
        let (_, hi) = bucket_range(seg_buckets[seg_buckets.len() - 1].fit_bucket);
        let samples = samples_for_range(buckets, lo, hi);
        let segment_idx = u16::try_from(idx + 1).unwrap_or(u16::MAX);
        let row = build_row(
            key,
            segment_idx,
            lo,
            hi,
            stats,
            seg_buckets,
            &samples,
            Some(grade_segment(stats, seg_buckets)),
        );
        seg_rows.push(row);
    }

    if seg_rows.iter().any(|r| r.verdict == Verdict::Good) {
        seg_rows
    } else {
        vec![whole_row]
    }
}

#[allow(clippy::too_many_arguments)]
fn build_row(
    key: &ClusterKey,
    segment_idx: u16,
    amount_lo: f64,
    amount_hi: f64,
    stats: FitStats,
    buckets: &[Bucket],
    samples: &[Sample],
    verdict_override: Option<Verdict>,
) -> ModelRow {
    let fit = fit_stats(stats);
    let dec = decompose(stats, buckets, fit.slope, fit.intercept);
    let grade_bps = if dec.above_n > 0 {
        dec.prop_bps
    } else {
        dec.fix_bps
    };
    let fan_frac = dispersion(samples, fit.slope, fit.intercept);
    let fan_money_bps = money_bps(samples, fit.slope, fit.intercept, dec.crossover_amount);
    let mut verdict = verdict_override.unwrap_or_else(|| {
        if stats.n < MIN_N {
            Verdict::Thin
        } else {
            grade_decomposed(dec)
        }
    });
    if verdict == Verdict::Good && fan_frac > FAN_FRAC && fan_money_bps > FAN_MONEY_SEVERE {
        verdict = Verdict::Fan;
    }
    ModelRow {
        key: key.clone(),
        segment_idx,
        amount_lo,
        amount_hi,
        pct_bps: fit.slope * 10_000.0,
        fixed: fit.intercept,
        n: stats.n,
        gross_sum: stats.sx,
        bps_rmse: fit.bps_rmse,
        grade_bps,
        pct_ci95_bps: if fit.se_pct_bps.is_finite() {
            1.96 * fit.se_pct_bps
        } else {
            f64::INFINITY
        },
        crossover_amount: dec.crossover_amount,
        prop_bps: dec.prop_bps,
        fix_abs: dec.fix_abs,
        fix_bps: dec.fix_bps,
        below_gross_frac: dec.below_frac,
        fan_frac,
        fan_money_bps,
        r2: fit.r2,
        verdict,
    }
}

fn merge_bucket_stats(buckets: &[Bucket]) -> FitStats {
    let mut out = FitStats::default();
    for b in buckets {
        out.merge(&b.stats);
    }
    out
}

fn fit_stats(s: FitStats) -> OlsFit {
    let n = f64_from_u64(s.n);
    if s.n < 2 {
        return nan_fit();
    }
    let denom = n * s.sxx - s.sx * s.sx;
    if denom <= 0.0 {
        return nan_fit();
    }
    let slope = (n * s.sxy - s.sx * s.sy) / denom;
    let intercept = (s.sy - slope * s.sx) / n;
    let bps_rmse = eval_bps(intercept, slope, s);
    let se_pct_bps = if s.n > 2 {
        let sse = s.syy - intercept * s.sy - slope * s.sxy;
        (sse.max(0.0) / (n - 2.0) * n / denom).sqrt() * 10_000.0
    } else {
        f64::INFINITY
    };
    let y_denom = n * s.syy - s.sy * s.sy;
    let r2 = if y_denom == 0.0 {
        f64::NAN
    } else {
        (n * s.sxy - s.sx * s.sy).powi(2) / (denom * y_denom)
    };
    OlsFit {
        slope,
        intercept,
        bps_rmse,
        se_pct_bps,
        r2,
    }
}

fn nan_fit() -> OlsFit {
    OlsFit {
        slope: f64::NAN,
        intercept: f64::NAN,
        bps_rmse: f64::NAN,
        se_pct_bps: f64::INFINITY,
        r2: f64::NAN,
    }
}

fn eval_bps(intercept: f64, slope: f64, s: FitStats) -> f64 {
    if s.n == 0 {
        return f64::NAN;
    }
    let n = f64_from_u64(s.n);
    let sum_sq = s.syyuu + intercept * intercept * s.suu + n * slope * slope
        - 2.0 * intercept * s.suuy
        - 2.0 * slope * s.suy
        + 2.0 * intercept * slope * s.su;
    (sum_sq.max(0.0) / n).sqrt() * 10_000.0
}

fn crossover(slope: f64, intercept: f64) -> f64 {
    if !slope.is_finite() || !intercept.is_finite() || slope <= 0.0 || intercept <= 0.0 {
        0.0
    } else {
        intercept / slope
    }
}

fn abs_rms(s: FitStats, intercept: f64, slope: f64) -> f64 {
    if s.n == 0 {
        return f64::NAN;
    }
    let n = f64_from_u64(s.n);
    let sse = s.syy - 2.0 * intercept * s.sy - 2.0 * slope * s.sxy
        + n * intercept * intercept
        + 2.0 * intercept * slope * s.sx
        + slope * slope * s.sxx;
    (sse.max(0.0) / n).sqrt()
}

fn decompose(whole: FitStats, buckets: &[Bucket], slope: f64, intercept: f64) -> Decomp {
    let crossover_amount = crossover(slope, intercept);
    let mut below = FitStats::default();
    let mut above = FitStats::default();
    for b in buckets {
        let (_, hi) = bucket_range(b.fit_bucket);
        if hi <= crossover_amount {
            below.merge(&b.stats);
        } else {
            above.merge(&b.stats);
        }
    }
    let prop_bps = if above.n > 0 {
        eval_bps(intercept, slope, above)
    } else {
        f64::NAN
    };
    let fix_abs = if below.n > 0 {
        abs_rms(below, intercept, slope)
    } else {
        f64::NAN
    };
    let fix_bps = if below.n > 0 && crossover_amount > 0.0 {
        fix_abs / crossover_amount * 10_000.0
    } else {
        f64::NAN
    };
    let total_vol = if whole.sx > 0.0 { whole.sx } else { 1.0 };
    Decomp {
        crossover_amount,
        prop_bps,
        fix_abs,
        fix_bps,
        below_n: below.n,
        above_n: above.n,
        below_frac: below.sx / total_vol,
    }
}

fn grade_decomposed(dec: Decomp) -> Verdict {
    if dec.above_n == 0 {
        return if dec.fix_bps.is_finite() && dec.fix_bps <= MAX_BPS {
            Verdict::Good
        } else {
            Verdict::NonLinear
        };
    }
    if !dec.prop_bps.is_finite() || dec.prop_bps > MAX_BPS {
        return Verdict::NonLinear;
    }
    let fix_ok = dec.below_n == 0
        || !dec.fix_bps.is_finite()
        || dec.fix_bps <= MAX_BPS
        || dec.below_frac <= FIX_VOL_TOL;
    if fix_ok {
        Verdict::Good
    } else {
        Verdict::NonLinear
    }
}

fn grade_segment(stats: FitStats, buckets: &[Bucket]) -> Verdict {
    if stats.n < SEG_FLOOR {
        return Verdict::Thin;
    }
    let fit = fit_stats(stats);
    if !fit.slope.is_finite() {
        return Verdict::NonLinear;
    }
    let dec = decompose(stats, buckets, fit.slope, fit.intercept);
    let grade_bps = if dec.above_n > 0 {
        dec.prop_bps
    } else {
        dec.fix_bps
    };
    if !grade_bps.is_finite() || grade_bps > MAX_BPS {
        return Verdict::NonLinear;
    }
    if stats.n >= MIN_N {
        return Verdict::Good;
    }
    if fit.se_pct_bps.is_finite() && 1.96 * fit.se_pct_bps <= L2_MAX_PCT_BPS_CI {
        Verdict::Good
    } else {
        Verdict::Thin
    }
}

fn dispersion(samples: &[Sample], slope: f64, intercept: f64) -> f64 {
    if !slope.is_finite() || !intercept.is_finite() {
        return 0.0;
    }
    let mut xs: Vec<f64> = samples.iter().filter(|p| p.x > 0.0).map(|p| p.x).collect();
    if xs.len() < 20 {
        return 0.0;
    }
    xs.sort_by(|a, b| a.total_cmp(b));
    let threshold = xs[xs.len() / 5];
    let mut off = 0_u64;
    let mut total = 0_u64;
    for p in samples {
        if p.x < threshold || p.x <= 0.0 {
            continue;
        }
        if ((p.y - (intercept + slope * p.x)) / p.x).abs() * 10_000.0 > FAN_BPS {
            off += 1;
        }
        total += 1;
    }
    if total == 0 {
        0.0
    } else {
        f64_from_u64(off) / f64_from_u64(total)
    }
}

fn money_bps(samples: &[Sample], slope: f64, intercept: f64, crossover_amount: f64) -> f64 {
    if !slope.is_finite() || !intercept.is_finite() {
        return 0.0;
    }
    let mut num = 0.0;
    let mut den = 0.0;
    for p in samples {
        if p.x <= 0.0 || p.x < crossover_amount {
            continue;
        }
        num += ((intercept + slope * p.x) - p.y).abs();
        den += p.x;
    }
    if den > 0.0 {
        num / den * 10_000.0
    } else {
        0.0
    }
}

fn segment_partitions(buckets: &[Bucket]) -> Vec<(usize, usize)> {
    let m = buckets.len();
    if m == 0 {
        return Vec::new();
    }

    let mut pref = Vec::with_capacity(m + 1);
    let mut running = FitStats::default();
    pref.push(running);
    for b in buckets {
        running.merge(&b.stats);
        pref.push(running);
    }
    let range_stats = |i: usize, j: usize| pref[j].minus(pref[i]);
    let cost = |i: usize, j: usize| -> f64 {
        let s = range_stats(i, j);
        if s.n < SEG_FLOOR {
            return f64::INFINITY;
        }
        let fit = fit_stats(s);
        if fit.bps_rmse.is_finite() {
            (fit.bps_rmse / 10_000.0).powi(2) * f64_from_u64(s.n)
        } else {
            f64::INFINITY
        }
    };

    let max_k = MAX_SEGMENTS.min(m);
    let mut dp = vec![vec![f64::INFINITY; m + 1]; max_k + 1];
    let mut back = vec![vec![usize::MAX; m + 1]; max_k + 1];
    dp[0][0] = 0.0;
    for k in 1..=max_k {
        for j in 1..=m {
            for i in 0..j {
                if !dp[k - 1][i].is_finite() {
                    continue;
                }
                let c = cost(i, j);
                if c.is_finite() && dp[k - 1][i] + c < dp[k][j] {
                    dp[k][j] = dp[k - 1][i] + c;
                    back[k][j] = i;
                }
            }
        }
    }

    let rebuild = |k: usize| -> Option<Vec<(usize, usize)>> {
        let mut parts = Vec::new();
        let mut kk = k;
        let mut j = m;
        while kk > 0 {
            let i = back[kk][j];
            if i == usize::MAX {
                return None;
            }
            parts.push((i, j));
            j = i;
            kk -= 1;
        }
        parts.reverse();
        Some(parts)
    };

    let mut best: Option<Vec<(usize, usize)>> = None;
    let mut best_good_vol = -1.0_f64;
    let mut best_k = usize::MAX;
    for k in 1..=max_k {
        if !dp[k][m].is_finite() {
            continue;
        }
        let Some(parts) = rebuild(k) else {
            continue;
        };
        let good_vol = parts
            .iter()
            .filter_map(|(p, q)| {
                let s = range_stats(*p, *q);
                if grade_segment(s, &buckets[*p..*q]) == Verdict::Good {
                    Some(s.sx)
                } else {
                    None
                }
            })
            .sum::<f64>();
        if good_vol > best_good_vol + 0.01
            || ((good_vol - best_good_vol).abs() <= 0.01 && k < best_k)
        {
            best_good_vol = good_vol;
            best_k = k;
            best = Some(parts);
        }
    }
    best.unwrap_or_else(|| vec![(0, m)])
}

fn samples_for_range(buckets: &[Bucket], lo: f64, hi: f64) -> Vec<Sample> {
    let mut out = Vec::new();
    for b in buckets {
        for p in &b.samples {
            if p.x >= lo && p.x < hi {
                out.push(*p);
            }
        }
    }
    out
}

fn bucket_range(bucket: i32) -> (f64, f64) {
    (
        10_f64.powf(f64::from(bucket) / BUCKETS_PER_DECADE),
        10_f64.powf(f64::from(bucket + 1) / BUCKETS_PER_DECADE),
    )
}

async fn insert_models(
    cfg: &ClickHouseAnalyticsConfig,
    connector: &str,
    account: &str,
    merchant_id: &str,
    report_date: &str,
    rows: &[ModelRow],
) -> Result<(), IngestError> {
    if rows.is_empty() {
        return Ok(());
    }
    let mut body = String::with_capacity(rows.len() * 512);
    for r in rows {
        let obj = json!({
            "report_date": report_date,
            "connector": connector,
            "account": account,
            "report_account": &r.key.report_account,
            "merchant_id": merchant_id,
            "card_network": &r.key.card_network,
            "variant": &r.key.variant,
            "funding": &r.key.funding,
            "issuer_country": &r.key.issuer_country,
            "currency": &r.key.currency,
            "ic_category": &r.key.ic_category,
            "interchange_bps": &r.key.interchange_bps,
            "segment_idx": r.segment_idx,
            "amount_lo": clean_float(r.amount_lo),
            "amount_hi": clean_float(r.amount_hi),
            "pct_bps": clean_float(r.pct_bps),
            "fixed": clean_float(r.fixed),
            "n": r.n,
            "gross_sum": clean_float(r.gross_sum),
            "bps_rmse": clean_float(r.bps_rmse),
            "grade_bps": clean_float(r.grade_bps),
            "pct_ci95_bps": clean_float(r.pct_ci95_bps),
            "crossover_amount": clean_float(r.crossover_amount),
            "prop_bps": clean_float(r.prop_bps),
            "fix_abs": clean_float(r.fix_abs),
            "fix_bps": clean_float(r.fix_bps),
            "below_gross_frac": clean_float(r.below_gross_frac),
            "fan_frac": clean_float(r.fan_frac),
            "fan_money_bps": clean_float(r.fan_money_bps),
            "r2": clean_float(r.r2),
            "verdict": r.verdict.as_str(),
        });
        body.push_str(
            &serde_json::to_string(&obj).map_err(|e| IngestError::Storage(e.to_string()))?,
        );
        body.push('\n');
    }

    let query = format!(
        "INSERT INTO {}.cost_fee_model ({INSERT_COLUMNS}) FORMAT JSONEachRow",
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
            "clickhouse model insert failed ({status}): {text}"
        )));
    }
    Ok(())
}

fn clean_float(v: f64) -> f64 {
    if v.is_finite() {
        v
    } else {
        0.0
    }
}

fn f64_from_u64(n: u64) -> f64 {
    n.to_string().parse::<f64>().unwrap_or(0.0)
}

/// Whether a fit result should trigger the empty-refit purge ([`PURGE_MODEL_SQL`]). Extracted as a
/// pure function because it is the one decision coupled to a destructive `DELETE`, so it is locked
/// in by unit tests.
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

/// POST a query to ClickHouse with `{name:Type}` parameters bound as `param_<name>`.
async fn exec(
    cfg: &ClickHouseAnalyticsConfig,
    query: &str,
    params: &[(&str, String)],
) -> Result<String, IngestError> {
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
    use super::*;

    fn stats(points: &[(f64, f64)]) -> FitStats {
        let mut out = FitStats::default();
        for (x, y) in points {
            let inv = 1.0 / x;
            out.n += 1;
            out.sx += x;
            out.sy += y;
            out.sxx += x * x;
            out.sxy += x * y;
            out.syy += y * y;
            out.su += inv;
            out.suu += inv * inv;
            out.suy += y * inv;
            out.suuy += y * inv * inv;
            out.syyuu += y * y * inv * inv;
        }
        out
    }

    fn bucket(fit_bucket: i32, points: &[(f64, f64)]) -> Bucket {
        Bucket {
            fit_bucket,
            stats: stats(points),
            samples: points
                .iter()
                .map(|(x, y)| Sample { x: *x, y: *y })
                .collect(),
        }
    }

    #[test]
    fn fit_recovers_linear_rate_and_fixed_fee() {
        let pts = [(100.0, 2.7), (200.0, 5.2), (300.0, 7.7)];
        let fit = fit_stats(stats(&pts));
        assert!((fit.slope * 10_000.0 - 250.0).abs() < 1e-9);
        assert!((fit.intercept - 0.2).abs() < 1e-9);
        assert!(fit.bps_rmse < 1e-5);
    }

    #[test]
    fn decomposition_rescues_fixed_fee_tail() {
        let mut buckets = Vec::new();
        let low: Vec<(f64, f64)> = (0..20)
            .map(|i| {
                let x = 1.0 + f64::from(i) * 0.01;
                (x, 0.2 + 0.02 * x)
            })
            .collect();
        let high: Vec<(f64, f64)> = (0..220)
            .map(|i| {
                let x = 50.0 + f64::from(i);
                (x, 0.2 + 0.02 * x)
            })
            .collect();
        buckets.push(bucket(0, &low));
        buckets.push(bucket(17, &high));
        let whole = merge_bucket_stats(&buckets);
        let fit = fit_stats(whole);
        let dec = decompose(whole, &buckets, fit.slope, fit.intercept);
        assert_eq!(grade_decomposed(dec), Verdict::Good);
        assert!(dec.below_frac < FIX_VOL_TOL);
    }

    #[test]
    fn l2_promotes_small_precise_segment() {
        let pts: Vec<(f64, f64)> = (0..40)
            .map(|i| {
                let x = 100.0 + f64::from(i);
                (x, 0.1 + 0.015 * x)
            })
            .collect();
        let b = vec![bucket(20, &pts)];
        assert_eq!(grade_segment(merge_bucket_stats(&b), &b), Verdict::Good);
    }

    #[test]
    fn fan_detector_flags_offline_minority() {
        let mut samples: Vec<Sample> = (0..200)
            .map(|i| {
                let x = 100.0 + f64::from(i);
                Sample {
                    x,
                    y: 0.2 + 0.02 * x,
                }
            })
            .collect();
        for i in 0..6 {
            let x = 5_000.0 + f64::from(i);
            samples.push(Sample {
                x,
                y: 0.2 + 0.12 * x,
            });
        }
        assert!(dispersion(&samples, 0.02, 0.2) > FAN_FRAC);
        assert!(money_bps(&samples, 0.02, 0.2, 0.0) > FAN_MONEY_SEVERE);
    }

    #[test]
    fn segment_partition_recovers_two_amount_tiers() {
        let low: Vec<(f64, f64)> = (0..80)
            .map(|i| {
                let x = 10.0 + f64::from(i) * 0.2;
                (x, 0.1 + 0.01 * x)
            })
            .collect();
        let high: Vec<(f64, f64)> = (0..80)
            .map(|i| {
                let x = 100.0 + f64::from(i);
                (x, 0.1 + 0.03 * x)
            })
            .collect();
        let buckets = vec![bucket(10, &low), bucket(20, &high)];
        let parts = segment_partitions(&buckets);
        assert!(parts.len() >= 2);
    }

    #[test]
    fn purges_on_definitive_zero() {
        assert!(should_purge_empty(Some(0), "adyen", "acc", "m1"));
    }

    #[test]
    fn never_purges_when_clusters_exist() {
        assert!(!should_purge_empty(Some(1), "adyen", "acc", "m1"));
        assert!(!should_purge_empty(Some(8318), "adyen", "acc", "m1"));
    }

    #[test]
    fn never_purges_on_parse_failure() {
        assert!(!should_purge_empty(None, "adyen", "acc", "m1"));
    }

    #[test]
    fn never_purges_with_blank_identifier() {
        assert!(!should_purge_empty(Some(0), "", "acc", "m1"));
        assert!(!should_purge_empty(Some(0), "adyen", "", "m1"));
        assert!(!should_purge_empty(Some(0), "adyen", "acc", ""));
    }
}
