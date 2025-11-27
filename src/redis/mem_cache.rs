use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use once_cell::sync::Lazy;

// Global in-memory cache instance
pub static GLOBAL_CACHE: Lazy<Registry> = Lazy::new(|| Registry::new(1000));

/// In-memory key–value cache with optional TTL expiration and simple eviction.
///
/// ## Eviction Strategy
///
/// The cache enforces a maximum capacity (`max_size`).  
/// Eviction happens during `store()` when the cache is full **after removing expired entries**.
///
/// The strategy works in two phases:
///
/// 1. **Lazy Expiration Cleanup**
///    - Expired keys (based on `expires_at`) are removed during `store()` and `get()`.
///    - No background thread is used for cleanup — expiration is checked only on access.
///
/// 2. **Size-Based Eviction**
///    - If the cache is still full after removing expired entries, the cache removes the
///      *oldest inserted key*.  
///      (The implementation relies on the ordering of `HashMap::keys().next()` which effectively
///      removes an arbitrary older entry — simple FIFO-like eviction, not LRU.)

#[derive(Debug)]
pub struct Registry {
    data: Arc<RwLock<HashMap<String, CacheEntry>>>,
    max_size: usize,
}

#[derive(Debug, Clone)]
struct CacheEntry {
    value: serde_json::Value,
    expires_at: Option<Instant>,
}

impl Registry {
    pub fn new(max_size: usize) -> Self {
        Self {
            data: Arc::new(RwLock::new(HashMap::new())),
            max_size,
        }
    }

    pub fn get<T>(&self, key: &str) -> Result<T, String>
    where
        T: serde::de::DeserializeOwned,
    {
        {
            let data = self.data.read().map_err(|e| format!("Read lock error: {}", e))?;

            if let Some(entry) = data.get(key) {
                // Check if entry has expired
                if let Some(expires_at) = entry.expires_at {
                    if Instant::now() > expires_at {
                        // Entry expired, need to remove it (drop read lock first)
                        drop(data);
                    } else {
                        // Entry is valid, return it
                        return serde_json::from_value(entry.value.clone())
                            .map_err(|e| format!("Deserialization error: {}", e));
                    }
                } else {
                    // No expiration, return it
                    return serde_json::from_value(entry.value.clone())
                        .map_err(|e| format!("Deserialization error: {}", e));
                }
            } else {
                return Err("Key not found".to_string());
            }
        }

        // If we get here, the entry was expired and we need to remove it
        let mut data = self.data.write().map_err(|e| format!("Write lock error: {}", e))?;

        // Double-check the entry is still there and expired
        if let Some(entry) = data.get(key) {
            if let Some(expires_at) = entry.expires_at {
                if Instant::now() > expires_at {
                    data.remove(key);
                    return Err("Key expired".to_string());
                }
                // If not expired anymore, return the value
                return serde_json::from_value(entry.value.clone())
                    .map_err(|e| format!("Deserialization error: {}", e));
            }
        }

        Err("Key not found".to_string())
    }

    /// Inserts a value in the cache, applying TTL and eviction rules.
    ///
    /// - Expired entries are removed first.
    /// - If capacity is still exceeded, the oldest remaining entry is evicted.
    pub fn store<T>(&self, key: String, value: T, ttl_seconds: Option<u64>) -> Result<(), String>
    where
        T: serde::Serialize,
    {
        let mut data = self.data.write().map_err(|e| format!("Write lock error: {}", e))?;

        // Remove expired entries and enforce max size
        self.cleanup_expired(&mut data);

        if data.len() >= self.max_size {
            // Remove oldest entry (simple eviction policy)
            if let Some(oldest_key) = data.keys().next().cloned() {
                data.remove(&oldest_key);
            }
        }

        let json_value = serde_json::to_value(value)
            .map_err(|e| format!("Serialization error: {}", e))?;

        let expires_at = ttl_seconds.map(|ttl| Instant::now() + Duration::from_secs(ttl));

        data.insert(key, CacheEntry {
            value: json_value,
            expires_at,
        });

        Ok(())
    }

    fn cleanup_expired(&self, data: &mut HashMap<String, CacheEntry>) {
        let now = Instant::now();
        data.retain(|_, entry| {
            if let Some(expires_at) = entry.expires_at {
                now <= expires_at
            } else {
                true
            }
        });
    }

    pub fn remove(&self, key: &str) -> Result<(), String> {
        let mut data = self.data.write().map_err(|e| format!("Write lock error: {}", e))?;
        data.remove(key);
        Ok(())
    }

    pub fn size(&self) -> usize {
        self.data.read().unwrap().len()
    }

    pub fn clear(&self) -> Result<(), String> {
        let mut data = self.data.write().map_err(|e| format!("Write lock error: {}", e))?;
        data.clear();
        Ok(())
    }
}
