//! The schedule — the only part that knows about time. It holds no commitment maths and no plan:
//! it watches each merchant's forecast cadence and POSTs the main server's run endpoint when one
//! comes due. The seam is HTTP, so *when* can later move to its own deployment without *what*
//! changing at all. Forecasting is the only scheduled job.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use futures::FutureExt;
use serde::Serialize;
use tokio::sync::RwLock;

use super::controller::{self, RunReport};
use super::Deps;
use crate::logger;

/// Watches the clock for every merchant with commitments.
pub struct Scheduler {
    deps: Arc<Deps>,
    http: reqwest::Client,
    /// Presented to the main server's run endpoint, which sits behind the same auth as any write.
    admin_secret: String,
    /// When each merchant was last handed to the main server.
    last_notified: RwLock<HashMap<String, DateTime<Utc>>>,
}

/// One row of the schedule, served by `GET /schedule` for inspection.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleEntry {
    pub merchant_id: String,
    /// This merchant's forecast cadence.
    pub every_secs: u64,
    /// `None` until this merchant's first run.
    pub last_notified_at_epoch_secs: Option<i64>,
    /// Zero means it fires on the next tick.
    pub due_in_secs: u64,
}

impl Scheduler {
    pub fn new(deps: Arc<Deps>, admin_secret: String) -> Self {
        Self {
            deps,
            admin_secret,
            // The main server is normally this same process; a short timeout keeps a wedged run
            // from pinning the loop, and the next tick simply tries again.
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
            last_notified: RwLock::new(HashMap::new()),
        }
    }

    /// Start the loop. Wakes on a fixed tick; each merchant comes due on its own cadence.
    pub fn spawn(self: Arc<Self>) {
        let tick = Duration::from_secs(self.deps.config.tick_secs.max(1));

        tokio::spawn(async move {
            logger::info!(
                tag = "volume_commitment",
                "volume commitment scheduler started; wakes every {:?}, notifies {}",
                tick,
                self.deps.config.main_server_url
            );
            let mut ticker = tokio::time::interval(tick);
            // A slow pass should not queue up the wake-ups it missed and fire them back to back.
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                ticker.tick().await;
                // Contain a panic so one bad pass cannot take the schedule down with it.
                if std::panic::AssertUnwindSafe(self.tick_once())
                    .catch_unwind()
                    .await
                    .is_err()
                {
                    logger::error!(
                        tag = "volume_commitment",
                        "panic in scheduler pass; the loop continues"
                    );
                }
            }
        });
    }

    /// One pass: notify every job that has come due.
    async fn tick_once(&self) {
        for entry in self.due_now().await {
            if self.notify(&entry.merchant_id).await {
                self.last_notified
                    .write()
                    .await
                    .insert(entry.merchant_id, Utc::now());
            }
        }
    }

    /// Everything currently due. A never-run merchant is due immediately, so one added mid-cycle
    /// gets a plan on the next tick.
    async fn due_now(&self) -> Vec<ScheduleEntry> {
        self.schedule()
            .await
            .into_iter()
            .filter(|entry| entry.due_in_secs == 0)
            .collect()
    }

    /// Every merchant's cadence and how long until its next forecast fires.
    pub async fn schedule(&self) -> Vec<ScheduleEntry> {
        let now = Utc::now();
        let last = self.last_notified.read().await.clone();
        let mut entries = Vec::new();

        for merchant_id in self.deps.inputs.list_active().await {
            let Some(inputs) = self.deps.inputs.load(&merchant_id).await else {
                continue;
            };
            let every_secs = controller::interval_secs(&inputs, &self.deps.config);
            let last_notified_at = last.get(&merchant_id).copied();
            let due_in_secs = match last_notified_at {
                None => 0,
                Some(at) => {
                    let elapsed = (now - at).num_seconds().max(0) as u64;
                    every_secs.saturating_sub(elapsed)
                }
            };
            entries.push(ScheduleEntry {
                merchant_id: merchant_id.clone(),
                every_secs,
                last_notified_at_epoch_secs: last_notified_at.map(|at| at.timestamp()),
                due_in_secs,
            });
        }
        entries
    }

    /// Tell the main server it is time. A failed notify is left un-stamped so the next tick
    /// retries it rather than waiting out a whole interval.
    async fn notify(&self, merchant_id: &str) -> bool {
        let url = format!(
            "{}/volume-commitment/run-forecast",
            self.deps.config.main_server_url.trim_end_matches('/'),
        );

        let request = self
            .http
            .post(&url)
            .header("x-admin-secret", &self.admin_secret)
            .query(&[("merchant_id", merchant_id)]);
        match request.send().await {
            Ok(response) if response.status().is_success() => {
                match response.json::<RunReport>().await {
                    Ok(report) => {
                        logger::info!(
                            tag = "volume_commitment",
                            merchant_id = merchant_id,
                            "forecast run done: processed={} skipped={} failed={}",
                            report.merchants_processed,
                            report.merchants_skipped,
                            report.merchants_failed,
                        );
                    }
                    // The run happened; only the reply was unreadable. Stamping it anyway is right —
                    // re-running on the next tick would repeat work that already succeeded.
                    Err(error) => logger::warn!(
                        tag = "volume_commitment",
                        merchant_id = merchant_id,
                        "forecast run accepted but its reply could not be read: {error}"
                    ),
                }
                true
            }
            Ok(response) => {
                logger::error!(
                    tag = "volume_commitment",
                    merchant_id = merchant_id,
                    "forecast run rejected by the main server: HTTP {}",
                    response.status()
                );
                false
            }
            Err(error) => {
                logger::error!(
                    tag = "volume_commitment",
                    merchant_id = merchant_id,
                    "could not reach the main server at {url} for a forecast run: {error}"
                );
                false
            }
        }
    }
}
