//! The connector extensibility seam: one trait, a registry, nothing connector-specific
//! outside an `impl` (see `scratch/inhouse-cost-architecture.md` §7.1–7.2).
//!
//! Adding Stripe / Checkout / Worldpay is a new `SettlementReportSource` impl plus its
//! credentials — the queue, staging, fit, and serving never change.

use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::sync::Arc;

use async_trait::async_trait;
use axum::http::HeaderMap;
use bytes::Bytes;
use masking::Secret;

use super::connectors::adyen::AdyenReportSource;
use super::connectors::braintree::BraintreeReportSource;
use super::connectors::chase::ChaseReportSource;
use super::connectors::checkout::CheckoutReportSource;
use super::connectors::stripe::StripeReportSource;
use super::mapping::ColumnMapping;
use super::types::{ConnectorCreds, IngestError, ReadyReport, ReportNotification, SettledFeeRow};

/// Everything connector-specific lives behind this trait. All methods are pure functions of
/// their inputs except `download_report` (network), so impls are straightforward to unit-test.
#[async_trait]
pub trait SettlementReportSource: Send + Sync {
    /// The connector id this source handles, e.g. `"adyen"`. Must match the value stored in the
    /// `connector` column and the `/webhooks/settlement/{connector}` path segment.
    fn connector(&self) -> &'static str;

    /// Extract the connector-side account from an **unverified** webhook body, so the caller can
    /// load that account's signing secret *before* verifying. Must not trust anything else in
    /// the payload — it only reads the account identifier.
    fn peek_account(&self, raw_body: &[u8]) -> Result<String, IngestError>;

    /// Verify the webhook signature against `secret` and, on success, extract the report handle.
    /// Different connectors sign differently (Adyen: HMAC in the body; Stripe: a header) — that
    /// difference is fully contained here.
    fn verify_and_parse_notification(
        &self,
        headers: &HeaderMap,
        raw_body: &[u8],
        secret: &Secret<String>,
    ) -> Result<ReportNotification, IngestError>;

    /// Whether this connector is discovered by **polling** its reporting API (a "pull" connector)
    /// rather than by a pushed webhook. Pull connectors are swept by the generic report poller;
    /// webhook connectors keep the default `false` and are driven by the webhook route instead.
    fn is_pull(&self) -> bool {
        false
    }

    /// For pull connectors: list the reports currently ready to ingest for one settlement source.
    /// The poller enqueues one job per returned [`ReadyReport`]. Webhook connectors don't override
    /// this (they receive pushes), so the default returns none.
    async fn poll_ready_reports(
        &self,
        _creds: &ConnectorCreds,
    ) -> Result<Vec<ReadyReport>, IngestError> {
        Ok(Vec::new())
    }

    /// Fetch the report bytes using the merchant's stored credentials. Buffered for now; large
    /// reports can move to a streamed body later without touching callers.
    async fn download_report(
        &self,
        creds: &ConnectorCreds,
        note: &ReportNotification,
    ) -> Result<Bytes, IngestError>;

    /// Whether one emitted row is assembled from **several** report rows (Checkout fans one payment
    /// across a capture line plus its fee lines, accumulates them, and flushes at end of file)
    /// rather than mapping one report row to one emitted row.
    ///
    /// This matters only where a report is read *partially*: the upload preflight parses the first
    /// few KB of a file, and a grouping connector's final group is then almost always cut in half —
    /// a capture with its fee lines still beyond the cut yields a row with real gross and zero fee.
    /// Consumers of partial parses must know not to trust such a row (see
    /// [`preview`](super::preflight::preview)); a full ingestion is unaffected, as it always reaches
    /// EOF with every group complete.
    fn groups_rows(&self) -> bool {
        false
    }

    /// Strip any connector-specific framing wrapping the CSV — J.P. Morgan preset reports arrive
    /// inside a `BEGIN` / `EntityId=…` / `END` envelope — leaving a plain `header + data` stream.
    /// Default: the report is already plain CSV.
    ///
    /// Exists as its own step (rather than living inline in `parse_rows`) so that anything reading
    /// a report's *header* without parsing it — notably the upload preflight — sees the same
    /// unwrapped stream `parse_rows` does, instead of mistaking a frame line for the header row.
    fn unwrap_envelope(&self, reader: Box<dyn Read + Send>) -> Box<dyn Read + Send> {
        reader
    }

