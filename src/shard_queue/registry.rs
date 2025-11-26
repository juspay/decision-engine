use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

/// Simple registry for caching with TTL support
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
        // Try read lock first (fast path for non-expired entries)
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_registry_basic_operations() {
        let registry = Registry::new(100);
        
        // Test store and get
        let value = json!({"test": "data"});
        assert!(registry.store("test_key".to_string(), value.clone(), Some(60)).is_ok());
        
        let retrieved: serde_json::Value = registry.get("test_key").unwrap();
        assert_eq!(retrieved, value);
        
        // Test non-existent key
        let result: Result<serde_json::Value, String> = registry.get("non_existent");
        assert!(result.is_err());
    }

    #[test]
    fn test_registry_ttl() {
        let registry = Registry::new(100);
        
        // Store with 0 TTL (should expire immediately)
        let value = json!({"test": "data"});
        registry.store("test_key".to_string(), value, Some(0)).unwrap();
        
        // Give it a moment to expire
        std::thread::sleep(std::time::Duration::from_millis(1));
        
        let result: Result<serde_json::Value, String> = registry.get("test_key");
        assert!(result.is_err());
    }
}
