use std::{
    collections::HashMap,
    hash::{Hash, Hasher},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use once_cell::sync::Lazy;
use tokio::{sync::mpsc, time};

use crate::{
    app::get_tenant_app_state,
    generics::{MeshError, StorageResult},
    logger,
};

use super::types::{ShardMetadata, ShardQueueError, ShardQueueItem, ShardQueueResult};

// Use our Registry pattern for service configuration caching
pub static GLOBAL_SHARD_REGISTRY: Lazy<super::registry::Registry> =
    Lazy::new(|| super::registry::Registry::new(1000));

/// Handler for the sharded queue system, following your existing patterns
#[derive(Clone)]
pub struct ShardedQueueHandler {
    inner: Arc<ShardedQueueHandlerInner>,
}

impl std::ops::Deref for ShardedQueueHandler {
    type Target = ShardedQueueHandlerInner;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

pub struct ShardedQueueHandlerInner {
    /// Metadata for each shard with last_modified
    shard_metadata: Arc<Mutex<HashMap<u8, ShardMetadata>>>,
    /// Polling interval (10 seconds)
    loop_interval: Duration,
    /// Running state for graceful shutdown
    running: Arc<AtomicBool>,
}

impl ShardedQueueHandler {
    /// Create new handler with 10 shards
    pub fn new() -> Self {
        let mut shard_metadata = HashMap::new();

        // Initialize metadata for 10 shards (0-9)
        for shard_id in 0..10 {
            shard_metadata.insert(shard_id, ShardMetadata::new());
        }

        let inner = ShardedQueueHandlerInner {
            shard_metadata: Arc::new(Mutex::new(shard_metadata)),
            loop_interval: Duration::from_secs(10), // 30 seconds for testing
            running: Arc::new(AtomicBool::new(true)),
        };

        Self {
            inner: Arc::new(inner),
        }
    }

    /// Calculate shard ID using hash modulo 10
    pub fn get_shard_id(&self, key: &str) -> u8 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hasher);
        (hasher.finish() % 10) as u8
    }

    /// Push item to appropriate Redis shard queue
    pub async fn push_to_shard(&self, item: ShardQueueItem) -> ShardQueueResult<()> {
        let shard_id = self.get_shard_id(&item.key);
        let redis_key = format!("shard_queue_{}", shard_id);

        let app_state = get_tenant_app_state().await;
        let redis_conn = app_state.redis_conn.clone();

        // Serialize the entire item (with timestamp) for Redis storage
        let serialized_item = serde_json::to_string(&item)
            .map_err(|e| ShardQueueError::QueueError(format!("Serialization error: {}", e)))?;

        redis_conn
            .append_to_list_start(&redis_key.into(), vec![serialized_item])
            .await
            .map_err(|e| ShardQueueError::QueueError(format!("Redis push failed: {:?}", e)))?;

        logger::debug!(
            "Item pushed to Redis shard queue {}: key={}",
            shard_id,
            item.key
        );
        Ok(())
    }

    /// Start the polling thread
    pub async fn spawn(&self) -> ShardQueueResult<()> {
        logger::info!(
            "Shard queue polling thread started, checking every {} seconds",
            self.loop_interval.as_secs()
        );

        while self.running.load(Ordering::SeqCst) {
            logger::debug!("Shard queue polling cycle started");

            // Process all shards (0-9)
            for shard_id in 0..10 {
                if let Err(e) = self.process_shard(shard_id).await {
                    logger::error!("Failed to process shard {}: {:?}", shard_id, e);
                }
            }

            // Sleep for 10 seconds
            time::sleep(self.loop_interval).await;
        }

        Ok(())
    }

    /// Process a single shard - poll items from Redis and filter by timestamp
    async fn process_shard(&self, shard_id: u8) -> ShardQueueResult<()> {
        let app_state = get_tenant_app_state().await;
        let redis_conn = app_state.redis_conn.clone();
        let redis_key = format!("shard_queue_{}", shard_id);

        let last_modified_at = {
            let metadata = self.shard_metadata.lock().map_err(|e| {
                ShardQueueError::QueueError(format!("Failed to acquire metadata lock: {}", e))
            })?;

            metadata
                .get(&shard_id)
                .map(|meta| meta.last_modified_at)
                .unwrap_or_else(|| chrono::Utc::now()) // Default to now if no metadata
        };

        let max_items_per_cycle = 100;
        let raw_items = redis_conn
            .get_range_from_list(&redis_key, 0, max_items_per_cycle - 1)
            .await
            .map_err(|e| ShardQueueError::QueueError(format!("Redis read failed: {:?}", e)))?;

        if raw_items.is_empty() {
            return Ok(());
        }

        logger::debug!(
            "Polled {} items from Redis shard queue {}",
            raw_items.len(),
            shard_id
        );

        for raw_item in raw_items {
            match serde_json::from_str::<ShardQueueItem>(&raw_item) {
                Ok(item) => {
                    if item.modified_at > last_modified_at {
                        new_items.push(item);
                    }
                }
                Err(e) => {
                    logger::error!("Failed to deserialize item from Redis queue: {}", e);
                }
            }
        }

        if new_items.is_empty() {
            return Ok(());
        }

        logger::debug!(
            "Processing {} new items from Redis shard {} (last_modified: {})",
            new_items.len(),
            shard_id,
            last_modified_at
        );

        // Store only new items in IMC using Registry pattern
        for item in &new_items {
            // Store in global registry with 600 second TTL
            if let Err(_) =
                GLOBAL_SHARD_REGISTRY.store(item.key.clone(), item.value.clone(), Some(600))
            {
                logger::error!("Failed to store item in registry: {}", item.key);
            } else {
                logger::debug!("Stored new item in IMC: {}", item.key);
            }
        }

        // Update shard metadata to current time after successful processing
        {
            let mut metadata = self.shard_metadata.lock().map_err(|e| {
                ShardQueueError::QueueError(format!("Failed to acquire metadata lock: {}", e))
            })?;

            if let Some(shard_meta) = metadata.get_mut(&shard_id) {
                shard_meta.update_last_modified();
                logger::debug!("Updated last_modified_at for shard {}", shard_id);
            }
        }

        Ok(())
    }

    /// Get shard metadata
    pub fn get_shard_metadata(&self, shard_id: u8) -> ShardQueueResult<Option<ShardMetadata>> {
        let metadata = self.shard_metadata.lock().map_err(|e| {
            ShardQueueError::QueueError(format!("Failed to acquire metadata lock: {}", e))
        })?;

        Ok(metadata.get(&shard_id).cloned())
    }

    /// Get all shard metadata
    pub fn get_all_shard_metadata(&self) -> ShardQueueResult<HashMap<u8, ShardMetadata>> {
        let metadata = self.shard_metadata.lock().map_err(|e| {
            ShardQueueError::QueueError(format!("Failed to acquire metadata lock: {}", e))
        })?;

        Ok(metadata.clone())
    }

    /// Get queue sizes for all Redis-backed shards
    pub async fn get_queue_sizes(&self) -> ShardQueueResult<HashMap<u8, usize>> {
        let app_state = get_tenant_app_state().await;
        let redis_conn = app_state.redis_conn.clone();

        let mut sizes = HashMap::new();

        // Check queue size for each shard (0-9)
        for shard_id in 0..10 {
            let redis_key = format!("shard_queue_{}", shard_id);

            match redis_conn.get_list_length(&redis_key).await {
                Ok(size) => {
                    sizes.insert(shard_id, size);
                }
                Err(e) => {
                    logger::warn!("Failed to get size for shard {}: {:?}", shard_id, e);
                    sizes.insert(shard_id, 0); // Default to 0 if we can't get the size
                }
            }
        }

        Ok(sizes)
    }

    /// Close the handler - similar to drainer close()
    pub fn close(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    /// Shutdown listener - similar to drainer shutdown_listener()
    pub async fn shutdown_listener(&self, mut rx: mpsc::Receiver<()>) {
        while let Some(_) = rx.recv().await {
            logger::info!("Shutdown signal received for shard queue handler");
            rx.close();
            self.close();
            break;
        }
        logger::info!("Shard queue handler shutdown completed");
    }

    /// Check if handler is running
    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

/// IMC functions following your existing pattern for service_configuration caching
pub fn find_config_in_mem(key: &str) -> StorageResult<serde_json::Value> {
    match GLOBAL_SHARD_REGISTRY.get::<serde_json::Value>(key) {
        Ok(value) => Ok(value),
        Err(_) => Err(MeshError::Others),
    }
}

pub fn store_config_in_mem(key: String, value: serde_json::Value) -> StorageResult<()> {
    GLOBAL_SHARD_REGISTRY
        .store(key, value, Some(600))
        .map_err(|_| MeshError::Others)
}

impl Default for ShardedQueueHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_shard_calculation() {
        let handler = ShardedQueueHandler::new();

        // Test that the same key always goes to the same shard
        let shard1 = handler.get_shard_id("test_key");
        let shard2 = handler.get_shard_id("test_key");
        assert_eq!(shard1, shard2);

        // Test that shard is within range 0-9
        assert!(shard1 < 10);
    }

    #[test]
    fn test_push_and_get_sizes() {
        let handler = ShardedQueueHandler::new();

        let item = ShardQueueItem::new("test_key".to_string(), json!({"data": "test"}));
        let result = handler.push_to_shard(item);

        assert!(result.is_ok());

        let sizes = handler.get_queue_sizes().unwrap();
        let total_items: usize = sizes.values().sum();
        assert_eq!(total_items, 1);
    }

    #[test]
    fn test_imc_operations() {
        let key = "test_config_key";
        let value = json!({"config": "value"});

        // Store in IMC
        let store_result = store_config_in_mem(key.to_string(), value.clone());
        assert!(store_result.is_ok());

        // Retrieve from IMC
        let retrieved = find_config_in_mem(key);
        assert!(retrieved.is_ok());
        assert_eq!(retrieved.unwrap(), value);
    }
}
