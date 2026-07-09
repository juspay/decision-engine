//! Merchant-facing API for **invoice** ingestion — the second cost data source.
//!
//! A merchant uploads a connector invoice (the monthly bill); we reduce it to a per-transaction
//! cost add-on that recovers the invoice-only fees the settlement report can't (see
//! [`crate::cost_ingestion::invoice`]), layered on top of the learned models at decide time.
//!
//! Unlike the settlement report (several GB, streamed to disk and processed in the background), an
//! invoice is small — a few hundred lines — so it is buffered and processed synchronously, and the
//! computed add-on is returned in the response.

use axum::body::Bytes;
use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::cost_ingestion::invoice::{self, StoredAddon};

use super::connector_fees::clickhouse_config;

/// Upper bound on an uploaded invoice. Invoices are small; this rejects an accidental report upload
/// to the wrong endpoint before parsing.
pub const MAX_INVOICE_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug, Deserialize)]
pub struct InvoiceParams {
    /// Connector-side account (e.g. Adyen `merchantAccountCode` group) the invoice covers.
    pub account: String,
    /// Invoice number, when the caller has it (idempotency / audit). Optional.
    #[serde(default)]
    pub invoice_ref: Option<String>,
}

/// One identified fee type, for the dashboard's "here's what we found" table.
#[derive(Debug, Serialize)]
pub struct InvoiceLineDto {
    pub description: String,
    /// `flat_per_txn` | `periodic` | `credit` | `already_modeled` | `volume`.
    pub kind: String,
    /// Whether this fee is added to the model (a missing PAR fee) vs ignored (already modeled/volume).
    pub added: bool,
    /// Total on the invoice for this fee type.
    pub amount: f64,
    /// Amortized contribution per transaction (`0` for ignored lines).
    pub per_txn: f64,
}

/// The computed add-on plus the identified detail returned after a successful invoice ingest.
#[derive(Debug, Serialize)]
pub struct InvoiceUploadResponse {
    pub merchant_id: String,
    pub connector: String,
    pub account: String,
    /// Amortized periodic-fee rate added to every learned cluster's `pct_bps`.
    pub pct_addon_bps: f64,
    /// Flat per-transaction fee added to every learned cluster's `fixed`.
    pub fixed_addon: f64,
    /// Headline: total additional fee applied **per transaction** for this connector account.
    pub total_addon_per_txn: f64,
    pub subtotal_ex_tax: Option<f64>,
    pub card_volume: Option<f64>,
    pub txn_count: Option<u64>,
    pub currency: String,
    pub lines: usize,
    /// Per-fee-type breakdown — the missing fees we identified (and the ones we ignored).
    pub breakdown: Vec<InvoiceLineDto>,
}

/// `POST /merchant-account/:merchant_id/connectors/:connector/invoice?account=…[&invoice_ref=…]`
pub async fn upload_invoice(
    Path((merchant_id, connector)): Path<(String, String)>,
    Query(params): Query<InvoiceParams>,
    body: Bytes, // buffered: invoices are small
) -> Result<Json<InvoiceUploadResponse>, (StatusCode, String)> {
    if body.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "empty invoice body".to_string()));
    }
    if body.len() > MAX_INVOICE_BYTES {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            format!("invoice exceeds {MAX_INVOICE_BYTES} byte limit"),
        ));
    }

    let cfg = clickhouse_config()?;
    let outcome = invoice::ingest_invoice_bytes(
        &cfg,
        &connector,
        &params.account,
        &merchant_id,
        params.invoice_ref.as_deref().unwrap_or(""),
        &body,
    )
    .await
    .map_err(|e| (StatusCode::UNPROCESSABLE_ENTITY, format!("{e:?}")))?;

    Ok(Json(InvoiceUploadResponse {
        merchant_id,
        connector: outcome.connector,
        account: outcome.account,
        pct_addon_bps: outcome.addon.pct_addon_bps,
        fixed_addon: outcome.addon.fixed_addon,
        total_addon_per_txn: outcome.total_addon_per_txn,
        subtotal_ex_tax: outcome.subtotal_ex_tax,
        card_volume: outcome.card_volume,
        txn_count: outcome.txn_count,
        currency: outcome.currency,
        lines: outcome.lines,
        breakdown: outcome
            .breakdown
            .into_iter()
            .map(|g| InvoiceLineDto {
                description: g.description,
                kind: g.kind.as_str().to_string(),
                added: g.kind.is_added(),
                amount: g.amount,
                per_txn: g.per_txn,
            })
            .collect(),
    }))
}

/// One stored add-on for the dashboard listing.
#[derive(Debug, Serialize)]
pub struct AddonDto {
    pub connector: String,
    #[serde(flatten)]
    pub addon: StoredAddon,
}

/// `GET /merchant-account/:merchant_id/invoice-addons` — the invoice add-ons in effect.
pub async fn list_addons(
    Path(merchant_id): Path<String>,
) -> Result<Json<Vec<AddonDto>>, (StatusCode, String)> {
    let list = invoice::store::list(&merchant_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:?}")))?;
    Ok(Json(
        list.into_iter().map(|(connector, addon)| AddonDto { connector, addon }).collect(),
    ))
}

/// `DELETE /merchant-account/:merchant_id/connectors/:connector/invoice-addon` — drop the add-on and
/// revert this merchant's served models to the learned-only cost.
pub async fn delete_addon(
    Path((merchant_id, connector)): Path<(String, String)>,
) -> Result<StatusCode, (StatusCode, String)> {
    let connector = connector.to_lowercase();
    invoice::store::delete(&merchant_id, &connector)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:?}")))?;
    super::connector_fees::refresh_serving(&merchant_id).await;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /merchant-account/:merchant_id/invoice-reconciliation` — tie each stored add-on back to its
/// invoice: coverage before/after and the remaining residual (architecture "Step 2").
pub async fn get_reconciliation(
    Path(merchant_id): Path<String>,
) -> Result<Json<Vec<invoice::Reconciliation>>, (StatusCode, String)> {
    let cfg = clickhouse_config()?;
    let recon = invoice::reconcile_merchant(&cfg, &merchant_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:?}")))?;
    Ok(Json(recon))
}
