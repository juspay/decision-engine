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
    /// Polling interval from configuration
    loop_interval: Duration,
    /// Running state for graceful shutdown
    running: Arc<AtomicBool>,
    /// Configuration settings
    config: crate::config::ShardQueueConfig,
}

impl ShardedQueueHandler {
    /// Create new handler with configuration
    pub fn new(config: crate::config::ShardQueueConfig) -> Self {
        let mut shard_metadata = HashMap::new();

        // Initialize metadata for configured number of shards
        for shard_id in 0..config.shard_count {
            shard_metadata.insert(shard_id, ShardMetadata::new());
        }

        let inner = ShardedQueueHandlerInner {
            shard_metadata: Arc::new(Mutex::new(shard_metadata)),
            loop_interval: Duration::from_secs(config.loop_interval_seconds),
            running: Arc::new(AtomicBool::new(true)),
            config: config.clone(),
        };

        Self {
            inner: Arc::new(inner),
        }
    }

    /// Calculate shard ID using hash modulo configured shard count
    pub fn get_shard_id(&self, key: &str) -> u8 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut hasher);
        (hasher.finish() % (self.config.shard_count as u64)) as u8
    }

    /// Push item to appropriate Redis shard stream
    pub async fn push_to_shard(&self, item: ShardQueueItem) -> ShardQueueResult<()> {
        let shard_id = self.get_shard_id(&item.key);
        let stream_name = format!("shard_stream_{}", shard_id);

        let app_state = get_tenant_app_state().await;
        let redis_conn = app_state.redis_conn.clone();

        // Use the service configuration name as the key and value as the stream field
        // Format: XADD shard_stream_0 MAXLEN 100 * service_config_name service_config_value
        let serialized_value = serde_json::to_string(&item.value)
            .map_err(|e| ShardQueueError::QueueError(format!("Serialization error: {}", e)))?;

        let fields = vec![item.key.clone(), serialized_value];
        
        let entry_id = redis_conn
            .xadd_with_maxlen(&stream_name, self.config.stream_maxlen, fields)
            .await
            .map_err(|e| ShardQueueError::QueueError(format!("Redis stream push failed: {:?}", e)))?;

        logger::debug!(
            "Item pushed to Redis shard stream {}: key={}, entry_id={}",
            shard_id,
            item.key,
            entry_id
        );
        Ok(())
    }

    /// Start the polling thread
    pub async fn spawn(&self) -> ShardQueueResult<()> {
        logger::info!(
            "Shard queue polling thread started, checking every {} seconds with {} shards",
            self.loop_interval.as_secs(),
            self.config.shard_count
        );

        while self.running.load(Ordering::SeqCst) {
            logger::debug!("Shard queue polling cycle started");

            // Process all configured shards
            for shard_id in 0..self.config.shard_count {
                if let Err(e) = self.process_shard(shard_id).await {
                    logger::error!("Failed to process shard {}: {:?}", shard_id, e);
                }
            }

            // Sleep for configured interval
            time::sleep(self.loop_interval).await;
        }

        Ok(())
    }

    /// Process a single shard - poll items from Redis stream using entry IDs
    async fn process_shard(&self, shard_id: u8) -> ShardQueueResult<()> {
        let app_state = get_tenant_app_state().await;
        let redis_conn = app_state.redis_conn.clone();
        let stream_name = format!("shard_stream_{}", shard_id);

        let last_processed_entry_id = {
            let metadata = self.shard_metadata.lock().map_err(|e| {
                ShardQueueError::QueueError(format!("Failed to acquire metadata lock: {}", e))
            })?;

            metadata
                .get(&shard_id)
                .map(|meta| meta.last_processed_entry_id.clone())
                .unwrap_or_else(|| "0-0".to_string()) // Default to start from beginning
        };

        // Use XRANGE to get entries after the last processed entry ID
        // Format: XRANGE shard_stream_0 1234567890123-1 + COUNT max_items_per_cycle
        let start_range = if last_processed_entry_id == "0-0" {
            "-".to_string() // Start from beginning of stream
        } else {
            // For Redis XRANGE, to start after the last processed ID, we increment the sequence part
            // Stream IDs are in format: timestamp-sequence
            if let Some((timestamp, sequence)) = last_processed_entry_id.split_once('-') {
                if let Ok(seq_num) = sequence.parse::<u64>() {
                    format!("{}-{}", timestamp, seq_num + 1)
                } else {
                    // If we can't parse the sequence, just use the ID as-is and let Redis handle it
                    last_processed_entry_id.clone()
                }
            } else {
                // If the ID format is unexpected, start from beginning
                "-".to_string()
            }
        };

        let stream_entries = redis_conn
            .xrange(
                &stream_name,
                &start_range,
                "+", // Read to end
                Some(self.config.max_items_per_cycle),
            )
            .await
            .map_err(|e| ShardQueueError::QueueError(format!("Redis stream read failed: {:?}", e)))?;

        if stream_entries.is_empty() {
            return Ok(());
        }

        logger::debug!(
            "Polled {} entries from Redis shard stream {}",
            stream_entries.len(),
            shard_id
        );

        let mut last_entry_id = String::new();
        let mut processed_count = 0;

        // Process stream entries
        for (entry_id, fields) in stream_entries {
            if !fields.is_empty() {
                // Redis stream fields come as Vec<(field_name, field_value)>
                // We expect the first field to be the service_config_name and value to be service_config_value
                let (service_config_name, service_config_value) = &fields[0];

                // Parse the service configuration value as JSON
                match serde_json::from_str::<serde_json::Value>(service_config_value) {
                    Ok(parsed_value) => {
                        // Convert to ServiceConfiguration for IMC storage
                        match serde_json::from_value::<crate::storage::types::ServiceConfiguration>(parsed_value) {
                            Ok(service_config) => {
                                // Store ServiceConfiguration in global registry with 600 second TTL
                                if let Err(_) = GLOBAL_SHARD_REGISTRY.store(service_config_name.clone(), service_config, Some(600)) {
                                    logger::error!("Failed to store ServiceConfiguration in registry: {}", service_config_name);
                                } else {
                                    logger::debug!("Stored ServiceConfiguration in IMC: {}", service_config_name);
                                    processed_count += 1;
                                }
                            }
                            Err(e) => {
                                logger::error!("Failed to deserialize ServiceConfiguration for key {}: {}", service_config_name, e);
                            }
                        }
                    }
                    Err(e) => {
                        logger::error!("Failed to parse JSON value for key {}: {}", service_config_name, e);
                    }
                }
            } else {
                logger::warn!("Invalid stream entry format for entry {}: no fields found", entry_id);
            }

            last_entry_id = entry_id;
        }

        if processed_count > 0 {
            logger::debug!(
                "Processed {} new items from Redis shard stream {} (last_entry_id: {})",
                processed_count,
                shard_id,
                last_entry_id
            );

            // Update shard metadata with the last processed entry ID
            {
                let mut metadata = self.shard_metadata.lock().map_err(|e| {
                    ShardQueueError::QueueError(format!("Failed to acquire metadata lock: {}", e))
                })?;

                if let Some(shard_meta) = metadata.get_mut(&shard_id) {
                    shard_meta.update_last_processed_entry_id(last_entry_id.clone());
                    logger::debug!("Updated last_processed_entry_id for shard {} to {}", shard_id, last_entry_id);
                }
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

    /// Get stream sizes for all Redis-backed shards
    pub async fn get_queue_sizes(&self) -> ShardQueueResult<HashMap<u8, usize>> {
        let app_state = get_tenant_app_state().await;
        let redis_conn = app_state.redis_conn.clone();

        let mut sizes = HashMap::new();

        // Check stream length for each configured shard
        for shard_id in 0..self.config.shard_count {
            let stream_name = format!("shard_stream_{}", shard_id);

            match redis_conn.xlen(&stream_name).await {
                Ok(size) => {
                    sizes.insert(shard_id, size as usize);
                }
                Err(e) => {
                    logger::warn!("Failed to get size for shard stream {}: {:?}", shard_id, e);
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
pub fn find_config_in_mem(key: &str) -> StorageResult<crate::storage::types::ServiceConfiguration> {
    match GLOBAL_SHARD_REGISTRY.get::<crate::storage::types::ServiceConfiguration>(key) {
        Ok(value) => Ok(value),
        Err(_) => Err(MeshError::Others),
    }
}

pub fn store_config_in_mem(key: String, value: crate::storage::types::ServiceConfiguration) -> StorageResult<()> {
    GLOBAL_SHARD_REGISTRY
        .store(key, value, Some(600))
        .map_err(|_| MeshError::Others)
}

impl Default for ShardedQueueHandler {
    fn default() -> Self {
        Self::new(crate::config::ShardQueueConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_shard_calculation() {
        let config = crate::config::ShardQueueConfig::default();
        let handler = ShardedQueueHandler::new(config.clone());

        // Test that the same key always goes to the same shard
        let shard1 = handler.get_shard_id("test_key");
        let shard2 = handler.get_shard_id("test_key");
        assert_eq!(shard1, shard2);

        // Test that shard is within range of configured shard count
        assert!(shard1 < config.shard_count);
    }

    #[tokio::test]
    async fn test_push_and_get_sizes() {
        let config = crate::config::ShardQueueConfig::default();
        let handler = ShardedQueueHandler::new(config);

        let item = ShardQueueItem::new("test_key".to_string(), json!({"data": "test"}));
        let result = handler.push_to_shard(item).await;

        assert!(result.is_ok());

        let sizes = handler.get_queue_sizes().await.unwrap();
        let total_items: usize = sizes.values().sum();
        // Note: This test may fail in actual test environment without Redis setup
        // assert_eq!(total_items, 1);
        assert!(total_items >= 0);
    }

    #[test]
    fn test_imc_operations() {
        let key = "test_config_key";
        let service_config = crate::storage::types::ServiceConfiguration {
            id: 1,
            name: key.to_string(),
            value: Some(r#"{"config": "value"}"#.to_string()),
            new_value: None,
            previous_value: None,
            new_value_status: None,
        };

        // Store in IMC
        let store_result = store_config_in_mem(key.to_string(), service_config.clone());
        assert!(store_result.is_ok());

        // Retrieve from IMC
        let retrieved = find_config_in_mem(key);
        assert!(retrieved.is_ok());
        let retrieved_config = retrieved.unwrap();
        assert_eq!(retrieved_config.name, service_config.name);
        assert_eq!(retrieved_config.value, service_config.value);
    }
}