    /// Stream-normalize the connector's native report into canonical [`SettledFeeRow`]s, calling
    /// `on_row` once per row. This is the *only* connector-specific parsing code — the column
    /// names / native format live here and nowhere else.
    ///
    /// Row-at-a-time by design: batching, backpressure, and staging are generic and handled by the
    /// pipeline (via [`parse_in_batches`]), so no connector reimplements them. Parsing off a
    /// `Read` (rather than a `&[u8]`) keeps memory flat for multi-GB reports.
    ///
    /// `on_row` is synchronous: this runs inside `spawn_blocking`, and the pipeline's callback
    /// buffers rows and pushes each full batch across a channel to the async inserter.
    ///
    /// `mapping` carries the merchant's `expected label -> their label` column overrides; pass
    /// [`ColumnMapping::none`] when there are none. It is an explicit parameter rather than ambient
    /// state because it changes which column feeds which fee component, and a wrong mapping produces
    /// a *plausible* cost model rather than an error — that is not a data flow to leave implicit.
    fn parse_rows(
        &self,
        reader: Box<dyn Read + Send>,
        mapping: &ColumnMapping,
        on_row: &mut dyn FnMut(SettledFeeRow) -> Result<(), IngestError>,
    ) -> Result<(), IngestError>;

    /// Buffered convenience over [`parse_rows`]: collect the whole report into one `Vec`, with no
    /// column mapping. Fine for tests and small inputs; the streaming path never uses this.
    fn parse_report(&self, bytes: &[u8]) -> Result<Vec<SettledFeeRow>, IngestError> {
        self.parse_report_mapped(bytes, ColumnMapping::none())
    }

    /// [`parse_report`](Self::parse_report) with a column mapping applied.
    fn parse_report_mapped(
        &self,
        bytes: &[u8],
        mapping: &ColumnMapping,
    ) -> Result<Vec<SettledFeeRow>, IngestError> {
        let mut all = Vec::new();
        self.parse_rows(Box::new(Cursor::new(bytes.to_vec())), mapping, &mut |row| {
            all.push(row);
            Ok(())
        })?;
        Ok(all)
    }
}

/// Drive a connector's per-row [`SettlementReportSource::parse_rows`] and emit rows in batches of
/// at most `batch_size` via `on_batch`. Connector-agnostic: the accumulate-and-flush logic lives
/// here once, so every connector gets identical batching.
pub fn parse_in_batches(
    source: &dyn SettlementReportSource,
    reader: Box<dyn Read + Send>,
    mapping: &ColumnMapping,
    batch_size: usize,
    mut on_batch: impl FnMut(Vec<SettledFeeRow>) -> Result<(), IngestError>,
) -> Result<(), IngestError> {
    let batch_size = batch_size.max(1);
    let mut batch = Vec::with_capacity(batch_size);
    source.parse_rows(reader, mapping, &mut |row| {
        batch.push(row);
        if batch.len() >= batch_size {
            on_batch(std::mem::replace(
                &mut batch,
                Vec::with_capacity(batch_size),
            ))?;
        }
        Ok(())
    })?;
    if !batch.is_empty() {
        on_batch(batch)?;
    }
    Ok(())
}

/// Immutable lookup of `connector -> source`, built once at startup and shared (`Arc`).
pub struct ConnectorRegistry {
    sources: HashMap<&'static str, Arc<dyn SettlementReportSource>>,
}

impl ConnectorRegistry {
    /// Registry seeded with every built-in connector. Adding a connector = one line here.
    pub fn with_builtins() -> Self {
        let mut sources: HashMap<&'static str, Arc<dyn SettlementReportSource>> = HashMap::new();
        Self::register(&mut sources, Arc::new(AdyenReportSource::new()));
        Self::register(&mut sources, Arc::new(BraintreeReportSource::new()));
        Self::register(&mut sources, Arc::new(ChaseReportSource::new()));
        Self::register(&mut sources, Arc::new(CheckoutReportSource::new()));
        Self::register(&mut sources, Arc::new(StripeReportSource::new()));
        Self { sources }
    }

    fn register(
        sources: &mut HashMap<&'static str, Arc<dyn SettlementReportSource>>,
        source: Arc<dyn SettlementReportSource>,
    ) {
        sources.insert(source.connector(), source);
    }

    /// Resolve a source by connector id, or `UnknownConnector` if none is registered.
    pub fn get(&self, connector: &str) -> Result<Arc<dyn SettlementReportSource>, IngestError> {
        self.sources
            .get(connector)
            .cloned()
            .ok_or_else(|| IngestError::UnknownConnector(connector.to_string()))
    }

    /// Every registered connector id. Used by the upload preflight to test a header row against all
    /// connectors, so a file uploaded under the wrong one can be spotted.
    pub fn connectors(&self) -> Vec<&'static str> {
        let mut ids: Vec<&'static str> = self.sources.keys().copied().collect();
        ids.sort_unstable(); // HashMap order is nondeterministic; keep suggestions stable.
        ids
    }

    /// Every registered **pull** connector (those discovered by polling). The report poller sweeps
    /// exactly these, so adding a pull PSP is just a new `is_pull() -> true` impl — no poller change.
    pub fn pull_sources(&self) -> Vec<Arc<dyn SettlementReportSource>> {
        self.sources
            .values()
            .filter(|s| s.is_pull())
            .cloned()
            .collect()
    }
}

impl Default for ConnectorRegistry {
    fn default() -> Self {
        Self::with_builtins()
    }
}
