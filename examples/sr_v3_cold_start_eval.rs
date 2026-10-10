//! Offline evaluation of the SR v3 cold-start prior (`decider::gatewaydecider::sr_prior`).
//!
//! ```text
//! cargo run --release --example sr_v3_cold_start_eval
//! ```
//!
//! A synthetic merchant routes payments over a few gateways across many segments (SR keys) whose
//! volumes follow a Zipf law, so most segments are sparse and a few are dense. Each segment's true
//! success rate per gateway is drawn from `Beta(κ·p_g, κ·(1−p_g))` around the gateway's
//! merchant-wide rate `p_g`: small κ means segments differ a lot, large κ means they are alike.
//!
//! The simulated decider reproduces SR v3 routing as it runs in production:
//!
//! * windows hold the last `B = 125` entries in Redis format (`LPUSH` + `LTRIM`), seeded with `B`
//!   synthetic successes on the first feedback; an absent window reads 1.0;
//! * 5% hedging as in `route_random_traffic` (head → 0.5, everyone else → 1.0), ties broken at
//!   random (`GatewayScoreMap` is a `HashMap`);
//! * the "autopilot" refreshes the prior snapshot every `R` payments from the last `L` outcomes,
//!   so the decider only ever sees past data (no leakage).
//!
//! Every score comes from the real implementation: `legacy_window_score` (what
//! `get_score_from_redis` computes) for existing routing, and `WindowStats::from_entries`,
//! `resolve_prior`, `SrPrior::posterior_mean`, `build_prior_entries` / `fit_prior_strength` for the
//! cold-start policies.
//!
//! Policies: `existing` (both flags off), `fixed` (cold-start prior with the configured strength,
//! tuned on separate seeds), `eb` (cold-start prior with the empirical-Bayes strength, falling
//! back to the configured one). Results are synthetic: they say how the estimator behaves under
//! the stated assumptions, not what it will do in production.

#![allow(
    clippy::as_conversions,
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::panic,
    clippy::print_stdout
)]

use std::collections::{HashMap, VecDeque};
use std::time::Instant;

use open_router::decider::gatewaydecider::sr_prior::{
    self, build_prior_entries, fit_prior_strength, legacy_window_score, resolve_prior,
    ColdStartFlags, GatewaySegmentOutcome, PriorBuildConfig, PriorFitConfig, SegmentOutcome,
    SrPriorSnapshot, WindowStats,
};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use rand_distr::{Beta, Distribution};

const BUCKET: usize = 125;
const HEDGE: f64 = 0.05;
const REFRESH_EVERY: usize = 2_500;
const LOOKBACK: usize = 10_000;
const MERCHANT: &str = "sim_merchant";
const PMT: &str = "CARD";
const PM: &str = "VISA";
const TUNING_SEEDS: std::ops::Range<u64> = 0..10;
const TEST_SEEDS: std::ops::Range<u64> = 100..130;
const STRENGTH_GRID: [f64; 7] = [2.0, 5.0, 10.0, 20.0, 50.0, 100.0, 200.0];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Policy {
    Existing,
    Fixed,
    Eb,
}

impl Policy {
    const ALL: [Self; 3] = [Self::Existing, Self::Fixed, Self::Eb];

    fn flags(self) -> ColdStartFlags {
        ColdStartFlags {
            prior_enabled: self != Self::Existing,
            learned_strength_enabled: self == Self::Eb,
        }
    }
}

#[derive(Debug, Clone)]
struct Scenario {
    name: &'static str,
    kappa: f64,
    segments: usize,
    payments: usize,
    base: Vec<f64>,
    /// `(at, base rate)`: an extra gateway becomes eligible at payment `at`.
    onboard: Option<(usize, f64)>,
    /// `(at, gateway, delta, fraction of segments)`.
    drift: Option<(usize, usize, f64, f64)>,
    /// Gateways the snapshot never has an entry for (e.g. too little traffic).
    missing_prior_for: Option<usize>,
    /// The prior job stops at this payment; snapshots expire `max_age` later.
    job_stops_at: Option<usize>,
    /// The prior job runs again from this payment.
    job_resumes_at: Option<usize>,
    /// Snapshot validity, in payments.
    max_age: usize,
}

impl Scenario {
    fn base(name: &'static str, kappa: f64) -> Self {
        Self {
            name,
            kappa,
            segments: 400,
            payments: 40_000,
            base: vec![0.90, 0.82, 0.70],
            onboard: None,
            drift: None,
            missing_prior_for: None,
            job_stops_at: None,
            job_resumes_at: None,
            max_age: 3 * REFRESH_EVERY,
        }
    }

