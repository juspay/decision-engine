//! Keeps analytics reads inside ClickHouse's per-query memory budget.
//!
//! Two independent limits:
//!
//! * **Query settings.** [`MEMORY_BOUNDED_SETTINGS`] caps read threads, the dominant cost when a query parses `details`, and makes large
//!   GROUP BY / ORDER BY spill to disk instead of growing in memory. ClickHouse refuses every
//!   per-query setting for a `readonly=1` profile (URL parameter and `SETTINGS` clause alike), so
//!   [`detect_settings_support`] reads the profile's `readonly` level once and [`bounded`] only
//!   attaches the settings when the server will accept them. Under `readonly=1` the same settings have to live in the
//!   user's server-side profile.
//! * **Concurrency.** Every analytics read on this pod runs through [`run`], which holds one of
//!   [`MAX_CONCURRENT_QUERIES`] slots, so a dashboard burst queues here instead of stacking
//!   parallel queries on ClickHouse. A single request can fan out more reads than there are slots
//!   (the overview alone issues nine), so its own queries queue behind each other; the wait is
//!   sized for that.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use clickhouse::{Client, Row};
use serde::Deserialize;
use tokio::sync::Semaphore;

use crate::error::ApiError;

/// Per-query settings that bound working memory. Shared with the cost pipeline's HTTP client,
/// which attaches them as URL parameters.
pub const MEMORY_BOUNDED_SETTINGS: &[(&str, &str)] = &[
    ("max_threads", "2"),
    ("max_bytes_before_external_group_by", "20000000"),
    ("max_bytes_before_external_sort", "20000000"),
];

/// How long one read may hold its slot. Enforced here rather than left to `max_execution_time`,
/// which a `readonly=1` profile refuses, so the slot is released on time whatever the server
/// accepted. Abandoning the response frees the slot immediately; ClickHouse stops the query when
/// it notices the closed connection, or at `max_execution_time` where that was accepted.
const QUERY_TIMEOUT: Duration = Duration::from_secs(30);

/// Analytics queries this pod runs against ClickHouse at once.
const MAX_CONCURRENT_QUERIES: usize = 4;

/// How long a read waits for a free slot before failing. Longer than [`QUERY_TIMEOUT`], so a
/// waiter gives up only once every slot ahead of it has had its full budget and been released —
/// never while the queries holding them are still within it.
const PERMIT_WAIT: Duration = Duration::from_secs(40);

const _: () = assert!(
    PERMIT_WAIT.as_secs() > QUERY_TIMEOUT.as_secs(),
    "a read must not give up waiting for a slot that a running query may still hold"
);

static QUERY_SLOTS: Semaphore = Semaphore::const_new(MAX_CONCURRENT_QUERIES);

static SETTINGS_ACCEPTED: AtomicBool = AtomicBool::new(false);

/// Run one ClickHouse read: wait for a free slot, then give the read its own deadline. `Err` means
/// the pod shed the read (no slot, or the read outran [`QUERY_TIMEOUT`]); `Ok` carries whatever the
/// query itself returned.
pub async fn run<F: std::future::Future>(query: F) -> Result<F::Output, ApiError> {
    run_with(&QUERY_SLOTS, query).await
}

async fn run_with<F: std::future::Future>(
    slots: &Semaphore,
    query: F,
) -> Result<F::Output, ApiError> {
    let permit = match tokio::time::timeout(PERMIT_WAIT, slots.acquire()).await {
        Ok(Ok(permit)) => permit,
        Ok(Err(_closed)) => return Err(ApiError::DatabaseError),
        Err(_elapsed) => {
            crate::logger::warn!(
                max_concurrent = MAX_CONCURRENT_QUERIES,
                wait_secs = PERMIT_WAIT.as_secs(),
                "clickhouse analytics query slots exhausted; rejecting read"
            );
            return Err(ApiError::DatabaseError);
        }
    };

    let outcome = tokio::time::timeout(QUERY_TIMEOUT, query).await;
    drop(permit);

    outcome.map_err(|_elapsed| {
        crate::logger::warn!(
            timeout_secs = QUERY_TIMEOUT.as_secs(),
            settings_accepted = settings_accepted(),
            "clickhouse analytics read exceeded its deadline; abandoning it"
        );
        ApiError::DatabaseError
    })
}

