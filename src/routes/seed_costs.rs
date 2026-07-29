//! Merchant-facing API for the multi-objective simulator's seed cost table (contract rates).
//!
//! The seed table prices candidate PSPs when `use_seed_costs = true` (simulator / offline). Each
//! merchant starts from the global config default (`config.hypersense.seed_costs`) and can edit it
//! to match their acquirer contracts — the edited table is stored per-merchant
//! ([`crate::decider::gatewaydecider::multi_objective::seed_store`]) and drives both the live
//! Decision Simulator and the lightweight cost preview below.
//!
//! On the wire the table is a flat list of [`SeedCostRow`]s (one PSP × one scenario), which maps
//! cleanly onto the dashboard's editable table. Rows are (de)normalized to/from the nested
//! `Vec<SeedCostEntry>` the resolver consumes.

use axum::extract::Path;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::app::get_tenant_app_state;
use crate::config::{fee_components, SeedCostEntry, SeedCostTier, SeedFeeModel};
use crate::decider::gatewaydecider::multi_objective::cluster_key::ClusterKey;
use crate::decider::gatewaydecider::multi_objective::{seed_costs, seed_store};

/// One row of the merchant's seed table: a PSP's fee for one card scenario, split into the contract
/// components (interchange / scheme / markup, all bps) plus the flat `fixed` per-transaction fee.
/// The matching dimensions (`card_network` … `card_issuing_country`) mirror the seed tier fields; a
/// `None`/empty dimension is a wildcard. `is_default` marks the PSP's fallback row (all dims empty).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SeedCostRow {
    pub psp: String,
    #[serde(default)]
    pub card_network: Option<String>,
    #[serde(default)]
    pub payment_method_type: Option<String>,
    #[serde(default)]
    pub card_type: Option<String>,
    #[serde(default)]
    pub transaction_currency: Option<String>,
    #[serde(default)]
    pub card_issuing_country: Option<String>,
    pub interchange_bps: f64,
    pub scheme_bps: f64,
    pub markup_bps: f64,
    pub fixed: f64,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub example_amount: Option<f64>,
    /// True for the PSP's fallback default row (no matching dimensions).
    #[serde(default)]
    pub is_default: bool,
    /// Computed Total Effective Rate percentage = interchange + scheme + markup. Output only.
    #[serde(default)]
    pub effective_pct_bps: f64,
}

/// Expand the nested seed entries into flat editable rows: one `is_default` row per PSP plus one
/// row per tier. The effective percentage is computed for display.
fn entries_to_rows(entries: &[SeedCostEntry]) -> Vec<SeedCostRow> {
    let mut rows = Vec::new();
    for e in entries {
        let (i, s, m) = fee_components(
            e.default.pct_bps,
            e.default.interchange_bps,
            e.default.scheme_bps,
            e.default.markup_bps,
        );
        rows.push(SeedCostRow {
            psp: e.psp.clone(),
            card_network: None,
            payment_method_type: None,
            card_type: None,
            transaction_currency: None,
            card_issuing_country: None,
            interchange_bps: i,
            scheme_bps: s,
            markup_bps: m,
            fixed: e.default.fixed,
            label: Some("Default (all other cards)".to_string()),
            example_amount: None,
            is_default: true,
            effective_pct_bps: i + s + m,
        });
        for t in &e.tiers {
            let (i, s, m) =
                fee_components(t.pct_bps, t.interchange_bps, t.scheme_bps, t.markup_bps);
            rows.push(SeedCostRow {
                psp: e.psp.clone(),
                card_network: t.card_network.clone(),
                payment_method_type: t.payment_method_type.clone(),
                card_type: t.card_type.clone(),
                transaction_currency: t.transaction_currency.clone(),
                card_issuing_country: t.card_issuing_country.clone(),
                interchange_bps: i,
                scheme_bps: s,
                markup_bps: m,
                fixed: t.fixed,
                label: t.label.clone(),
                example_amount: t.example_amount,
                is_default: false,
                effective_pct_bps: i + s + m,
            });
        }
    }
    rows
}

