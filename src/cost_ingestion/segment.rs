//! Piecewise (L1) segmentation of a cost cluster — the Rust core of the cap/tier-aware fit.
//!
//! A single OLS line (`fee = pct_bps/10⁴·gross + fixed`) is exact for unregulated interchange
//! (US/EU), but wrong where interchange has an **absolute cap** (UAE `1.00% cap AED 50`) or a
//! **tiered schedule** — there `fee ~ gross` is *piecewise* linear, and one line reads
//! `NON_LINEAR`, forcing the router to drop the cluster to a coarse blend.
//!
//! This module recovers such a cluster into up to [`MAX_SEGMENTS`] straight pieces, each with its
//! own `{pct_bps, fixed}` over an amount range. It operates purely on the per-log-amount-band
//! sufficient statistics the fit already computes (`Σx, Σy, Σxx, Σxy, Σyy` …), so it needs no raw
//! transactions and runs as a cheap Rust post-step over the ClickHouse band rollup.
//!
//! It is a faithful port of `scratch/cluster_explorer.py::segment_partitions` (validated against
//! the browser prototype): a partition DP over band boundaries that keeps the split turning the
//! **most volume** into reliable `GOOD` pieces with the **fewest cuts** — so a genuinely linear
//! cluster is left uncut, and cuts land only at real cap/tier boundaries.

/// Log-amount buckets per decade — the granularity of candidate breakpoints (matches the fit's
/// `cost_daily_stats.band`).
const BUCKETS_PER_DECADE: f64 = 10.0;
/// Sample gate for a `GOOD` verdict on the strong path.
const MIN_N: f64 = 200.0;
/// Max per-transaction fit error (bps) a piece may carry and still be `GOOD`.
const MAX_BPS: f64 = 15.0;
/// Most pieces a cluster may be split into.
pub const MAX_SEGMENTS: usize = 5;
/// Fewest transactions a piece needs to be considered at all.
const SEG_FLOOR: f64 = 25.0;
/// L2 reliability gate: a piece with `SEG_FLOOR..MIN_N` txns is still `GOOD` if its slope 95% CI
/// is at most this many bps (the rate is well-pinned despite few samples).
const L2_MAX_PCT_BPS_CI: f64 = 15.0;

/// Additive OLS sufficient statistics for one bucket or a merged range. The `su*` terms carry the
/// `1/x`-weighted sums the proportional (bps) error metric needs.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct SuffStats {
    pub n: f64,
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

impl SuffStats {
    /// Fold one `(amount, fee)` transaction in.
    pub fn add(&mut self, x: f64, y: f64) {
        let u = 1.0 / x;
        self.n += 1.0;
        self.sx += x;
        self.sy += y;
        self.sxx += x * x;
        self.sxy += x * y;
        self.syy += y * y;
        self.su += u;
        self.suu += u * u;
        self.suy += u * y;
        self.suuy += u * u * y;
        self.syyuu += y * y * u * u;
    }

    fn add_assign(&mut self, o: &Self) {
        self.n += o.n;
        self.sx += o.sx;
        self.sy += o.sy;
        self.sxx += o.sxx;
        self.sxy += o.sxy;
        self.syy += o.syy;
        self.su += o.su;
        self.suu += o.suu;
        self.suy += o.suy;
        self.suuy += o.suuy;
        self.syyuu += o.syyuu;
    }

    fn sub(&self, o: &Self) -> Self {
        Self {
            n: self.n - o.n,
            sx: self.sx - o.sx,
            sy: self.sy - o.sy,
            sxx: self.sxx - o.sxx,
            sxy: self.sxy - o.sxy,
            syy: self.syy - o.syy,
            su: self.su - o.su,
            suu: self.suu - o.suu,
            suy: self.suy - o.suy,
            suuy: self.suuy - o.suuy,
            syyuu: self.syyuu - o.syyuu,
        }
    }
}

