//! Invoice reconciliation (the accuracy proof, architecture doc "Step 2").
//!
//! Ties the stored add-on back to the bill: compare the invoice subtotal (true all-in cost) against
//! what the model now predicts for the same settled book — the four fee columns the OLS fit captures
//! (`Σ total_fee` = `Σ sy` in `cost_daily_stats`) **plus** the add-on applied to volume and count.
//! The residual is the remaining, unexplained gap; `coverage_after` is the share of the true cost
//! the model captures once the add-on is in. This turns "we think we're accurate" into a measured
//! number, and — over time — lets the flat add-on be recalibrated from the bill instead of assumed.

use crate::config::ClickHouseAnalyticsConfig;
use crate::cost_ingestion::types::IngestError;

use super::pipeline::exec;
use super::store;

/// The reconciliation of one merchant's stored add-on against its invoice.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Reconciliation {
    pub connector: String,
    /// Invoice subtotal excluding taxes — the true all-in cost.
    pub invoice_subtotal: f64,
    /// What the OLS fit alone captured (Σ of the four fee columns over the settled book).
    pub model_captured: f64,
    /// The add-on's contribution: `pct_addon_bps/1e4·volume + fixed_addon·count`.
    pub addon_contribution: f64,
    /// `model_captured + addon_contribution` — the model's all-in prediction.
    pub model_all_in: f64,
    /// `invoice_subtotal - model_all_in` — the remaining unexplained gap (positive ⇒ still under).
    pub residual: f64,
    /// Share of the true cost captured before the add-on (`model_captured / invoice_subtotal`).
    pub coverage_before: f64,
    /// Share of the true cost captured with the add-on (`model_all_in / invoice_subtotal`).
    pub coverage_after: f64,
}

/// Reconcile every stored add-on for `merchant_id` against its invoice. Returns one row per
/// connector that has both an add-on and a stated subtotal. A connector whose add-on lacks a
/// subtotal (the invoice didn't state one) is skipped — there is nothing to tie back to.
pub async fn reconcile_merchant(
    cfg: &ClickHouseAnalyticsConfig,
    merchant_id: &str,
) -> Result<Vec<Reconciliation>, IngestError> {
    let addons = store::list(merchant_id).await?;
    let mut out = Vec::new();
    for (connector, a) in addons {
        let Some(subtotal) = a.subtotal_ex_tax.filter(|s| *s > 0.0) else {
            continue;
        };

        // The settled book the model priced: Σ sy = captured fee cost, Σ sx = turnover, Σ n = count.
        let sql = format!(
            "SELECT sum(sy), sum(sx), sum(n) FROM {db}.cost_daily_stats FINAL \
             WHERE connector = {{connector:String}} AND merchant_id = {{merchant_id:String}} \
             FORMAT TSV",
            db = cfg.database,
        );
        let params = [
            ("connector", connector.clone()),
            ("merchant_id", merchant_id.to_string()),
        ];
        let row = exec(cfg, &sql, &params).await?;
        let mut cols = row.trim().split('\t');
        let model_captured = cols
            .next()
            .and_then(|s| s.trim().parse::<f64>().ok())
            .unwrap_or(0.0);
        let volume = cols
            .next()
            .and_then(|s| s.trim().parse::<f64>().ok())
            .unwrap_or(0.0);
        let count = cols
            .next()
            .and_then(|s| s.trim().parse::<f64>().ok())
            .unwrap_or(0.0);

        let addon_contribution = a.pct_addon_bps / 10_000.0 * volume + a.fixed_addon * count;
        let model_all_in = model_captured + addon_contribution;

        out.push(Reconciliation {
            connector,
            invoice_subtotal: subtotal,
            model_captured,
            addon_contribution,
            model_all_in,
            residual: subtotal - model_all_in,
            coverage_before: model_captured / subtotal,
            coverage_after: model_all_in / subtotal,
        });
    }
    Ok(out)
}
