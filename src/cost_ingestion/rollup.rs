//! Streaming aggregation of a settlement report into per-day sufficient statistics.
//!
//! Individual transactions are never stored. As a report streams in (batch by batch, off the
//! connector parser), each fee-bearing transaction is folded into one bucket keyed by
//! `(cluster × transaction-day × predictor-band × fit-bucket × channel)`. A bucket accumulates the additive sums an
//! OLS fit needs — `n, Σx, Σy, Σx², Σxy, Σy²` and the reciprocal terms for the bps-RMSE /
//! NON_LINEAR check — so summing buckets over any window reconstructs the exact same line the raw
//! rows would give (see `scratch/settlement-table-removal-worked-example.md`).
//!
//! Peak memory is O(distinct buckets) for one `(connector, account, merchant)` report — clusters ×
//! days × bands × channels, a few MB even for a multi-GB monthly file — not O(transactions).

use std::collections::HashMap;

use chrono::NaiveDate;
use rand::{rngs::StdRng, Rng, SeedableRng};

use super::types::{amount_band, fit_bucket, SettledFeeRow};

/// Per-bucket bounded sample used by the fitter's fan detector. The sufficient statistics remain
/// authoritative for the OLS fit; samples only answer "does a minority of rows sit far off-line?".
const SAMPLE_CAP_PER_BUCKET: usize = 64;

/// Identity of one rollup bucket. `band`/`channel` are predictor features the fit sums away; the
/// rest is the fit's cluster key plus the transaction day.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct BucketKey {
    txn_date: NaiveDate,
    card_network: String,
    variant: String,
    funding: String,
    issuer_country: String,
    currency: String,
    ic_category: String,
    interchange_bps: String,
    channel: String,
    band: &'static str,
    fit_bucket: i32,
}

/// Additive sufficient statistics for the transactions in one bucket. Every field is a plain sum,
/// so merging two buckets (or summing across days at fit time) is field-wise addition.
#[derive(Debug, Clone, Copy, Default)]
struct Stats {
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

#[derive(Debug, Clone, Default)]
struct BucketStats {
    sums: Stats,
    samples: Vec<(f64, f64)>,
}

impl Stats {
    /// Fold one transaction (gross `x`, fee `y`) into the sums. Caller guarantees `x >= floor > 0`.
    fn add(&mut self, x: f64, y: f64) {
        let inv = 1.0 / x;
        let inv2 = inv * inv;
        self.n += 1;
        self.sx += x;
        self.sy += y;
        self.sxx += x * x;
        self.sxy += x * y;
        self.syy += y * y;
        self.su += inv;
        self.suu += inv2;
        self.suy += y * inv;
        self.suuy += y * inv2;
        self.syyuu += y * y * inv2;
    }
}

impl BucketStats {
    fn add(&mut self, x: f64, y: f64, rng: &mut StdRng) {
        let seen = self.sums.n;
        self.sums.add(x, y);
        if self.samples.len() < SAMPLE_CAP_PER_BUCKET {
            self.samples.push((x, y));
            return;
        }
        let j = rng.gen_range(0..=seen);
        if let Ok(idx) = usize::try_from(j) {
            if idx < SAMPLE_CAP_PER_BUCKET {
                self.samples[idx] = (x, y);
            }
        }
    }
}

/// One fully-aggregated bucket, ready to insert into `cost_daily_stats`.
pub struct DailyStatRow {
    pub txn_date: NaiveDate,
    pub card_network: String,
    pub variant: String,
    pub funding: String,
    pub issuer_country: String,
    pub currency: String,
    pub ic_category: String,
    pub interchange_bps: String,
    pub channel: String,
    pub band: &'static str,
    pub fit_bucket: i32,
    pub n: u64,
    pub sx: f64,
    pub sy: f64,
    pub sxx: f64,
    pub sxy: f64,
    pub syy: f64,
    pub su: f64,
    pub suu: f64,
    pub suy: f64,
    pub suuy: f64,
    pub syyuu: f64,
    pub sample_x: Vec<f64>,
    pub sample_y: Vec<f64>,
}

/// Accumulates a report's transactions into per-day sufficient statistics.
pub struct RollupAccumulator {
    buckets: HashMap<BucketKey, BucketStats>,
    rng: StdRng,
}

impl Default for RollupAccumulator {
    fn default() -> Self {
        Self::new()
    }
}

impl RollupAccumulator {
    pub fn new() -> Self {
        Self {
            buckets: HashMap::new(),
            rng: StdRng::seed_from_u64(7),
        }
    }

