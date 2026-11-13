//! Invoice ingestion — the second data source that closes the coverage gap the settlement report
//! structurally cannot.
//!
//! The settlement report (PAR) gives us the four per-transaction fee columns the OLS fit consumes;
//! it captures ~90% of a card transaction's true all-in cost. The remaining ~9% lives only on the
//! monthly invoice — flat per-transaction fees (Processing, RevenueProtect), periodic/non-transactional
//! fees, and credits — none of which appear on the settled rows (see
//! `scratch/cost-estimate-coverage-and-accuracy.md`).
//!
//! This module ingests that invoice and reduces it to a two-parameter **cost add-on**
//! (`{pct_addon_bps, fixed_addon}`) per `(merchant, connector)`, layered on top of every *learned*
//! cluster cost at serving time (`serving::lookup`) — the same overlay mechanism the manual fee
//! overrides use. Flow:
//!
//! ```text
//! invoice bytes ─► InvoiceSource::parse_invoice ─► ParsedInvoice ─► reduce_to_addon ─► CostAddon
//!                     (connector-specific)            (generic)         (generic)         │
//!                                                                                         ▼
//!                                                      service_configuration KV (store) ─► serving overlay
//! ```
//!
//! Everything connector-specific lives behind [`source::InvoiceSource`]; the reduction, storage,
//! serving overlay, and reconciliation are written once for all connectors.

pub mod adyen;
pub mod pipeline;
pub mod reconcile;
pub mod reduce;
pub mod source;
pub mod store;
pub mod types;

pub use pipeline::{ingest_invoice_bytes, InvoiceOutcome};
pub use reconcile::{reconcile_merchant, Reconciliation};
pub use source::{InvoiceRegistry, InvoiceSource};
pub use store::StoredAddon;
pub use types::{CostAddon, InvoiceLine, InvoiceSummary, LineKind, ParsedInvoice};