/// How trustworthy a fitted piece is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// One line hugs the piece tightly enough to price on.
    Good,
    /// Too few transactions to trust the rate.
    Thin,
    /// A line can't hug the piece (a kink/fan remains inside it).
    NonLinear,
}

impl Verdict {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Good => "GOOD",
            Self::Thin => "THIN",
            Self::NonLinear => "NON_LINEAR",
        }
    }
}

/// One recovered piece of a cluster: a `{pct_bps, fixed}` line over `[lo, hi)` amount.
#[derive(Clone, Debug)]
pub struct Segment {
    pub lo: f64,
    pub hi: f64,
    pub n: u64,
    pub vol: f64,
    /// `None` only for an unfittable piece (no amount spread).
    pub pct_bps: Option<f64>,
    pub fixed: Option<f64>,
    pub bps_rmse: Option<f64>,
    pub verdict: Verdict,
}

/// Amount bounds `[lo, hi)` of a log-amount band index.
fn amt_range(bi: i64) -> (f64, f64) {
    (
        10f64.powf(bi as f64 / BUCKETS_PER_DECADE),
        10f64.powf((bi as f64 + 1.0) / BUCKETS_PER_DECADE),
    )
}

/// Which log-amount band an amount falls in.
pub fn band_of(amount: f64) -> i64 {
    (amount.log10() * BUCKETS_PER_DECADE).floor() as i64
}

fn sum_bands(bands: &[(i64, SuffStats)]) -> SuffStats {
    let mut acc = SuffStats::default();
    for (_, st) in bands {
        acc.add_assign(st);
    }
    acc
}

/// Per-transaction proportional (bps) RMS error of the line `(intercept a, slope b)`, from the
/// `1/x`-weighted sufficient statistics.
fn eval_bps(a: f64, b: f64, s: &SuffStats) -> f64 {
    let n = s.n;
    if n <= 0.0 {
        return f64::NAN;
    }
    let q = s.syyuu + a * a * s.suu + n * b * b - 2.0 * a * s.suuy - 2.0 * b * s.suy
        + 2.0 * a * b * s.su;
    (q.max(0.0) / n).sqrt() * 1e4
}

/// OLS fit → `(slope, intercept, bps_rmse, slope_se_bps)`. NaN components when unfittable.
fn fit(s: &SuffStats) -> (f64, f64, f64, f64) {
    let n = s.n;
    if n < 2.0 {
        return (f64::NAN, f64::NAN, f64::NAN, f64::NAN);
    }
    let denom = n * s.sxx - s.sx * s.sx;
    if denom <= 0.0 {
        return (f64::NAN, f64::NAN, f64::NAN, f64::NAN);
    }
    let slope = (n * s.sxy - s.sx * s.sy) / denom;
    let intercept = (s.sy - slope * s.sx) / n;
    let bps_rmse = eval_bps(intercept, slope, s);
    let se = if n > 2.0 {
        let sse = s.syy - intercept * s.sy - slope * s.sxy;
        (sse.max(0.0) / (n - 2.0) * n / denom).sqrt() * 1e4
    } else {
        f64::INFINITY
    };
    (slope, intercept, bps_rmse, se)
}

/// `a* = fixed / rate` — the amount where the flat fee equals the proportional fee. `0` when there
/// is no positive fixed fee (the fixed/proportional split is then a no-op).
fn crossover(slope: f64, intercept: f64) -> f64 {
    if slope.is_nan() || slope <= 0.0 || intercept <= 0.0 {
        0.0
    } else {
        intercept / slope
    }
}

/// RMS of the absolute (currency) fee residual for `(a, b)` from the sufficient statistics.
fn abs_rms(s: &SuffStats, a: f64, b: f64) -> f64 {
    let n = s.n;
    if n <= 0.0 {
        return f64::NAN;
    }
    let sse =
        s.syy - 2.0 * a * s.sy - 2.0 * b * s.sxy + n * a * a + 2.0 * a * b * s.sx + b * b * s.sxx;
    (sse.max(0.0) / n).sqrt()
}

