//! Generic background report poller for **pull** connectors — those discovered by polling their
//! reporting API rather than by a pushed webhook. A connector opts in by implementing
//! [`SettlementReportSource::is_pull`] + `poll_ready_reports`.
//!
//! Each cycle it sweeps every registered pull connector, lists each configured source's ready
//! reports, and enqueues a `pending` job per report (`source = "poll"`). From there it is identical
//! to a webhook delivery: the existing `worker` claims the job, downloads, parses, and fits. Enqueue
//! is idempotent on `(connector, report_id)`, so re-listing an already-ingested report is a no-op.
//!
//! Modeled on `worker::spawn` — a panic-isolated interval loop that runs only where
//! `cost_ingestion.report_poll_enabled` is set, so a dedicated ingest deployment owns it. Nothing
//! here names a specific connector: adding a pull PSP requires no change to this file.

use std::time::Duration;

use futures::FutureExt;

use crate::config::CostIngestionConfig;
use crate::logger;

use super::source::SettlementReportSource;
use super::{creds, store, ConnectorCredsStore, ConnectorRegistry};

/// Spawn the recurring report poll loop. Call once at startup after `APP_STATE` is set. A no-op
/// unless `report_poll_enabled` is true.
pub fn spawn(config: CostIngestionConfig) {
    if !config.report_poll_enabled {
        logger::info!(tag = "report_poller", "report poller disabled");
        return;
    }
    let interval = Duration::from_secs(config.report_poll_interval_secs.max(1));

    tokio::spawn(async move {
        logger::info!(
            tag = "report_poller",
            "report poller started; interval {:?}",
            interval
        );
        let mut ticker = tokio::time::interval(interval);
        loop {
            ticker.tick().await;
            // Isolate each cycle so a panic doesn't kill the loop.
            if std::panic::AssertUnwindSafe(run_once())
                .catch_unwind()
                .await
                .is_err()
            {
                logger::error!(
                    tag = "report_poller",
                    "poll cycle panicked; continuing next cycle"
                );
            }
        }
    });
}

async fn run_once() {
    let registry = ConnectorRegistry::with_builtins();
    let pull = registry.pull_sources();
    if pull.is_empty() {
        return;
    }

    // Build the credential store once per cycle; all sources share the same keyring.
    let app_state = crate::app::get_tenant_app_state().await;
    let cfg = &app_state.config.cost_ingestion;
    let creds_store = match ConnectorCredsStore::from_keyring(
        &cfg.creds_encryption_current,
        &cfg.creds_encryption_keys,
    ) {
        Some(s) => s,
        None => {
            logger::warn!(
                tag = "report_poller",
                "credential encryption keyring not configured; skipping cycle"
            );
            return;
        }
    };

    for source in pull {
        let connector = source.connector();
        let sources = match creds::list_poll_sources(connector).await {
            Ok(s) => s,
            Err(e) => {
                logger::warn!(
                    tag = "report_poller",
                    "list poll sources for {} failed: {:?}",
                    connector,
                    e
                );
                continue;
            }
        };
        for src in sources {
            if let Err(e) = poll_source(&creds_store, source.as_ref(), &src.account).await {
                // One bad source (expired key, API error) must not stop the others.
                logger::warn!(
                    tag = "report_poller",
                    "poll of {}/{} failed: {:?}",
                    connector,
                    src.account,
                    e
                );
            }
        }
    }
}

/// List one source's ready reports and enqueue any not already seen.
async fn poll_source(
    creds_store: &ConnectorCredsStore,
    source: &dyn SettlementReportSource,
    account: &str,
) -> Result<(), super::IngestError> {
    let connector = source.connector();
    let resolved = creds_store
        .get(connector, account)
        .await?
        .ok_or_else(|| {
            super::IngestError::Storage(format!("no credentials for {connector}/{account}"))
        })?;

    let ready = source.poll_ready_reports(&resolved.creds).await?;
    let mut enqueued = 0usize;
    for report in ready {
        // Idempotent on (connector, report_id): already-enqueued reports are skipped.
        // `source = "poll"` distinguishes pull-discovered reports from pushed webhooks in history.
        let created = store::enqueue_pending(
            connector,
            account,
            &resolved.merchant_id,
            &report.report_id,
            &report.report_ref,
            "poll",
        )
        .await?;
        if created {
            enqueued += 1;
        }
    }
    if enqueued > 0 {
        logger::info!(
            tag = "report_poller",
            "{}/{}: enqueued {} new report(s)",
            connector,
            account,
            enqueued
        );
    }
    Ok(())
}