    /// Payment from which "post-change" metrics are measured.
    fn change_at(&self) -> Option<usize> {
        self.drift
            .map(|d| d.0)
            .or(self.onboard.map(|o| o.0))
            .or(self.job_stops_at.map(|s| s + self.max_age))
    }
}

fn stationary() -> Vec<Scenario> {
    vec![
        Scenario::base("κ=3", 3.0),
        Scenario::base("κ=10", 10.0),
        Scenario::base("κ=40", 40.0),
        Scenario::base("κ=400", 400.0),
    ]
}

fn all_scenarios() -> Vec<Scenario> {
    let mut v = stationary();
    v.push(Scenario {
        onboard: Some((13_000, 0.93)),
        ..Scenario::base("onboard", 40.0)
    });
    v.push(Scenario {
        drift: Some((20_000, 0, -0.25, 1.0)),
        ..Scenario::base("drop all", 40.0)
    });
    v.push(Scenario {
        drift: Some((20_000, 0, -0.35, 0.25)),
        ..Scenario::base("drop 25%", 40.0)
    });
    v.push(Scenario {
        missing_prior_for: Some(1),
        ..Scenario::base("prior missing", 40.0)
    });
    v.push(Scenario {
        job_stops_at: Some(20_000),
        ..Scenario::base("job stops", 40.0)
    });
    v.push(Scenario {
        job_stops_at: Some(15_000),
        job_resumes_at: Some(25_000),
        ..Scenario::base("job resumes", 40.0)
    });
    v.push(Scenario {
        segments: 8,
        ..Scenario::base("8 segments", 40.0)
    });
    v
}

#[derive(Debug, Clone, Default)]
struct RunResult {
    successes: u64,
    payments: u64,
    regret: f64,
    /// (sum regret, payments) per volume bucket: sparse < 50, medium 50–499, dense ≥ 500 payments.
    by_volume: [(f64, u64); 3],
    post_early: (f64, u64),
    post_late: (f64, u64),
    learned_strengths: Vec<f64>,
    /// For job interruptions: regret over [stop, expiry), [expiry, resume), [resume, end).
    job_phases: [(f64, u64); 3],
}

impl RunResult {
    fn auth_rate(&self) -> f64 {
        self.successes as f64 / self.payments as f64
    }
    fn regret(&self) -> f64 {
        self.regret / self.payments as f64
    }
}

fn ratio(x: (f64, u64)) -> Option<f64> {
    (x.1 > 0).then(|| x.0 / x.1 as f64)
}

struct World {
    truth: Vec<Vec<f64>>,
    segment_of: Vec<usize>,
    outcome_u: Vec<f64>,
    drift_segments: Vec<bool>,
}

fn build_world(s: &Scenario, seed: u64) -> World {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut base = s.base.clone();
    if let Some((_, p)) = s.onboard {
        base.push(p);
    }
    let truth: Vec<Vec<f64>> = (0..s.segments)
        .map(|_| {
            base.iter()
                .map(|&p| {
                    Beta::new(p * s.kappa, (1.0 - p) * s.kappa)
                        .expect("valid beta")
                        .sample(&mut rng)
                })
                .collect()
        })
        .collect();
    let weights: Vec<f64> = (0..s.segments)
        .map(|i| 1.0 / ((i + 1) as f64).powf(1.1))
        .collect();
    let total: f64 = weights.iter().sum();
    let cdf: Vec<f64> = weights
        .iter()
        .scan(0.0, |acc, w| {
            *acc += w / total;
            Some(*acc)
        })
        .collect();
    let segment_of = (0..s.payments)
        .map(|_| {
            let u: f64 = rng.gen();
            cdf.iter().position(|&c| u <= c).unwrap_or(s.segments - 1)
        })
        .collect();
    let outcome_u = (0..s.payments).map(|_| rng.gen()).collect();
    let drift_segments = (0..s.segments)
        .map(|_| rng.gen::<f64>() < s.drift.map_or(0.0, |d| d.3))
        .collect();
    World {
        truth,
        segment_of,
        outcome_u,
        drift_segments,
    }
}

fn gateway_name(g: usize) -> String {
    format!("gw{g}")
}

