use crate::analytics::{AnalyticsError, AnalyticsResult, KafkaConfig, RoutingEventData};
use kafka::producer::{Producer, Record, RequiredAcks};
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{error, info, warn, debug};

#[derive(Clone)]
pub struct KafkaProducer {
    config: KafkaConfig,
    topic: String,
}

impl KafkaProducer {
    pub fn new(config: KafkaConfig) -> AnalyticsResult<Self> {
        let topic = format!("{}-routing-events", config.topic_prefix);
        
        // Validate broker configuration
        if config.brokers.is_empty() {
            return Err(AnalyticsError::Configuration(
                "No Kafka brokers configured".to_string()
            ));
        }
        
        debug!("Initializing Kafka producer with brokers: {:?}", config.brokers);
        
        Ok(Self { config, topic })
    }

    /// Test Kafka connectivity
    pub async fn test_connection(&self) -> AnalyticsResult<()> {
        debug!("Testing Kafka connection to brokers: {:?}", self.config.brokers);
        
        let producer = Producer::from_hosts(self.config.brokers.clone())
            .with_ack_timeout(Duration::from_secs(5))
            .with_required_acks(RequiredAcks::One)
            .create()
            .map_err(|e| {
                error!("Failed to create Kafka producer for connection test: {:?}", e);
                AnalyticsError::Kafka(e)
            })?;
        
        info!("Kafka connection test successful");
        Ok(())
    }

    pub async fn send_event(&self, event: &RoutingEventData) -> AnalyticsResult<()> {
        let json_data = serde_json::to_string(event)?;
        
        // Create producer with configuration
        let mut producer = Producer::from_hosts(self.config.brokers.clone())
            .with_ack_timeout(Duration::from_secs(1))
            .with_required_acks(RequiredAcks::One)
            .create()
            .map_err(AnalyticsError::Kafka)?;

        // Send the record
        let record = Record::from_key_value(&self.topic, event.event_id.as_bytes(), json_data.as_bytes());
        
        producer
            .send(&record)
            .map_err(AnalyticsError::Kafka)?;

        Ok(())
    }

    pub async fn send_events_batch(&self, events: &[RoutingEventData]) -> AnalyticsResult<()> {
        if events.is_empty() {
            return Ok(());
        }

        let mut producer = Producer::from_hosts(self.config.brokers.clone())
            .with_ack_timeout(Duration::from_secs(5))
            .with_required_acks(RequiredAcks::One)
            .create()
            .map_err(|e| {
                error!("Failed to create Kafka producer for batch send: {:?}", e);
                warn!("Kafka brokers configured: {:?}", self.config.brokers);
                AnalyticsError::Kafka(e)
            })?;

        for (index, event) in events.iter().enumerate() {
            let json_data = serde_json::to_string(event)?;
            let record = Record::from_key_value(&self.topic, event.event_id.as_bytes(), json_data.as_bytes());
            
            if let Err(e) = producer.send(&record) {
                error!("Failed to send event {} of {} to Kafka: {:?}", index + 1, events.len(), e);
                return Err(AnalyticsError::Kafka(e));
            }
        }

        info!("Successfully sent {} events to Kafka topic: {}", events.len(), self.topic);
        Ok(())
    }

    /// Send events batch with graceful error handling
    pub async fn send_events_batch_graceful(&self, events: &[RoutingEventData]) -> bool {
        match self.send_events_batch(events).await {
            Ok(()) => true,
            Err(e) => {
                warn!("Failed to send events batch to Kafka, continuing without analytics: {:?}", e);
                false
            }
        }
    }

    pub fn start_batch_processor(
        &self,
        mut receiver: mpsc::Receiver<RoutingEventData>,
    ) -> tokio::task::JoinHandle<()> {
        let producer = self.clone();
        let batch_size = self.config.batch_size;
        let batch_timeout = Duration::from_millis(self.config.batch_timeout_ms);
        let max_consecutive_failures = self.config.max_consecutive_failures;

        tokio::spawn(async move {
            let mut batch = Vec::with_capacity(batch_size);
            let mut last_flush = tokio::time::Instant::now();
            let mut consecutive_failures = 0;
            info!("Starting Kafka batch processor with batch_size: {}, timeout: {}ms, max_consecutive_failures: {}", 
                  batch_size, batch_timeout.as_millis(), max_consecutive_failures);

            loop {
                tokio::select! {
                    // Receive new events
                    event = receiver.recv() => {
                        match event {
                            Some(event) => {
                                batch.push(event);
                                
                                // Flush if batch is full
                                if batch.len() >= batch_size {
                                    let success = producer.send_events_batch_graceful(&batch).await;
                                    if success {
                                        consecutive_failures = 0;
                                    } else {
                                        consecutive_failures += 1;
                                        if consecutive_failures >= max_consecutive_failures {
                                            warn!("Too many consecutive Kafka failures ({}), continuing to collect events but not sending", 
                                                  consecutive_failures);
                                        }
                                    }
                                    batch.clear();
                                    last_flush = tokio::time::Instant::now();
                                }
                            }
                            None => {
                                // Channel closed, flush remaining events and exit
                                if !batch.is_empty() {
                                    info!("Channel closed, sending final batch of {} events", batch.len());
                                    producer.send_events_batch_graceful(&batch).await;
                                }
                                info!("Kafka batch processor shutting down");
                                break;
                            }
                        }
                    }
                    
                    // Timeout-based flush
                    _ = tokio::time::sleep_until(last_flush + batch_timeout) => {
                        if !batch.is_empty() {
                            let success = producer.send_events_batch_graceful(&batch).await;
                            if success {
                                consecutive_failures = 0;
                            } else {
                                consecutive_failures += 1;
                                if consecutive_failures >= max_consecutive_failures {
                                    warn!("Too many consecutive Kafka failures ({}), continuing to collect events but not sending", 
                                          consecutive_failures);
                                }
                            }
                            batch.clear();
                            last_flush = tokio::time::Instant::now();
                        }
                    }
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::OffsetDateTime;

    #[tokio::test]
    async fn test_kafka_producer_creation() {
        let config = KafkaConfig {
            brokers: vec!["localhost:9092".to_string()],
            topic_prefix: "test".to_string(),
            batch_size: 10,
            batch_timeout_ms: 1000,
            max_consecutive_failures: 5,
        };

        let producer = KafkaProducer::new(config);
        assert!(producer.is_ok());
    }

    #[test]
    fn test_event_serialization() {
        let event = RoutingEventData {
            event_id: "test-event-1".to_string(),
            merchant_id: "merchant-123".to_string(),
            request_id: "req-456".to_string(),
            endpoint: "/routing/evaluate".to_string(),
            method: "POST".to_string(),
            request_payload: r#"{"test": "data"}"#.to_string(),
            response_payload: r#"{"result": "success"}"#.to_string(),
            status_code: 200,
            processing_time_ms: 150,
            gateway_selected: Some("stripe".to_string()),
            routing_algorithm_id: Some("algo-789".to_string()),
            error_message: None,
            user_agent: Some("test-agent".to_string()),
            ip_address: Some("127.0.0.1".to_string()),
            created_at: OffsetDateTime::now_utc(),
            sign_flag: 1,
        };

        let json = serde_json::to_string(&event);
        assert!(json.is_ok());
    }
}
