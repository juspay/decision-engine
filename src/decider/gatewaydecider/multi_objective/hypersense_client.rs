use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use masking::PeekInterface;
use serde::{Deserialize, Serialize};

use crate::app::{get_tenant_app_state, TenantAppState};
use crate::config::HypersenseConfig;
use crate::logger;

use super::cluster_key::ClusterKey;
use super::{CostModel, CostSource};

const SIGNIN_PATH: &str = "/api/onboarding/signin_v2";
const FEE_ESTIMATE_PATH: &str = "/api/fee-analysis/get-fee-rate-estimate";
const TOKEN_REDIS_KEY: &str = "hypersense_access_token";
const TOKEN_SAFETY_BUFFER_SECS: i64 = 600;
const SIGNIN_TIMEOUT_MS: u64 = 5_000;
const FEE_TIMEOUT_MS: u64 = 2_000;

#[derive(Debug, Clone)]
pub struct PspCost {
    pub available: bool,
    pub effective_cost_bps: f64,
    /// Which source produced this cost (for observability on the response).
    pub source: CostSource,
    /// The fitted model behind `effective_cost_bps`, when the source exposes it.
    pub cost_model: Option<CostModel>,
}

#[derive(Debug, Serialize)]
struct SigninRequest {
    payload: SigninPayload,
}

#[derive(Debug, Serialize)]
struct SigninPayload {
    username: String,
    password: String,
}

#[derive(Debug, Deserialize)]
struct SigninResponse {
    #[serde(rename = "Access Token")]
    access_token: String,
    /// Token lifetime in seconds as reported by the API (e.g. 86400).
    #[serde(default)]
    expires_in: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct FeeRateRow {
    gateway: String,
    #[serde(default)]
    available: bool,
    #[serde(default)]
    effective_cost_bps: Option<f64>,
}

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .pool_idle_timeout(Some(Duration::from_secs(30)))
            // Hard ceiling; per-call `tokio::time::timeout` enforces the tighter bound.
            .timeout(Duration::from_secs(10))
            .build()
            .expect("failed to build hypersense reqwest client")
    })
}

/// Cap on distinct cached lookup scenarios. Generous: the cardinality is bounded
/// by (merchant × cluster *shape* × candidate PSP set) — note the cache key drops
/// `amount`, so traffic volume does not inflate it.
const COST_CACHE_MAX_ENTRIES: usize = 10_000;

/// Minimum relative gap between the two probe amounts before we trust the solved
/// split. Near-equal amounts make `1/a1 − 1/a2` tiny and amplify rounding noise in
/// the reported bps into a wildly wrong fixed-fee term, so we keep probing instead.
const MIN_PROBE_AMOUNT_REL_GAP: f64 = 0.01;

/// Upper sanity bound (basis points) on a solved percentage component. ~65% of the
/// ticket as pure markup is already implausible; anything past it means the linear
/// fit broke (e.g. a non-linear/capped fee), so we discard it and re-probe live.
const MAX_PLAUSIBLE_PCT_BPS: f64 = 6_500.0;

/// Upper sanity bound (cluster major-currency units) on a solved flat per-transaction
/// fee. Real fixed fees are cents-scale; a larger value means the solver collapsed a
/// non-linear/tiered curve (e.g. small-ticket interchange) into a bogus flat fee, which
/// then explodes at small amounts. Reject and re-probe live instead of caching it.
const MAX_PLAUSIBLE_FIXED: f64 = 2.0;

/// Max residual (basis points) allowed between a solved model's prediction and an
/// actual observation before we declare the linear fee model a bad fit. The upstream
/// fee calc is deterministic, so a truly linear PSP fits to well under 1 bp; anything
/// larger means the curve isn't `pct + fixed/amount` and must not be cached.
const FIT_TOLERANCE_BPS: f64 = 1.0;

/// Minimum number of distinct-amount observations required before a fit is trusted.
/// Two points solve the line but fit themselves exactly; a third is the held-out point
/// that actually reveals non-linearity (tiered/small-ticket curves), so we never
/// promote a scenario to `Solved` on two probes alone.
const MIN_OBSERVATIONS_TO_SOLVE: usize = 3;