fn simulate(s: &Scenario, seed: u64, policy: Policy, configured_strength: f64) -> RunResult {
    let mut world = build_world(s, seed);
    let mut rng = StdRng::seed_from_u64(seed.wrapping_mul(7_919).wrapping_add(policy as u64));
    let gateways = s.base.len() + usize::from(s.onboard.is_some());
    let names: Vec<String> = (0..gateways).map(gateway_name).collect();
    let mut windows: HashMap<(usize, usize), Vec<String>> = HashMap::new();
    let mut lookback: VecDeque<(usize, usize, bool)> = VecDeque::with_capacity(LOOKBACK);
    let mut snapshot: Option<SrPriorSnapshot> = None;
    let volume: Vec<u64> = {
        let mut v = vec![0u64; s.segments];
        for &seg in &world.segment_of {
            v[seg] += 1;
        }
        v
    };
    let build_cfg = PriorBuildConfig {
        learn_strength: policy == Policy::Eb,
        ..PriorBuildConfig::default()
    };
    let mut out = RunResult::default();
    let change_at = s.change_at();

    for t in 0..s.payments {
        if let Some((at, g, delta, _)) = s.drift {
            if t == at {
                for (seg, rates) in world.truth.iter_mut().enumerate() {
                    if world.drift_segments[seg] {
                        rates[g] = (rates[g] + delta).max(0.01);
                    }
                }
            }
        }
        let job_running = match (s.job_stops_at, s.job_resumes_at) {
            (Some(stop), Some(resume)) => t < stop || t >= resume,
            (Some(stop), None) => t < stop,
            _ => true,
        };
        if policy != Policy::Existing && t > 0 && t % REFRESH_EVERY == 0 && job_running {
            snapshot = Some(refresh_snapshot(&lookback, &names, s, t, &build_cfg));
            if policy == Policy::Eb {
                if let Some(snap) = &snapshot {
                    out.learned_strengths
                        .extend(snap.entries.iter().filter_map(|e| e.learned_prior_strength));
                }
            }
        }

        let seg = world.segment_of[t];
        let eligible = match s.onboard {
            Some((at, _)) if t < at => gateways - 1,
            _ => gateways,
        };
        let mut scores: Vec<f64> = (0..eligible)
            .map(|g| {
                let window = windows.get(&(seg, g));
                let legacy = || window.and_then(|w| legacy_window_score(w)).unwrap_or(1.0);
                let prior = resolve_prior(
                    policy.flags(),
                    snapshot.as_ref(),
                    MERCHANT,
                    t as i64,
                    PMT,
                    PM,
                    &names[g],
                    Some(configured_strength),
                );
                match prior {
                    None => legacy(),
                    Some(prior) => {
                        let stats = window
                            .map(|w| WindowStats::from_entries(w))
                            .unwrap_or_default();
                        prior.posterior_mean(&stats)
                    }
                }
            })
            .collect();
        if eligible > 1 && rng.gen::<f64>() < HEDGE {
            let head = argmax(&scores, &mut rng);
            for (g, score) in scores.iter_mut().enumerate() {
                *score = if g == head { 0.5 } else { 1.0 };
            }
        }
        let chosen = argmax(&scores, &mut rng);
        let rates = &world.truth[seg];
        let ok = world.outcome_u[t] < rates[chosen];
        let best = rates[..eligible].iter().cloned().fold(f64::MIN, f64::max);
        let regret = best - rates[chosen];

        out.payments += 1;
        out.successes += u64::from(ok);
        out.regret += regret;
        let bucket = match volume[seg] {
            v if v < 50 => 0,
            v if v < 500 => 1,
            _ => 2,
        };
        out.by_volume[bucket].0 += regret;
        out.by_volume[bucket].1 += 1;
        if let Some(stop) = s.job_stops_at {
            let expiry = stop + s.max_age;
            let resume = s.job_resumes_at.unwrap_or(usize::MAX);
            let phase = if t < stop {
                None
            } else if t < expiry.min(resume) {
                Some(0)
            } else if t < resume {
                Some(1)
            } else {
                Some(2)
            };
            if let Some(i) = phase {
                out.job_phases[i].0 += regret;
                out.job_phases[i].1 += 1;
            }
        }
        if let Some(at) = change_at {
            if t >= at && t < at + 2_000 {
                out.post_early.0 += regret;
                out.post_early.1 += 1;
            } else if t >= at + 2_000 {
                out.post_late.0 += regret;
                out.post_late.1 += 1;
            }
        }

        // Feedback, as `updateScoreAndQueue`: seed on first use, LPUSH the outcome, LTRIM.
        let window = windows.entry((seg, chosen)).or_insert_with(|| {
            std::iter::repeat_n(sr_prior::SYNTHETIC_SUCCESS.to_string(), BUCKET).collect()
        });
        window.insert(0, if ok { "1" } else { "0" }.to_string());
        window.truncate(BUCKET);
        if lookback.len() == LOOKBACK {
            lookback.pop_front();
        }
        lookback.push_back((seg, chosen, ok));
    }
    out
}

