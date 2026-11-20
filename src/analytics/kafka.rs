use async_trait::async_trait;
use masking::PeekInterface;
use rdkafka::config::ClientConfig;
use rdkafka::error::{KafkaError, RDKafkaErrorCode};
use rdkafka::producer::{FutureProducer, FutureRecord, Producer};
use rdkafka::util::Timeout;
use serde::Serialize;
use tokio::time::Instant;

use crate::analytics::events::{ApiEvent, DomainAnalyticsEvent};
use crate::analytics::store::AnalyticsWriteStore;
use crate::config::KafkaAnalyticsConfig;
use crate::error::{ApiError, ConfigurationError};
use crate::metrics::{ANALYTICS_KAFKA_DELIVERY_LATENCY_HISTOGRAM, ANALYTICS_KAFKA_PRODUCE_TOTAL};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KafkaPublishOutcome {
    EncodingFailed,
    EnqueueFailedQueueFull,
    EnqueueFailedProducerError,
    DeliverySuccess,
    DeliveryFailed,
    DeliveryCanceled,
}

impl KafkaPublishOutcome {
    fn metric_result(self) -> &'static str {
        match self {
            Self::EncodingFailed => "encoding_failed",
            Self::EnqueueFailedQueueFull => "enqueue_failed_queue_full",
            Self::EnqueueFailedProducerError => "enqueue_failed_producer_error",
            Self::DeliverySuccess => "delivery_success",
            Self::DeliveryFailed => "delivery_failed",
            Self::DeliveryCanceled => "delivery_canceled",
        }
    }

    fn is_success(self) -> bool {
        matches!(self, Self::DeliverySuccess)
    }
}

/// One Kafka message: the event's fields, preceded by the envelope ClickHouse ingests with them.
#[derive(Debug, Serialize)]
pub struct KafkaEventRow<'a, T> {
    pub schema_version: u8,
    pub produced_at_ms: i64,
    #[serde(flatten)]
    pub event: &'a T,
}

