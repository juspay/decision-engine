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
    /// Last processed entry ID from Redis stream (e.g., "1-0")
    pub last_processed_entry_id: String,
}

impl ShardMetadata {
    pub fn new() -> Self {
        Self {
            // Start from "0-0" to process all entries from beginning
            last_processed_entry_id: "0-0".to_string(),
        }
    }
    
    pub fn update_last_processed_entry_id(&mut self, entry_id: String) {
        self.last_processed_entry_id = entry_id;
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
