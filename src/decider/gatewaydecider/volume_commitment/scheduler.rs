//! Owns the clock only: POSTs the main server's run endpoint when each merchant's cadence comes
//! due — or its cycle rolls over, whichever is sooner, since the plan (and its Redis key) dies at
//! the cycle boundary and waiting out the cadence would leave the fresh cycle unsteered and
//! "Forecast pending" for up to a whole interval. Every replica may run one; a Redis lease per
//! merchant and interval makes sure only one does.

use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use futures::FutureExt;
use serde::Serialize;

use super::controller::{self, RunReport};
use super::Deps;
use crate::logger;

/// Watches the clock for every merchant with commitments.
pub struct Scheduler {
    deps: Arc<Deps>,
    http: reqwest::Client,
    /// Presented to the main server's run endpoint, which sits behind the same auth as any write.
    admin_secret: String,
}

/// One row of the schedule, served by `GET /schedule` for inspection.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleEntry {
    pub merchant_id: String,
    /// This merchant's forecast cadence.
    pub every_secs: u64,
    /// Soonest instant any of this merchant's commitments closes its cycle. The plan expires
    /// there, so the forecast is re-run at rollover rather than waiting out the cadence.
    pub period_end_epoch_secs: i64,
    /// When the current interval's run was claimed, by any replica. `None` until this merchant's
    /// first run, or once the last one has expired.
    pub last_notified_at_epoch_secs: Option<i64>,
    /// Zero means it fires on the next tick.
    pub due_in_secs: u64,
}

/// Seconds until a merchant is due again: its cadence less what has elapsed since the last run
/// was claimed, but never later than its cycle's rollover — the plan dies there. Never run, or
/// the claim expired, means now.
fn due_in_secs(
    every_secs: u64,
    last_started_at: Option<i64>,
    now_epoch_secs: i64,
    until_rollover_secs: u64,
) -> u64 {
    let cadence = match last_started_at {
        None => 0,
        Some(at) => {
            let elapsed = u64::try_from(now_epoch_secs - at).unwrap_or(0);
            every_secs.saturating_sub(elapsed)
        }
    };
    cadence.min(until_rollover_secs)
}

/// Seconds from `now` until the soonest cycle end, zero once it has passed.
fn until_rollover_secs(period_end_epoch_secs: i64, now_epoch_secs: i64) -> u64 {
    u64::try_from(period_end_epoch_secs.saturating_sub(now_epoch_secs)).unwrap_or(0)
}

impl Scheduler {
    pub fn new(deps: Arc<Deps>, admin_secret: String) -> Self {
        Self {
            deps,
            admin_secret,
            // Short timeout so a wedged run cannot pin the loop; the next tick retries.
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .unwrap_or_default(),
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

    /// One pass: claim and notify every job that has come due. The lease lives in Redis for one
    /// interval — capped at the merchant's cycle end, so it expires with the plan and the fresh
    /// cycle is forecast on the next tick — and it doubles as the last-run record: a restart does
    /// not make everyone due, and a second replica finds the interval already taken.
    async fn tick_once(&self) {
        for entry in self.due_now().await {
            let ttl = entry
                .every_secs
                .min(until_rollover_secs(
                    entry.period_end_epoch_secs,
                    Utc::now().timestamp(),
                ))
                .max(1);
            if !self
                .deps
                .state
                .try_acquire_run_lease(&entry.merchant_id, ttl)
                .await
            {
                continue;
            }
            if !self.notify(&entry.merchant_id).await {
                // Give the interval back so the next tick retries rather than waiting it out.
                self.deps.state.release_run_lease(&entry.merchant_id).await;
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
        let now = Utc::now().timestamp();
        let mut entries = Vec::new();

        for merchant_id in self.deps.inputs.list_active().await {
            let Some(inputs) = self.deps.inputs.load(&merchant_id).await else {
                continue;
            };
            let every_secs = controller::interval_secs(&inputs, &self.deps.config);
            // `load` recomputes cycle windows from "now", so right after a rollover this is
            // already the new cycle's end — the boundary itself is enforced by the lease TTL.
            let period_end_epoch_secs = inputs
                .commitments
                .iter()
                .map(|c| c.period_end_ms / 1000)
                .min()
                .unwrap_or(i64::MAX);
            let last_started_at = self.deps.state.last_run_started_at(&merchant_id).await;
            entries.push(ScheduleEntry {
                every_secs,
                period_end_epoch_secs,
                last_notified_at_epoch_secs: last_started_at,
                due_in_secs: due_in_secs(
                    every_secs,
                    last_started_at,
                    now,
                    until_rollover_secs(period_end_epoch_secs, now),
                ),
                merchant_id,
            });
        }
        entries
    }

    /// Tell the main server it is time. True only when the run happened: a rejected call, an
    /// unreachable server, or a run that failed to measure all hand the lease back so the next
    /// tick retries rather than waiting out a whole interval.
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
                        report.merchants_failed == 0
                    }
                    // The run happened; only the reply was unreadable, so it still counts as done.
                    Err(error) => {
                        logger::warn!(
                            tag = "volume_commitment",
                            merchant_id = merchant_id,
                            "forecast run accepted but its reply could not be read: {error}"
                        );
                        true
                    }
                }
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

#[cfg(test)]
mod tests {
    use super::{due_in_secs, until_rollover_secs};

    /// No rollover in sight — cadence alone decides.
    const FAR: u64 = u64::MAX;

    #[test]
    fn a_merchant_never_run_is_due_now() {
        assert_eq!(due_in_secs(3600, None, 1_000_000, FAR), 0);
    }

    #[test]
    fn a_fresh_lease_waits_out_the_rest_of_the_interval() {
        assert_eq!(due_in_secs(3600, Some(1_000_000), 1_000_600, FAR), 3000);
    }

    /// Past the interval the lease has expired anyway; the entry reads as due, never negative.
    #[test]
    fn an_old_lease_reads_as_due() {
        assert_eq!(due_in_secs(60, Some(1_000_000), 1_000_600, FAR), 0);
    }

    /// Clock skew between replicas cannot push a merchant into the future.
    #[test]
    fn a_lease_from_the_future_is_treated_as_just_taken() {
        assert_eq!(due_in_secs(60, Some(1_000_100), 1_000_000, FAR), 60);
    }

    /// The plan dies at cycle end, so due-ness is capped there: a lease with 3000 s of cadence
    /// left still comes due at a rollover 100 s away.
    #[test]
    fn a_rollover_overrides_the_cadence() {
        assert_eq!(due_in_secs(3600, Some(1_000_000), 1_000_600, 100), 100);
        assert_eq!(due_in_secs(3600, Some(1_000_000), 1_000_600, 0), 0);
    }

    #[test]
    fn time_to_rollover_never_goes_negative() {
        assert_eq!(until_rollover_secs(1_000_100, 1_000_000), 100);
        assert_eq!(until_rollover_secs(1_000_000, 1_000_100), 0);
    }
}