fn argmax(scores: &[f64], rng: &mut StdRng) -> usize {
    let max = scores.iter().cloned().fold(f64::MIN, f64::max);
    let ties: Vec<usize> = (0..scores.len()).filter(|&g| scores[g] == max).collect();
    ties[rng.gen_range(0..ties.len())]
}

/// Parent window in payments (the autopilot's `prior_parent_lookback_secs` analogue). Override
/// with `SR_EVAL_PARENT_LOOKBACK` for a sensitivity run.
fn parent_lookback() -> usize {
    std::env::var("SR_EVAL_PARENT_LOOKBACK")
        .ok()
        .and_then(|v| v.parse().ok())
        .filter(|&v| v > 0)
        .unwrap_or(REFRESH_EVERY)
}

/// Per-(segment, gateway) rows over the given outcomes, as the ClickHouse query returns them.
fn segment_rows<'a>(
    outcomes: impl Iterator<Item = &'a (usize, usize, bool)>,
    names: &[String],
) -> Vec<GatewaySegmentOutcome> {
    let mut counts: HashMap<(usize, usize), SegmentOutcome> = HashMap::new();
    for &(seg, g, ok) in outcomes {
        let c = counts.entry((seg, g)).or_default();
        c.observations += 1;
        c.successes += u64::from(ok);
    }
    counts
        .into_iter()
        .map(|((_, g), outcome)| GatewaySegmentOutcome {
            payment_method_type: PMT.into(),
            payment_method: PM.into(),
            gateway: names[g].clone(),
            outcome,
        })
        .collect()
}

/// What the autopilot step writes: parent rates over the recent window, prior strengths over the
/// long window, aggregated by the real `build_prior_entries`.
fn refresh_snapshot(
    lookback: &VecDeque<(usize, usize, bool)>,
    names: &[String],
    s: &Scenario,
    now: usize,
    cfg: &PriorBuildConfig,
) -> SrPriorSnapshot {
    let recent = lookback.len().saturating_sub(parent_lookback());
    let parent_rows = segment_rows(lookback.iter().skip(recent), names);
    let fit_rows = segment_rows(lookback.iter(), names);
    let mut built = build_prior_entries(&parent_rows, &fit_rows, cfg);
    if let Some(missing) = s.missing_prior_for {
        // The gateway's outcomes still count toward the pooled entry, as for a thin gateway.
        built.entries.retain(|e| e.gateway != names[missing]);
    }
    SrPriorSnapshot {
        version: sr_prior::PRIOR_SNAPSHOT_VERSION,
        merchant_id: MERCHANT.into(),
        computed_at_ms: now as i64,
        valid_until_ms: (now + s.max_age) as i64,
        lookback_secs: parent_lookback() as u64,
        fit_lookback_secs: Some(LOOKBACK as u64),
        entries: built.entries,
        pooled_entries: built.pooled_entries,
    }
}

// ── statistics ──

fn t975(df: usize) -> f64 {
    const TABLE: [f64; 30] = [
        12.706, 4.303, 3.182, 2.776, 2.571, 2.447, 2.365, 2.306, 2.262, 2.228, 2.201, 2.179, 2.160,
        2.145, 2.131, 2.120, 2.110, 2.101, 2.093, 2.086, 2.080, 2.074, 2.069, 2.064, 2.060, 2.056,
        2.052, 2.048, 2.045, 2.042,
    ];
    TABLE.get(df.saturating_sub(1)).copied().unwrap_or(1.96)
}

fn mean_ci(xs: &[f64]) -> (f64, f64) {
    let n = xs.len() as f64;
    let mean = xs.iter().sum::<f64>() / n;
    let var = xs.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (n - 1.0);
    (mean, t975(xs.len() - 1) * (var / n).sqrt())
}

fn median_iqr(mut xs: Vec<f64>) -> Option<(f64, f64, f64)> {
    if xs.is_empty() {
        return None;
    }
    xs.sort_by(|a, b| a.total_cmp(b));
    let q = |f: f64| xs[((xs.len() - 1) as f64 * f).round() as usize];
    Some((q(0.25), q(0.5), q(0.75)))
}

fn lookup<'a>(
    table: &'a HashMap<(&'static str, Policy), Vec<RunResult>>,
    scenario: &'static str,
    policy: Policy,
) -> &'a [RunResult] {
    &table[&(scenario, policy)]
}

// ── parallel runner ──

type Job = (Scenario, u64, Policy, f64);

