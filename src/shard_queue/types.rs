use std::collections::HashMap;
use chrono::{DateTime, Utc};

/// Custom object for shard queues - simple structure  
#[derive(Debug, Clone)]
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

/// Simple metadata for each shard
#[derive(Debug, Clone)]
pub struct ShardMetadata {
    pub shard_id: u8,
    pub last_modified_at: DateTime<Utc>,
}

impl ShardMetadata {
    pub fn new(shard_id: u8) -> Self {
        Self {
            shard_id,
            last_modified_at: Utc::now(),
        }
    }
    
    pub fn update_last_modified(&mut self) {
        self.last_modified_at = Utc::now();
    }
}

/// Simple in-memory cache
pub type InMemoryCache = HashMap<String, serde_json::Value>;

/// Simple errors
#[derive(Debug, thiserror::Error)]
pub enum ShardQueueError {
    #[error("Invalid shard ID: {0}")]
    InvalidShardId(u8),
    #[error("Queue error: {0}")]
    QueueError(String),
}

pub type ShardQueueResult<T> = Result<T, ShardQueueError>;
