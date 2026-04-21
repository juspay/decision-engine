use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;

use crate::analytics::clickhouse::ClickHouseAnalyticsStore;
use crate::analytics::events::{ApiEvent, DomainAnalyticsEvent};
use crate::analytics::kafka::KafkaAnalyticsStore;
use crate::analytics::store::{
    AnalyticsReadStore, AnalyticsWriteStore, NoopAnalyticsWriteStore, UnavailableAnalyticsReadStore,
};
use crate::config::AnalyticsConfig;
use crate::error::ConfigurationError;
use crate::metrics::{ANALYTICS_EVENTS_DROPPED_TOTAL, ANALYTICS_SINK_QUEUE_DEPTH};

#[derive(Clone)]
pub struct AnalyticsRuntime {
    config: AnalyticsConfig,
    read_store: Arc<dyn AnalyticsReadStore>,
    write_store: Arc<dyn AnalyticsWriteStore>,
    domain_tx: mpsc::Sender<DomainAnalyticsEvent>,
    api_tx: mpsc::Sender<ApiEvent>,
    domain_depth: Arc<AtomicUsize>,
    api_depth: Arc<AtomicUsize>,
}

impl AnalyticsRuntime {
    pub async fn new(config: AnalyticsConfig) -> Result<Arc<Self>, ConfigurationError> {
        let (read_store, write_store): (Arc<dyn AnalyticsReadStore>, Arc<dyn AnalyticsWriteStore>) =
            if !config.enabled {
                (
                    Arc::new(UnavailableAnalyticsReadStore),
                    Arc::new(NoopAnalyticsWriteStore),
                )
            } else {
                let clickhouse_store = Arc::new(
                    ClickHouseAnalyticsStore::new(config.clickhouse.clone())
                        .await
                        .map_err(|_| {
                            ConfigurationError::InvalidConfigurationValueError(
                                "analytics.clickhouse".to_string(),
                            )
                        })?,
                );
                let kafka_store = Arc::new(KafkaAnalyticsStore::new(config.kafka.clone()).await?);
                (clickhouse_store, kafka_store)
            };

        let queue_capacity = config.kafka.queue_capacity.max(1);
        let (domain_tx, domain_rx) = mpsc::channel(queue_capacity);
        let (api_tx, api_rx) = mpsc::channel(queue_capacity);

        let runtime = Arc::new(Self {
            config,
            read_store,
            write_store,
            domain_tx,
            api_tx,
            domain_depth: Arc::new(AtomicUsize::new(0)),
            api_depth: Arc::new(AtomicUsize::new(0)),
        });

        runtime.spawn_domain_worker(domain_rx);
        runtime.spawn_api_worker(api_rx);

        Ok(runtime)
    }

    pub fn read_store(&self) -> Arc<dyn AnalyticsReadStore> {
        self.read_store.clone()
    }

    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    pub fn body_max_bytes(&self) -> usize {
        self.config.capture.body_max_bytes
    }

    pub fn request_body_limit_bytes(&self) -> usize {
        self.config.capture.request_body_limit_bytes
    }

    pub fn details_max_bytes(&self) -> usize {
        self.config.capture.details_max_bytes
    }

    pub fn enqueue_domain_event(&self, event: DomainAnalyticsEvent) {
        if !self.config.enabled {
            return;
        }

        if self.domain_tx.try_send(event).is_err() {
            ANALYTICS_EVENTS_DROPPED_TOTAL
                .with_label_values(&["domain", "queue_full"])
                .inc();
            crate::logger::warn!("Dropping analytics domain event because the queue is full");
            return;
        }

        update_depth("domain", &self.domain_depth, 1);
    }

    pub fn enqueue_api_event(&self, event: ApiEvent) {
        if !self.config.enabled {
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

    fn spawn_domain_worker(self: &Arc<Self>, mut receiver: mpsc::Receiver<DomainAnalyticsEvent>) {
        let write_store = self.write_store.clone();
        let batch_size = self.config.worker.batch_size.max(1);
        let flush_interval = Duration::from_millis(self.config.worker.flush_interval_ms.max(1));
        let depth = self.domain_depth.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(flush_interval);
            let mut batch = Vec::with_capacity(batch_size);

            loop {
                tokio::select! {
                    maybe_event = receiver.recv() => {
                        match maybe_event {
                            Some(event) => {
                                batch.push(event);
                                update_depth("domain", &depth, -1);
                                if batch.len() >= batch_size {
                                    flush_domain_batch(write_store.clone(), &mut batch).await;
                                }
                            }
                            None => {
                                flush_domain_batch(write_store.clone(), &mut batch).await;
                                break;
                            }
                        }
                    }
                    _ = interval.tick() => {
                        if !batch.is_empty() {
                            flush_domain_batch(write_store.clone(), &mut batch).await;
                        }
                    }
                }
            }
        });
    }

    fn spawn_api_worker(self: &Arc<Self>, mut receiver: mpsc::Receiver<ApiEvent>) {
        let write_store = self.write_store.clone();
        let batch_size = self.config.worker.batch_size.max(1);
        let flush_interval = Duration::from_millis(self.config.worker.flush_interval_ms.max(1));
        let depth = self.api_depth.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(flush_interval);
            let mut batch = Vec::with_capacity(batch_size);

            loop {
                tokio::select! {
                    maybe_event = receiver.recv() => {
                        match maybe_event {
                            Some(event) => {
                                batch.push(event);
                                update_depth("api", &depth, -1);
                                if batch.len() >= batch_size {
                                    flush_api_batch(write_store.clone(), &mut batch).await;
                                }
                            }
                            None => {
                                flush_api_batch(write_store.clone(), &mut batch).await;
                                break;
                            }
                        }
                    }
                    _ = interval.tick() => {
                        if !batch.is_empty() {
                            flush_api_batch(write_store.clone(), &mut batch).await;
                        }
                    }
                }
            }
        });
    }
}

async fn flush_domain_batch(
    write_store: Arc<dyn AnalyticsWriteStore>,
    batch: &mut Vec<DomainAnalyticsEvent>,
) {
    if batch.is_empty() {
        return;
    }

    let events = std::mem::take(batch);
    if let Err(error) = write_store.persist_domain_events(&events).await {
        crate::logger::warn!(error = %error, "Failed to flush analytics domain batch");
    }
}

async fn flush_api_batch(write_store: Arc<dyn AnalyticsWriteStore>, batch: &mut Vec<ApiEvent>) {
    if batch.is_empty() {
        return;
    }

    let events = std::mem::take(batch);
    if let Err(error) = write_store.persist_api_events(&events).await {
        crate::logger::warn!(error = %error, "Failed to flush analytics api batch");
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
