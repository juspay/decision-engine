use async_trait::async_trait;
use masking::PeekInterface;
use rdkafka::config::ClientConfig;
use rdkafka::error::{KafkaError, RDKafkaErrorCode};
use rdkafka::producer::{FutureProducer, FutureRecord, Producer};
use rdkafka::util::Timeout;
use serde::{Deserialize, Serialize};
use tokio::time::Instant;

use crate::analytics::events::{ApiEvent, DomainAnalyticsEvent};
use crate::analytics::store::AnalyticsWriteStore;
use crate::config::KafkaAnalyticsConfig;
use crate::error::{ApiError, ConfigurationError};
use crate::metrics::{
    ANALYTICS_EVENTS_DROPPED_TOTAL, ANALYTICS_KAFKA_DELIVERY_LATENCY_HISTOGRAM,
    ANALYTICS_KAFKA_PRODUCE_TOTAL,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsKafkaEnvelopeV1<T> {
    pub version: u8,
    pub produced_at_ms: i64,
    pub stream: String,
    pub payload: T,
}

impl<T> AnalyticsKafkaEnvelopeV1<T> {
    fn new(stream: &'static str, payload: T) -> Self {
        Self {
            version: 1,
            produced_at_ms: crate::analytics::now_ms(),
            stream: stream.to_string(),
            payload,
        }
    }
}

#[derive(Clone)]
pub struct KafkaAnalyticsStore {
    producer: FutureProducer,
    config: KafkaAnalyticsConfig,
}

impl KafkaAnalyticsStore {
    pub async fn new(config: KafkaAnalyticsConfig) -> Result<Self, ConfigurationError> {
        let producer = build_client_config(&config)
            .create::<FutureProducer>()
            .map_err(|_| {
                ConfigurationError::InvalidConfigurationValueError("analytics.kafka".to_string())
            })?;

        producer
            .client()
            .fetch_metadata(None, Timeout::After(std::time::Duration::from_secs(5)))
            .map_err(|_| {
                ConfigurationError::InvalidConfigurationValueError(
                    "analytics.kafka.brokers".to_string(),
                )
            })?;

        Ok(Self { producer, config })
    }

    async fn send_api_event(&self, event: &ApiEvent) -> Result<(), ApiError> {
        let envelope = AnalyticsKafkaEnvelopeV1::new("api", event.clone());
        let payload = serde_json::to_vec(&envelope).map_err(|_| ApiError::EncodingError)?;
        let key = api_event_key(event);
        self.send_payload("api", &self.config.api_topic, &key, payload)
            .await
    }

    async fn send_domain_event(&self, event: &DomainAnalyticsEvent) -> Result<(), ApiError> {
        let envelope = AnalyticsKafkaEnvelopeV1::new("domain", event.clone());
        let payload = serde_json::to_vec(&envelope).map_err(|_| ApiError::EncodingError)?;
        let key = domain_event_key(event);
        self.send_payload("domain", &self.config.domain_topic, &key, payload)
            .await
    }

    async fn send_payload(
        &self,
        stream: &'static str,
        topic: &str,
        key: &str,
        payload: Vec<u8>,
    ) -> Result<(), ApiError> {
        if payload.len() > self.config.max_message_bytes {
            ANALYTICS_EVENTS_DROPPED_TOTAL
                .with_label_values(&[stream, "message_too_large"])
                .inc();
            ANALYTICS_KAFKA_PRODUCE_TOTAL
                .with_label_values(&[stream, "dropped"])
                .inc();
            return Err(ApiError::EncodingError);
        }

        let started_at = Instant::now();
        let delivery = self
            .producer
            .send_result(FutureRecord::to(topic).key(key).payload(&payload))
            .map_err(|(error, _message)| {
                let reason = match error {
                    KafkaError::MessageProduction(RDKafkaErrorCode::QueueFull) => {
                        "producer_queue_full"
                    }
                    _ => "producer_error",
                };
                ANALYTICS_EVENTS_DROPPED_TOTAL
                    .with_label_values(&[stream, reason])
                    .inc();
                ANALYTICS_KAFKA_PRODUCE_TOTAL
                    .with_label_values(&[stream, "dropped"])
                    .inc();
                ApiError::UnknownError
            })?;

        match delivery.await {
            Ok(Ok(_)) => {
                ANALYTICS_KAFKA_PRODUCE_TOTAL
                    .with_label_values(&[stream, "success"])
                    .inc();
                ANALYTICS_KAFKA_DELIVERY_LATENCY_HISTOGRAM
                    .with_label_values(&[stream])
                    .observe(started_at.elapsed().as_secs_f64());
                Ok(())
            }
            Ok(Err((_error, _message))) => {
                ANALYTICS_KAFKA_PRODUCE_TOTAL
                    .with_label_values(&[stream, "failure"])
                    .inc();
                Err(ApiError::UnknownError)
            }
            Err(_canceled) => {
                ANALYTICS_KAFKA_PRODUCE_TOTAL
                    .with_label_values(&[stream, "failure"])
                    .inc();
                Err(ApiError::UnknownError)
            }
        }
    }
}

#[async_trait]
impl AnalyticsWriteStore for KafkaAnalyticsStore {
    async fn persist_domain_events(&self, events: &[DomainAnalyticsEvent]) -> Result<(), ApiError> {
        let mut last_error = None;
        for event in events {
            if let Err(error) = self.send_domain_event(event).await {
                last_error = Some(error);
            }
        }
        last_error.map_or(Ok(()), Err)
    }

    async fn persist_api_events(&self, events: &[ApiEvent]) -> Result<(), ApiError> {
        let mut last_error = None;
        for event in events {
            if let Err(error) = self.send_api_event(event).await {
                last_error = Some(error);
            }
        }
        last_error.map_or(Ok(()), Err)
    }

    fn sink_name(&self) -> &'static str {
        "kafka"
    }
}