/// Trim to a wildcard: an empty/whitespace dimension becomes `None` (matches any).
fn norm(s: Option<String>) -> Option<String> {
    s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

/// Rebuild the nested seed entries from flat rows. Rows are grouped by PSP (first-seen order
/// preserved); the `is_default` row becomes the entry's `default`, the rest become tiers. A PSP
/// with no default row gets a zeroed default (it then only prices scenarios it explicitly lists).
fn rows_to_entries(rows: Vec<SeedCostRow>) -> Vec<SeedCostEntry> {
    use std::collections::HashMap;
    let mut order: Vec<String> = Vec::new();
    let mut by_psp: HashMap<String, (Option<SeedFeeModel>, Vec<SeedCostTier>)> = HashMap::new();

    for r in rows {
        if !by_psp.contains_key(&r.psp) {
            order.push(r.psp.clone());
        }
        let entry = by_psp.entry(r.psp.clone()).or_insert((None, Vec::new()));
        if r.is_default {
            entry.0 = Some(SeedFeeModel {
                pct_bps: 0.0,
                fixed: r.fixed,
                interchange_bps: Some(r.interchange_bps),
                scheme_bps: Some(r.scheme_bps),
                markup_bps: Some(r.markup_bps),
            });
        } else {
            entry.1.push(SeedCostTier {
                card_network: norm(r.card_network),
                payment_method_type: norm(r.payment_method_type),
                card_type: norm(r.card_type),
                transaction_currency: norm(r.transaction_currency),
                card_issuing_country: norm(r.card_issuing_country),
                pct_bps: 0.0,
                fixed: r.fixed,
                interchange_bps: Some(r.interchange_bps),
                scheme_bps: Some(r.scheme_bps),
                markup_bps: Some(r.markup_bps),
                label: r.label,
                example_amount: r.example_amount,
            });
        }
    }

    order
        .into_iter()
        .filter_map(|psp| {
            let (default, tiers) = by_psp.remove(&psp)?;
            Some(SeedCostEntry {
                psp,
                default: default.unwrap_or(SeedFeeModel {
                    fixed: 0.0,
                    interchange_bps: Some(0.0),
                    scheme_bps: Some(0.0),
                    markup_bps: Some(0.0),
                    ..Default::default()
                }),
                tiers,
            })
        })
        .collect()
}

/// `GET /merchant-account/:merchant-id/seed-costs` — the merchant's editable contract-rate table
/// (their saved edits, else the config default).
pub async fn get_seed_costs(
    Path(merchant_id): Path<String>,
) -> Result<Json<Vec<SeedCostRow>>, (StatusCode, String)> {
    let app_state = get_tenant_app_state().await;
    let entries =
        seed_store::effective_seed_entries(&merchant_id, &app_state.config.hypersense).await;
    Ok(Json(entries_to_rows(&entries)))
}

#[derive(Debug, Deserialize)]
pub struct SaveSeedCostsRequest {
    pub rows: Vec<SeedCostRow>,
}

/// `PUT /merchant-account/:merchant-id/seed-costs` — replace the merchant's whole table.
pub async fn set_seed_costs(
    Path(merchant_id): Path<String>,
    Json(body): Json<SaveSeedCostsRequest>,
) -> Result<Json<Vec<SeedCostRow>>, (StatusCode, String)> {
    for r in &body.rows {
        if r.psp.trim().is_empty() {
            return Err((
                StatusCode::BAD_REQUEST,
                "each row must name a PSP".to_string(),
            ));
        }
        for v in [r.interchange_bps, r.scheme_bps, r.markup_bps, r.fixed] {
            if !v.is_finite() || v < 0.0 {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "fee components and fixed fee must be finite and non-negative".to_string(),
                ));
            }
        }
    }
    let entries = rows_to_entries(body.rows);
    seed_store::put_seed_table(&merchant_id, &entries)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(entries_to_rows(&entries)))
}

