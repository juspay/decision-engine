use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use once_cell::sync::Lazy;

// Global in-memory cache instance
pub static GLOBAL_CACHE: Lazy<Registry> = Lazy::new(|| Registry::new(1000));

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
                if let Some(expires_at) = entry.expires_at {
                    if Instant::now() > expires_at {
                        drop(data);
                    } else {
                        return serde_json::from_value(entry.value.clone())
                            .map_err(|e| format!("Deserialization error: {}", e));
                    }
                } else {
                    return serde_json::from_value(entry.value.clone())
                        .map_err(|e| format!("Deserialization error: {}", e));
                }
            } else {
                return Err("Key not found".to_string());
            }
        }

        let mut data = self.data.write().map_err(|e| format!("Write lock error: {}", e))?;

        if let Some(entry) = data.get(key) {
            if let Some(expires_at) = entry.expires_at {
                if Instant::now() > expires_at {
                    data.remove(key);
                    return Err("Key expired".to_string());
                }

                return serde_json::from_value(entry.value.clone())
                    .map_err(|e| format!("Deserialization error: {}", e));
            }
        }

        Err("Key not found".to_string())
    }

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