/// Cap on pending probe observations retained per scenario while still solving. Bounds
/// memory if a scenario keeps mis-fitting (never solves); we keep the most recent few.
const MAX_PENDING_OBSERVATIONS: usize = 6;

/// Amount-independent fee model for one gateway in one scenario. The upstream
/// `effective_cost_bps` is a blend of a percentage markup and a flat per-txn fee:
///
/// ```text
/// effective_cost_bps(amount) = pct_bps + (fixed / amount) * 10_000
/// ```
///
/// Caching `{pct_bps, fixed}` (both amount-independent) lets us serve every future
/// amount in the scenario with one multiply-add instead of another network call.
#[derive(Debug, Clone, Copy)]
struct FeeModel {
    available: bool,
    pct_bps: f64,
    /// Flat per-transaction fee in the cluster's major currency unit.
    fixed: f64,
}

impl FeeModel {
    fn effective_cost_bps(&self, amount: f64) -> f64 {
        if amount > 0.0 {
            self.pct_bps + (self.fixed / amount) * 10_000.0
        } else {
            self.pct_bps
        }
    }
}

/// A single live observation: the reported `(available, effective_cost_bps)` per
/// gateway at one specific `amount`. Two of these at distinct amounts solve `FeeModel`.
type Observation = HashMap<String, (bool, f64)>;

/// Per-scenario cache state.
#[derive(Clone)]
enum ScenarioCost {
    /// Live observations gathered so far (distinct amounts), not yet a trusted fit. We
    /// keep accumulating until at least `MIN_OBSERVATIONS_TO_SOLVE` distinct amounts
    /// agree on one linear model — two to solve it, a third to confirm it.
    Probing { obs: Vec<(f64, Observation)> },
    /// Solved amount-independent model per gateway; serves all amounts locally.
    Solved(HashMap<String, FeeModel>),
}

/// Solves the `{pct_bps, fixed}` split for every gateway from two observations at
/// distinct amounts. Returns `None` (keep probing live) unless *every* gateway is
/// available in both samples and yields a plausible, non-negative fit (within both the
/// `pct_bps` and `fixed` sanity bounds) — we never want to pin a half-trusted or
/// implausible model into the cache.
fn solve_fee_models(
    a1: f64,
    obs1: &Observation,
    a2: f64,
    obs2: &Observation,
) -> Option<HashMap<String, FeeModel>> {
    if a1 <= 0.0 || a2 <= 0.0 {
        return None;
    }
    let rel_gap = (a1 - a2).abs() / a1.max(a2);
    if rel_gap < MIN_PROBE_AMOUNT_REL_GAP {
        return None;
    }
    let inv_delta = 1.0 / a1 - 1.0 / a2;
    if inv_delta == 0.0 {
        return None;
    }

    let mut models = HashMap::with_capacity(obs1.len());
    for (gw, &(avail1, c1)) in obs1 {
        let &(avail2, c2) = obs2.get(gw)?;
        if !avail1 || !avail2 {
            return None;
        }
        // c = pct_bps + (fixed * 10_000) / amount  ⇒  solve the two-point line.
        let fixed_scaled = (c1 - c2) / inv_delta; // == fixed * 10_000
        let fixed = fixed_scaled / 10_000.0;
        let pct_bps = c1 - fixed_scaled / a1;
        // Tiny negative values are float noise around an exactly-zero component;
        // clamp those, but reject anything clearly outside a sane fee shape.
        let fixed = if (-1e-6..0.0).contains(&fixed) {
            0.0
        } else {
            fixed
        };
        let pct_bps = if (-1e-6..0.0).contains(&pct_bps) {
            0.0
        } else {
            pct_bps
        };
        if !(0.0..=MAX_PLAUSIBLE_FIXED).contains(&fixed)
            || !(0.0..=MAX_PLAUSIBLE_PCT_BPS).contains(&pct_bps)
        {
            return None;
        }
        models.insert(
            gw.clone(),
            FeeModel {
                available: true,
                pct_bps,
                fixed,
            },
        );
    }
    Some(models)
}