/// `DELETE /merchant-account/:merchant-id/seed-costs` — clear edits, reverting to the config
/// default; returns the default table.
pub async fn delete_seed_costs(
    Path(merchant_id): Path<String>,
) -> Result<Json<Vec<SeedCostRow>>, (StatusCode, String)> {
    seed_store::delete_seed_table(&merchant_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let app_state = get_tenant_app_state().await;
    let entries =
        seed_store::effective_seed_entries(&merchant_id, &app_state.config.hypersense).await;
    Ok(Json(entries_to_rows(&entries)))
}

#[derive(Debug, Deserialize)]
pub struct SimulateRequest {
    pub amount: f64,
    #[serde(default)]
    pub transaction_currency: Option<String>,
    #[serde(default)]
    pub card_network: Option<String>,
    #[serde(default)]
    pub payment_method_type: Option<String>,
    #[serde(default)]
    pub card_type: Option<String>,
    #[serde(default)]
    pub card_issuing_country: Option<String>,
    /// PSPs to price; empty ⇒ every PSP in the merchant's table.
    #[serde(default)]
    pub psps: Vec<String>,
}

/// One PSP's estimated cost for the simulated scenario, from the merchant's configured values.
#[derive(Debug, Serialize)]
pub struct SimulateRow {
    pub psp: String,
    pub interchange_bps: f64,
    pub scheme_bps: f64,
    pub markup_bps: f64,
    pub fixed: f64,
    /// Total Effective Rate percentage = interchange + scheme + markup.
    pub effective_pct_bps: f64,
    /// All-in effective rate at this amount = pct + fixed/amount·10_000.
    pub effective_cost_bps: f64,
    /// Absolute cost in the transaction currency at this amount.
    pub cost_amount: f64,
}

/// `POST /merchant-account/:merchant-id/seed-costs/simulate` — price the candidate PSPs for one
/// (currency, scenario, amount) using the merchant's configured seed table, via the exact tier
/// resolver the decide path uses. This is the lightweight cost preview beside the editor.
pub async fn simulate_seed_costs(
    Path(merchant_id): Path<String>,
    Json(body): Json<SimulateRequest>,
) -> Result<Json<Vec<SimulateRow>>, (StatusCode, String)> {
    if !body.amount.is_finite() || body.amount <= 0.0 {
        return Err((
            StatusCode::BAD_REQUEST,
            "amount must be a positive number".to_string(),
        ));
    }
    let app_state = get_tenant_app_state().await;
    let entries =
        seed_store::effective_seed_entries(&merchant_id, &app_state.config.hypersense).await;

    let cluster = ClusterKey {
        amount: Some(body.amount),
        transaction_currency: norm(body.transaction_currency),
        payment_method_type: norm(body.payment_method_type),
        card_type: norm(body.card_type),
        card_network: norm(body.card_network),
        card_issuing_country: norm(body.card_issuing_country),
        ..Default::default()
    };

    // Requested PSPs, else every PSP in the table (preserving table order).
    let psps: Vec<String> = if body.psps.is_empty() {
        entries.iter().map(|e| e.psp.clone()).collect()
    } else {
        body.psps.clone()
    };

    let out: Vec<SimulateRow> = psps
        .iter()
        .filter_map(|psp| {
            let entry = entries.iter().find(|e| e.psp.eq_ignore_ascii_case(psp))?;
            let fee = seed_costs::resolve_fee(entry, &cluster);
            let pct = fee.pct_bps();
            let effective_cost_bps = pct + (fee.fixed / body.amount) * 10_000.0;
            Some(SimulateRow {
                psp: psp.clone(),
                interchange_bps: fee.interchange_bps,
                scheme_bps: fee.scheme_bps,
                markup_bps: fee.markup_bps,
                fixed: fee.fixed,
                effective_pct_bps: pct,
                effective_cost_bps,
                cost_amount: (effective_cost_bps / 10_000.0) * body.amount,
            })
        })
        .collect();

    Ok(Json(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(psp: &str, card_type: Option<&str>) -> SeedCostRow {
        SeedCostRow {
            psp: psp.to_string(),
            card_network: Some("visa".to_string()),
            payment_method_type: Some("credit".to_string()),
            card_type: card_type.map(str::to_string),
            transaction_currency: None,
            card_issuing_country: Some("us".to_string()),
            interchange_bps: 200.0,
            scheme_bps: 13.0,
            markup_bps: 9.0,
            fixed: 0.24,
            label: Some("US Visa credit".to_string()),
            example_amount: None,
            is_default: false,
            effective_pct_bps: 0.0,
        }
    }

    /// The card program must survive the flat-row ⇄ nested-entry round trip the editor saves
    /// through. Interchange varies severalfold across programs (config: standard 222 / premium 317 /
    /// commercial 334 bps on the *same* network+funding+country), so dropping `card_type` anywhere
    /// in this conversion would silently collapse three different contract rates into one.
    #[test]
    fn card_program_survives_the_row_entry_round_trip() {
        let rows = vec![
            row("adyen", Some("premium")),
            row("adyen", Some("commercial")),
        ];
        let back = entries_to_rows(&rows_to_entries(rows));

        // One synthesized default (no dims) plus the two program tiers.
        let tiers: Vec<&SeedCostRow> = back.iter().filter(|r| !r.is_default).collect();
        assert_eq!(tiers.len(), 2, "both program tiers survive");
        let programs: Vec<&str> = tiers
            .iter()
            .map(|r| r.card_type.as_deref().unwrap_or(""))
            .collect();
        assert!(programs.contains(&"premium"), "got {programs:?}");
        assert!(programs.contains(&"commercial"), "got {programs:?}");
    }

    /// A blank program means "any card", not the empty-string program — otherwise the tier would be
    /// compared against `card_program` by equality and match nothing at all.
    #[test]
    fn blank_program_normalizes_to_a_wildcard() {
        let back = entries_to_rows(&rows_to_entries(vec![row("adyen", Some("  "))]));
        let tier = back.iter().find(|r| !r.is_default).expect("tier row");
        assert_eq!(
            tier.card_type, None,
            "whitespace is a wildcard, not a value"
        );
    }
}
