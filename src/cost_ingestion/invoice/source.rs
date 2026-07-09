//! The invoice connector seam: one trait, a registry, nothing connector-specific outside an `impl`.
//!
//! Parallel to [`crate::cost_ingestion::source::SettlementReportSource`] but for the *invoice*
//! document rather than the settlement report. Adding another connector's invoice is a new
//! `InvoiceSource` impl plus a registry line — the reduction, storage, and serving overlay never
//! change.

use std::collections::HashMap;
use std::sync::Arc;

use super::adyen::AdyenInvoiceSource;
use super::types::ParsedInvoice;
use crate::cost_ingestion::types::IngestError;

/// Everything invoice-specific for one connector lives behind this trait. Parsing is a pure
/// function of the bytes, so impls are straightforward to unit-test against a sample invoice.
pub trait InvoiceSource: Send + Sync {
    /// The connector id this source handles, e.g. `"adyen"`. Must match the value stored in the
    /// `connector` column and the `/…/invoice` path segment.
    fn connector(&self) -> &'static str;

    /// Normalize the connector's native invoice (CSV/JSON export) into canonical [`ParsedInvoice`].
    /// This is the *only* connector-specific code: the native line descriptions, column labels, and
    /// their classification into [`super::types::LineKind`] live here and nowhere else.
    fn parse_invoice(&self, bytes: &[u8]) -> Result<ParsedInvoice, IngestError>;
}

/// Immutable lookup of `connector -> invoice source`, built once and shared.
pub struct InvoiceRegistry {
    sources: HashMap<&'static str, Arc<dyn InvoiceSource>>,
}

impl InvoiceRegistry {
    /// Registry seeded with every built-in invoice connector. Adding a connector = one line here.
    pub fn with_builtins() -> Self {
        let mut sources: HashMap<&'static str, Arc<dyn InvoiceSource>> = HashMap::new();
        let adyen: Arc<dyn InvoiceSource> = Arc::new(AdyenInvoiceSource::new());
        sources.insert(adyen.connector(), adyen);
        Self { sources }
    }

    /// Resolve a source by connector id, or `UnknownConnector` if none is registered.
    pub fn get(&self, connector: &str) -> Result<Arc<dyn InvoiceSource>, IngestError> {
        self.sources
            .get(connector)
            .cloned()
            .ok_or_else(|| IngestError::UnknownConnector(connector.to_string()))
    }
}

impl Default for InvoiceRegistry {
    fn default() -> Self {
        Self::with_builtins()
    }
}