/// Fits a per-gateway fee model from accumulated live observations *and validates it*.
///
/// A two-point solve fits its own two points exactly, so it can never reveal that the
/// real cost curve is non-linear (tiered/small-ticket interchange) — it just collapses
/// the curvature into a bogus flat fee that explodes at small amounts. To catch that we
/// require a held-out third point: solve the line from the two most-separated amounts
/// (largest `inv_delta`, least noise amplification), then require *every* observation —
/// including the ones not used to solve — to fall within `FIT_TOLERANCE_BPS` of the
/// model. Returns `None` (keep probing live) until at least `MIN_OBSERVATIONS_TO_SOLVE`
/// distinct amounts agree on a single linear model.
fn fit_from_observations(obs: &[(f64, Observation)]) -> Option<HashMap<String, FeeModel>> {
    if obs.len() < MIN_OBSERVATIONS_TO_SOLVE {
        return None;
    }

    // Solve from the two most-separated amounts; the rest are the validation set.
    let lo = obs
        .iter()
        .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))?;
    let hi = obs
        .iter()
        .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))?;
    let models = solve_fee_models(lo.0, &lo.1, hi.0, &hi.1)?;

    // Reject unless the solved model reproduces every observation (the held-out points
    // are the real test) for every gateway, within tolerance and still available.
    for (amount, rows) in obs {
        for (gw, model) in &models {
            let &(avail, actual) = rows.get(gw)?;
            if !avail || (model.effective_cost_bps(*amount) - actual).abs() > FIT_TOLERANCE_BPS {
                return None;
            }
        }
    }
    Some(models)
}

/// Adds a fresh observation to a scenario's pending probe set. A sample at a near-equal
/// amount (within `MIN_PROBE_AMOUNT_REL_GAP`) *replaces* the existing one — repeated
/// amounts refresh rather than crowd out the distinct probes a fit needs — and the
/// buffer is capped at the most recent `MAX_PENDING_OBSERVATIONS` distinct amounts.
fn upsert_observation(obs: &mut Vec<(f64, Observation)>, amount: f64, rows: Observation) {
    if let Some(slot) = obs
        .iter_mut()
        .find(|(a, _)| (*a - amount).abs() / a.max(amount).max(1.0) < MIN_PROBE_AMOUNT_REL_GAP)
    {
        slot.1 = rows;
        return;
    }
    obs.push((amount, rows));
    if obs.len() > MAX_PENDING_OBSERVATIONS {
        obs.remove(0);
    }
}

/// Temporary, best-effort in-process cache that fronts the fee-rate endpoint by
/// caching the *fee model* (percentage + fixed split), not the per-amount response.
///
/// Conservative by construction: a scenario is only served from cache once its split
/// has been solved *and validated* against a held-out observation (≥3 consistent live
/// samples; see `fit_from_observations`); transient failures and partial/unavailable
/// scenarios are never cached, and entries expire past TTL.
struct CostCache {
    data: Mutex<HashMap<String, (ScenarioCost, Instant)>>,
    max_size: usize,
}

impl CostCache {
    fn new(max_size: usize) -> Self {
        Self {
            data: Mutex::new(HashMap::new()),
            max_size,
        }
    }

    /// Returns per-gateway costs computed from a solved model at `amount`, when one
    /// is cached and unexpired. Returns `None` (caller does a live lookup) while a
    /// scenario is still probing, expired, or on lock contention.
    fn get(&self, key: &str, amount: f64, ttl: Duration) -> Option<HashMap<String, PspCost>> {
        let data = self.data.try_lock().ok()?;
        let (state, stored_at) = data.get(key)?;
        if stored_at.elapsed() >= ttl {
            return None;
        }
        match state {
            ScenarioCost::Solved(models) => Some(
                models
                    .iter()
                    .map(|(gw, m)| {
                        (
                            gw.clone(),
                            PspCost {
                                available: m.available,
                                effective_cost_bps: m.effective_cost_bps(amount),
                                source: CostSource::Hypersense,
                                cost_model: Some(CostModel {
                                    pct_bps: Some(m.pct_bps),
                                    fixed_fee: Some(m.fixed),
                                    brand: None,
                                    variant: None,
                                    issuer: None,
                                    ccy: None,
                                    ic_category: None,
                                    interchange_bps: None,
                                    segment_idx: None,
                                    amount_lo: None,
                                    amount_hi: None,
                                }),
                            },
                        )
                    })
                    .collect(),
            ),
            ScenarioCost::Probing { .. } => None,
        }
    }

