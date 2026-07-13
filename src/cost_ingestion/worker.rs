//! Background ingest worker: drains pending `cost_ingestion` jobs (webhook-delivered reports),
//! downloading, staging, and fitting each.
//!
//! Modeled on `sr_auto_calibration::spawn` — a panic-isolated interval loop. Everything heavy
//! (download, parse, ClickHouse insert, fit) happens here, off the webhook's request path. Runs
//! only where `cost_ingestion.worker_enabled` is set, so a dedicated ingest deployment can own it.
//! Manual uploads do NOT flow through here — they run their own task (see `routes::report_upload`)
//! but write to the same `cost_ingestion` table, so history and progress are unified.

use std::time::Duration;

use futures::FutureExt;

use crate::app::get_tenant_app_state;
use crate::config::{ClickHouseAnalyticsConfig, CostIngestionConfig};
use crate::logger;
use crate::storage::types::CostIngestion;

use super::pipeline::IngestOutcome;
use super::types::{IngestError, ReportNotification};
use super::{pipeline, store, ConnectorCredsStore, ConnectorRegistry};

/// Spawn the recurring ingest loop. Call once at startup after `APP_STATE` is set. A no-op unless
/// `worker_enabled` is true. The ClickHouse config is passed in because it lives on the global
/// config, not the per-tenant config the worker later resolves.
pub fn spawn(config: CostIngestionConfig, clickhouse: ClickHouseAnalyticsConfig) {
    if !config.worker_enabled {
        logger::info!(tag = "ingest_worker", "settlement ingest worker disabled");
        return;
    }
    let interval = Duration::from_secs(config.worker_interval_secs.max(1));
    let batch = config.worker_batch_size.max(1);

    tokio::spawn(async move {
        logger::info!(
            tag = "ingest_worker",
            "settlement ingest worker started; interval {:?}, batch {}",
            interval,
            batch
        );
        let mut ticker = tokio::time::interval(interval);
        loop {
            ticker.tick().await;
            // Isolate each cycle so a panic doesn't kill the loop.
            if std::panic::AssertUnwindSafe(run_once(batch, &clickhouse))
                .catch_unwind()
                .await
                .is_err()
            {
                logger::error!(
                    tag = "ingest_worker",
                    "ingest cycle panicked; continuing next cycle"
                );
            }
        }
    });
}

async fn run_once(batch: usize, clickhouse: &ClickHouseAnalyticsConfig) {
    let claimed = match store::claim_pending(batch).await {
        Ok(rows) => rows,
        Err(e) => {
            logger::warn!(tag = "ingest_worker", "claim failed: {:?}", e);
            return;
        }
    };

    for job in claimed {
        let id = job.id.clone();
        let merchant_id = job.merchant_id.clone();
        match process(job, clickhouse).await {
            Ok(outcome) => {
                log_fit(&id, &outcome);
                if let Err(e) = store::mark_completed(&id, &outcome.to_completion()).await {
                    logger::warn!(
                        tag = "ingest_worker",
                        "mark_completed {} failed: {:?}",
                        id,
                        e
                    );
                }
                // Serve the freshly-fitted models immediately (don't wait for the periodic refresh).
                // Per-merchant: only this merchant is rebuilt, off the global-refresh path.
                if let Err(e) = super::serving::refresh_merchant(clickhouse, &merchant_id).await {
                    logger::warn!(
                        tag = "ingest_worker",
                        "serving refresh after ingest failed: {}",
                        e
                    );
                }
            }
            Err(e) => {
                let msg = format!("{e:?}");
                logger::warn!(tag = "ingest_worker", "job {} failed: {}", id, msg);
                if let Err(e2) = store::mark_failed(&id, &msg).await {
                    logger::warn!(tag = "ingest_worker", "mark_failed {} failed: {:?}", id, e2);
                }
            }
        }
    }
}

/// Download → parse → stage → fit one job. Errors bubble up so `run_once` parks the job as failed.
async fn process(
    job: CostIngestion,
    clickhouse: &ClickHouseAnalyticsConfig,
) -> Result<IngestOutcome, IngestError> {
    let app_state = get_tenant_app_state().await;
    let cfg = &app_state.config.cost_ingestion;

    let registry = ConnectorRegistry::with_builtins();
    let source = registry.get(&job.connector)?;

    // Credentials for this (connector, account).
    let store_ = ConnectorCredsStore::from_keyring(
        &cfg.creds_encryption_current,
        &cfg.creds_encryption_keys,
    )
    .ok_or_else(|| {
        IngestError::Storage("credential encryption keyring not configured".to_string())
    })?;
    let resolved = store_
        .get(&job.connector, &job.account)
        .await?
        .ok_or_else(|| {
            IngestError::Storage(format!(
                "no credentials for {}/{}",
                job.connector, job.account
            ))
        })?;

    // Download the report (buffered) via the connector, then normalize it.
    let note = ReportNotification {
        notification_id: job.notification_id.clone().unwrap_or_default(),
        report_ref: job.report_ref.clone(),
        report_date: None,
        account: job.account.clone(),
    };
    let bytes = source.download_report(&resolved.creds, &note).await?;

    // Same parse → stage → fit path a manual upload uses; tick progress against this job row.
    pipeline::ingest_report_bytes(
        clickhouse,
        &job.connector,
        &job.account,
        &job.merchant_id,
        bytes,
        Some(job.id.as_str()),
    )
    .await
}

/// Log the fit outcome. A snapshot with no GOOD clusters is written but flagged — serving only ever
/// reads GOOD rows (§10), so this is "no coverage", never a bad cost.
fn log_fit(id: &str, outcome: &IngestOutcome) {
    if outcome.summary.good_clusters == 0 {
        logger::warn!(
            tag = "ingest_worker",
            "job {} fit produced 0 GOOD clusters out of {} — snapshot has no usable cost models",
            id,
            outcome.summary.total_clusters
        );
    } else {
        logger::info!(
            tag = "ingest_worker",
            "job {} fit: {}/{} clusters GOOD ({} rows staged)",
            id,
            outcome.summary.good_clusters,
            outcome.summary.total_clusters,
            outcome.staged
        );
    }
}
