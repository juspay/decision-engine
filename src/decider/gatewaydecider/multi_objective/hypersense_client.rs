use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Duration;

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
    let mut request = cluster.clone();
    request.psp_array = psps.to_vec();

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

    body.into_iter()
        .map(|row| {
            (
                row.gateway,
                PspCost {
                    available: row.available,
                    effective_cost_bps: row.effective_cost_bps.unwrap_or(0.0),
                },
            )
        })
        .collect()
}
