use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::decider::configs::env_vars::resolve_env;
use crate::logger;

use super::cluster_key::ClusterKey;

const TIMEOUT_MS: u64 = 30;

#[derive(Debug, Clone)]
pub struct PspCost {
    pub available: bool,
    pub effective_cost_bps: f64,
}

#[derive(Debug, Serialize)]
struct LookupRequest<'a> {
    merchant_id: &'a str,
    psps: &'a [String],
    cluster: &'a ClusterKey,
    as_of: String,
    fallback_to_parent_cluster: bool,
}

#[derive(Debug, Deserialize)]
struct LookupResponse {
    #[serde(default)]
    psps: Vec<PspRow>,
}

#[derive(Debug, Deserialize)]
struct PspRow {
    psp: String,
    available: bool,
    #[serde(default)]
    effective_cost_bps: Option<f64>,
}

fn base_url() -> String {
    resolve_env("HYPERSENSE_BASE_URL".to_string(), || {
        "http://localhost:4000".to_string()
    })
}

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .pool_idle_timeout(Some(Duration::from_secs(30)))
            .timeout(Duration::from_millis(TIMEOUT_MS))
            .build()
            .expect("failed to build hypersense reqwest client")
    })
}

/// Best-effort cost lookup. Returns an empty map on any failure; the algorithm
/// treats an empty map as "no PSP has cost data" and short-circuits to no-op.
pub async fn lookup_costs(
    merchant_id: &str,
    cluster: &ClusterKey,
    psps: &[String],
) -> HashMap<String, PspCost> {
    if psps.is_empty() {
        return HashMap::new();
    }

    let url = format!("{}/v1/cost/lookup", base_url().trim_end_matches('/'));
    let req = LookupRequest {
        merchant_id,
        psps,
        cluster,
        as_of: time::OffsetDateTime::now_utc()
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap_or_default(),
        fallback_to_parent_cluster: true,
    };

    let fut = client().post(&url).json(&req).send();
    let result = tokio::time::timeout(Duration::from_millis(TIMEOUT_MS), fut).await;

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
                TIMEOUT_MS,
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

    let body = match response.json::<LookupResponse>().await {
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

    body.psps
        .into_iter()
        .map(|row| {
            (
                row.psp,
                PspCost {
                    available: row.available,
                    effective_cost_bps: row.effective_cost_bps.unwrap_or(0.0),
                },
            )
        })
        .collect()
}