    /// Folds a fresh live observation into the cache: appends it to the scenario's
    /// pending probes and promotes to a solved model only once a validated fit emerges
    /// (see `fit_from_observations`). Skips silently on lock contention and never
    /// downgrades an already-solved entry.
    fn record(&self, key: &str, amount: f64, rows: Observation, ttl: Duration) {
        let Ok(mut data) = self.data.try_lock() else {
            return;
        };
        let now = Instant::now();

        // Fold into an existing, unexpired probe set for this scenario.
        if let Some((state, stored_at)) = data.get_mut(key) {
            if stored_at.elapsed() < ttl {
                match state {
                    // Already solved — leave it; it serves all amounts.
                    ScenarioCost::Solved(_) => return,
                    ScenarioCost::Probing { obs } => {
                        upsert_observation(obs, amount, rows);
                        if let Some(models) = fit_from_observations(obs) {
                            *state = ScenarioCost::Solved(models);
                            *stored_at = now;
                        }
                        return;
                    }
                }
            }
        }

        // Fresh scenario (or the prior one expired) — start a new probe set.
        self.evict_if_full(&mut data, key);
        data.insert(
            key.to_string(),
            (
                ScenarioCost::Probing {
                    obs: vec![(amount, rows)],
                },
                now,
            ),
        );
    }

    fn evict_if_full(
        &self,
        data: &mut HashMap<String, (ScenarioCost, Instant)>,
        incoming_key: &str,
    ) {
        if data.len() < self.max_size || data.contains_key(incoming_key) {
            return;
        }
        let evict_key = data
            .iter()
            .find(|(_, (_, stored_at))| stored_at.elapsed() >= self.eviction_age())
            .map(|(k, _)| k.clone())
            .or_else(|| data.keys().next().cloned());
        if let Some(k) = evict_key {
            data.remove(&k);
        }
    }

    /// Entries older than this are preferred eviction victims. Decoupled from the
    /// per-call TTL (which we don't carry here); a generous fixed age is enough to
    /// prefer stale rows over live ones under capacity pressure.
    fn eviction_age(&self) -> Duration {
        Duration::from_secs(3_600)
    }
}

fn cost_cache() -> &'static CostCache {
    static CACHE: OnceLock<CostCache> = OnceLock::new();
    CACHE.get_or_init(|| CostCache::new(COST_CACHE_MAX_ENTRIES))
}

/// Builds a deterministic, **amount-independent** cache key for a scenario. `amount`
/// is cleared (the fee model is computed per-amount on top of the cached split) and
/// the PSP list is sorted so call-site ordering doesn't fragment the cache. The
/// merchant id is prefixed to keep merchants isolated. Returns `None` if the request
/// can't be serialized (caching is then simply skipped).
fn cost_cache_key(merchant_id: &str, request: &ClusterKey) -> Option<String> {
    let mut keyed = request.clone();
    keyed.amount = None;
    keyed.psp_array.sort();
    serde_json::to_string(&keyed)
        .ok()
        .map(|body| format!("{merchant_id}|{body}"))
}

