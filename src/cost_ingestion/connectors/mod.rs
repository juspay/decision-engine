//! Per-connector [`SettlementReportSource`](super::source::SettlementReportSource) implementations.
//!
//! One module per connector; everything connector-specific (report format, notification shape,
//! signature scheme, download auth) is contained here. Adding a connector = a new module + one
//! line in [`super::source::ConnectorRegistry::with_builtins`].

pub mod adyen;
pub mod braintree;
pub mod chase;
pub mod checkout;
pub mod csv_reader;
pub mod stripe;
