use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnalyticsConfig {
    pub enabled: bool,
    pub kafka: KafkaConfig,
    pub clickhouse: ClickhouseConfig,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct KafkaConfig {
    pub brokers: Vec<String>,
    pub topic_prefix: String,
    pub batch_size: usize,
    pub batch_timeout_ms: u64,
    pub max_consecutive_failures: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ClickhouseConfig {
    pub host: String,
    pub username: String,
    pub password: Option<String>,
    pub database: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RoutingEventData {
    pub event_id: String,
    pub merchant_id: String,
    pub request_id: String,
    pub endpoint: String,
    pub method: String,
    pub request_payload: String,
    pub response_payload: String,
    pub status_code: u16,
    pub processing_time_ms: u32,
    pub gateway_selected: Option<String>,
    pub routing_algorithm_id: Option<String>,
    pub error_message: Option<String>,
    pub user_agent: Option<String>,
    pub ip_address: Option<String>,
    #[serde(with = "clickhouse_datetime")]
    pub created_at: OffsetDateTime,
    pub sign_flag: i8,
}

impl Default for AnalyticsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            kafka: KafkaConfig {
                brokers: vec!["localhost:9092".to_string()],
                topic_prefix: "decision-engine".to_string(),
                batch_size: 100,
                batch_timeout_ms: 1000,
                max_consecutive_failures: 5,
            },
            clickhouse: ClickhouseConfig {
                host: "http://localhost:8123".to_string(),
                username: "analytics_user".to_string(),
                password: Some("analytics_pass".to_string()),
                database: "decision_engine_analytics".to_string(),
            },
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AnalyticsError {
    #[error("Kafka error: {0}")]
    Kafka(#[from] kafka::Error),
    #[error("ClickHouse error: {0}")]
    ClickHouse(String),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("Configuration error: {0}")]
    Configuration(String),
}

pub type AnalyticsResult<T> = Result<T, AnalyticsError>;

// Custom datetime serialization for ClickHouse compatibility
mod clickhouse_datetime {
    use serde::{self, Deserialize, Deserializer, Serializer};
    use time::OffsetDateTime;
    use time::format_description::well_known::Rfc3339;

    pub fn serialize<S>(date: &OffsetDateTime, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Format as Unix timestamp for ClickHouse compatibility
        let timestamp = date.unix_timestamp();
        serializer.serialize_i64(timestamp)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<OffsetDateTime, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        
        // Try parsing as Unix timestamp first
        if let Ok(timestamp) = s.parse::<i64>() {
            return OffsetDateTime::from_unix_timestamp(timestamp)
                .map_err(serde::de::Error::custom);
        }
        
        // Fallback to RFC3339 parsing
        OffsetDateTime::parse(&s, &Rfc3339)
            .map_err(serde::de::Error::custom)
    }
}
