use std::collections::HashMap;
use std::future::Future;
use std::time::Duration;

use rdkafka::consumer::{CommitMode, Consumer, StreamConsumer};
use rdkafka::message::Message;
use rdkafka::util::Timeout;
use rdkafka::{Offset, TopicPartitionList};

use crate::analytics::clickhouse::ClickHouseAnalyticsStore;
use crate::analytics::events::{ApiEvent, DomainAnalyticsEvent};
use crate::analytics::kafka::{worker_client_config, AnalyticsKafkaEnvelopeV1};
use crate::config::AnalyticsConfig;
use crate::error::ConfigurationError;
use crate::metrics::{
    ANALYTICS_EVENTS_DROPPED_TOTAL, ANALYTICS_WORKER_BATCHES_TOTAL, ANALYTICS_WORKER_RETRY_TOTAL,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct TopicPartitionKey {
    topic: String,
    partition: i32,
}

impl TopicPartitionKey {
    fn new(topic: String, partition: i32) -> Self {
        Self { topic, partition }
    }
}

#[derive(Debug, Clone)]
struct WorkerBatch<T> {
    topic: String,
    partition: i32,
    first_offset: i64,
    last_offset: i64,
    events: Vec<T>,
}

impl<T> WorkerBatch<T> {
    fn new(topic: String, partition: i32, offset: i64) -> Self {
        Self {
            topic,
            partition,
            first_offset: offset,
            last_offset: offset,
            events: Vec::new(),
        }
    }

    fn push(&mut self, event: T, offset: i64) {
        self.last_offset = offset;
        self.events.push(event);
    }

    fn deduplication_token(&self) -> String {
        format!(
            "{}:{}:{}-{}",
            self.topic, self.partition, self.first_offset, self.last_offset
        )
    }
}

pub struct AnalyticsWorker {
    consumer: StreamConsumer,
    clickhouse: ClickHouseAnalyticsStore,
    config: AnalyticsConfig,
}

impl AnalyticsWorker {
    pub async fn new(config: AnalyticsConfig) -> Result<Self, ConfigurationError> {
        if !config.enabled {
            return Err(ConfigurationError::InvalidConfigurationValueError(
                "analytics.enabled".to_string(),
            ));
        }

        let clickhouse = ClickHouseAnalyticsStore::new(config.clickhouse.clone())
            .await
            .map_err(|_| {
                ConfigurationError::InvalidConfigurationValueError(
                    "analytics.clickhouse".to_string(),
                )
            })?;

        let consumer = worker_client_config(&config.kafka, &config.worker.consumer_group)
            .create::<StreamConsumer>()
            .map_err(|_| {
                ConfigurationError::InvalidConfigurationValueError("analytics.kafka".to_string())
            })?;

        consumer
            .client()
            .fetch_metadata(None, Timeout::After(Duration::from_secs(5)))
            .map_err(|_| {
                ConfigurationError::InvalidConfigurationValueError(
                    "analytics.kafka.brokers".to_string(),
                )
            })?;

        consumer
            .subscribe(&[&config.kafka.api_topic, &config.kafka.domain_topic])
            .map_err(|_| {
                ConfigurationError::InvalidConfigurationValueError("analytics.kafka".to_string())
            })?;

        Ok(Self {
            consumer,
            clickhouse,
            config,
        })
    }

    pub async fn run_until_shutdown<F>(self, shutdown: F) -> Result<(), ConfigurationError>
    where
        F: Future<Output = ()>,
    {
        let mut api_batches = HashMap::<TopicPartitionKey, WorkerBatch<ApiEvent>>::new();
        let mut domain_batches =
            HashMap::<TopicPartitionKey, WorkerBatch<DomainAnalyticsEvent>>::new();
        let mut interval = tokio::time::interval(self.flush_interval());
        tokio::pin!(shutdown);

        loop {
            tokio::select! {
                _ = &mut shutdown => {
                    self.flush_all_batches(&mut api_batches, &mut domain_batches).await;
                    break;
                }
                message = tokio::time::timeout(self.poll_timeout(), self.consumer.recv()) => {
                    match message {
                        Ok(Ok(message)) => {
                            self.handle_message(message, &mut api_batches, &mut domain_batches)
                                .await;
                        }
                        Ok(Err(error)) => {
                            crate::logger::warn!(error = %error, "Failed to receive Kafka analytics message");
                        }
                        Err(_elapsed) => {}
                    }
                }
                _ = interval.tick() => {
                    self.flush_all_batches(&mut api_batches, &mut domain_batches).await;
                }
            }
        }

        Ok(())
    }

    async fn handle_message(
        &self,
        message: rdkafka::message::BorrowedMessage<'_>,
        api_batches: &mut HashMap<TopicPartitionKey, WorkerBatch<ApiEvent>>,
        domain_batches: &mut HashMap<TopicPartitionKey, WorkerBatch<DomainAnalyticsEvent>>,
    ) {
        let topic = message.topic().to_string();
        let partition = message.partition();
        let offset = message.offset();
        let Some(payload) = message.payload() else {
            self.skip_bad_message("unknown", &topic, partition, offset, "missing_payload");
            return;
        };

        if topic == self.config.kafka.api_topic {
            match serde_json::from_slice::<AnalyticsKafkaEnvelopeV1<ApiEvent>>(payload) {
                Ok(envelope) => {
                    let key = TopicPartitionKey::new(topic.clone(), partition);
                    let should_flush = {
                        let batch = api_batches
                            .entry(key.clone())
                            .or_insert_with(|| WorkerBatch::new(topic.clone(), partition, offset));
                        batch.push(envelope.payload, offset);
                        batch.events.len() >= self.batch_size()
                    };
                    if should_flush {
                        if let Some(batch) = api_batches.remove(&key) {
                            self.flush_api_batch(batch).await;
                        }
                    }
                }
                Err(error) => {
                    self.skip_bad_message("api", &topic, partition, offset, &error.to_string());
                }
            }
        } else if topic == self.config.kafka.domain_topic {
            match serde_json::from_slice::<AnalyticsKafkaEnvelopeV1<DomainAnalyticsEvent>>(payload)
            {
                Ok(envelope) => {
                    let key = TopicPartitionKey::new(topic.clone(), partition);
                    let should_flush = {
                        let batch = domain_batches
                            .entry(key.clone())
                            .or_insert_with(|| WorkerBatch::new(topic.clone(), partition, offset));
                        batch.push(envelope.payload, offset);
                        batch.events.len() >= self.batch_size()
                    };
                    if should_flush {
                        if let Some(batch) = domain_batches.remove(&key) {
                            self.flush_domain_batch(batch).await;
                        }
                    }
                }
                Err(error) => {
                    self.skip_bad_message("domain", &topic, partition, offset, &error.to_string());
                }
            }
        } else {
            self.skip_bad_message("unknown", &topic, partition, offset, "unexpected_topic");
        }
    }

    async fn flush_all_batches(
        &self,
        api_batches: &mut HashMap<TopicPartitionKey, WorkerBatch<ApiEvent>>,
        domain_batches: &mut HashMap<TopicPartitionKey, WorkerBatch<DomainAnalyticsEvent>>,
    ) {
        for batch in std::mem::take(api_batches).into_values() {
            self.flush_api_batch(batch).await;
        }

        for batch in std::mem::take(domain_batches).into_values() {
            self.flush_domain_batch(batch).await;
        }
    }

    async fn flush_api_batch(&self, batch: WorkerBatch<ApiEvent>) {
        if batch.events.is_empty() {
            return;
        }

        let token = batch.deduplication_token();
        loop {
            match self
                .clickhouse
                .persist_api_events_with_token(&batch.events, Some(&token))
                .await
            {
                Ok(()) => match self.commit_next_offset(
                    &batch.topic,
                    batch.partition,
                    batch.last_offset + 1,
                ) {
                    Ok(()) => {
                        ANALYTICS_WORKER_BATCHES_TOTAL
                            .with_label_values(&["api", "success"])
                            .inc();
                        break;
                    }
                    Err(error) => {
                        ANALYTICS_WORKER_BATCHES_TOTAL
                            .with_label_values(&["api", "failure"])
                            .inc();
                        ANALYTICS_WORKER_RETRY_TOTAL
                            .with_label_values(&["api"])
                            .inc();
                        crate::logger::warn!(
                            error = %error,
                            topic = %batch.topic,
                            partition = batch.partition,
                            token = %token,
                            "Failed to commit Kafka analytics api offsets"
                        );
                    }
                },
                Err(error) => {
                    ANALYTICS_WORKER_BATCHES_TOTAL
                        .with_label_values(&["api", "failure"])
                        .inc();
                    ANALYTICS_WORKER_RETRY_TOTAL
                        .with_label_values(&["api"])
                        .inc();
                    crate::logger::warn!(
                        error = %error,
                        topic = %batch.topic,
                        partition = batch.partition,
                        token = %token,
                        "Failed to persist Kafka analytics api batch"
                    );
                }
            }

            tokio::time::sleep(self.retry_backoff()).await;
        }
    }

    async fn flush_domain_batch(&self, batch: WorkerBatch<DomainAnalyticsEvent>) {
        if batch.events.is_empty() {
            return;
        }

        let token = batch.deduplication_token();
        loop {
            match self
                .clickhouse
                .persist_domain_events_with_token(&batch.events, Some(&token))
                .await
            {
                Ok(()) => match self.commit_next_offset(
                    &batch.topic,
                    batch.partition,
                    batch.last_offset + 1,
                ) {
                    Ok(()) => {
                        ANALYTICS_WORKER_BATCHES_TOTAL
                            .with_label_values(&["domain", "success"])
                            .inc();
                        break;
                    }
                    Err(error) => {
                        ANALYTICS_WORKER_BATCHES_TOTAL
                            .with_label_values(&["domain", "failure"])
                            .inc();
                        ANALYTICS_WORKER_RETRY_TOTAL
                            .with_label_values(&["domain"])
                            .inc();
                        crate::logger::warn!(
                            error = %error,
                            topic = %batch.topic,
                            partition = batch.partition,
                            token = %token,
                            "Failed to commit Kafka analytics domain offsets"
                        );
                    }
                },
                Err(error) => {
                    ANALYTICS_WORKER_BATCHES_TOTAL
                        .with_label_values(&["domain", "failure"])
                        .inc();
                    ANALYTICS_WORKER_RETRY_TOTAL
                        .with_label_values(&["domain"])
                        .inc();
                    crate::logger::warn!(
                        error = %error,
                        topic = %batch.topic,
                        partition = batch.partition,
                        token = %token,
                        "Failed to persist Kafka analytics domain batch"
                    );
                }
            }

            tokio::time::sleep(self.retry_backoff()).await;
        }
    }

    fn skip_bad_message(
        &self,
        stream: &'static str,
        topic: &str,
        partition: i32,
        offset: i64,
        reason: &str,
    ) {
        ANALYTICS_EVENTS_DROPPED_TOTAL
            .with_label_values(&[stream, "decode_error"])
            .inc();
        crate::logger::warn!(
            topic = %topic,
            partition,
            offset,
            reason,
            "Skipping malformed Kafka analytics message"
        );

        if let Err(error) = self.commit_next_offset(topic, partition, offset + 1) {
            crate::logger::warn!(
                error = %error,
                topic = %topic,
                partition,
                offset,
                "Failed to commit Kafka offset for skipped analytics message"
            );
        }
    }

    fn commit_next_offset(
        &self,
        topic: &str,
        partition: i32,
        next_offset: i64,
    ) -> Result<(), ConfigurationError> {
        let mut tpl = TopicPartitionList::new();
        tpl.add_partition_offset(topic, partition, Offset::Offset(next_offset))
            .map_err(|_| {
                ConfigurationError::InvalidConfigurationValueError(
                    "analytics.worker.commit_offset".to_string(),
                )
            })?;
        self.consumer.commit(&tpl, CommitMode::Sync).map_err(|_| {
            ConfigurationError::InvalidConfigurationValueError(
                "analytics.worker.commit_offset".to_string(),
            )
        })
    }

    fn flush_interval(&self) -> Duration {
        Duration::from_millis(self.config.worker.flush_interval_ms.max(1))
    }

    fn retry_backoff(&self) -> Duration {
        Duration::from_millis(self.config.worker.retry_backoff_ms.max(1))
    }

    fn batch_size(&self) -> usize {
        self.config.worker.batch_size.max(1)
    }

    fn poll_timeout(&self) -> Duration {
        Duration::from_millis(self.config.worker.poll_timeout_ms.max(1))
    }
}

#[cfg(test)]
mod tests {
    use super::WorkerBatch;

    #[test]
    fn deduplication_token_includes_partition_offset_range() {
        let mut batch = WorkerBatch::new("topic-a".to_string(), 3, 42);
        batch.push("one", 42);
        batch.push("two", 44);
        assert_eq!(batch.deduplication_token(), "topic-a:3:42-44");
    }
}
