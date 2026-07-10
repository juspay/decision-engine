//! In-house cost estimation — settlement-report ingestion.
//!
//! Connector-generic pipeline that turns a connector's settlement report into fitted per-cluster
//! cost models. This module owns the *ingestion* half (receive → download → parse → stage); the
//! fit and serving live alongside the multi-objective router. See
//! `scratch/inhouse-cost-architecture.md` §7.
//!
//! The extensibility seam is [`source::SettlementReportSource`]: everything connector-specific
//! lives behind that trait, so adding a connector is a new impl plus its credentials — the queue,
//! staging, fit, and serving never change.

pub mod blended;
pub mod connectors;
pub mod coverage;
pub mod creds;
pub mod detect;
pub mod fit;
pub mod invoice;
pub mod overrides;
pub mod pipeline;
pub mod poller;
pub mod rollup;
pub mod serving;
pub mod sink;
pub mod source;
pub mod store;
pub mod types;
pub mod worker;

pub use creds::{ConnectorCredsStore, ResolvedCreds};
pub use source::{ConnectorRegistry, SettlementReportSource};
pub use types::{ConnectorCreds, IngestError, ReportNotification, SettledFeeRow};
