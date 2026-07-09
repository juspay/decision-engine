//! Persistence for the invoice-derived cost add-on.
//!
//! Like the manual fee overrides ([`crate::cost_ingestion::overrides`]) — and for the same reasons
//! (it is one tiny row per `(merchant, connector)`, computed in Rust, layered at serving time) — the
//! add-on lives in the generic `service_configuration` key-value store, not a dedicated ClickHouse
//! table. Two indices keep listing cheap: a per-merchant connector list, and a global merchant list
//! so the periodic serving refresh can hydrate every merchant that has an add-on.
//!
//! The stored record keeps the invoice provenance (ref, subtotal, volume, period) alongside the
//! two served parameters so reconciliation ([`super::reconcile`]) and the dashboard can show *why*
//! the add-on is what it is, without re-parsing the invoice.

use serde::{Deserialize, Serialize};

use crate::cost_ingestion::types::IngestError;
use crate::types::service_configuration;

use super::types::{CostAddon, InvoiceSummary};

/// A stored invoice add-on: the two served parameters plus the provenance to reconcile/display it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StoredAddon {
    /// Amortized periodic-fee rate (bps) added to each learned cluster's `pct_bps`.
    pub pct_addon_bps: f64,
    /// Flat per-transaction fee (invoice currency) added to each learned cluster's `fixed`.
    pub fixed_addon: f64,
    /// Invoice number this add-on was derived from (idempotency / audit).
    pub invoice_ref: String,
    /// Invoice subtotal excluding taxes — the "true all-in cost" reconciliation ties back to.
    pub subtotal_ex_tax: Option<f64>,
    /// Card turnover the periodic fees were amortized over.
    pub card_volume: Option<f64>,
    /// Settled transaction count the flat fees were blended over.
    pub txn_count: Option<u64>,
    pub currency: String,
    pub period_start: Option<String>,
    pub period_end: Option<String>,
    /// RFC3339 timestamp of the last ingest (for the dashboard "updated …" line).
    pub updated_at: String,
}

impl StoredAddon {
    /// The served two-parameter view (what the serving overlay actually adds).
    pub fn addon(&self) -> CostAddon {
        CostAddon { pct_addon_bps: self.pct_addon_bps, fixed_addon: self.fixed_addon }
    }

    /// Build from a computed add-on + the invoice summary + a timestamp (RFC3339, passed in because
    /// the wall clock is sourced by the caller).
    pub fn new(addon: CostAddon, summary: &InvoiceSummary, updated_at: String) -> Self {
        StoredAddon {
            pct_addon_bps: addon.pct_addon_bps,
            fixed_addon: addon.fixed_addon,
            invoice_ref: summary.invoice_ref.clone(),
            subtotal_ex_tax: summary.subtotal_ex_tax,
            card_volume: summary.card_volume,
            txn_count: summary.txn_count,
            currency: summary.currency.clone(),
            period_start: summary.period_start.map(|d| d.to_string()),
            period_end: summary.period_end.map(|d| d.to_string()),
            updated_at,
        }
    }
}

fn addon_name(merchant_id: &str, connector: &str) -> String {
    format!("cost_invoice_addon::{merchant_id}::{connector}")
}

fn merchant_index_name(merchant_id: &str) -> String {
    format!("cost_invoice_addon_index::{merchant_id}")
}

/// Global list of merchants that have at least one invoice add-on, so the periodic (all-merchant)
/// serving refresh can hydrate add-on-only merchants (ones with no ClickHouse cost data yet).
const GLOBAL_INDEX_NAME: &str = "cost_invoice_addon_merchants";

// ── generic JSON get/set over service_configuration (same pattern as overrides) ─────────────────

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

async fn read_list(name: String) -> Result<Vec<String>, IngestError> {
    Ok(read_json::<Vec<String>>(name).await?.unwrap_or_default())
}

async fn index_add(name: String, item: &str) -> Result<(), IngestError> {
    let mut list = read_list(name.clone()).await?;
    if list.iter().any(|c| c == item) {
        return Ok(());
    }
    list.push(item.to_string());
    write_json(name, &list).await
}

async fn index_remove(name: String, item: &str) -> Result<usize, IngestError> {
    let mut list = read_list(name.clone()).await?;
    list.retain(|c| c != item);
    let remaining = list.len();
    write_json(name, &list).await?;
    Ok(remaining)
}

// ── public API ──────────────────────────────────────────────────────────────────────────────────

/// The connectors a merchant has an invoice add-on for.
pub async fn list_connectors(merchant_id: &str) -> Result<Vec<String>, IngestError> {
    read_list(merchant_index_name(merchant_id)).await
}

/// The add-on for one `(merchant, connector)`, if set.
pub async fn get(merchant_id: &str, connector: &str) -> Result<Option<StoredAddon>, IngestError> {
    read_json::<StoredAddon>(addon_name(merchant_id, connector)).await
}

/// All `(connector, add-on)` a merchant has.
pub async fn list(merchant_id: &str) -> Result<Vec<(String, StoredAddon)>, IngestError> {
    let connectors = list_connectors(merchant_id).await?;
    let mut out = Vec::with_capacity(connectors.len());
    for connector in connectors {
        if let Some(a) = get(merchant_id, &connector).await? {
            out.push((connector, a));
        }
    }
    Ok(out)
}

/// Merchants that currently have at least one invoice add-on (global index).
pub async fn list_merchants() -> Result<Vec<String>, IngestError> {
    read_list(GLOBAL_INDEX_NAME.to_string()).await
}

/// Upsert a merchant's invoice add-on for a connector, recording it in both indices.
pub async fn put(merchant_id: &str, connector: &str, addon: &StoredAddon) -> Result<(), IngestError> {
    let connector = connector.to_lowercase();
    write_json(addon_name(merchant_id, &connector), addon).await?;
    index_add(merchant_index_name(merchant_id), &connector).await?;
    index_add(GLOBAL_INDEX_NAME.to_string(), merchant_id).await?;
    Ok(())
}

/// Remove a merchant's add-on for a connector; drops it from the global index once none remain.
pub async fn delete(merchant_id: &str, connector: &str) -> Result<(), IngestError> {
    let connector = connector.to_lowercase();
    service_configuration::delete_config(addon_name(merchant_id, &connector))
        .await
        .map_err(|e| IngestError::Storage(e.to_string()))?;
    let remaining = index_remove(merchant_index_name(merchant_id), &connector).await?;
    if remaining == 0 {
        index_remove(GLOBAL_INDEX_NAME.to_string(), merchant_id).await?;
    }
    Ok(())
}