    /// Fold one transaction. Non-positive/invalid gross is skipped because reciprocal fit terms
    /// require `gross > 0`; fee-bearing micro transactions are retained and judged by fit quality
    /// rather than a currency-blind amount floor. `fallback_date` dates rows whose report carried
    /// no txn date.
    pub fn add_row(&mut self, row: &SettledFeeRow, fallback_date: NaiveDate) {
        if !row.gross.is_finite() || row.gross <= 0.0 || !row.total_fee.is_finite() {
            return;
        }
        let key = BucketKey {
            txn_date: row.txn_date.unwrap_or(fallback_date),
            card_network: row.card_network.clone(),
            variant: row.variant.clone(),
            funding: row.funding.clone(),
            issuer_country: row.issuer_country.clone(),
            currency: row.currency.clone(),
            ic_category: row.ic_category.clone(),
            interchange_bps: row.interchange_bps.clone(),
            channel: row.channel.clone(),
            band: amount_band(row.gross),
            fit_bucket: fit_bucket(row.gross),
        };
        self.buckets
            .entry(key)
            .or_default()
            .add(row.gross, row.total_fee, &mut self.rng);
    }

    /// Number of distinct buckets accumulated (for capacity hints / diagnostics).
    pub fn len(&self) -> usize {
        self.buckets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buckets.is_empty()
    }

    /// Drain into insertable rows. Consumes the accumulator.
    pub fn into_rows(self) -> Vec<DailyStatRow> {
        self.buckets
            .into_iter()
            .map(|(k, s)| DailyStatRow {
                txn_date: k.txn_date,
                card_network: k.card_network,
                variant: k.variant,
                funding: k.funding,
                issuer_country: k.issuer_country,
                currency: k.currency,
                ic_category: k.ic_category,
                interchange_bps: k.interchange_bps,
                channel: k.channel,
                band: k.band,
                fit_bucket: k.fit_bucket,
                n: s.sums.n,
                sx: s.sums.sx,
                sy: s.sums.sy,
                sxx: s.sums.sxx,
                sxy: s.sums.sxy,
                syy: s.sums.syy,
                su: s.sums.su,
                suu: s.sums.suu,
                suy: s.sums.suy,
                suuy: s.sums.suuy,
                syyuu: s.sums.syyuu,
                sample_x: s.samples.iter().map(|(x, _)| *x).collect(),
                sample_y: s.samples.iter().map(|(_, y)| *y).collect(),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(gross: f64, fee: f64, date: &str) -> SettledFeeRow {
        SettledFeeRow {
            txn_ref: String::new(),
            card_network: "visa".into(),
            variant: "visacredit".into(),
            funding: "credit".into(),
            issuer_country: "GB".into(),
            currency: "GBP".into(),
            ic_category: "".into(),
            interchange_bps: "".into(),
            txn_date: Some(NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap()),
            channel: "ecom".into(),
            gross,
            total_fee: fee,
            interchange: 0.0,
            scheme_fee: 0.0,
            markup: 0.0,
            commission: 0.0,
        }
    }

    #[test]
    fn keeps_positive_micro_amounts() {
        let mut acc = RollupAccumulator::new();
        let d = NaiveDate::parse_from_str("2026-06-28", "%Y-%m-%d").unwrap();
        acc.add_row(&row(4.99, 0.5, "2026-06-28"), d);
        assert!(
            !acc.is_empty(),
            "positive micro txn should still create a bucket"
        );
    }

    #[test]
    fn matches_hand_computed_sums() {
        // The worked example's Upload A (Jun 28): (100,2.70),(200,5.20),(300,7.50).
        let d = NaiveDate::parse_from_str("2026-06-28", "%Y-%m-%d").unwrap();
        let mut acc = RollupAccumulator::new();
        for (g, f) in [(100.0, 2.70), (200.0, 5.20), (300.0, 7.50)] {
            acc.add_row(&row(g, f, "2026-06-28"), d);
        }
        // All three land in the same cluster/day; different bands, so expect per-band buckets that
        // sum back to the doc's totals.
        let rows = acc.into_rows();
        let n: u64 = rows.iter().map(|r| r.n).sum();
        let sx: f64 = rows.iter().map(|r| r.sx).sum();
        let sy: f64 = rows.iter().map(|r| r.sy).sum();
        let sxy: f64 = rows.iter().map(|r| r.sxy).sum();
        let syy: f64 = rows.iter().map(|r| r.syy).sum();
        assert_eq!(n, 3);
        assert!((sx - 600.0).abs() < 1e-9);
        assert!((sy - 15.40).abs() < 1e-9);
        assert!((sxy - 3560.0).abs() < 1e-9);
        assert!((syy - 90.58).abs() < 1e-9);
    }
}
