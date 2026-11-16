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

/// Max events drained from the queue per publish call. Keeps the publisher ahead of
/// burst producers (a simulator run emits hundreds of events/second) so the bounded
/// queue doesn't fill and drop events.
const PUBLISH_BATCH_SIZE: usize = 200;

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

    pub fn enqueue_api_event(&self, mut event: ApiEvent) {
        if !self.write_enabled() {
            return;
        }

        super::redaction::redact_api_event(&mut event);

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
            // Drain in batches: one awaited publish per event can't keep up with bursts
            // (e.g. simulator runs), which fills the bounded queue and silently drops
            // events — surfacing as "no outcome" A/B payments and missing audit events.
            let mut batch = Vec::with_capacity(PUBLISH_BATCH_SIZE);
            while receiver.recv_many(&mut batch, PUBLISH_BATCH_SIZE).await > 0 {
                update_depth("domain", &depth, -(batch.len() as isize));
                if let Err(error) = write_store.persist_domain_events(&batch).await {
                    crate::logger::warn!(error = %error, "Failed to publish analytics domain events");
                }
                batch.clear();
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

#[cfg(test)]
mod tests {
    // Assertions report test failures; Result propagates queue and serialization errors.
    #![allow(clippy::panic_in_result_fn)]

    use super::*;
    use crate::analytics::{ApiFlow, FlowType, KafkaApiEventRow};
    use serde_json::json;

    #[test]
    fn api_key_fields_are_redacted_before_queueing_and_kafka_serialization(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut config = AnalyticsConfig::default();
        config.kafka.enabled = true;
        let (domain_tx, _domain_rx) = mpsc::channel(1);
        let (api_tx, mut api_rx) = mpsc::channel(1);
        let runtime = AnalyticsRuntime {
            config,
            read_store: Arc::new(UnavailableAnalyticsReadStore),
            write_store: Arc::new(NoopAnalyticsWriteStore),
            domain_tx,
            api_tx,
            domain_depth: Arc::new(AtomicUsize::new(0)),
            api_depth: Arc::new(AtomicUsize::new(0)),
        };

        for status_code in [200, 400] {
            let response = json!({
                "message": if status_code == 200 {
                    "Merchant account created successfully"
                } else {
                    "Merchant account creation failed"
                },
                "merchant_id": "merchant_test",
                "api_key": "test-secret-issued-key"
            });
            let event = ApiEvent {
                event_id: "event_test".to_string(),
                merchant_id: Some("merchant_test".to_string()),
                payment_id: None,
                api_flow: ApiFlow::MerchantAccount,
                flow_type: FlowType::MerchantAccountCreate,
                created_at_timestamp: 1,
                request_id: "request_test".to_string(),
                global_request_id: None,
                trace_id: None,
                latency: 1,
                status_code,
                auth_type: None,
                request: json!({"merchant_id": "merchant_test", "api_key": "test-secret-input"})
                    .to_string(),
                user_agent: None,
                ip_addr: None,
                url_path: "/merchant-account/create".to_string(),
                response: Some(response.to_string()),
                error: (status_code >= 400).then(|| response.clone()),
                http_method: "POST".to_string(),
            };

            runtime.enqueue_api_event(event);
            let queued = api_rx.try_recv()?;
            let queued_json = serde_json::to_value(&queued)?;
            assert!(!queued_json.to_string().contains("test-secret"));
            assert_eq!(queued_json["status_code"], status_code);
            assert_eq!(queued_json["merchant_id"], "merchant_test");
            let captured_response: serde_json::Value =
                serde_json::from_str(queued.response.as_deref().ok_or("missing response")?)?;
            assert_eq!(captured_response["api_key"], "[REDACTED]");
            assert_eq!(captured_response["message"], response["message"]);
            if status_code >= 400 {
                assert_eq!(queued_json["error"]["api_key"], "[REDACTED]");
            }
            let row = KafkaApiEventRow::from(queued);
            let kafka_json = serde_json::to_string(&row)?;
            assert!(!kafka_json.contains("test-secret"));
        }
        Ok(())
    }
}
