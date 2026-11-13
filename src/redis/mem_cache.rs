use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};
use std::time::{Duration, Instant};

use crate::config::MemCacheConfig;

/// Populated once from `GlobalConfig` before the server starts serving requests.
static MEM_CACHE_CONFIG: OnceLock<MemCacheConfig> = OnceLock::new();

/// Called from `main()` immediately after the config is loaded.
/// Must be called before the first request is served.
pub fn init(config: MemCacheConfig) {
    let _ = MEM_CACHE_CONFIG.set(config);
}

pub fn mem_cache_config() -> &'static MemCacheConfig {
    MEM_CACHE_CONFIG.get_or_init(MemCacheConfig::default)
}

// ── SR V3 score cache ─────────────────────────────────────────────────────────
//
// Short-lived (75ms) in-process cache for gateway SR scores read in the routing
// hot-path (`get_cached_scores_based_on_srv3`).  The async-feedback path already
// means Redis scores lag reality by a round-trip; 75ms of additional staleness
// is imperceptible to the MAB algorithm (scores average over 200-500 txns).
//
// The analytics snapshot caller (`gateway_scoring_service.rs`) deliberately
// bypasses this cache by calling `get_score_from_redis` directly.
//
// Both `get` and `store` use `try_lock` (non-blocking).  On contention they
// simply skip the cache — a missed hit costs one Redis round-trip, whereas
// blocking a Tokio worker thread waiting for the lock costs far more.
pub static SR_SCORE_CACHE: Lazy<SrScoreCache> =
    Lazy::new(|| SrScoreCache::new(mem_cache_config().sr_score_ttl_ms, 50_000));

pub struct SrScoreCache {
    data: std::sync::Mutex<HashMap<String, (f64, Instant)>>,
    ttl: Duration,
    max_size: usize,
}

impl SrScoreCache {
    pub fn new(ttl_ms: u64, max_size: usize) -> Self {
        Self {
            data: std::sync::Mutex::new(HashMap::new()),
            ttl: Duration::from_millis(ttl_ms),
            max_size,
        }
    }

    /// Returns the cached score for `key` if it was stored within the TTL window.
    /// Returns `None` on contention — the caller falls back to Redis.
    pub fn get(&self, key: &str) -> Option<f64> {
        let data = self.data.try_lock().ok()?;
        if let Some(&(score, stored_at)) = data.get(key) {
            if stored_at.elapsed() < self.ttl {
                return Some(score);
            }
        }
        None
    }

    /// Stores a score. Silently skips on lock contention or when at capacity
    /// with no expired entries to evict.
    pub fn store(&self, key: String, score: f64) {
        let Ok(mut data) = self.data.try_lock() else {
            return;
        };
        if data.len() >= self.max_size {
            let ttl = self.ttl;
            let evict_key = data
                .iter()
                .find(|(_, (_, stored_at))| stored_at.elapsed() >= ttl)
                .map(|(k, _)| k.clone())
                .or_else(|| data.keys().next().cloned());
            if let Some(k) = evict_key {
                data.remove(&k);
            }
        }
        data.insert(key, (score, Instant::now()));
    }
}

// ── Generic hot-path cache ────────────────────────────────────────────────────
//
// Strongly-typed, non-blocking TTL cache for any Clone + Send + Sync value.
//
// Design properties (same non-blocking contract as SrScoreCache):
//   - `std::sync::Mutex::try_lock()` on every access — never blocks a Tokio
//     worker thread.  Safe on single-core runtimes.
//   - Cache miss on lock contention: caller falls back to the authoritative
//     source rather than waiting.  Correctness is never compromised.
//   - Values are stored as `Arc<T>`. The Mutex is held only for a HashMap
//     lookup + `Arc::clone` (one atomic increment — nanoseconds).  The actual
//     `T::clone` happens after the lock is released, so large values (e.g.
//     `Vec<GatewayOutage>`) never cause lock contention under high concurrency.
//   - `Arc::new(value)` in `store` runs before the lock is acquired for the
//     same reason.
//   - Bounded by `max_size` with expired-first eviction (falls back to
//     arbitrary eviction only when no expired entry exists).
pub struct TypedCache<T: Clone + Send + Sync> {
    data: std::sync::Mutex<HashMap<String, (Arc<T>, Instant)>>,
    ttl: Duration,
    max_size: usize,
}

impl<T: Clone + Send + Sync> TypedCache<T> {
    pub fn new(ttl_ms: u64, max_size: usize) -> Self {
        Self {
            data: std::sync::Mutex::new(HashMap::new()),
            ttl: Duration::from_millis(ttl_ms),
            max_size,
        }
    }

    /// Returns the cached value if present and within TTL.
    /// Lock is held only for the HashMap lookup + Arc::clone (nanoseconds).
    /// T::clone runs outside the lock so large values don't create contention.
    /// Returns `None` on lock contention — caller falls back to the source.
    pub fn get(&self, key: &str) -> Option<T> {
        let arc = {
            let data = self.data.try_lock().ok()?;
            let (arc, stored_at) = data.get(key)?;
            if stored_at.elapsed() >= self.ttl {
                return None;
            }
            arc.clone() // Arc::clone — one atomic increment
        }; // Mutex released here
        Some((*arc).clone()) // T::clone outside the lock
    }

    /// Stores a value. Arc allocation happens before acquiring the lock.
    /// Silently skips on lock contention or when at capacity
    /// with no expired entries to evict.
    pub fn store(&self, key: String, value: T) {
        let arc = Arc::new(value); // heap allocation before the lock
        let Ok(mut data) = self.data.try_lock() else {
            return;
        };
        if data.len() >= self.max_size {
            let ttl = self.ttl;
            let evict_key = data
                .iter()
                .find(|(_, (_, stored_at))| stored_at.elapsed() >= ttl)
                .map(|(k, _)| k.clone())
                .or_else(|| data.keys().next().cloned());
            if let Some(k) = evict_key {
                data.remove(&k);
            }
        }
        data.insert(key, (arc, Instant::now()));
    }
}

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
            let data = self
                .data
                .read()
                .map_err(|e| format!("Read lock error: {}", e))?;

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
        let mut data = self
            .data
            .write()
            .map_err(|e| format!("Write lock error: {}", e))?;

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
        let mut data = self
            .data
            .write()
            .map_err(|e| format!("Write lock error: {}", e))?;

        // Remove expired entries and enforce max size
        self.cleanup_expired(&mut data);

        if data.len() >= self.max_size {
            // Remove oldest entry (simple eviction policy)
            if let Some(oldest_key) = data.keys().next().cloned() {
                data.remove(&oldest_key);
            }
        }

        let json_value =
            serde_json::to_value(value).map_err(|e| format!("Serialization error: {}", e))?;

        let expires_at = ttl_seconds.map(|ttl| Instant::now() + Duration::from_secs(ttl));

        data.insert(
            key,
            CacheEntry {
                value: json_value,
                expires_at,
            },
        );

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
        let mut data = self
            .data
            .write()
            .map_err(|e| format!("Write lock error: {}", e))?;
        data.remove(key);
        Ok(())
    }

    pub fn size(&self) -> usize {
        self.data.read().unwrap().len()
    }

    pub fn clear(&self) -> Result<(), String> {
        let mut data = self
            .data
            .write()
            .map_err(|e| format!("Write lock error: {}", e))?;
        data.clear();
        Ok(())
    }
}
