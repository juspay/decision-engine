//! Manual per-connector blended-fee overrides.
//!
//! When a merchant sets a blended fee for a connector, that flat `{pct_bps, fixed}` replaces the
//! learned PAR model for *every* EV calculation on that connector (see [`serving::lookup`], which
//! checks overrides first). This is the "contract-terms overlay" path from the architecture doc
//! §3.3/§6.1 — the way a connector with no ingested settlement report (e.g. Stripe) still gets a
//! cost so EV can rank it.
//!
//! Like connector credentials ([`super::creds`]), overrides live in the generic
//! `service_configuration` key-value store rather than a dedicated table. Two small indices make
//! listing cheap without scanning: a per-merchant list of connectors, and a global list of
//! merchants that have any override (so the periodic serving refresh can hydrate them all).

use serde::{Deserialize, Serialize};

use crate::types::service_configuration;

use super::types::IngestError;

/// A merchant-authored blended fee for one connector. `pct_bps` is the percentage rate in basis
/// points; `fixed` is the per-transaction flat fee (report currency units).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FeeOverride {
    pub pct_bps: f64,
    pub fixed: f64,
    /// RFC3339 timestamp of the last edit (for the dashboard "edited …" line).
    pub updated_at: String,
}

/// The seven dimensions that identify one fitted cost segment (mirrors `cost_fee_model`'s cluster key
/// and the serving `fine_key`). Includes `variant` (card program/tier) so an override targets exactly
/// the product a merchant picked — two products in the same category are distinct override targets.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClusterDims {
    pub connector: String,
    pub card_network: String,
    pub variant: String,
    pub funding: String,
    pub issuer_country: String,
    pub currency: String,
    pub ic_category: String,
}

impl ClusterDims {
    /// Parse the `connector|network|variant|funding|issuer|currency|ic_category` key used on the wire
    /// (URL path). `ic_category` may legitimately be empty (flat-fee clusters), so we split on exactly
    /// the seven fields and allow a trailing empty last segment.
    pub fn from_key(key: &str) -> Option<Self> {
        let p: Vec<&str> = key.split('|').collect();
        if p.len() != 7 {
            return None;
        }
        Some(Self {
            connector: p[0].to_lowercase(),
            card_network: p[1].to_lowercase(),
            variant: p[2].to_lowercase(),
            funding: p[3].to_lowercase(),
            issuer_country: p[4].to_lowercase(),
            currency: p[5].to_lowercase(),
            ic_category: p[6].to_lowercase(),
        })
    }
}

/// A merchant-authored fee for one specific fitted cluster. Highest precedence at lookup (see
/// `serving::lookup`): cluster override > connector override > learned model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterOverride {
    #[serde(flatten)]
    pub dims: ClusterDims,
    pub pct_bps: f64,
    pub fixed: f64,
    pub updated_at: String,
}

fn override_name(merchant_id: &str, connector: &str) -> String {
    format!("cost_fee_override::{merchant_id}::{connector}")
}

fn merchant_index_name(merchant_id: &str) -> String {
    format!("cost_fee_override_index::{merchant_id}")
}

/// Per-merchant cluster overrides live in one JSON array (capped small — the dashboard only exposes
/// the top clusters), keyed by the cluster dims, so no per-cluster config key or index is needed.
fn cluster_overrides_name(merchant_id: &str) -> String {
    format!("cost_cluster_overrides::{merchant_id}")
}

/// Names the global list of merchants that currently have at least one override (connector *or*
/// cluster). Lets the periodic (all-merchant) serving refresh hydrate override-only merchants that
/// never appear in ClickHouse.
const GLOBAL_INDEX_NAME: &str = "cost_fee_override_merchants";

// ── generic JSON get/set over service_configuration ───────────────────────────────────────────

async fn read_json<T: for<'de> Deserialize<'de>>(name: String) -> Result<Option<T>, IngestError> {
    let stored = service_configuration::find_config_by_name(name)
        .await
        .map_err(|e| IngestError::Storage(e.to_string()))?;
    match stored.and_then(|c| c.value) {
        Some(v) => serde_json::from_str(&v)
            .map(Some)
            .map_err(|e| IngestError::Storage(e.to_string())),
        None => Ok(None),
    }
}

async fn write_json<T: Serialize>(name: String, value: &T) -> Result<(), IngestError> {
    let serialized =
        serde_json::to_string(value).map_err(|e| IngestError::Storage(e.to_string()))?;
    let exists = service_configuration::find_config_by_name(name.clone())
        .await
        .map_err(|e| IngestError::Storage(e.to_string()))?
        .is_some();
    if exists {
        service_configuration::update_config(name, Some(serialized)).await
    } else {
        service_configuration::insert_config(name, Some(serialized)).await
    }
    .map_err(|e| IngestError::Storage(e.to_string()))
}

