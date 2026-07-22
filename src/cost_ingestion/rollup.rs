//! Streaming aggregation of a settlement report into per-day sufficient statistics.
//!
//! Individual transactions are never stored. As a report streams in (batch by batch, off the
//! connector parser), each fee-bearing transaction is folded into one bucket keyed by
//! `(cluster × transaction-day × amount-band × channel)`. A bucket accumulates the additive sums an
//! OLS fit needs — `n, Σx, Σy, Σx², Σxy, Σy²` and the reciprocal terms for the bps-RMSE /
//! NON_LINEAR check — so summing buckets over any window reconstructs the exact same line the raw
//! rows would give (see `scratch/settlement-table-removal-worked-example.md`).
//!
//! Peak memory is O(distinct buckets) for one `(connector, account, merchant)` report — clusters ×
//! days × bands × channels, a few MB even for a multi-GB monthly file — not O(transactions).

use std::collections::HashMap;

use chrono::NaiveDate;

use super::types::{amount_band, SettledFeeRow};

/// Guard only: rows with non-positive gross are skipped so the reciprocal terms (`1/gross`) can't
/// divide by zero. There is deliberately NO economic floor — a merchant's genuine small-ticket
/// business is never deleted. The fixed-fee-dominated region of each cluster is instead identified
/// per-cluster by the `a*` crossover at fit time (see `fit.rs`) and excluded from the
/// proportional-error grade, so sub-economic dust can't detonate `bps_rmse` — with no hardcoded,
/// currency-blind threshold anywhere.
const MIN_GROSS: f64 = 0.0;

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
    channel: String,
    band: String,
}

/// Identity of one global BIN → card-product observation. Deliberately carries no merchant /
/// connector / day — a card's product is universal, so this aggregate is global (architecture §7).
/// `funding` is the resolved product (debit/credit/commercial, already inferred from the interchange
/// rate at ingestion); it is the signal Step B reads to resolve co-badge cards.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct BinKey {
    bin: String,
    card_network: String,
    issuer_country: String,
    funding: String,
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

/// One fully-aggregated bucket, ready to insert into `cost_daily_stats`.
pub struct DailyStatRow {
    pub txn_date: NaiveDate,
    pub card_network: String,
    pub variant: String,
    pub funding: String,
    pub issuer_country: String,
    pub currency: String,
    pub ic_category: String,
    pub channel: String,
    pub band: String,
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
}

/// One aggregated global BIN → card-product observation, ready to insert into `cost_bin_product`.
pub struct BinProductRow {
    pub bin: String,
    pub card_network: String,
    pub issuer_country: String,
    pub funding: String,
    pub support_n: u64,
}

/// Accumulates a report's transactions into per-day sufficient statistics, and — in the same pass —
/// per-BIN card-product observations for the global `cost_bin_product` map.
#[derive(Default)]
pub struct RollupAccumulator {
    buckets: HashMap<BucketKey, Stats>,
    bins: HashMap<BinKey, u64>,
}

