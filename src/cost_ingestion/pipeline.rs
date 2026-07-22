//! Shared parse → stage → fit path.
//!
//! Both entry points converge here: the ingest worker (after downloading a report via a webhook)
//! and manual dashboard upload (a report file). Keeping it in one place means an uploaded report
//! is processed *identically* to a webhook-delivered one.
//!
//! Ingestion streams: the connector parses records off a `Read` in a blocking task and hands
//! fixed-size batches across a bounded channel to the aggregator here. Each batch is folded into
//! per-day sufficient statistics (never stored row-by-row), so peak memory is O(distinct buckets)
//! for one report — clusters × days × bands × channels, a few MB — not O(transactions). As batches
//! flow we also fold the report's shape (period, currencies, countries, volume) for the history
//! record and tick per-job progress. When the report is fully aggregated we insert the buckets once.

use std::collections::BTreeSet;
use std::io::{Cursor, Read};

use bytes::Bytes;
use chrono::NaiveDate;

use crate::config::ClickHouseAnalyticsConfig;

use super::fit::{self, FitSummary};
use super::rollup::RollupAccumulator;
use super::sink;
use super::source::{parse_in_batches, ConnectorRegistry};
use super::store;
use super::types::{IngestError, SettledFeeRow};

/// Rows per staging INSERT. Bounds both the parsed-row buffer and the ClickHouse request body.
/// ~50k JSONEachRow rows is a few MB — well within a single insert, far below a memory concern.
const BATCH_SIZE: usize = 50_000;

/// Depth of the parse→insert channel. Small on purpose: it provides backpressure so the parser
/// can run at most this many batches ahead of the (slower) ClickHouse inserts.
const CHANNEL_DEPTH: usize = 2;

/// Outcome of ingesting one report — the fit result plus the report's shape (for history).
#[derive(Debug, Clone)]
pub struct IngestOutcome {
    pub staged: usize,
    pub report_date: String,
    pub period_start: Option<NaiveDate>,
    pub period_end: Option<NaiveDate>,
    /// Distinct settlement currencies, sorted.
    pub currencies: Vec<String>,
    /// Distinct issuer countries, sorted.
    pub countries: Vec<String>,
    pub total_gross: f64,
    pub summary: FitSummary,
}

impl IngestOutcome {
    /// Map to the history/completion record persisted by both the worker and the manual task.
    pub fn to_completion(&self) -> store::Completion {
        store::Completion {
            staged_rows: self.staged as i64,
            report_date: NaiveDate::parse_from_str(&self.report_date, "%Y-%m-%d").ok(),
            period_start: self.period_start,
            period_end: self.period_end,
            currencies: self.currencies.clone(),
            countries: self.countries.clone(),
            total_gross: self.total_gross,
            total_clusters: self.summary.total_clusters as i64,
            good_clusters: self.summary.good_clusters as i64,
        }
    }
}

/// Normalize, stage, and fit an already-buffered report for `(connector, account, merchant_id)`.
/// Used by the worker, whose connector download is buffered; the `Bytes` streams through the same
/// batched path (wrapped in a `Cursor`, no copy). `progress_job`, when set, is the `cost_ingestion`
/// row id to tick staged-row progress against.
pub async fn ingest_report_bytes(
    clickhouse: &ClickHouseAnalyticsConfig,
    connector: &str,
    account: &str,
    merchant_id: &str,
    bytes: Bytes,
    progress_job: Option<&str>,
) -> Result<IngestOutcome, IngestError> {
    ingest_report_reader(
        clickhouse,
        connector,
        account,
        merchant_id,
        Box::new(Cursor::new(bytes)),
        progress_job,
    )
    .await
}