/// Split a piece's bands into (below-a*, at/above-a*): a band sitting entirely below `a*` is
/// fixed-fee-dominated; any straddling/higher band is proportional.
fn split_regime(bands: &[(i64, SuffStats)], astar: f64) -> (SuffStats, SuffStats) {
    let (mut below, mut above) = (SuffStats::default(), SuffStats::default());
    for (bi, st) in bands {
        let (_, hi) = amt_range(*bi);
        if hi <= astar {
            below.add_assign(st);
        } else {
            above.add_assign(st);
        }
    }
    (below, above)
}

/// The proportional-regime bps error (above a*) — the accuracy the rate is judged on, with the
/// fixed-fee-dominated micro tail excluded.
fn prop_bps(whole: &SuffStats, bands: &[(i64, SuffStats)], slope: f64, intercept: f64) -> f64 {
    let astar = crossover(slope, intercept);
    let (below, above) = split_regime(bands, astar);
    if above.n > 0.0 {
        eval_bps(intercept, slope, &above)
    } else if below.n > 0.0 && astar > 0.0 {
        // Entirely fixed-fee regime: grade on the fixed tail's absolute error, expressed in bps.
        abs_rms(&below, intercept, slope) / astar * 1e4
    } else {
        let _ = whole;
        f64::NAN
    }
}

/// Grade one candidate piece with the reliability gate: accurate on its proportional regime AND
/// either enough samples or a tightly-pinned slope.
fn grade_seg(s: &SuffStats, bands: &[(i64, SuffStats)]) -> Verdict {
    let n = s.n;
    if n < SEG_FLOOR {
        return Verdict::Thin;
    }
    let (slope, intercept, _, se) = fit(s);
    if slope.is_nan() {
        return Verdict::NonLinear;
    }
    let b = prop_bps(s, bands, slope, intercept);
    if b.is_nan() || b > MAX_BPS {
        return Verdict::NonLinear;
    }
    if n >= MIN_N {
        return Verdict::Good;
    }
    if 1.96 * se <= L2_MAX_PCT_BPS_CI {
        Verdict::Good
    } else {
        Verdict::Thin
    }
}

/// Find the breakpoints that turn the most volume into `GOOD` pieces using the fewest cuts.
///
/// A partition DP: `dp[k][j]` is the minimum residual to split the first `j` bands into `k`
/// pieces (each ≥ `SEG_FLOOR` txns). For every reachable segment count it rebuilds the partition,
/// then picks the `k` whose pieces make the most `GOOD` volume, breaking ties toward fewer cuts.
/// Returns the chosen `[start, end)` band-index ranges. Never empty for non-empty input.
fn segment_partitions(bands: &[(i64, SuffStats)]) -> Vec<(usize, usize)> {
    let m = bands.len();
    if m == 0 {
        return vec![];
    }
    // Prefix sums so any range fit is O(1).
    let mut pref = vec![SuffStats::default(); m + 1];
    for i in 0..m {
        let mut acc = pref[i];
        acc.add_assign(&bands[i].1);
        pref[i + 1] = acc;
    }
    let range = |i: usize, j: usize| -> SuffStats { pref[j].sub(&pref[i]) };
    let cost = |i: usize, j: usize| -> f64 {
        let s = range(i, j);
        if s.n < SEG_FLOOR {
            return f64::INFINITY;
        }
        let b = fit(&s).2;
        if b.is_nan() {
            f64::INFINITY
        } else {
            (b / 1e4).powi(2) * s.n
        }
    };

    let inf = f64::INFINITY;
    let mut dp = vec![vec![inf; m + 1]; MAX_SEGMENTS + 1];
    let mut back = vec![vec![usize::MAX; m + 1]; MAX_SEGMENTS + 1];
    dp[0][0] = 0.0;
    for k in 1..=MAX_SEGMENTS {
        for j in 1..=m {
            for i in 0..j {
                if dp[k - 1][i] == inf {
                    continue;
                }
                let c = cost(i, j);
                if c != inf && dp[k - 1][i] + c < dp[k][j] {
                    dp[k][j] = dp[k - 1][i] + c;
                    back[k][j] = i;
                }
            }
        }
    }

    let rebuild = |k: usize| -> Option<Vec<(usize, usize)>> {
        let mut parts = vec![];
        let (mut j, mut k) = (m, k);
        while k > 0 {
            let i = back[k][j];
            if i == usize::MAX {
                return None;
            }
            parts.push((i, j));
            j = i;
            k -= 1;
        }
        parts.reverse();
        Some(parts)
    };

    let mut best: Option<Vec<(usize, usize)>> = None;
    let mut best_key: Option<(f64, i64)> = None;
    for (k, dp_k) in dp.iter().enumerate().take(MAX_SEGMENTS + 1).skip(1) {
        if dp_k[m] == inf {
            continue;
        }
        let Some(parts) = rebuild(k) else { continue };
        let mut good_vol = 0.0;
        for &(p, q) in &parts {
            if grade_seg(&range(p, q), &bands[p..q]) == Verdict::Good {
                good_vol += pref[q].sx - pref[p].sx;
            }
        }
        // More GOOD volume wins; ties break toward fewer segments (larger -k).
        let key = ((good_vol * 100.0).round() / 100.0, -(k as i64));
        if best_key.is_none_or(|bk| key.0 > bk.0 || (key.0 == bk.0 && key.1 > bk.1)) {
            best = Some(parts);
            best_key = Some(key);
        }
    }
    best.unwrap_or_else(|| vec![(0, m)])
}