impl RollupAccumulator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Fold one transaction. Rows below the micro-amount floor (or with non-positive gross, which
    /// would make the reciprocal terms explode) are skipped — the same rows the fit/predictor
    /// filtered out at read time. `fallback_date` dates rows whose report carried no txn date.
    pub fn add_row(&mut self, row: &SettledFeeRow, fallback_date: NaiveDate) {
        // Fold the BIN observation first — a card's product doesn't depend on the amount, so we
        // capture it even for sub-floor rows (more BIN coverage). Rows without a PAN contribute none.
        if !row.bin.is_empty() {
            let bkey = BinKey {
                bin: row.bin.clone(),
                card_network: row.card_network.clone(),
                issuer_country: row.issuer_country.clone(),
                funding: row.funding.clone(),
            };
            *self.bins.entry(bkey).or_default() += 1;
        }
        if row.gross.is_nan() || row.gross <= MIN_GROSS {
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
            channel: row.channel.clone(),
            band: amount_band(row.gross),
        };
        self.buckets
            .entry(key)
            .or_default()
            .add(row.gross, row.total_fee);
    }

    /// Number of distinct buckets accumulated (for capacity hints / diagnostics).
    pub fn len(&self) -> usize {
        self.buckets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.buckets.is_empty()
    }

    /// The per-BIN card-product observations gathered this report, for the global `cost_bin_product`
    /// map. Borrows (call before [`into_rows`] consumes the accumulator).
    pub fn bin_rows(&self) -> Vec<BinProductRow> {
        self.bins
            .iter()
            .map(|(k, &n)| BinProductRow {
                bin: k.bin.clone(),
                card_network: k.card_network.clone(),
                issuer_country: k.issuer_country.clone(),
                funding: k.funding.clone(),
                support_n: n,
            })
            .collect()
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
                channel: k.channel,
                band: k.band,
                n: s.n,
                sx: s.sx,
                sy: s.sy,
                sxx: s.sxx,
                sxy: s.sxy,
                syy: s.syy,
                su: s.su,
                suu: s.suu,
                suy: s.suy,
                suuy: s.suuy,
                syyuu: s.syyuu,
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
            txn_date: Some(NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap()),
            channel: "ecom".into(),
            gross,
            total_fee: fee,
            interchange: 0.0,
            scheme_fee: 0.0,
            markup: 0.0,
            commission: 0.0,
            bin: String::new(),
        }
    }

    #[test]
    fn skips_only_non_positive_gross() {
        let d = NaiveDate::parse_from_str("2026-06-28", "%Y-%m-%d").unwrap();
        let mut acc = RollupAccumulator::new();
        acc.add_row(&row(0.0, 0.5, "2026-06-28"), d); // gross 0 → skipped (1/x guard)
        assert!(acc.is_empty(), "non-positive gross makes no bucket");
        acc.add_row(&row(4.99, 0.5, "2026-06-28"), d); // genuine small ticket → KEPT, no floor
        assert!(!acc.is_empty(), "genuine small-ticket business is not deleted");
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

    /// A co-badge card row carrying a BIN and its resolved `funding` (the product the interchange
    /// rate was inferred to at ingestion), as the rollup receives it.
    fn card_row(gross: f64, bin: &str, funding: &str) -> SettledFeeRow {
        SettledFeeRow {
            txn_ref: String::new(),
            card_network: "visa".into(),
            variant: "cartebancaire".into(),
            funding: funding.into(),
            issuer_country: "FR".into(),
            currency: "EUR".into(),
            ic_category: "".into(),
            txn_date: None,
            channel: "pos".into(),
            gross,
            total_fee: gross * 0.002,
            interchange: 0.0,
            scheme_fee: 0.0,
            markup: 0.0,
            commission: 0.0,
            bin: bin.into(),
        }
    }

    #[test]
    fn bin_observations_aggregate_support_per_product() {
        let d = NaiveDate::parse_from_str("2026-06-28", "%Y-%m-%d").unwrap();
        let mut acc = RollupAccumulator::new();
        // Same co-badge BIN seen 3× as consumer-debit, once as commercial (funding already resolved
        // from the interchange rate at ingestion). Support accumulates per resolved product.
        acc.add_row(&card_row(50.0, "497040", "debit"), d);
        acc.add_row(&card_row(80.0, "497040", "debit"), d);
        acc.add_row(&card_row(120.0, "497040", "debit"), d);
        acc.add_row(&card_row(200.0, "497040", "commercial"), d);
        let mut rows = acc.bin_rows();
        rows.sort_by(|a, b| b.support_n.cmp(&a.support_n));
        assert_eq!(rows.len(), 2, "one row per distinct (bin, funding) product");
        assert_eq!(rows[0].bin, "497040");
        assert_eq!((&*rows[0].funding, rows[0].support_n), ("debit", 3));
        assert_eq!((&*rows[1].funding, rows[1].support_n), ("commercial", 1));
    }

    #[test]
    fn bin_captured_even_when_gross_is_skipped() {
        let d = NaiveDate::parse_from_str("2026-06-28", "%Y-%m-%d").unwrap();
        let mut acc = RollupAccumulator::new();
        // gross 0 makes no fit bucket, but the card's product observation is still valid.
        acc.add_row(&card_row(0.0, "513770", "debit"), d);
        assert!(acc.is_empty(), "non-positive gross makes no fit bucket");
        let rows = acc.bin_rows();
        assert_eq!(rows.len(), 1, "but its BIN observation is still captured");
        assert_eq!((&*rows[0].bin, &*rows[0].funding, rows[0].support_n), ("513770", "debit", 1));
    }

    #[test]
    fn rows_without_a_pan_contribute_no_bin() {
        let d = NaiveDate::parse_from_str("2026-06-28", "%Y-%m-%d").unwrap();
        let mut acc = RollupAccumulator::new();
        acc.add_row(&card_row(50.0, "", "credit"), d); // trimmed/tokenized report: no PAN
        assert!(acc.bin_rows().is_empty(), "no PAN ⇒ no BIN observation");
        assert!(!acc.is_empty(), "but the fit bucket is still recorded");
    }

    #[test]
    fn resolved_and_unresolved_observations_stay_distinct() {
        let d = NaiveDate::parse_from_str("2026-06-28", "%Y-%m-%d").unwrap();
        let mut acc = RollupAccumulator::new();
        // Filtered report carried the rate ⇒ funding resolved; a later trimmed report of the same
        // BIN had no rate ⇒ funding blank. The two observations key distinctly on funding.
        acc.add_row(&card_row(60.0, "497040", "debit"), d);
        acc.add_row(&card_row(60.0, "497040", ""), d);
        let rows = acc.bin_rows();
        assert_eq!(rows.len(), 2, "resolved and unresolved observations are distinct");
        assert!(rows.iter().any(|r| r.funding == "debit"));
        assert!(rows.iter().any(|r| r.funding.is_empty()), "no rate ⇒ funding blank (unresolved)");
    }
}