fn run_all(jobs: Vec<Job>) -> Vec<RunResult> {
    let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
    let chunk = jobs.len().div_ceil(threads).max(1);
    std::thread::scope(|scope| {
        let handles: Vec<_> = jobs
            .chunks(chunk)
            .map(|batch| {
                scope.spawn(move || {
                    batch
                        .iter()
                        .map(|(s, seed, p, m)| simulate(s, *seed, *p, *m))
                        .collect::<Vec<_>>()
                })
            })
            .collect();
        handles
            .into_iter()
            .flat_map(|h| h.join().expect("worker panicked"))
            .collect()
    })
}

fn tune_strength() -> f64 {
    let jobs: Vec<_> = STRENGTH_GRID
        .iter()
        .flat_map(|&m| {
            stationary().into_iter().flat_map(move |s| {
                TUNING_SEEDS.map(move |seed| (s.clone(), seed, Policy::Fixed, m))
            })
        })
        .collect();
    let results = run_all(jobs);
    let per_m = results.len() / STRENGTH_GRID.len();
    println!(
        "## Tuning the configured prior strength (seeds {TUNING_SEEDS:?}, stationary scenarios)\n"
    );
    println!("| m | mean regret/payment |\n|---|---|");
    let mut best = (f64::MAX, STRENGTH_GRID[0]);
    for (i, &m) in STRENGTH_GRID.iter().enumerate() {
        let slice = &results[i * per_m..(i + 1) * per_m];
        let r = slice.iter().map(RunResult::regret).sum::<f64>() / slice.len() as f64;
        println!("| {m} | {r:.5} |");
        if r < best.0 {
            best = (r, m);
        }
    }
    println!("\nSelected configured strength: **{}**\n", best.1);
    best.1
}

fn phase_regret(runs: &[RunResult], i: usize) -> f64 {
    let (sum, n) = runs.iter().fold((0.0, 0u64), |acc, r| {
        (acc.0 + r.job_phases[i].0, acc.1 + r.job_phases[i].1)
    });
    ratio((sum, n)).unwrap_or(f64::NAN)
}

