use crate::analytics::{
    AnalyticsConfig, AnalyticsError, AnalyticsResult, KafkaProducer, RoutingEvent, RoutingEventData,
};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

#[derive(Clone)]
pub struct AnalyticsClient {
    config: AnalyticsConfig,
    kafka_producer: Option<Arc<KafkaProducer>>,
    event_sender: Option<mpsc::Sender<RoutingEventData>>,
}

impl AnalyticsClient {
    pub fn new(config: AnalyticsConfig) -> AnalyticsResult<Self> {
        if !config.enabled {
            info!("Analytics is disabled");
            return Ok(Self {
                config,
                kafka_producer: None,
                event_sender: None,
            });
        }

        // Initialize Kafka producer
        let kafka_producer = Arc::new(KafkaProducer::new(config.kafka.clone())?);

        // Create event channel for batching
        let (event_sender, event_receiver) = mpsc::channel::<RoutingEventData>(1000);

        // Start batch processor
        let _batch_processor = kafka_producer.start_batch_processor(event_receiver);

        info!("Analytics client initialized successfully");

        Ok(Self {
            config,
            kafka_producer: Some(kafka_producer),
            event_sender: Some(event_sender),
        })
    }

    pub async fn track_routing_event(&self, event: RoutingEvent) -> AnalyticsResult<()> {
        if !self.config.enabled {
            return Ok(());
        }

        let event_data = event.to_event_data();

        if let Some(sender) = &self.event_sender {
            if let Err(e) = sender.try_send(event_data) {
                match e {
                    mpsc::error::TrySendError::Full(_) => {
                        warn!("Analytics event queue is full, dropping event");
                    }
                    mpsc::error::TrySendError::Closed(_) => {
                        error!("Analytics event channel is closed");
                        return Err(AnalyticsError::Configuration(
                            "Event channel is closed".to_string(),
                        ));
                    }
                }
            }
        }

        Ok(())
    }

    pub async fn track_routing_event_sync(&self, event: RoutingEvent) -> AnalyticsResult<()> {
        if !self.config.enabled {
            return Ok(());
        }

        let event_data = event.to_event_data();

        if let Some(producer) = &self.kafka_producer {
            producer.send_event(&event_data).await?;
        }

        Ok(())
    }

    pub async fn flush_events(&self) -> AnalyticsResult<()> {
        if !self.config.enabled {
            return Ok(());
        }

        // Close the sender to trigger flush in batch processor
        if let Some(sender) = &self.event_sender {
            sender.closed().await;
        }

        Ok(())
    }

    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    pub async fn health_check(&self) -> AnalyticsResult<()> {
        if !self.config.enabled {
            return Ok(());
        }

        // Test Kafka connectivity by sending a test event
        if let Some(producer) = &self.kafka_producer {
            let test_event = RoutingEventData {
                event_id: "health-check".to_string(),
                merchant_id: "health-check".to_string(),
                request_id: "health-check".to_string(),
                endpoint: "/health".to_string(),
                method: "GET".to_string(),
                request_payload: "{}".to_string(),
                response_payload: r#"{"status": "ok"}"#.to_string(),
                status_code: 200,
                processing_time_ms: 1,
                gateway_selected: None,
                routing_algorithm_id: None,
                error_message: None,
                user_agent: Some("health-check".to_string()),
                ip_address: Some("127.0.0.1".to_string()),
                created_at: time::OffsetDateTime::now_utc(),
                sign_flag: 1,
            };

            producer.send_event(&test_event).await?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analytics::{ClickhouseConfig, KafkaConfig};

    #[tokio::test]
    async fn test_analytics_client_disabled() {
        let config = AnalyticsConfig {
            enabled: false,
            kafka: KafkaConfig::default(),
            clickhouse: ClickhouseConfig::default(),
        };

        let client = AnalyticsClient::new(config).unwrap();
        assert!(!client.is_enabled());

        let event = RoutingEvent::new(
            "merchant-123".to_string(),
            "req-456".to_string(),
            "/routing/evaluate".to_string(),
            "POST".to_string(),
        );

        // Should not error when disabled
        let result = client.track_routing_event(event).await;
        assert!(result.is_ok());
    }

    #[test]
    fn test_analytics_config_default() {
        let config = AnalyticsConfig::default();
        assert!(!config.enabled);
        assert_eq!(config.kafka.brokers, vec!["localhost:9092"]);
        assert_eq!(config.kafka.topic_prefix, "decision-engine");
        assert_eq!(config.clickhouse.host, "http://localhost:8123");
        assert_eq!(config.clickhouse.username, "analytics_user");
    }
}
