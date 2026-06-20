use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use masking::PeekInterface;
use serde::{Deserialize, Serialize};

use crate::app::{get_tenant_app_state, TenantAppState};
use crate::config::HypersenseConfig;
use crate::logger;

use super::cluster_key::ClusterKey;

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
    /// One observation so far — we still need a second at a sufficiently different
    /// amount to separate the percentage and fixed components.
    Probing { amount: f64, rows: Observation },
    /// Solved amount-independent model per gateway; serves all amounts locally.
    Solved(HashMap<String, FeeModel>),
}

/// Solves the `{pct_bps, fixed}` split for every gateway from two observations at
/// distinct amounts. Returns `None` (keep probing live) unless *every* gateway is
/// available in both samples and yields a plausible, non-negative fit — we never
/// want to pin a half-trusted or implausible model into the cache.
fn solve_fee_models(a1: f64, obs1: &Observation, a2: f64, obs2: &Observation) -> Option<HashMap<String, FeeModel>> {
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
        let fixed = if (-1e-6..0.0).contains(&fixed) { 0.0 } else { fixed };
        let pct_bps = if (-1e-6..0.0).contains(&pct_bps) { 0.0 } else { pct_bps };
        if fixed < 0.0 || pct_bps < 0.0 || pct_bps > MAX_PLAUSIBLE_PCT_BPS {
            return None;
        }
        models.insert(gw.clone(), FeeModel { available: true, pct_bps, fixed });
    }
    Some(models)
}

/// Temporary, best-effort in-process cache that fronts the fee-rate endpoint by
/// caching the *fee model* (percentage + fixed split), not the per-amount response.
///
/// Conservative by construction: a scenario is only served from cache once its
/// split has been solved from two consistent live observations; transient failures
/// and partial/unavailable scenarios are never cached, and entries expire past TTL.
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
                            },
                        )
                    })
                    .collect(),
            ),
            ScenarioCost::Probing { .. } => None,
        }
    }

    /// Folds a fresh live observation into the cache: solves and stores the model if
    /// a usable prior probe (distinct amount, same scenario) exists, otherwise keeps
    /// this observation as the pending probe. Skips silently on lock contention and
    /// never downgrades an already-solved entry.
    fn record(&self, key: &str, amount: f64, rows: Observation, ttl: Duration) {
        let Ok(mut data) = self.data.try_lock() else {
            return;
        };
        let now = Instant::now();

        // Try to solve against an existing, unexpired probe for this scenario.
        if let Some((state, stored_at)) = data.get(key) {
            if stored_at.elapsed() < ttl {
                match state {
                    // Already solved — leave it; it serves all amounts.
                    ScenarioCost::Solved(_) => return,
                    ScenarioCost::Probing { amount: prior_amount, rows: prior_rows } => {
                        if let Some(models) = solve_fee_models(*prior_amount, prior_rows, amount, &rows) {
                            data.insert(key.to_string(), (ScenarioCost::Solved(models), now));
                            return;
                        }
                    }
                }
            }
        }

        // No usable prior probe yet — store this observation as the pending probe.
        self.evict_if_full(&mut data, key);
        data.insert(key.to_string(), (ScenarioCost::Probing { amount, rows }, now));
    }

    fn evict_if_full(&self, data: &mut HashMap<String, (ScenarioCost, Instant)>, incoming_key: &str) {
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

/// Best-effort cost lookup. Returns an empty map on any failure; the algorithm
/// treats an empty map as "no PSP has cost data" and short-circuits to no-op.
///
/// Flow: resolve an access token (Redis-cached, minted via the sign-in API on
/// miss), then call the fee-rate-estimate API with it for the candidate PSPs.
pub async fn lookup_costs(
    merchant_id: &str,
    cluster: &ClusterKey,
    psps: &[String],
) -> HashMap<String, PspCost> {
    if psps.is_empty() {
        return HashMap::new();
    }

    let app_state = get_tenant_app_state().await;
    let cfg = &app_state.config.hypersense;

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

    let mut body = match response.json::<Vec<FeeRateRow>>().await {
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

    for row in body.iter_mut() {
        row.gateway = match row.gateway.as_str() {
            "stripe" => "adyen".to_string(),
            "adyen" => "stripe".to_string(),
            other => other.to_string(),
        };
    }

    let costs: HashMap<String, PspCost> = body
        .into_iter()
        .map(|row| {
            (
                row.gateway,
                PspCost {
                    available: row.available,
                    effective_cost_bps: row.effective_cost_bps.unwrap_or(0.0),
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
}