fn job_interruption(table: &HashMap<(&'static str, Policy), Vec<RunResult>>) {
    println!("## Prior job interruption (regret/payment by phase, pooled over seeds)\n");
    println!("`job stops`: stops at 20,000 and never resumes. `job resumes`: stops at 15,000, the snapshot expires at 22,500 (validity 7,500), the job runs again from 25,000.\n");
    println!("| scenario | phase | existing | fixed m | EB m |\n|---|---|---|---|---|");
    for name in ["job stops", "job resumes"] {
        for (i, label) in [
            "job down, snapshot valid",
            "snapshot expired",
            "job resumed",
        ]
        .iter()
        .enumerate()
        {
            let r = |p| phase_regret(lookup(table, name, p), i);
            if r(Policy::Existing).is_nan() {
                continue;
            }
            println!(
                "| {name} | {label} | {:.4} | {:.4} | {:.4} |",
                r(Policy::Existing),
                r(Policy::Fixed),
                r(Policy::Eb)
            );
        }
    }
    println!();
}

fn open_loop_strength_estimates() {
    println!("## Prior-strength estimation without selection bias (uniform routing)\n");
    println!(
        "Segments drawn as in the simulation, outcomes observed under uniform random routing,"
    );
    println!("fitted with `fit_prior_strength` and the default guards. 200 repetitions.\n");
    println!("| κ | segments | payments | median m̂ | IQR | m̂/κ (median) | fit failed |\n|---|---|---|---|---|---|---|");
    let mut rng = StdRng::seed_from_u64(99);
    for kappa in [3.0, 10.0, 40.0, 400.0] {
        for (segments, payments) in [(20usize, 2_000usize), (100, 10_000), (400, 40_000)] {
            let mut estimates = Vec::new();
            let mut failed = 0;
            for _ in 0..200 {
                let s = Scenario {
                    segments,
                    payments,
                    ..Scenario::base("open", kappa)
                };
                let world = build_world(&s, rng.gen());
                let mut counts = vec![SegmentOutcome::default(); segments];
                for t in 0..payments {
                    let seg = world.segment_of[t];
                    // gateway 0 observed with probability 1/3, as under uniform routing
                    if rng.gen::<f64>() < 1.0 / 3.0 {
                        counts[seg].observations += 1;
                        counts[seg].successes +=
                            u64::from(world.outcome_u[t] < world.truth[seg][0]);
                    }
                }
                match fit_prior_strength(&counts, &PriorFitConfig::default()) {
                    Ok(m) => estimates.push(m),
                    Err(_) => failed += 1,
                }
            }
            match median_iqr(estimates) {
                Some((q1, med, q3)) => println!(
                    "| {kappa} | {segments} | {payments} | {med:.1} | [{q1:.1}, {q3:.1}] | {:.2} | {failed}/200 |",
                    med / kappa
                ),
                None => println!("| {kappa} | {segments} | {payments} | – | – | – | {failed}/200 |"),
            }
        }
    }
    println!();
}

fn snapshot_with(payment_methods: usize, gateways: usize) -> SrPriorSnapshot {
    let entries = (0..payment_methods)
        .flat_map(|pm| {
            (0..gateways).map(move |g| sr_prior::SrPriorEntry {
                payment_method_type: PMT.into(),
                payment_method: if pm == 0 {
                    PM.into()
                } else {
                    format!("PM_{pm}")
                },
                gateway: gateway_name(g),
                parent_success_rate: 0.8,
                observations: 1_000,
                learned_prior_strength: Some(25.0),
                fit_segments: Some(50),
                fit_error: None,
            })
        })
        .collect();
    SrPriorSnapshot {
        version: sr_prior::PRIOR_SNAPSHOT_VERSION,
        merchant_id: MERCHANT.into(),
        computed_at_ms: 0,
        valid_until_ms: i64::MAX,
        lookback_secs: 3_600,
        fit_lookback_secs: Some(86_400),
        entries,
        pooled_entries: Vec::new(),
    }
}

/// Median and range (ns per call) of `trials` timed runs of `iters` calls each, after a warm-up.
/// Callers interleave the paths they compare, trial by trial, so both see the same machine state.
fn time_ns(iters: u32, mut f: impl FnMut() -> f64) -> f64 {
    let start = Instant::now();
    let mut acc = 0.0;
    for _ in 0..iters {
        acc += f();
    }
    std::hint::black_box(acc);
    start.elapsed().as_nanos() as f64 / f64::from(iters)
}

fn summarize(mut xs: Vec<f64>) -> String {
    xs.sort_by(|a, b| a.total_cmp(b));
    format!(
        "{:.0} [{:.0}–{:.0}]",
        xs[xs.len() / 2],
        xs[0],
        xs[xs.len() - 1]
    )
}

fn latency() {
    use std::hint::black_box;

    const TRIALS: usize = 9;
    const ITERS: u32 = 50_000;
    println!("## Scoring cost per decision (release build, in-process CPU work only)\n");
    println!("Method: identical 125-entry windows (legacy `1`/`0` and synthetic `1.0` entries mixed) for every path; inputs passed through `black_box`; one warm-up run, then {TRIALS} trials of {ITERS} decisions per path, paths interleaved trial by trial; median ns/decision with the [min–max] over trials. Redis I/O, network, cache hits and allocator contention under load are not measured — these numbers do not establish production latency.\n");
    let window: Vec<String> = (0..BUCKET)
        .map(|i| match i % 7 {
            0 => "0".to_string(),
            1 | 2 => sr_prior::SYNTHETIC_SUCCESS.to_string(),
            _ => "1".to_string(),
        })
        .collect();
    let small = snapshot_with(1, 8);
    let large = snapshot_with(25, 8);
    let small_json = serde_json::to_string(&small).unwrap_or_default();
    let large_json = serde_json::to_string(&large).unwrap_or_default();
    let flag_json = {
        let merchants: Vec<String> = (0..20)
            .map(|i| format!(r#"{{"merchantId":"merchant_{i}","rollout":100}}"#))
            .collect();
        format!(
            r#"{{"enableAll":false,"enableAllRollout":null,"disableAny":null,"merchants":[{}]}}"#,
            merchants.join(",")
        )
    };
    let names: Vec<String> = (0..8).map(gateway_name).collect();
    let flags = Policy::Eb.flags();

    println!("| gateways | existing: window score | prior, decoded snapshot cached ({} B) | prior, decoded snapshot cached ({} B) | prior, snapshot decoded per decision ({} B) | prior, snapshot decoded per decision ({} B) |\n|---|---|---|---|---|---|", small_json.len(), large_json.len(), small_json.len(), large_json.len());
    for k in [3usize, 8] {
        let existing = |w: &[String]| {
            (0..k)
                .map(|_| legacy_window_score(black_box(w)).unwrap_or(1.0))
                .sum::<f64>()
        };
        let with_prior = |w: &[String], snap: &SrPriorSnapshot| {
            names
                .iter()
                .take(k)
                .map(|name| {
                    resolve_prior(flags, Some(snap), MERCHANT, 1, PMT, PM, name, Some(20.0))
                        .map(|p| p.posterior_mean(&WindowStats::from_entries(black_box(w))))
                        .unwrap_or(1.0)
                })
                .sum::<f64>()
        };
        let decode_then = |json: &str, w: &[String]| {
            serde_json::from_str::<SrPriorSnapshot>(black_box(json))
                .map(|snap| with_prior(w, &snap))
                .unwrap_or(0.0)
        };
        // Warm-up.
        time_ns(ITERS / 5, || existing(&window));
        time_ns(ITERS / 5, || with_prior(&window, &small));
        time_ns(ITERS / 5, || decode_then(&small_json, &window));
        time_ns(ITERS / 50, || decode_then(&large_json, &window));
        let mut a = Vec::new();
        let mut b = Vec::new();
        let mut c = Vec::new();
        let mut d = Vec::new();
        let mut e = Vec::new();
        for _ in 0..TRIALS {
            a.push(time_ns(ITERS, || existing(&window)));
            b.push(time_ns(ITERS, || with_prior(&window, &small)));
            e.push(time_ns(ITERS, || with_prior(&window, &large)));
            c.push(time_ns(ITERS, || decode_then(&small_json, &window)));
            d.push(time_ns(ITERS / 10, || decode_then(&large_json, &window)));
        }
        println!(
            "| {k} | {} | {} | {} | {} | {} |",
            summarize(a),
            summarize(b),
            summarize(e),
            summarize(c),
            summarize(d)
        );
    }
    let mut flag_ns = Vec::new();
    time_ns(ITERS / 5, || {
        serde_json::from_str::<open_router::redis::types::FeatureConf>(black_box(&flag_json))
            .map(|c| f64::from(u8::from(c.enableAll)))
            .unwrap_or(0.0)
    });
    for _ in 0..TRIALS {
        flag_ns.push(time_ns(ITERS, || {
            serde_json::from_str::<open_router::redis::types::FeatureConf>(black_box(&flag_json))
                .map(|c| f64::from(u8::from(c.enableAll)))
                .unwrap_or(0.0)
        }));
    }
    println!(
        "\nFlags-off overhead: one extra feature-flag decode per decision (`findByNameFromRedis` re-parses the cached JSON on every call; a 20-merchant `FeatureConf`): {} ns.",
        summarize(flag_ns)
    );
    println!(
        "Snapshot sizes: 8 gateways × 1 payment method = {} bytes; 8 gateways × 25 payment methods = {} bytes.\n",
        small_json.len(),
        large_json.len()
    );
}

fn main() {
    println!("# SR v3 cold-start prior — offline evaluation\n");
    println!(
        "Synthetic data. B={BUCKET}, hedging={}%, prior refresh every {REFRESH_EVERY} payments (parent rates over the last {}, prior strength over the last {LOOKBACK}); tuning seeds {TUNING_SEEDS:?}, test seeds {TEST_SEEDS:?}.\n",
        HEDGE * 100.0,
        parent_lookback()
    );
    if std::env::var("SR_EVAL_ONLY").as_deref() == Ok("latency") {
        latency();
        return;
    }
    let m = tune_strength();

    let scenarios = all_scenarios();
    let jobs: Vec<_> = scenarios
        .iter()
        .flat_map(|s| TEST_SEEDS.flat_map(move |seed| Policy::ALL.map(|p| (s.clone(), seed, p, m))))
        .collect();
    let results = run_all(jobs);
    let mut table: HashMap<(&str, Policy), Vec<RunResult>> = HashMap::new();
    let mut idx = 0;
    for s in &scenarios {
        for _ in TEST_SEEDS {
            for p in Policy::ALL {
                table
                    .entry((s.name, p))
                    .or_default()
                    .push(results[idx].clone());
                idx += 1;
            }
        }
    }
    let get = |s: &'static str, p: Policy| lookup(&table, s, p);

    println!(
        "## Held-out results ({} seeds per scenario)\n",
        TEST_SEEDS.count()
    );
    println!("| scenario | auth existing | auth fixed m | auth EB m | regret existing | regret fixed m | regret EB m |\n|---|---|---|---|---|---|---|");
    for s in &scenarios {
        let auth = |p| {
            get(s.name, p).iter().map(RunResult::auth_rate).sum::<f64>() / TEST_SEEDS.count() as f64
        };
        let reg = |p| {
            get(s.name, p).iter().map(RunResult::regret).sum::<f64>() / TEST_SEEDS.count() as f64
        };
        println!(
            "| {} | {:.4} | {:.4} | {:.4} | {:.4} | {:.4} | {:.4} |",
            s.name,
            auth(Policy::Existing),
            auth(Policy::Fixed),
            auth(Policy::Eb),
            reg(Policy::Existing),
            reg(Policy::Fixed),
            reg(Policy::Eb)
        );
    }

    println!("\n## Paired differences in regret/payment (mean ± 95% CI, t with n−1 df)\n");
    println!("Positive = the second policy has lower regret. `wins` = seeds where it does.\n");
    println!("| scenario | existing − fixed m | wins | existing − EB m | wins | fixed m − EB m | wins |\n|---|---|---|---|---|---|---|");
    for s in &scenarios {
        let diff = |a: Policy, b: Policy| {
            let xs: Vec<f64> = get(s.name, a)
                .iter()
                .zip(get(s.name, b))
                .map(|(x, y)| x.regret() - y.regret())
                .collect();
            let (mean, ci) = mean_ci(&xs);
            let wins = xs.iter().filter(|x| **x > 0.0).count();
            let sig = if mean.abs() > ci { " *" } else { "" };
            (format!("{mean:+.4} ± {ci:.4}{sig}"), wins)
        };
        let (a, aw) = diff(Policy::Existing, Policy::Fixed);
        let (b, bw) = diff(Policy::Existing, Policy::Eb);
        let (c, cw) = diff(Policy::Fixed, Policy::Eb);
        let n = TEST_SEEDS.count();
        println!(
            "| {} | {a} | {aw}/{n} | {b} | {bw}/{n} | {c} | {cw}/{n} |",
            s.name
        );
    }
    println!("\n`*` = the 95% CI excludes zero.\n");

    println!("## Regret/payment by segment volume (stationary κ=40, pooled over seeds)\n");
    println!("| volume bucket | existing | fixed m | EB m |\n|---|---|---|---|");
    for (i, label) in ["sparse (<50 payments)", "medium (50–499)", "dense (≥500)"]
        .iter()
        .enumerate()
    {
        let r = |p| {
            let (sum, n) = get("κ=40", p).iter().fold((0.0, 0u64), |acc, r| {
                (acc.0 + r.by_volume[i].0, acc.1 + r.by_volume[i].1)
            });
            sum / n.max(1) as f64
        };
        println!(
            "| {label} | {:.4} | {:.4} | {:.4} |",
            r(Policy::Existing),
            r(Policy::Fixed),
            r(Policy::Eb)
        );
    }

    println!("\n## Adaptation after a change (regret/payment)\n");
    println!("`early` = first 2,000 payments after the change; `late` = the rest of the run. For `job stops` the change is the snapshot expiring.\n");
    println!("| scenario | existing early | fixed m early | EB m early | existing late | fixed m late | EB m late |\n|---|---|---|---|---|---|---|");
    for s in scenarios.iter().filter(|s| s.change_at().is_some()) {
        let r = |p, early: bool| {
            let (sum, n) = get(s.name, p).iter().fold((0.0, 0u64), |acc, r| {
                let x = if early { r.post_early } else { r.post_late };
                (acc.0 + x.0, acc.1 + x.1)
            });
            ratio((sum, n)).unwrap_or(f64::NAN)
        };
        println!(
            "| {} | {:.4} | {:.4} | {:.4} | {:.4} | {:.4} | {:.4} |",
            s.name,
            r(Policy::Existing, true),
            r(Policy::Fixed, true),
            r(Policy::Eb, true),
            r(Policy::Existing, false),
            r(Policy::Fixed, false),
            r(Policy::Eb, false)
        );
    }

    println!("\n## Learned prior strength inside the routing loop (selection bias)\n");
    println!("Every per-gateway fit across refreshes and seeds. The routing loop decides which outcomes are observed, so this includes selection bias; compare with the uniform-routing table below.\n");
    println!("| scenario | κ | fits | median m̂ | IQR | m̂/κ (median) | at upper clamp |\n|---|---|---|---|---|---|---|");
    for s in &scenarios {
        let fits: Vec<f64> = get(s.name, Policy::Eb)
            .iter()
            .flat_map(|r| r.learned_strengths.iter().copied())
            .collect();
        let capped = fits
            .iter()
            .filter(|m| **m >= sr_prior::MAX_PRIOR_STRENGTH)
            .count();
        let n = fits.len();
        match median_iqr(fits) {
            Some((q1, med, q3)) => println!(
                "| {} | {} | {n} | {med:.1} | [{q1:.1}, {q3:.1}] | {:.2} | {:.0}% |",
                s.name,
                s.kappa,
                med / s.kappa,
                100.0 * capped as f64 / n as f64
            ),
            None => println!("| {} | {} | 0 | – | – | – | – |", s.name, s.kappa),
        }
    }
    println!();
    job_interruption(&table);
    open_loop_strength_estimates();
    latency();
}