/// Returns a valid access token, reading it from Redis when present and otherwise
/// signing in and caching the freshly issued token. Returns `None` only when a
/// token could not be obtained at all.
async fn get_access_token(app_state: &TenantAppState, cfg: &HypersenseConfig) -> Option<String> {
    // 1. Fast path: reuse the cached token.
    if let Ok(token) = app_state.redis_conn.get_key_string(TOKEN_REDIS_KEY).await {
        if !token.is_empty() {
            return Some(token);
        }
    }

    // 2. Cache miss / expiry: sign in for a fresh token.
    let signin = sign_in(cfg).await?;

    // 3. Cache it with a TTL strictly below the token's own lifetime.
    let upper = signin
        .expires_in
        .unwrap_or(86_400)
        .saturating_sub(TOKEN_SAFETY_BUFFER_SECS)
        .max(1);
    let ttl = cfg.token_ttl_secs.clamp(1, upper);

    if let Err(e) = app_state
        .redis_conn
        .set_key_with_ttl(TOKEN_REDIS_KEY, &signin.access_token, ttl)
        .await
    {
        // Non-fatal: we still have a usable token for this request.
        logger::warn!(
            tag = "hypersense",
            "failed to cache access token (ttl {}s): {:?}",
            ttl,
            e
        );
    }

    Some(signin.access_token)
}

/// Calls the sign-in API with the configured credentials.
async fn sign_in(cfg: &HypersenseConfig) -> Option<SigninResponse> {
    let url = format!("{}{}", cfg.base_url.trim_end_matches('/'), SIGNIN_PATH);
    let req = SigninRequest {
        payload: SigninPayload {
            username: cfg.username.clone(),
            password: cfg.password.peek().clone(),
        },
    };

    let fut = client().post(&url).json(&req).send();
    let response = match tokio::time::timeout(Duration::from_millis(SIGNIN_TIMEOUT_MS), fut).await {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => {
            logger::warn!(tag = "hypersense", "sign-in request error: {:?}", e);
            return None;
        }
        Err(_) => {
            logger::warn!(
                tag = "hypersense",
                "sign-in timeout after {}ms",
                SIGNIN_TIMEOUT_MS
            );
            return None;
        }
    };

    if !response.status().is_success() {
        logger::warn!(tag = "hypersense", "sign-in non-2xx: {}", response.status());
        return None;
    }

    match response.json::<SigninResponse>().await {
        Ok(body) => Some(body),
        Err(e) => {
            logger::warn!(tag = "hypersense", "sign-in parse error: {:?}", e);
            None
        }
    }
}

/// Best-effort cost lookup for the candidate PSPs. Returns an empty map when nothing can price a
/// PSP; the algorithm treats a missing PSP as "no cost data" and leaves it unranked.
///
/// **Source precedence: our own ingested data first.** Each PSP is priced from the in-house
/// serving view of the fitted `cost_fee_model` when a GOOD model covers its decide-time key;
/// anything not covered there falls back to the configured seed / live-Hypersense source. In-house
/// coverage is money-weighted (see the coverage card), so the tail legitimately relies on fallback.
pub async fn lookup_costs(
    merchant_id: &str,
    cluster: &ClusterKey,
    psps: &[String],
) -> HashMap<String, PspCost> {
    if psps.is_empty() {
        return HashMap::new();
    }

    // 1. In-house first, straight from memory (no network, no ClickHouse on the hot path).
    let mut costs = inhouse_costs(merchant_id, cluster, psps);

    // 2. Fall back to seed / live Hypersense only for PSPs in-house couldn't price.
    let missing: Vec<String> = psps
        .iter()
        .filter(|p| !costs.contains_key(*p))
        .cloned()
        .collect();
    if !missing.is_empty() {
        for (psp, cost) in fallback_lookup_costs(merchant_id, cluster, &missing).await {
            costs.entry(psp).or_insert(cost);
        }
    }
    costs
}