impl<'a, T> KafkaEventRow<'a, T> {
    pub fn new(event: &'a T) -> Self {
        Self {
            schema_version: 1,
            produced_at_ms: crate::analytics::now_ms(),
            event,
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
            .map_err(|error| {
                crate::logger::error!(
                    ?error,
                    kafka_brokers = %config.brokers,
                    api_topic = %config.api_topic,
                    domain_topic = %config.domain_topic,
                    "failed to create kafka analytics producer"
                );
                ConfigurationError::InvalidConfigurationValueError("analytics.kafka".to_string())
            })?;

        producer
            .client()
            .fetch_metadata(None, Timeout::After(std::time::Duration::from_secs(5)))
            .map_err(|error| {
                crate::logger::error!(
                    ?error,
                    kafka_brokers = %config.brokers,
                    api_topic = %config.api_topic,
                    domain_topic = %config.domain_topic,
                    "failed to fetch kafka metadata during analytics startup"
                );
                ConfigurationError::InvalidConfigurationValueError(
                    "analytics.kafka.brokers".to_string(),
                )
            })?;

        Ok(Self { producer, config })
    }

    async fn send_api_event(&self, event: &ApiEvent) -> KafkaPublishOutcome {
        let payload = match serde_json::to_vec(&KafkaEventRow::new(event)) {
            Ok(payload) => payload,
            Err(_) => {
                return KafkaPublishOutcome::EncodingFailed;
            }
        };
        let key = api_event_key(event);
        self.send_payload("api", &self.config.api_topic, &key, payload)
            .await
    }

    async fn send_domain_event(&self, event: &DomainAnalyticsEvent) -> KafkaPublishOutcome {
        let payload = match serde_json::to_vec(&KafkaEventRow::new(event)) {
            Ok(payload) => payload,
            Err(_) => {
                return KafkaPublishOutcome::EncodingFailed;
            }
        };
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
    ) -> KafkaPublishOutcome {
        let started_at = Instant::now();
        let delivery = match self
            .producer
            .send_result(FutureRecord::to(topic).key(key).payload(&payload))
        {
            Ok(delivery) => delivery,
            Err((error, _message)) => {
                let outcome = match error {
                    KafkaError::MessageProduction(RDKafkaErrorCode::QueueFull) => {
                        KafkaPublishOutcome::EnqueueFailedQueueFull
                    }
                    _ => KafkaPublishOutcome::EnqueueFailedProducerError,
                };
                ANALYTICS_KAFKA_PRODUCE_TOTAL
                    .with_label_values(&[stream, outcome.metric_result()])
                    .inc();
                return outcome;
            }
        };

        ANALYTICS_KAFKA_PRODUCE_TOTAL
            .with_label_values(&[stream, "enqueued"])
            .inc();

        let outcome = match delivery.await {
            Ok(Ok(_)) => {
                ANALYTICS_KAFKA_DELIVERY_LATENCY_HISTOGRAM
                    .with_label_values(&[stream])
                    .observe(started_at.elapsed().as_secs_f64());
                KafkaPublishOutcome::DeliverySuccess
            }
            Ok(Err((_error, _message))) => KafkaPublishOutcome::DeliveryFailed,
            Err(_canceled) => KafkaPublishOutcome::DeliveryCanceled,
        };

        ANALYTICS_KAFKA_PRODUCE_TOTAL
            .with_label_values(&[stream, outcome.metric_result()])
            .inc();

        outcome
    }
}

#[async_trait]
impl AnalyticsWriteStore for KafkaAnalyticsStore {
    async fn persist_domain_events(&self, events: &[DomainAnalyticsEvent]) -> Result<(), ApiError> {
        // Publish the batch concurrently: each send awaits its own delivery report, so a
        // sequential loop costs one broker round-trip per event and can't keep up with
        // bursts (the runtime queue then fills and drops events). Concurrent sends share
        // the producer's internal batching, so the whole batch costs ~one round-trip.
        let outcomes =
            futures::future::join_all(events.iter().map(|event| self.send_domain_event(event)))
                .await;
        for (event, outcome) in events.iter().zip(outcomes) {
            if !outcome.is_success() {
                crate::logger::warn!(
                    outcome = ?outcome,
                    event_id = event.event_id,
                    "Failed to publish analytics domain event"
                );
            }
        }
        Ok(())
    }

    async fn persist_api_events(&self, events: &[ApiEvent]) -> Result<(), ApiError> {
        let outcomes =
            futures::future::join_all(events.iter().map(|event| self.send_api_event(event))).await;
        for (event, outcome) in events.iter().zip(outcomes) {
            if !outcome.is_success() {
                crate::logger::warn!(
                    outcome = ?outcome,
                    event_id = event.event_id,
                    "Failed to publish analytics api event"
                );
            }
        }
        Ok(())
    }

    fn sink_name(&self) -> &'static str {
        "kafka"
    }
}

pub fn api_event_key(event: &ApiEvent) -> String {
    first_non_empty([
        Some(event.request_id.clone()),
        event.payment_id.clone(),
        Some(event.event_id.clone()),
    ])
}