pub fn api_event_key(event: &ApiEvent) -> String {
    format!(
        "{}:{}",
        event.tenant_id,
        first_non_empty([
            Some(event.request_id.clone()),
            event.payment_id.clone(),
            Some(event.event_id.to_string()),
        ])
    )
}

pub fn domain_event_key(event: &DomainAnalyticsEvent) -> String {
    format!(
        "{}:{}",
        event.tenant_id,
        first_non_empty([
            event.payment_id.clone(),
            event.request_id.clone(),
            Some(event.event_id.to_string()),
        ])
    )
}

fn first_non_empty<I>(values: I) -> String
where
    I: IntoIterator<Item = Option<String>>,
{
    values
        .into_iter()
        .flatten()
        .find(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}

fn build_client_config(config: &KafkaAnalyticsConfig) -> ClientConfig {
    let mut client_config = ClientConfig::new();
    client_config
        .set("bootstrap.servers", &config.brokers)
        .set("client.id", &config.client_id)
        .set("acks", &config.acks)
        .set("compression.type", &config.compression)
        .set("message.timeout.ms", config.message_timeout_ms.to_string())
        .set(
            "queue.buffering.max.messages",
            config.queue_capacity.to_string(),
        );

    if let Some(protocol) = &config.security_protocol {
        client_config.set("security.protocol", protocol);
    }
    if let Some(mechanism) = &config.sasl_mechanism {
        client_config.set("sasl.mechanism", mechanism);
    }
    if let Some(username) = &config.sasl_username {
        client_config.set("sasl.username", username);
    }
    if let Some(password) = &config.sasl_password {
        client_config.set("sasl.password", password.peek());
    }

    client_config
}

pub fn worker_client_config(config: &KafkaAnalyticsConfig, consumer_group: &str) -> ClientConfig {
    let mut client_config = build_client_config(config);
    client_config
        .set("group.id", consumer_group)
        .set("enable.auto.commit", "false")
        .set("enable.auto.offset.store", "false")
        .set("auto.offset.reset", "earliest");
    client_config
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use crate::analytics::{ApiEvent, DomainAnalyticsEvent};

    use super::{api_event_key, domain_event_key, AnalyticsKafkaEnvelopeV1};

    #[test]
    fn envelope_serializes_stably() {
        let event = DomainAnalyticsEvent {
            event_id: 1,
            tenant_id: "public".to_string(),
            event_type: "decision".to_string(),
            merchant_id: None,
            payment_id: Some("pay_1".to_string()),
            request_id: Some("req_1".to_string()),
            payment_method_type: None,
            payment_method: None,
            card_network: None,
            card_is_in: None,
            currency: None,
            country: None,
            auth_type: None,
            gateway: None,
            event_stage: None,
            routing_approach: None,
            rule_name: None,
            status: None,
            error_code: None,
            error_message: None,
            score_value: None,
            sigma_factor: None,
            average_latency: None,
            tp99_latency: None,
            transaction_count: None,
            route: None,
            details: None,
            created_at_ms: 123,
        };
        let envelope = AnalyticsKafkaEnvelopeV1::new("domain", event);
        let json = serde_json::to_string(&envelope).unwrap();
        assert!(json.contains("\"version\":1"));
        assert!(json.contains("\"stream\":\"domain\""));
    }

    #[test]
    fn message_keys_are_deterministic() {
        let api = ApiEvent {
            event_id: 10,
            tenant_id: "tenant".to_string(),
            merchant_id: None,
            payment_id: Some("pay_123".to_string()),
            api_flow: "decide_gateway".to_string(),
            created_at_timestamp: 1,
            request_id: "req_123".to_string(),
            latency: 1,
            status_code: 200,
            auth_type: None,
            request: "{}".to_string(),
            user_agent: None,
            ip_addr: None,
            url_path: "/decide-gateway".to_string(),
            response: None,
            error: None,
            event_type: "success".to_string(),
            http_method: "POST".to_string(),
            infra_components: None,
            request_truncated: false,
            response_truncated: false,
        };
        let domain = DomainAnalyticsEvent {
            event_id: 11,
            tenant_id: "tenant".to_string(),
            event_type: "decision".to_string(),
            merchant_id: None,
            payment_id: Some("pay_456".to_string()),
            request_id: Some("req_456".to_string()),
            payment_method_type: None,
            payment_method: None,
            card_network: None,
            card_is_in: None,
            currency: None,
            country: None,
            auth_type: None,
            gateway: None,
            event_stage: None,
            routing_approach: None,
            rule_name: None,
            status: None,
            error_code: None,
            error_message: None,
            score_value: None,
            sigma_factor: None,
            average_latency: None,
            tp99_latency: None,
            transaction_count: None,
            route: None,
            details: None,
            created_at_ms: 1,
        };

        assert_eq!(api_event_key(&api), "tenant:req_123");
        assert_eq!(domain_event_key(&domain), "tenant:pay_456");
    }
}
