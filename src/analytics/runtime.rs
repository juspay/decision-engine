use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

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
            crate::logger::info!(
                "analytics clickhouse disabled; using unavailable read store"
            );
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

    pub fn enqueue_domain_event(&self, event: DomainAnalyticsEvent) {
        if !self.write_enabled() {
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

    fn spawn_domain_publisher(
        self: &Arc<Self>,
        mut receiver: mpsc::Receiver<DomainAnalyticsEvent>,
    ) {
        let write_store = self.write_store.clone();
        let depth = self.domain_depth.clone();

        tokio::spawn(async move {
            while let Some(event) = receiver.recv().await {
                update_depth("domain", &depth, -1);
                let batch = [event];
                if let Err(error) = write_store.persist_domain_events(&batch).await {
                    crate::logger::warn!(error = %error, "Failed to publish analytics domain event");
                }
            }
        });
    }

    fn spawn_api_publisher(self: &Arc<Self>, mut receiver: mpsc::Receiver<ApiEvent>) {
        let write_store = self.write_store.clone();
        let depth = self.api_depth.clone();

        tokio::spawn(async move {
            while let Some(event) = receiver.recv().await {
                update_depth("api", &depth, -1);
                let batch = [event];
                if let Err(error) = write_store.persist_api_events(&batch).await {
                    crate::logger::warn!(error = %error, "Failed to publish analytics api event");
                }
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