/// A recovered cluster is only worth surfacing as "tiers" when its GOOD segments cover essentially
/// all the volume. Below this, the split is a **fan** (overlapping interchange rates under one key)
/// or otherwise unrecoverable — mostly NON_LINEAR pieces that would mislead if shown as clean tiers.
/// Mirrors the prototype's L1 `l1_vol` recovery gate (a fan there yields 0 GOOD volume).
pub const MIN_GOOD_VOLUME_FRACTION: f64 = 0.9;

/// Whether a segmentation is a genuine multi-tier recovery worth surfacing: at least two pieces, and
/// the `GOOD` pieces cover [`MIN_GOOD_VOLUME_FRACTION`] of the volume. A fan / mostly-NON_LINEAR
/// split returns `false` (the cluster should fall back to the coarse blend, not show fake tiers).
pub fn is_clean_recovery(segs: &[Segment]) -> bool {
    if segs.len() < 2 {
        return false;
    }
    let total: f64 = segs.iter().map(|s| s.vol).sum();
    if total <= 0.0 {
        return false;
    }
    let good: f64 = segs
        .iter()
        .filter(|s| s.verdict == Verdict::Good)
        .map(|s| s.vol)
        .sum();
    good / total >= MIN_GOOD_VOLUME_FRACTION
}

/// Segment a cluster from its per-band sufficient statistics (sorted ascending by band index).
///
/// Always returns at least one segment; a genuinely linear cluster comes back as a single
/// uncut piece. Each returned [`Segment`] carries its own `{pct_bps, fixed}`, amount range,
/// fit error, and reliability [`Verdict`].
pub fn segment_cluster(bands: &[(i64, SuffStats)]) -> Vec<Segment> {
    segment_partitions(bands)
        .into_iter()
        .map(|(p, q)| {
            let seg = sum_bands(&bands[p..q]);
            let (slope, intercept, bps_rmse, _) = fit(&seg);
            let lo = amt_range(bands[p].0).0;
            let hi = amt_range(bands[q - 1].0).1;
            Segment {
                lo,
                hi,
                n: seg.n as u64,
                vol: seg.sx,
                pct_bps: if slope.is_nan() {
                    None
                } else {
                    Some(slope * 1e4)
                },
                fixed: if intercept.is_nan() {
                    None
                } else {
                    Some(intercept)
                },
                bps_rmse: if bps_rmse.is_nan() {
                    None
                } else {
                    Some(bps_rmse)
                },
                verdict: grade_seg(&seg, &bands[p..q]),
            }
        })
        .collect()
}

