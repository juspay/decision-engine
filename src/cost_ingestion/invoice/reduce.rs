//! Reduce a parsed invoice to the serving-time [`CostAddon`] — the connector-agnostic core.
//!
//! The shape of each line (set by the connector's classifier) decides how it folds in:
//!
//! * **Flat per-transaction** fees → a per-transaction `fixed` add-on, blended across *all*
//!   transactions: `fixed_addon = Σ(flat line totals) / txn_count`. Blending (rather than each
//!   line's own quantity) is deliberate — a fee like RevenueProtect that only lands on a subset of
//!   transactions must be spread across the whole book, because the add-on is applied uniformly to
//!   every cluster's `fixed` (we don't know at decide time which transactions were authenticated).
//!
//! * **Periodic** and **credit** lines → amortized onto settled volume as a `pct_bps` add-on:
//!   `pct_addon_bps = (Σ periodic + Σ credit) / card_volume · 10⁴`. Credits are negative, so they
//!   net the periodic total down — the add-on is net-of-rebate.
//!
//! * **AlreadyModeled** (acquiring interchange/scheme/markup/commission) and **Volume** lines
//!   contribute nothing: the former is already priced by the OLS fit, the latter is only the
//!   amortization denominator.
//!
//! The denominators (`txn_count`, `card_volume`) come from the invoice when it states them, else
//! from the settled volume in ClickHouse for the same period (`fallback`), so the reduction never
//! silently divides by a partial figure.

use super::types::{CostAddon, LineKind, ParsedInvoice};

/// Settled-volume denominators for a `(merchant, connector)` over the invoice period, read from
/// `cost_daily_stats` — the fallback when the invoice does not state its own turnover / count.
#[derive(Debug, Clone, Copy, Default)]
pub struct VolumeFallback {
    pub card_volume: f64,
    pub txn_count: u64,
}

/// Reduce an invoice to its per-transaction cost add-on. `fallback` supplies the amortization
/// denominators the invoice omits; pass [`VolumeFallback::default`] when none is available (a
/// missing denominator zeroes only the term that needs it, never the whole add-on).
pub fn reduce_to_addon(invoice: &ParsedInvoice, fallback: VolumeFallback) -> CostAddon {
    let sum_kind = |kind: LineKind| -> f64 {
        invoice
            .lines
            .iter()
            .filter(|l| l.kind == kind)
            .map(|l| l.amount)
            .sum()
    };

    let flat_total = sum_kind(LineKind::FlatPerTxn);
    let periodic_total = sum_kind(LineKind::Periodic);
    let credit_total = sum_kind(LineKind::Credit); // already negative

    let txn_count = invoice.summary.txn_count.unwrap_or(fallback.txn_count);
    let card_volume = invoice
        .summary
        .card_volume
        .filter(|v| *v > 0.0)
        .unwrap_or(fallback.card_volume);

    let fixed_addon = if txn_count > 0 {
        flat_total / txn_count as f64
    } else {
        0.0
    };
    let pct_addon_bps = if card_volume > 0.0 {
        (periodic_total + credit_total) / card_volume * 10_000.0
    } else {
        0.0
    };

    CostAddon {
        pct_addon_bps,
        fixed_addon,
    }
}

#[cfg(test)]
mod tests {
    use super::super::types::{InvoiceLine, InvoiceSummary};
    use super::*;

    fn line(desc: &str, kind: LineKind, amount: f64, quantity: u64) -> InvoiceLine {
        InvoiceLine {
            description: desc.into(),
            kind,
            amount,
            quantity,
            currency: "EUR".into(),
        }
    }

    /// The FootLocker-Eurasia October-2025 figures from the coverage analysis reduce to the add-on
    /// the doc predicts: ~€0.04/txn flat and a small positive bps once credits net the periodics.
    #[test]
    fn reduces_october_figures() {
        let invoice = ParsedInvoice {
            summary: InvoiceSummary {
                card_volume: Some(101_600_000.0),
                txn_count: Some(1_418_233),
                ..Default::default()
            },
            lines: vec![
                line("processing fee", LineKind::FlatPerTxn, 42_547.0, 1_418_233),
                line("revenueprotect", LineKind::FlatPerTxn, 17_524.0, 1_418_233),
                line("managed risk service", LineKind::Periodic, 5_500.0, 0),
                line(
                    "non-transactional scheme fees",
                    LineKind::Periodic,
                    4_480.0,
                    0,
                ),
                line("chargeback service", LineKind::Periodic, 1_505.0, 0),
                line(
                    "management + reconciliation",
                    LineKind::Periodic,
                    1_351.0,
                    0,
                ),
                line("dcc markup", LineKind::Credit, -12_183.0, 0),
                line("interchange", LineKind::AlreadyModeled, 549_256.0, 0),
                line("turnover", LineKind::Volume, 101_600_000.0, 0),
            ],
        };
        let addon = reduce_to_addon(&invoice, VolumeFallback::default());

        // Flat: (42547 + 17524) / 1_418_233 ≈ €0.0424/txn.
        assert!(
            (addon.fixed_addon - 0.04236).abs() < 1e-4,
            "fixed={}",
            addon.fixed_addon
        );

        // pct: (5500 + 4480 + 1505 + 1351 - 12183) / 101.6M · 1e4 ≈ 0.064 bps.
        let expected =
            (5_500.0 + 4_480.0 + 1_505.0 + 1_351.0 - 12_183.0) / 101_600_000.0 * 10_000.0;
        assert!(
            (addon.pct_addon_bps - expected).abs() < 1e-6,
            "pct={}",
            addon.pct_addon_bps
        );
    }

    #[test]
    fn missing_denominators_zero_only_their_own_term() {
        // No txn_count and no volume anywhere ⇒ both terms zero, never a divide-by-zero or NaN.
        let invoice = ParsedInvoice {
            summary: InvoiceSummary::default(),
            lines: vec![
                line("processing fee", LineKind::FlatPerTxn, 100.0, 0),
                line("managed risk", LineKind::Periodic, 50.0, 0),
            ],
        };
        let addon = reduce_to_addon(&invoice, VolumeFallback::default());
        assert_eq!(addon.fixed_addon, 0.0);
        assert_eq!(addon.pct_addon_bps, 0.0);
    }

    #[test]
    fn falls_back_to_settled_volume_when_invoice_is_silent() {
        let invoice = ParsedInvoice {
            summary: InvoiceSummary::default(), // states neither volume nor count
            lines: vec![
                line("processing fee", LineKind::FlatPerTxn, 100.0, 0),
                line("managed risk", LineKind::Periodic, 200.0, 0),
            ],
        };
        let fallback = VolumeFallback {
            card_volume: 1_000_000.0,
            txn_count: 10_000,
        };
        let addon = reduce_to_addon(&invoice, fallback);
        assert!((addon.fixed_addon - 0.01).abs() < 1e-9); // 100 / 10_000
        assert!((addon.pct_addon_bps - 2.0).abs() < 1e-9); // 200 / 1M · 1e4
    }
}
