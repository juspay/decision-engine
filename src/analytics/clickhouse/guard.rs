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
//! * **Concurrency.** Every analytics read on this pod holds a [`QueryPermit`], so a dashboard
//!   burst queues here instead of stacking parallel queries on ClickHouse.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use clickhouse::{Client, Row};
use serde::Deserialize;
use tokio::sync::{Semaphore, SemaphorePermit};

use crate::error::ApiError;

/// Per-query settings that bound working memory. Shared with the cost pipeline's HTTP client,
/// which attaches them as URL parameters.
pub const MEMORY_BOUNDED_SETTINGS: &[(&str, &str)] = &[
    ("max_threads", "2"),
    ("max_bytes_before_external_group_by", "20000000"),
    ("max_bytes_before_external_sort", "20000000"),
];

/// Dashboard reads that run longer than this are abandoned by the server rather than holding
/// memory while the caller has already timed out.
const ANALYTICS_MAX_EXECUTION_SECS: &str = "30";

/// Analytics queries this pod runs against ClickHouse at once.
const MAX_CONCURRENT_QUERIES: usize = 4;

/// How long a read waits for a free slot before failing.
const PERMIT_WAIT: Duration = Duration::from_secs(15);

static QUERY_SLOTS: Semaphore = Semaphore::const_new(MAX_CONCURRENT_QUERIES);

static SETTINGS_ACCEPTED: AtomicBool = AtomicBool::new(false);

pub type QueryPermit = SemaphorePermit<'static>;

pub async fn acquire() -> Result<QueryPermit, ApiError> {
    match tokio::time::timeout(PERMIT_WAIT, QUERY_SLOTS.acquire()).await {
        Ok(Ok(permit)) => Ok(permit),
        Ok(Err(_closed)) => Err(ApiError::DatabaseError),
        Err(_elapsed) => {
            crate::logger::warn!(
                max_concurrent = MAX_CONCURRENT_QUERIES,
                wait_secs = PERMIT_WAIT.as_secs(),
                "clickhouse analytics query slots exhausted; rejecting read"
            );
            Err(ApiError::DatabaseError)
        }
    }
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
        .with_option("max_execution_time", ANALYTICS_MAX_EXECUTION_SECS)
}
