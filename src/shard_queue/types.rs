use std::collections::HashMap;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ShardQueueItem {
    pub key: String,
    pub value: serde_json::Value,
    pub modified_at: DateTime<Utc>,
}

impl ShardQueueItem {
    pub fn new(key: String, value: serde_json::Value) -> Self {
        Self {
            key,
            value,
            modified_at: Utc::now(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ShardMetadata {
    pub last_modified_at: DateTime<Utc>,
}

impl ShardMetadata {
    pub fn new() -> Self {
        Self {
            last_modified_at: Utc::now(),
        }
    }
    
    pub fn update_last_modified(&mut self) {
        self.last_modified_at = Utc::now();
    }
}

impl Default for ShardMetadata {
    fn default() -> Self {
        Self::new()
    }
}

pub type InMemoryCache = HashMap<String, serde_json::Value>;

#[derive(Debug, thiserror::Error)]
pub enum ShardQueueError {
    #[error("Invalid shard ID: {0}")]
    InvalidShardId(u8),
    #[error("Queue error: {0}")]
    QueueError(String),
}

pub type ShardQueueResult<T> = Result<T, ShardQueueError>;