/// Normalize, stage, and fit a report read from `reader`, streaming in batches. `reader` is moved
/// into a blocking task; callers with a file (manual upload) pass it directly so the report is
/// never fully resident. `progress_job`, when set, is the `cost_ingestion` row id to tick progress.
pub async fn ingest_report_reader(
    clickhouse: &ClickHouseAnalyticsConfig,
    connector: &str,
    account: &str,
    merchant_id: &str,
    reader: Box<dyn Read + Send>,
    progress_job: Option<&str>,
) -> Result<IngestOutcome, IngestError> {
    let registry = ConnectorRegistry::with_builtins();
    let source = registry.get(connector)?;

    // The merchant's saved column mapping for this settlement source, if they had to map their
    // report's labels onto the connector's. Empty (the common case) means the file is parsed exactly
    // as it always was. Loaded here rather than passed in so *every* caller — manual upload, webhook
    // worker, poller — applies it without having to remember to.
    let mapping = super::mapping::load(merchant_id, connector, account).await?;

    // The fit runs today (the snapshot's `report_date` = fit-run date); it windows on transaction
    // date internally, so a monthly file, daily files, or a mix all fold into the same model.
    let report_date = crate::utils::date_time::now().date().to_string();
    // Rows whose report carries no transaction date are dated to the ingest day for the rollup.
    let fallback_date = NaiveDate::parse_from_str(&report_date, "%Y-%m-%d")
        .map_err(|e| IngestError::Storage(format!("bad report date: {e}")))?;

    // Blocking CSV parse → bounded channel → async aggregation. `blocking_send` inside the parser
    // applies backpressure, so parsing never runs more than CHANNEL_DEPTH batches ahead.
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Vec<SettledFeeRow>>(CHANNEL_DEPTH);
    let parse = tokio::task::spawn_blocking(move || {
        // Connector parses row-by-row; `parse_in_batches` does the generic accumulate-and-flush.
        parse_in_batches(source.as_ref(), reader, &mapping, BATCH_SIZE, |batch| {
            tx.blocking_send(batch)
                // The receiver drops only when the consume loop has already errored out; end the
                // parse quietly and let that error surface to the caller.
                .map_err(|_| IngestError::Storage("aggregate side closed".to_string()))
        })
    });

    // Folded as batches flow, for the history record.
    let mut currencies: BTreeSet<String> = BTreeSet::new();
    let mut countries: BTreeSet<String> = BTreeSet::new();
    let mut period_start: Option<NaiveDate> = None;
    let mut period_end: Option<NaiveDate> = None;
    let mut total_gross = 0.0_f64;

    // The report is aggregated into per-day sufficient statistics as it streams (never stored
    // row-by-row); we insert the buckets once, after the whole report is folded.
    let mut acc = RollupAccumulator::new();
    let mut processed = 0usize;
    while let Some(batch) = rx.recv().await {
        for row in &batch {
            if !row.currency.is_empty() {
                currencies.insert(row.currency.clone());
            }
            if !row.issuer_country.is_empty() {
                countries.insert(row.issuer_country.clone());
            }
            if let Some(d) = row.txn_date {
                period_start = Some(period_start.map_or(d, |p| p.min(d)));
                period_end = Some(period_end.map_or(d, |p| p.max(d)));
            }
            total_gross += row.gross;
            acc.add_row(row, fallback_date);
        }
        processed += batch.len();
        if let Some(id) = progress_job {
            // Best-effort: a failed progress tick must not fail the ingest.
            let _ = store::update_progress(id, processed as i64).await;
        }
    }
    drop(rx);

    // Join the parser. A parse error wins over any "aggregate side closed" it may have observed.
    let parse_result = parse
        .await
        .map_err(|e| IngestError::Storage(format!("parse task panicked: {e}")))?;
    parse_result?;

    // One insert of the fully-aggregated buckets, then fit from what the rollup now holds. The
    // history record reports transactions *processed* (`staged`), not bucket count.
    // Grab the per-BIN observations first (borrows) before `into_rows` consumes the accumulator.
    let bin_rows = acc.bin_rows();
    let rows = acc.into_rows();
    sink::insert_daily_stats(
        clickhouse,
        connector,
        account,
        merchant_id,
        progress_job.unwrap_or_default(),
        &rows,
    )
    .await?;

    // Feed the global BIN → card-product map (Open Risk #4 seed). Best-effort: this is additive data
    // collection that no live path depends on yet (Step B), so a failure here must not fail the
    // report's ingest/fit — log and continue.
    if let Err(e) = sink::insert_bin_product(clickhouse, &bin_rows).await {
        crate::logger::warn!(
            tag = "cost_ingestion",
            "bin_product insert failed ({} bins): {}",
            bin_rows.len(),
            e
        );
    }

    let summary =
        fit::fit_snapshot(clickhouse, connector, account, merchant_id, &report_date).await?;

    Ok(IngestOutcome {
        staged: processed,
        report_date,
        period_start,
        period_end,
        currencies: currencies.into_iter().collect(),
        countries: countries.into_iter().collect(),
        total_gross,
        summary,
    })
}