/// Price PSPs from the in-house serving view of the fitted models. PSPs with no GOOD model for the
/// decide-time key are simply absent (caller falls back). `psp` maps to the `connector` column by
/// lowercase name.
fn inhouse_costs(
    merchant_id: &str,
    cluster: &ClusterKey,
    psps: &[String],
) -> HashMap<String, PspCost> {
    let network = cluster.card_network.as_deref().unwrap_or("");
    let funding = cluster.payment_method_type.as_deref().unwrap_or("");
    let program = cluster.card_type.as_deref().unwrap_or("");
    let currency = cluster.transaction_currency.as_deref().unwrap_or("");
    let issuer = cluster.card_issuing_country_raw.as_deref().unwrap_or("");
    let region = cluster.card_issuing_country.as_deref().unwrap_or("");
    let channel = cluster.channel.as_deref().unwrap_or("");
    let wallet = cluster.wallet.as_deref().unwrap_or("");
    let amount = cluster.amount.unwrap_or(0.0);

    let mut out = HashMap::new();
    for psp in psps {
        if let Some(m) = crate::cost_ingestion::serving::lookup(
            merchant_id,
            &psp.to_lowercase(),
            network,
            funding,
            program,
            currency,
            issuer,
            region,
            channel,
            wallet,
            amount,
        ) {
            out.insert(
                psp.clone(),
                PspCost {
                    available: true,
                    effective_cost_bps: m.effective_bps,
                    source: CostSource::InHouse,
                    cost_model: Some(CostModel {
                        brand: Some(m.brand),
                        variant: m.variant,
                        issuer: m.issuer,
                        ccy: Some(m.currency),
                        ic_category: m.ic_category,
                        interchange_bps: m.interchange_bps,
                        segment_idx: m.segment_idx,
                        amount_lo: m.amount_lo,
                        amount_hi: m.amount_hi,
                        pct_bps: Some(m.pct_bps),
                        fixed_fee: Some(m.fixed),
                    }),
                },
            );
        }
    }
    out
}