// ── index maintenance ─────────────────────────────────────────────────────────────────────────

async fn read_list(name: String) -> Result<Vec<String>, IngestError> {
    Ok(read_json::<Vec<String>>(name).await?.unwrap_or_default())
}

/// Add `item` to the JSON string-list at `name` (idempotent).
async fn index_add(name: String, item: &str) -> Result<(), IngestError> {
    let mut list = read_list(name.clone()).await?;
    if list.iter().any(|c| c == item) {
        return Ok(());
    }
    list.push(item.to_string());
    write_json(name, &list).await
}

/// Remove `item` from the JSON string-list at `name`; returns the remaining length.
async fn index_remove(name: String, item: &str) -> Result<usize, IngestError> {
    let mut list = read_list(name.clone()).await?;
    list.retain(|c| c != item);
    let remaining = list.len();
    write_json(name, &list).await?;
    Ok(remaining)
}

// ── public API ────────────────────────────────────────────────────────────────────────────────

/// The connectors a merchant has overrides for (from its index).
pub async fn list_connectors(merchant_id: &str) -> Result<Vec<String>, IngestError> {
    read_list(merchant_index_name(merchant_id)).await
}

/// All `(connector, override)` a merchant has configured.
pub async fn list(merchant_id: &str) -> Result<Vec<(String, FeeOverride)>, IngestError> {
    let connectors = list_connectors(merchant_id).await?;
    let mut out = Vec::with_capacity(connectors.len());
    for connector in connectors {
        if let Some(ov) = get(merchant_id, &connector).await? {
            out.push((connector, ov));
        }
    }
    Ok(out)
}

/// Merchants that currently have at least one override (global index).
pub async fn list_merchants() -> Result<Vec<String>, IngestError> {
    read_list(GLOBAL_INDEX_NAME.to_string()).await
}

/// The override for one `(merchant, connector)`, if set.
pub async fn get(merchant_id: &str, connector: &str) -> Result<Option<FeeOverride>, IngestError> {
    read_json::<FeeOverride>(override_name(merchant_id, connector)).await
}

/// Upsert a merchant's blended-fee override for a connector, and record it in both indices.
pub async fn put(
    merchant_id: &str,
    connector: &str,
    ov: &FeeOverride,
) -> Result<(), IngestError> {
    write_json(override_name(merchant_id, connector), ov).await?;
    index_add(merchant_index_name(merchant_id), connector).await?;
    index_add(GLOBAL_INDEX_NAME.to_string(), merchant_id).await?;
    Ok(())
}

/// Remove a merchant's override for a connector; drops the merchant from the global index once it
/// has no overrides of *either* kind left, so the periodic refresh stops hydrating it.
pub async fn delete(merchant_id: &str, connector: &str) -> Result<(), IngestError> {
    service_configuration::delete_config(override_name(merchant_id, connector))
        .await
        .map_err(|e| IngestError::Storage(e.to_string()))?;
    index_remove(merchant_index_name(merchant_id), connector).await?;
    prune_global_index(merchant_id).await
}

/// Drop a merchant from the global override index once it has neither a connector nor a cluster
/// override remaining.
async fn prune_global_index(merchant_id: &str) -> Result<(), IngestError> {
    let connectors = list_connectors(merchant_id).await?;
    let clusters = list_clusters(merchant_id).await?;
    if connectors.is_empty() && clusters.is_empty() {
        index_remove(GLOBAL_INDEX_NAME.to_string(), merchant_id).await?;
    }
    Ok(())
}

// ── cluster overrides ─────────────────────────────────────────────────────────────────────────

/// A merchant's cluster overrides (identity is the dims).
pub async fn list_clusters(merchant_id: &str) -> Result<Vec<ClusterOverride>, IngestError> {
    Ok(read_json::<Vec<ClusterOverride>>(cluster_overrides_name(merchant_id))
        .await?
        .unwrap_or_default())
}

/// Upsert a cluster override (replaces any existing one with the same dims).
pub async fn put_cluster(merchant_id: &str, ov: &ClusterOverride) -> Result<(), IngestError> {
    let mut list = list_clusters(merchant_id).await?;
    list.retain(|c| c.dims != ov.dims);
    list.push(ov.clone());
    write_json(cluster_overrides_name(merchant_id), &list).await?;
    index_add(GLOBAL_INDEX_NAME.to_string(), merchant_id).await?;
    Ok(())
}

/// Remove the cluster override matching `dims`; prunes the global index when nothing is left.
pub async fn delete_cluster(merchant_id: &str, dims: &ClusterDims) -> Result<(), IngestError> {
    let mut list = list_clusters(merchant_id).await?;
    let before = list.len();
    list.retain(|c| &c.dims != dims);
    if list.len() != before {
        write_json(cluster_overrides_name(merchant_id), &list).await?;
    }
    prune_global_index(merchant_id).await
}
