use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use tokio::sync::{mpsc, OwnedSemaphorePermit, Semaphore};

use crate::analytics::clickhouse::ClickHouseAnalyticsStore;
use crate::analytics::events::{ApiEvent, DomainAnalyticsEvent};
use crate::analytics::kafka::KafkaAnalyticsStore;
use crate::analytics::store::{
    AnalyticsReadStore, AnalyticsWriteStore, NoopAnalyticsWriteStore, UnavailableAnalyticsReadStore,
};
use crate::config::AnalyticsConfig;
use crate::error::ConfigurationError;
use crate::metrics::{ANALYTICS_EVENTS_DROPPED_TOTAL, ANALYTICS_SINK_QUEUE_DEPTH};

#[cfg(test)]
mod tests;

/// Max events drained from the queue per publish call. Keeps the publisher ahead of
/// burst producers (a simulator run emits hundreds of events/second) so the bounded
/// queue doesn't fill and drop events.
const PUBLISH_BATCH_SIZE: usize = 200;
// Deferred jobs retain full payloads until the publisher serializes them.
const MAX_DEFERRED_DETAILS: usize = 16;

struct DeferredDetails {
    build: Box<dyn FnOnce(usize) -> Option<String> + Send>,
    _permit: OwnedSemaphorePermit,
}

struct PendingDomainEvent {
    event: DomainAnalyticsEvent,
    details: Option<DeferredDetails>,
}

impl PendingDomainEvent {
    fn prepare(mut self, max_bytes: usize) -> DomainAnalyticsEvent {
        if let Some(details) = self.details {
            self.event.details = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                (details.build)(max_bytes)
            }))
            .unwrap_or_else(|_| {
                crate::logger::warn!(
                    "Failed to prepare analytics details; publishing event without details"
                );
                None
            });
        }
        self.event.details = bound_domain_details(self.event.details, max_bytes);
        self.event
    }
}

fn bound_domain_details(details: Option<String>, max_bytes: usize) -> Option<String> {
    let details = details?;
    if details.len() <= max_bytes {
        return Some(details);
    }
    let marker = serde_json::json!({
        "truncated": true,
        "original_bytes": details.len(),
    })
    .to_string();
    (marker.len() <= max_bytes).then_some(marker)
}

pub(crate) fn serialize_bounded_details<T: serde::Serialize>(
    details: &T,
    max_bytes: usize,
) -> Option<String> {
    struct LimitedWriter {
        bytes: Vec<u8>,
        limit: usize,
    }

    impl std::io::Write for LimitedWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if bytes.len() > self.limit.saturating_sub(self.bytes.len()) {
                return Err(std::io::Error::other("analytics details exceed size limit"));
            }
            self.bytes.extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let mut writer = LimitedWriter {
        bytes: Vec::with_capacity(max_bytes.min(4096)),
        limit: max_bytes,
    };
    serde_json::to_writer(&mut writer, details).ok()?;
    String::from_utf8(writer.bytes).ok()
}

#[derive(Clone)]
pub struct AnalyticsRuntime {
    config: AnalyticsConfig,
    read_store: Arc<dyn AnalyticsReadStore>,
    write_store: Arc<dyn AnalyticsWriteStore>,
    domain_tx: mpsc::Sender<PendingDomainEvent>,
    deferred_details: Arc<Semaphore>,
    api_tx: mpsc::Sender<ApiEvent>,
    domain_depth: Arc<AtomicUsize>,
    api_depth: Arc<AtomicUsize>,
}

impl AnalyticsRuntime {
    pub async fn new(config: AnalyticsConfig) -> Result<Arc<Self>, ConfigurationError> {
        let read_store: Arc<dyn AnalyticsReadStore> = if config.clickhouse.enabled {
            Arc::new(
                ClickHouseAnalyticsStore::new(config.clickhouse.clone())
                    .await
                    .map_err(|_| {
                        ConfigurationError::InvalidConfigurationValueError(
                            "analytics.clickhouse".to_string(),
                        )
                    })?,
            )
        } else {
            crate::logger::info!("analytics clickhouse disabled; using unavailable read store");
            Arc::new(UnavailableAnalyticsReadStore)
        };

        let write_store: Arc<dyn AnalyticsWriteStore> = if config.kafka.enabled {
            match KafkaAnalyticsStore::new(config.kafka.clone()).await {
                Ok(kafka_store) => Arc::new(kafka_store),
                Err(error) => {
                    crate::logger::warn!(
                        ?error,
                        kafka_brokers = %config.kafka.brokers,
                        api_topic = %config.kafka.api_topic,
                        domain_topic = %config.kafka.domain_topic,
                        "analytics kafka startup failed; continuing with noop write store"
                    );
                    Arc::new(NoopAnalyticsWriteStore)
                }
            }
        } else {
            crate::logger::info!("analytics kafka disabled; using noop write store");
            Arc::new(NoopAnalyticsWriteStore)
        };

        let queue_capacity = config.kafka.queue_capacity.max(1);
        let (domain_tx, domain_rx) = mpsc::channel(queue_capacity);
        let (api_tx, api_rx) = mpsc::channel(queue_capacity);

        let runtime = Arc::new(Self {
            config,
            read_store,
            write_store,
            domain_tx,
            deferred_details: Arc::new(Semaphore::new(MAX_DEFERRED_DETAILS)),
            api_tx,
            domain_depth: Arc::new(AtomicUsize::new(0)),
            api_depth: Arc::new(AtomicUsize::new(0)),
        });

        runtime.spawn_domain_publisher(domain_rx);
        runtime.spawn_api_publisher(api_rx);

        Ok(runtime)
    }

