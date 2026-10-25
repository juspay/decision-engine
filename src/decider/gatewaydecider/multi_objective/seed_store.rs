//! Per-merchant seed cost tables for the multi-objective simulator.
//!
//! The global `config.hypersense.seed_costs` table is the default pricing every merchant starts
//! from. When a merchant edits their contract rates in the dashboard (Cost estimation → Contract
//! rates), the edited `Vec<SeedCostEntry>` is stored per-merchant in the generic
//! `service_configuration` key-value store — the same mechanism the connector/cluster fee
//! overrides use ([`crate::cost_ingestion::overrides`]) — and supersedes the config default at
//! decide / simulate time.
//!
//! Seed costs are only consulted when `use_seed_costs = true` (simulator / offline), so reading the
//! merchant's table here is not on the production routing hot path.

use crate::config::{HypersenseConfig, SeedCostEntry};
use crate::error::ErrorMessage;
use crate::logger;
use crate::types::service_configuration;
use error_stack::{Report, Result};

/// Config-store key holding one merchant's edited seed table (JSON `Vec<SeedCostEntry>`).
fn seed_costs_name(merchant_id: &str) -> String {
    format!("cost_seed_costs::{merchant_id}")
}

/// The merchant's stored seed table, if they have saved one. Returns `None` on any miss (never
/// stored, empty value, or a parse/store failure) so the caller falls back to the config default.
pub async fn get_seed_table(merchant_id: &str) -> Option<Vec<SeedCostEntry>> {
    let stored = service_configuration::find_config_by_name(seed_costs_name(merchant_id))
        .await
        .inspect_err(|error| {
            logger::error!(error = ?error, "Seed cost lookup failed; retaining config fallback");
        })
        .ok()?;
    let value = stored?.value?;
    serde_json::from_str(&value)
        .inspect_err(|error| {
            logger::error!(error = ?error, "Seed cost parsing failed; retaining config fallback");
        })
        .ok()
}

/// Upsert (create or replace) the merchant's seed table.
pub async fn put_seed_table(
    merchant_id: &str,
    table: &[SeedCostEntry],
) -> Result<(), ErrorMessage> {
    let serialized = serde_json::to_string(table).map_err(|error| {
        let context = ErrorMessage(error.to_string());
        Report::from(error).change_context(context)
    })?;
    let name = seed_costs_name(merchant_id);
    let exists = service_configuration::find_config_by_name(name.clone())
        .await
        .map_err(|error| {
            let context = ErrorMessage(error.to_string());
            Report::from(error).change_context(context)
        })?
        .is_some();
    if exists {
        service_configuration::update_config(name, Some(serialized)).await
    } else {
        service_configuration::insert_config(name, Some(serialized)).await
    }
    .map_err(|error| {
        let context = ErrorMessage(error.to_string());
        Report::from(error).change_context(context)
    })
}

/// Reset the merchant to the config default by removing any stored table.
pub async fn delete_seed_table(merchant_id: &str) -> Result<(), ErrorMessage> {
    service_configuration::delete_config(seed_costs_name(merchant_id))
        .await
        .map_err(|error| {
            let context = ErrorMessage(error.to_string());
            Report::from(error).change_context(context)
        })
}

/// The seed entries in effect for a merchant: their stored table when present and non-empty, else
/// the global config default. This is the single resolver both the decide path
/// ([`super::hypersense_client`]) and the simulation-preview route consult, so the two never drift.
pub async fn effective_seed_entries(
    merchant_id: &str,
    cfg: &HypersenseConfig,
) -> Vec<SeedCostEntry> {
    match get_seed_table(merchant_id).await {
        Some(table) if !table.is_empty() => table,
        _ => cfg.seed_costs.clone(),
    }
}