/// Whole-cluster verdict on a single OLS line — the gate that decides whether [`segment_cluster`]
/// is worth running (only clusters that are not already `GOOD` benefit from segmentation).
pub fn whole_cluster_verdict(bands: &[(i64, SuffStats)]) -> Verdict {
    let whole = sum_bands(bands);
    if whole.n < MIN_N {
        return Verdict::Thin;
    }
    let (slope, intercept, _, _) = fit(&whole);
    if slope.is_nan() {
        return Verdict::NonLinear;
    }
    let b = prop_bps(&whole, bands, slope, intercept);
    if b.is_nan() || b > MAX_BPS {
        Verdict::NonLinear
    } else {
        Verdict::Good
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// Tiny deterministic LCG so the synthetic clouds are reproducible without a dep.
    struct Lcg(u64);
    impl Lcg {
        fn next_f64(&mut self) -> f64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((self.0 >> 11) as f64) / ((1u64 << 53) as f64)
        }
        fn log_uniform(&mut self, lo: f64, hi: f64) -> f64 {
            let l = lo.log10() + (hi.log10() - lo.log10()) * self.next_f64();
            10f64.powf(l)
        }
        fn jitter(&mut self, x: f64) -> f64 {
            x * (1.0 + (self.next_f64() - 0.5) * 0.04)
        }
    }

    fn bands_from_points(pts: &[(f64, f64)]) -> Vec<(i64, SuffStats)> {
        let mut m: BTreeMap<i64, SuffStats> = BTreeMap::new();
        for &(x, y) in pts {
            m.entry(band_of(x)).or_default().add(x, y);
        }
        m.into_iter().collect()
    }

    /// All-in fee (adds 0.10% scheme + 0.60% markup + 0.42 commission) around an interchange amount.
    fn all_in(rng: &mut Lcg, gross: f64, interchange: f64) -> f64 {
        rng.jitter(interchange) + rng.jitter(0.0010 * gross) + rng.jitter(0.0060 * gross) + 0.42
    }

    #[test]
    fn capped_debit_splits_into_two_segments_at_the_knot() {
        // UAE debit: 1.00% interchange capped at AED 50 -> knot at AED 5,000.
        let mut rng = Lcg(7);
        let pts: Vec<(f64, f64)> = (0..700)
            .map(|_| {
                let g = rng.log_uniform(20.0, 20000.0);
                let ic = (0.0100 * g).min(50.0);
                (g, all_in(&mut rng, g, ic))
            })
            .collect();
        let bands = bands_from_points(&pts);

        assert_eq!(whole_cluster_verdict(&bands), Verdict::NonLinear);
        let segs = segment_cluster(&bands);
        assert_eq!(segs.len(), 2, "one cap knot -> two segments");
        assert!(segs.iter().all(|s| s.verdict == Verdict::Good));
        // Below the cap: full rate ~1.70% (100 ic + 10 scheme + 60 markup).
        assert!(
            (segs[0].pct_bps.unwrap() - 170.0).abs() < 8.0,
            "lo seg ~170 bps"
        );
        // Above the cap: interchange is flat, so only ~0.70% remains in the rate.
        assert!(
            (segs[1].pct_bps.unwrap() - 70.0).abs() < 8.0,
            "hi seg ~70 bps"
        );
        // Knot near AED 5,000 (bucket boundary 10^3.7 ≈ 5012).
        assert!((segs[0].hi - 5012.0).abs() < 200.0, "knot ~AED 5,000");
        assert!(
            segs[1].fixed.unwrap() > 40.0,
            "capped interchange moves into the flat term"
        );
    }

    #[test]
    fn three_tier_schedule_recovers_all_three_segments() {
        // Marginal tiers: 2.40% <2k, 1.20% 2k-10k, 0.20% >10k -> two knots, three well-separated
        // slopes (gaps wide enough that no coarser merge can pass the GOOD gate).
        let marginal_ic = |g: f64| -> f64 {
            let tiers = [(2000.0, 0.0240), (10000.0, 0.0120), (f64::INFINITY, 0.0020)];
            let (mut ic, mut lo) = (0.0, 0.0);
            for (ub, rate) in tiers {
                if g <= lo {
                    break;
                }
                ic += (g.min(ub) - lo) * rate;
                lo = ub;
                if g <= ub {
                    break;
                }
            }
            ic
        };
        let mut rng = Lcg(11);
        let pts: Vec<(f64, f64)> = (0..2000)
            .map(|_| {
                let g = rng.log_uniform(20.0, 40000.0);
                (g, all_in(&mut rng, g, marginal_ic(g)))
            })
            .collect();
        let bands = bands_from_points(&pts);

        assert_eq!(whole_cluster_verdict(&bands), Verdict::NonLinear);
        let segs = segment_cluster(&bands);
        assert_eq!(segs.len(), 3, "two tier knots -> three segments");
        assert!(segs.iter().all(|s| s.verdict == Verdict::Good));
        // Per-segment slope = the marginal rate + 0.70% scheme/markup: 3.10% / 1.90% / 0.90%.
        let rate = |i: usize| segs[i].pct_bps.unwrap();
        assert!((rate(0) - 310.0).abs() < 12.0, "tier1 ~3.10%");
        assert!((rate(1) - 190.0).abs() < 12.0, "tier2 ~1.90%");
        assert!((rate(2) - 90.0).abs() < 12.0, "tier3 ~0.90%");
        // Monotonic knots roughly at the two thresholds.
        assert!((segs[0].hi - 1995.0).abs() < 300.0);
        assert!((segs[1].hi - 10000.0).abs() < 600.0);
    }

    #[test]
    fn linear_cluster_is_left_uncut() {
        // A plain single-rate cluster must come back as one GOOD piece — no over-segmentation.
        let mut rng = Lcg(3);
        let pts: Vec<(f64, f64)> = (0..800)
            .map(|_| {
                let g = rng.log_uniform(20.0, 40000.0);
                (g, all_in(&mut rng, g, 0.0120 * g))
            })
            .collect();
        let bands = bands_from_points(&pts);

        assert_eq!(whole_cluster_verdict(&bands), Verdict::Good);
        let segs = segment_cluster(&bands);
        assert_eq!(segs.len(), 1, "linear cluster stays a single segment");
        assert_eq!(segs[0].verdict, Verdict::Good);
    }

    #[test]
    fn empty_input_is_safe() {
        assert!(segment_cluster(&[]).is_empty());
    }

    #[test]
    fn is_clean_recovery_gates_fans() {
        let seg = |vol: f64, v: Verdict| Segment {
            lo: 0.0,
            hi: 0.0,
            n: 0,
            vol,
            pct_bps: None,
            fixed: None,
            bps_rmse: None,
            verdict: v,
        };
        // A real cap/tier recovery: all volume in GOOD pieces.
        assert!(is_clean_recovery(&[
            seg(100.0, Verdict::Good),
            seg(100.0, Verdict::Good)
        ]));
        // A GOOD tier plus a small THIN/NON_LINEAR micro tail still counts (GOOD covers ~99%).
        assert!(is_clean_recovery(&[
            seg(2.0, Verdict::NonLinear),
            seg(200.0, Verdict::Good)
        ]));
        // The real-world fan from the screenshot: 3 NON_LINEAR pieces + one small GOOD (~6% of vol).
        assert!(!is_clean_recovery(&[
            seg(479.0, Verdict::NonLinear),
            seg(28_600.0, Verdict::NonLinear),
            seg(45_300.0, Verdict::Good),
            seg(725_100.0, Verdict::NonLinear),
        ]));
        // A single piece is never a recovery.
        assert!(!is_clean_recovery(&[seg(100.0, Verdict::Good)]));
    }
}