/// Whether the configured ClickHouse user accepts per-query settings, as detected by the last
/// [`detect_settings_support`] call.
pub fn settings_accepted() -> bool {
    SETTINGS_ACCEPTED.load(Ordering::Relaxed)
}

#[derive(Debug, Deserialize, Row)]
struct ReadonlyRow {
    readonly: u64,
}

/// Read the user's `readonly` level once and record whether per-query settings are accepted.
pub async fn detect_settings_support(client: &Client) {
    let readonly = client
        .query("SELECT toUInt64(getSetting('readonly')) AS readonly")
        .fetch_one::<ReadonlyRow>()
        .await;

    let accepted = match readonly {
        Ok(ReadonlyRow { readonly: 1 }) => {
            crate::logger::warn!(
                "clickhouse analytics user has readonly=1, which rejects per-query settings; \
                 max_threads, max_bytes_before_external_group_by and \
                 max_bytes_before_external_sort must be set on the user's server-side profile"
            );
            false
        }
        Ok(ReadonlyRow { readonly }) => {
            crate::logger::info!(
                readonly,
                "clickhouse analytics reads use memory-bounded query settings"
            );
            true
        }
        Err(error) => {
            crate::logger::warn!(
                ?error,
                "could not read the clickhouse profile's readonly level; sending no query settings"
            );
            false
        }
    };
    SETTINGS_ACCEPTED.store(accepted, Ordering::Relaxed);
}

/// `client` with [`MEMORY_BOUNDED_SETTINGS`] attached when the server accepts them, unchanged
/// otherwise.
pub fn bounded(client: &Client) -> Client {
    if !settings_accepted() {
        return client.clone();
    }
    MEMORY_BOUNDED_SETTINGS
        .iter()
        .fold(client.clone(), |client, (name, value)| {
            client.with_option(*name, *value)
        })
        .with_option("max_execution_time", QUERY_TIMEOUT.as_secs().to_string())
}

#[cfg(test)]
mod tests {
    use std::future::pending;

    use tokio::sync::Semaphore;

    use super::{run_with, MAX_CONCURRENT_QUERIES, PERMIT_WAIT, QUERY_TIMEOUT};

    /// A slot pool of this module's size, private to one test: the real one is process-wide.
    fn slots() -> &'static Semaphore {
        Box::leak(Box::new(Semaphore::new(MAX_CONCURRENT_QUERIES)))
    }

    /// Occupy `count` slots (or queue for them) with reads that never answer, as queries stuck on
    /// ClickHouse would.
    async fn stall(slots: &'static Semaphore, count: usize) {
        for _ in 0..count {
            tokio::spawn(async move { run_with(slots, pending::<()>()).await });
        }
        tokio::task::yield_now().await;
    }

    #[tokio::test(start_paused = true)]
    async fn a_read_gives_up_its_slot_at_the_deadline() {
        let started = tokio::time::Instant::now();
        assert!(run_with(slots(), pending::<()>()).await.is_err());
        assert_eq!(started.elapsed(), QUERY_TIMEOUT);
    }

    #[tokio::test(start_paused = true)]
    async fn a_waiter_outlasts_a_slot_held_for_a_whole_query_budget() {
        let slots = slots();
        stall(slots, MAX_CONCURRENT_QUERIES).await;

        // Every slot is held for its full budget, which is the case the wait must ride out rather
        // than fail in.
        let waited = tokio::time::Instant::now();
        assert!(run_with(slots, async { 7 }).await.is_ok());
        assert_eq!(waited.elapsed(), QUERY_TIMEOUT);
    }

    #[tokio::test(start_paused = true)]
    async fn a_read_is_shed_once_the_wait_runs_out() {
        let slots = slots();
        // Enough stalled reads to fill the slots twice over, so this read's turn comes only after
        // two full budgets - longer than it is willing to wait.
        stall(slots, MAX_CONCURRENT_QUERIES * 2).await;

        let waited = tokio::time::Instant::now();
        assert!(run_with(slots, async { 7 }).await.is_err());
        assert_eq!(waited.elapsed(), PERMIT_WAIT);
    }
}