/// Fallback cost lookup: config seed table (simulator) or the live Hypersense fee-rate API.
///
/// Flow (live): resolve an access token (Redis-cached, minted via the sign-in API on miss), then
/// call the fee-rate-estimate API with it for the candidate PSPs.
async fn fallback_lookup_costs(
    merchant_id: &str,
    cluster: &ClusterKey,
    psps: &[String],
) -> HashMap<String, PspCost> {
    if psps.is_empty() {
        return HashMap::new();
    }

    let app_state = get_tenant_app_state().await;
    let cfg = &app_state.config.hypersense;

    // Simulator / offline mode: serve realistic IC++-vs-blended costs from the config seed
    // table, skipping the token fetch, network round-trip, and the live model cache.
    if cfg.use_seed_costs {
        return super::seed_costs::lookup_seed_costs(&cfg.seed_costs, cluster, psps);
    }

    // The request we'd send doubles as the (amount-independent) cache key, so build
    // it up front. `amount` is read separately — it parameterises the cached fee
    // model rather than keying it.
    let mut request = cluster.clone();
    request.psp_array = psps.to_vec();
    let amount = cluster.amount.unwrap_or(0.0);

    // Temporary cache fast-path: once a scenario's fee split is solved, compute this
    // amount's cost locally and skip both the token fetch and the network round-trip.
    // A zero TTL disables the cache entirely.
    let cache_ttl = Duration::from_secs(cfg.cost_cache_ttl_secs);
    let cache_key = if cache_ttl.is_zero() {
        None
    } else {
        cost_cache_key(merchant_id, &request)
    };
    if let Some(key) = &cache_key {
        if let Some(cached) = cost_cache().get(key, amount, cache_ttl) {
            return cached;
        }
    }

    let token = match get_access_token(&app_state, cfg).await {
        Some(t) => t,
        None => {
            logger::warn!(
                tag = "hypersense",
                "no access token available for {}; skipping cost lookup",
                merchant_id
            );
            return HashMap::new();
        }
    };

    let url = format!(
        "{}{}",
        cfg.base_url.trim_end_matches('/'),
        FEE_ESTIMATE_PATH
    );

    // The fee API authenticates with the raw token in `Authorization` (no Bearer prefix).
    let fut = client()
        .post(&url)
        .header(reqwest::header::AUTHORIZATION, token)
        .json(&request)
        .send();
    let result = tokio::time::timeout(Duration::from_millis(FEE_TIMEOUT_MS), fut).await;

    let response = match result {
        Ok(Ok(r)) => r,
        Ok(Err(e)) => {
            logger::warn!(
                tag = "hypersense",
                "lookup error for {}: {:?}",
                merchant_id,
                e
            );
            return HashMap::new();
        }
        Err(_) => {
            logger::warn!(
                tag = "hypersense",
                "lookup timeout after {}ms for {}",
                FEE_TIMEOUT_MS,
                merchant_id
            );
            return HashMap::new();
        }
    };

    if !response.status().is_success() {
        logger::warn!(
            tag = "hypersense",
            "lookup non-2xx for {}: {}",
            merchant_id,
            response.status()
        );
        return HashMap::new();
    }

    let body = match response.json::<Vec<FeeRateRow>>().await {
        Ok(b) => b,
        Err(e) => {
            logger::warn!(
                tag = "hypersense",
                "lookup parse error for {}: {:?}",
                merchant_id,
                e
            );
            return HashMap::new();
        }
    };

    let costs: HashMap<String, PspCost> = body
        .into_iter()
        .map(|row| {
            (
                row.gateway,
                PspCost {
                    available: row.available,
                    effective_cost_bps: row.effective_cost_bps.unwrap_or(0.0),
                    source: CostSource::Hypersense,
                    cost_model: None,
                },
            )
        })
        .collect();

    // Fold this live observation into the model cache. We cache the amount-independent
    // fee split (pct + fixed), solved once two distinct amounts have been seen for the
    // scenario — so subsequent amounts are served locally without a network call.
    // Empty maps ("no cost data" / swallowed failure) and amount-less requests are
    // never recorded, so a blip can't pin the cache to a bad model.
    if let Some(key) = cache_key {
        if !costs.is_empty() && amount > 0.0 {
            let observation: Observation = costs
                .iter()
                .map(|(gw, c)| (gw.clone(), (c.available, c.effective_cost_bps)))
                .collect();
            cost_cache().record(&key, amount, observation, cache_ttl);
        }
    }

    costs
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obs(amount: f64, pct_bps: f64, fixed: f64) -> (f64, f64) {
        // Mirrors the upstream blend: effective_cost_bps = pct + (fixed/amount)*1e4.
        (amount, pct_bps + (fixed / amount) * 10_000.0)
    }

    #[test]
    fn solves_split_from_two_amounts_matching_real_data() {
        // Stripe on the MC-credit cluster: pct = 225 bps, fixed = $0.15 — the exact
        // shape reconstructed from the live samples (226.9531 @ $768, 229.0107 @ $374).
        let (a1, c1) = obs(768.0, 225.0, 0.15);
        let (a2, c2) = obs(374.0, 225.0, 0.15);
        let o1 = HashMap::from([("stripe".to_string(), (true, c1))]);
        let o2 = HashMap::from([("stripe".to_string(), (true, c2))]);

        let models = solve_fee_models(a1, &o1, a2, &o2).expect("should solve");
        let m = models.get("stripe").unwrap();
        assert!((m.pct_bps - 225.0).abs() < 1e-6, "pct_bps = {}", m.pct_bps);
        assert!((m.fixed - 0.15).abs() < 1e-9, "fixed = {}", m.fixed);

        // And the solved model reproduces the cost at an unseen amount.
        let expected = 225.0 + (0.15 / 512.0) * 10_000.0;
        assert!((m.effective_cost_bps(512.0) - expected).abs() < 1e-6);
    }

    #[test]
    fn rejects_near_equal_amounts() {
        let (a1, c1) = obs(700.0, 210.0, 0.15);
        let (a2, c2) = obs(701.0, 210.0, 0.15); // < 1% apart → too noisy to trust
        let o1 = HashMap::from([("stripe".to_string(), (true, c1))]);
        let o2 = HashMap::from([("stripe".to_string(), (true, c2))]);
        assert!(solve_fee_models(a1, &o1, a2, &o2).is_none());
    }

    #[test]
    fn rejects_when_a_gateway_is_unavailable_in_either_probe() {
        let (a1, c1) = obs(768.0, 225.0, 0.15);
        let (a2, c2) = obs(374.0, 225.0, 0.15);
        let o1 = HashMap::from([("stripe".to_string(), (true, c1))]);
        let o2 = HashMap::from([("stripe".to_string(), (false, c2))]);
        assert!(solve_fee_models(a1, &o1, a2, &o2).is_none());
    }

    #[test]
    fn rejects_when_a_gateway_is_missing_from_one_probe() {
        let (a1, c1) = obs(768.0, 225.0, 0.15);
        let (a2, c2) = obs(374.0, 225.0, 0.15);
        let o1 = HashMap::from([
            ("stripe".to_string(), (true, c1)),
            ("adyen".to_string(), (true, c1 + 30.0)),
        ]);
        let o2 = HashMap::from([("stripe".to_string(), (true, c2))]);
        assert!(solve_fee_models(a1, &o1, a2, &o2).is_none());
    }

    // A solved `fixed` past the sanity cap is rejected. This is the exact failure that
    // mis-priced stripe: a non-linear curve collapsed into a multi-dollar flat fee that
    // exploded at small amounts. Two clean points implying fixed = $3.30 must not solve.
    #[test]
    fn rejects_implausibly_large_fixed_fee() {
        let (a1, c1) = obs(768.0, 275.0, 3.30);
        let (a2, c2) = obs(374.0, 275.0, 3.30);
        let o1 = HashMap::from([("stripe".to_string(), (true, c1))]);
        let o2 = HashMap::from([("stripe".to_string(), (true, c2))]);
        assert!(
            solve_fee_models(a1, &o1, a2, &o2).is_none(),
            "fixed = $3.30 exceeds MAX_PLAUSIBLE_FIXED and must be rejected"
        );
    }

    // One live observation at one amount: a single point can't separate pct from fixed,
    // so the fit must keep probing.
    fn pt(amount: f64, pct: f64, fixed: f64) -> (f64, Observation) {
        (
            amount,
            HashMap::from([(
                "stripe".to_string(),
                (true, pct + (fixed / amount) * 10_000.0),
            )]),
        )
    }

    // Three consistent linear samples confirm a single model, which then reproduces the
    // cost at the small amount (44) that the old two-point cache got catastrophically
    // wrong (1025 bps vs the true 321 bps).
    #[test]
    fn fit_accepts_three_consistent_linear_points() {
        let obs = vec![
            pt(768.0, 287.0, 0.15),
            pt(374.0, 287.0, 0.15),
            pt(120.0, 287.0, 0.15),
        ];
        let models = fit_from_observations(&obs).expect("consistent linear samples should solve");
        let m = models.get("stripe").unwrap();
        let expected = 287.0 + (0.15 / 44.0) * 10_000.0; // ≈ 321.09
        assert!(
            (m.effective_cost_bps(44.0) - expected).abs() < 1e-6,
            "got {} expected {}",
            m.effective_cost_bps(44.0),
            expected
        );
    }

    // The core fix: a held-out third point reveals non-linearity that a two-point solve
    // cannot. Extremes (120, 768) solve a clean line; the middle sample is bumped off it
    // (small-ticket-style curvature), so the fit is rejected and we fall back to live.
    #[test]
    fn fit_rejects_nonlinear_held_out_point() {
        let mut middle = pt(374.0, 287.0, 0.15);
        // Bump the middle observation 5 bps off the line through the two extremes.
        middle.1.insert(
            "stripe".to_string(),
            (true, 287.0 + (0.15 / 374.0) * 10_000.0 + 5.0),
        );
        let obs = vec![pt(768.0, 287.0, 0.15), middle, pt(120.0, 287.0, 0.15)];
        assert!(
            fit_from_observations(&obs).is_none(),
            "5 bps non-linearity at the held-out point must reject the fit"
        );
    }

    // Two points alone are never enough now — a third confirming sample is required so a
    // line (which fits its own two points exactly) can't be trusted on its own.
    #[test]
    fn fit_requires_a_confirming_third_point() {
        let obs = vec![pt(768.0, 287.0, 0.15), pt(374.0, 287.0, 0.15)];
        assert!(
            fit_from_observations(&obs).is_none(),
            "two points must keep probing until a third confirms the model"
        );
    }
}