pub fn domain_event_key(event: &DomainAnalyticsEvent) -> String {
    first_non_empty([
        event.lookup_key.clone(),
        event.payment_id.clone(),
        event.request_id.clone(),
        Some(event.event_id.clone()),
    ])
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

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use serde_json::json;

    use crate::analytics::{
        AnalyticsFlowContext, AnalyticsRoute, ApiEvent, ApiFlow, DomainAnalyticsEvent, FlowType,
    };

    use super::{api_event_key, domain_event_key, KafkaEventRow};

    fn domain_event(payment_id: Option<&str>, request_id: Option<&str>) -> DomainAnalyticsEvent {
        DomainAnalyticsEvent {
            event_id: "0195f4a8-bdf0-7f36-a227-c0ffefeed001".to_string(),
            payment_id: payment_id.map(str::to_string),
            request_id: request_id.map(str::to_string),
            lookup_key: payment_id.or(request_id).map(str::to_string),
            global_request_id: Some("global_1".to_string()),
            trace_id: Some("trace_1".to_string()),
            created_at_ms: 123,
            ..DomainAnalyticsEvent::new(
                AnalyticsFlowContext::new(ApiFlow::DynamicRouting, FlowType::DecideGatewayDecision),
                AnalyticsRoute::DecideGateway,
            )
        }
    }

    fn api_event() -> ApiEvent {
        ApiEvent {
            event_id: "0195f4a8-bdf0-7f36-a227-c0ffefeed010".to_string(),
            merchant_id: None,
            payment_id: Some("pay_123".to_string()),
            api_flow: ApiFlow::DynamicRouting,
            flow_type: FlowType::DecideGateway,
            created_at_timestamp: 1,
            request_id: "req_123".to_string(),
            global_request_id: Some("global_123".to_string()),
            trace_id: Some("trace_123".to_string()),
            latency: 1,
            status_code: 400,
            request: "{}".to_string(),
            user_agent: None,
            ip_addr: None,
            url_path: "/decide-gateway".to_string(),
            response: Some(r#"{"code":"bad_request"}"#.to_string()),
            error: Some(r#"{"code":"bad_request"}"#.to_string()),
            http_method: "POST".to_string(),
        }
    }

    /// The message ClickHouse ingests: every column, by name, with the envelope.
    fn message<T: serde::Serialize>(event: &T) -> serde_json::Value {
        let mut message = serde_json::to_value(KafkaEventRow::new(event)).unwrap();
        assert!(message["produced_at_ms"].as_i64().unwrap() > 0);
        message["produced_at_ms"] = json!(0);
        message
    }

    #[test]
    fn domain_message_carries_every_column() {
        let event = domain_event(Some("pay_1"), Some("req_1"));
        assert_eq!(
            message(&event),
            json!({
                "schema_version": 1,
                "produced_at_ms": 0,
                "event_id": "0195f4a8-bdf0-7f36-a227-c0ffefeed001",
                "api_flow": "dynamic_routing",
                "flow_type": "decide_gateway_decision",
                "summary_kind": "dynamic",
                "merchant_id": null,
                "payment_id": "pay_1",
                "request_id": "req_1",
                "lookup_key": "pay_1",
                "global_request_id": "global_1",
                "trace_id": "trace_1",
                "payment_method_type": null,
                "payment_method": null,
                "card_network": null,
                "card_is_in": null,
                "currency": null,
                "country": null,
                "auth_type": null,
                "gateway": null,
                "event_stage": null,
                "routing_approach": null,
                "rule_name": null,
                "status": null,
                "error_code": null,
                "error_message": null,
                "score_value": null,
                "sigma_factor": null,
                "average_latency": null,
                "tp99_latency": null,
                "transaction_count": null,
                "route": "decide_gateway",
                "details": null,
                "created_at_ms": 123,
            })
        );
    }

    #[test]
    fn api_message_carries_json_fields_as_strings() {
        assert_eq!(
            message(&api_event()),
            json!({
                "schema_version": 1,
                "produced_at_ms": 0,
                "event_id": "0195f4a8-bdf0-7f36-a227-c0ffefeed010",
                "merchant_id": null,
                "payment_id": "pay_123",
                "api_flow": "dynamic_routing",
                "flow_type": "decide_gateway",
                "created_at_timestamp": 1,
                "request_id": "req_123",
                "global_request_id": "global_123",
                "trace_id": "trace_123",
                "latency": 1,
                "status_code": 400,
                "request": "{}",
                "user_agent": null,
                "ip_addr": null,
                "url_path": "/decide-gateway",
                "response": "{\"code\":\"bad_request\"}",
                "error": "{\"code\":\"bad_request\"}",
                "http_method": "POST",
            })
        );
    }

    #[test]
    fn message_keys_are_deterministic() {
        assert_eq!(api_event_key(&api_event()), "req_123");
        assert_eq!(
            domain_event_key(&domain_event(Some("pay_456"), Some("req_456"))),
            "pay_456"
        );
    }

    #[test]
    fn domain_key_falls_back_to_request_lookup_key() {
        assert_eq!(
            domain_event_key(&domain_event(None, Some("req_only"))),
            "req_only"
        );
    }
}