    pub fn read_store(&self) -> Arc<dyn AnalyticsReadStore> {
        self.read_store.clone()
    }

    pub fn read_enabled(&self) -> bool {
        self.config.clickhouse.enabled
    }

    pub fn write_enabled(&self) -> bool {
        self.config.kafka.enabled
    }

    pub fn details_max_bytes(&self) -> usize {
        self.config.capture.details_max_bytes
    }

    pub fn enqueue_domain_event(&self, mut event: DomainAnalyticsEvent) {
        if !self.write_enabled() {
            return;
        }

        event.details = bound_domain_details(event.details, self.details_max_bytes());

        if self
            .domain_tx
            .try_send(PendingDomainEvent {
                event,
                details: None,
            })
            .is_err()
        {
            ANALYTICS_EVENTS_DROPPED_TOTAL
                .with_label_values(&["domain", "queue_full"])
                .inc();
            crate::logger::warn!("Dropping analytics domain event because the queue is full");
            return;
        }

        update_depth("domain", &self.domain_depth, 1);
    }

    pub(crate) fn enqueue_domain_event_with_details<F, D>(
        &self,
        event: DomainAnalyticsEvent,
        make_details: F,
    ) where
        F: FnOnce() -> D,
        D: FnOnce(usize) -> Option<String> + Send + 'static,
    {
        if !self.write_enabled() {
            return;
        }
        let Ok(details_permit) = self.deferred_details.clone().try_acquire_owned() else {
            ANALYTICS_EVENTS_DROPPED_TOTAL
                .with_label_values(&["domain", "details_queue_full"])
                .inc();
            return;
        };
        let Ok(queue_permit) = self.domain_tx.try_reserve() else {
            ANALYTICS_EVENTS_DROPPED_TOTAL
                .with_label_values(&["domain", "queue_full"])
                .inc();
            return;
        };
        let details = DeferredDetails {
            build: Box::new(make_details()),
            _permit: details_permit,
        };
        update_depth("domain", &self.domain_depth, 1);
        queue_permit.send(PendingDomainEvent {
            event,
            details: Some(details),
        });
    }

    pub fn enqueue_api_event(&self, event: ApiEvent) {
        if !self.write_enabled() {
            return;
        }

        if self.api_tx.try_send(event).is_err() {
            ANALYTICS_EVENTS_DROPPED_TOTAL
                .with_label_values(&["api", "queue_full"])
                .inc();
            crate::logger::warn!("Dropping analytics api event because the queue is full");
            return;
        }

        update_depth("api", &self.api_depth, 1);
    }

    fn spawn_domain_publisher(self: &Arc<Self>, mut receiver: mpsc::Receiver<PendingDomainEvent>) {
        let write_store = self.write_store.clone();
        let depth = self.domain_depth.clone();
        let max_bytes = self.details_max_bytes();

        tokio::spawn(async move {
            // Drain in batches: one awaited publish per event can't keep up with bursts
            // (e.g. simulator runs), which fills the bounded queue and silently drops
            // events — surfacing as "no outcome" A/B payments and missing audit events.
            let mut batch = Vec::with_capacity(PUBLISH_BATCH_SIZE);
            while receiver.recv_many(&mut batch, PUBLISH_BATCH_SIZE).await > 0 {
                update_depth("domain", &depth, -(batch.len() as isize));
                let pending = std::mem::take(&mut batch);
                let prepared = tokio::task::spawn_blocking(move || {
                    pending
                        .into_iter()
                        .map(|event| event.prepare(max_bytes))
                        .collect::<Vec<_>>()
                })
                .await;
                let events = match prepared {
                    Ok(events) => events,
                    Err(error) => {
                        crate::logger::warn!(error = %error, "Failed to prepare analytics domain events");
                        continue;
                    }
                };
                if let Err(error) = write_store.persist_domain_events(&events).await {
                    crate::logger::warn!(error = %error, "Failed to publish analytics domain events");
                }
            }
        });
    }

    fn spawn_api_publisher(self: &Arc<Self>, mut receiver: mpsc::Receiver<ApiEvent>) {
        let write_store = self.write_store.clone();
        let depth = self.api_depth.clone();

        tokio::spawn(async move {
            let mut batch = Vec::with_capacity(PUBLISH_BATCH_SIZE);
            while receiver.recv_many(&mut batch, PUBLISH_BATCH_SIZE).await > 0 {
                update_depth("api", &depth, -(batch.len() as isize));
                if let Err(error) = write_store.persist_api_events(&batch).await {
                    crate::logger::warn!(error = %error, "Failed to publish analytics api events");
                }
                batch.clear();
            }
        });
    }
}

fn update_depth(stream: &'static str, depth: &AtomicUsize, delta: isize) {
    let new_value = if delta.is_positive() {
        depth.fetch_add(delta as usize, Ordering::Relaxed) + delta as usize
    } else {
        depth
            .fetch_sub(delta.unsigned_abs(), Ordering::Relaxed)
            .saturating_sub(delta.unsigned_abs())
    };

    ANALYTICS_SINK_QUEUE_DEPTH
        .with_label_values(&[stream])
        .set(new_value as i64);
}
