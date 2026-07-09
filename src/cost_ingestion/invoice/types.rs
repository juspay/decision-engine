//! Canonical, connector-agnostic types for **invoice** ingestion — the second data source that
//! closes the gap the settlement report (PAR) structurally cannot.
//!
//! The settlement report gives us the four per-transaction fee columns the OLS fit consumes
//! (`interchange + scheme + markup + commission`). The monthly invoice carries everything *else*:
//! flat per-transaction fees (Adyen Processing Fee, RevenueProtect), periodic/non-transactional
//! fees (Managed Risk, Non-Transactional Scheme Fees, Chargeback, Management/Reconciliation) and
//! credits (DCC markup, corrections). Those never appear on the settled rows, so no amount of PAR
//! enrichment recovers them (see `scratch/cost-estimate-coverage-and-accuracy.md`).
//!
//! Every connector's native invoice is normalized into [`InvoiceLine`]s + an [`InvoiceSummary`];
//! once an invoice reaches that shape, the reduction to a serving-time cost add-on
//! ([`super::reduce`]) is identical regardless of which connector produced it — exactly the
//! `SettledFeeRow` pattern one layer down.

use chrono::NaiveDate;

/// How a single invoice line participates in the per-transaction cost — the classification is what
/// decides the *shape* of the correction, and the shape decides the math (a flat per-txn fee lands
/// in `fixed`, a periodic fee is amortized into `pct_bps`). The connector parser is the only place
/// that maps a native line description onto one of these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    /// A flat fee charged once per transaction (Adyen Processing Fee €0.03, RevenueProtect €0.05).
    /// Does not vary with amount ⇒ belongs entirely in each cluster's `fixed` term. This is ~90% of
    /// the coverage gap.
    FlatPerTxn,
    /// A periodic / non-transactional fee billed per-MID/per-period, not per settled transaction
    /// (Managed Risk, Non-Transactional Scheme Fees, Chargeback, Management, Reconciliation).
    /// Amortized back onto settled volume ⇒ a small `pct_bps` addition.
    Periodic,
    /// A credit that reduces net cost (DCC markup rebate, refund/overcharge corrections). Carried as
    /// a signed amount; folded in so the add-on is net-of-rebate rather than gross.
    Credit,
    /// A line already captured by the settlement-report fit — the acquiring interchange / scheme /
    /// markup / commission lines. **Excluded from the add-on** so we never double-count what the OLS
    /// model already prices. Kept (not dropped) only so reconciliation can tie back to the subtotal.
    AlreadyModeled,
    /// Turnover / volume line: not a fee. Used only as the amortization denominator when the invoice
    /// states card volume, never added to cost.
    Volume,
}

impl LineKind {
    /// Stable snake_case tag for API responses / the dashboard.
    pub fn as_str(&self) -> &'static str {
        match self {
            LineKind::FlatPerTxn => "flat_per_txn",
            LineKind::Periodic => "periodic",
            LineKind::Credit => "credit",
            LineKind::AlreadyModeled => "already_modeled",
            LineKind::Volume => "volume",
        }
    }

    /// Whether this line contributes to the served add-on (the "missing" PAR fees). `AlreadyModeled`
    /// and `Volume` do not — the former is already priced by the fit, the latter is just a denominator.
    pub fn is_added(&self) -> bool {
        matches!(self, LineKind::FlatPerTxn | LineKind::Periodic | LineKind::Credit)
    }
}

/// One normalized invoice line — the atom the add-on reduction consumes. Deliberately free of
/// ingestion context (`connector`/`merchant_id` are stamped by the pipeline), so a connector's
/// parser stays reusable and unit-testable.
#[derive(Debug, Clone, PartialEq)]
pub struct InvoiceLine {
    /// The connector's native line description, lowercased (e.g. `"processing fee"`). Kept for
    /// observability / reconciliation drill-down; classification is already resolved into `kind`.
    pub description: String,
    /// What role this line plays in the per-transaction cost.
    pub kind: LineKind,
    /// Signed line total in `currency` (credits are negative). For `FlatPerTxn` this is the line's
    /// *total*, not the unit price — the unit rate is `amount / quantity`.
    pub amount: f64,
    /// Billed quantity when the line is per-unit (the transaction count for a `FlatPerTxn` line).
    /// `0` when the line is not quantity-based (periodic lumps, turnover).
    pub quantity: u64,
    /// Line currency, uppercased: `EUR`, `AUD`, …
    pub currency: String,
}

/// Invoice-level totals a connector states directly, used to amortize periodic fees and to
/// reconcile. Every field is optional because not all connectors state all of them; the reducer
/// falls back to values derived from the lines (and, ultimately, to the settled volume in
/// ClickHouse) when a field is absent.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct InvoiceSummary {
    /// Invoice number / id (the idempotency key for re-ingesting the same invoice).
    pub invoice_ref: String,
    /// Connector-side account the invoice covers (e.g. Adyen `merchantAccountCode` group).
    pub account: String,
    /// Card turnover the periodic fees amortize over, when the invoice states it. `None` ⇒ derive
    /// from the `Volume` lines, else from settled `cost_daily_stats` for the period.
    pub card_volume: Option<f64>,
    /// Total settled transaction count, when stated. `None` ⇒ derive from `FlatPerTxn` quantities.
    pub txn_count: Option<u64>,
    /// Invoice subtotal excluding taxes — the "true all-in cost" reconciliation ties back to.
    pub subtotal_ex_tax: Option<f64>,
    /// Primary invoice currency, uppercased.
    pub currency: String,
    /// Billing period start/end, when stated (used to window the reconciliation query).
    pub period_start: Option<NaiveDate>,
    pub period_end: Option<NaiveDate>,
}

/// A parsed invoice: the normalized lines plus the connector-stated totals. The single value a
/// connector's [`super::source::InvoiceSource::parse_invoice`] returns.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedInvoice {
    pub summary: InvoiceSummary,
    pub lines: Vec<InvoiceLine>,
}

/// The serving-time cost add-on an invoice reduces to: the two-parameter correction layered on top
/// of every *learned* cluster cost for a `(merchant, connector)`. Mirrors the `{pct_bps, fixed}`
/// shape `ServingCost` already carries, so the overlay is one addition in the effective-cost
/// formula (`serving::lookup`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CostAddon {
    /// Amortized periodic-fee rate, in basis points, added to each cluster's `pct_bps`.
    pub pct_addon_bps: f64,
    /// Flat per-transaction fee (invoice currency units), added to each cluster's `fixed`.
    pub fixed_addon: f64,
}

impl CostAddon {
    /// A no-op add-on (the identity for the serving overlay).
    pub const ZERO: CostAddon = CostAddon { pct_addon_bps: 0.0, fixed_addon: 0.0 };
}
