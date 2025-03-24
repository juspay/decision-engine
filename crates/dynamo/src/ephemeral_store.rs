use std::sync::Arc;

use redis_interface::RedisConnectionPool;
use rustc_hash::FxHashMap;
use tokio::sync::RwLock;

use crate::{
    configs::KeysTtl,
    ephemeral_store::success_rate::SuccessRateEphemeralStoreInterface,
    success_rate::block::{Block, CurrentBlock},
};

pub mod success_rate;

#[derive(Clone)]
pub struct RedisEphemeralStore {
    pub redis_conn: Arc<RedisConnectionPool>,
    pub ttl: KeysTtl,
}

impl RedisEphemeralStore {
    pub fn new(redis_conn: Arc<RedisConnectionPool>, ttl: KeysTtl) -> Self {
        Self { redis_conn, ttl }
    }
}

#[derive(Clone)]
pub struct InMemoryEphemeralStore {
    success_rate: Arc<RwLock<SuccessRateImcEphemeralStore>>,
}

struct SuccessRateImcEphemeralStore {
    aggregates: FxHashMap<String, Vec<Block>>,
    current_blocks: FxHashMap<String, CurrentBlock>,
}

impl Default for InMemoryEphemeralStore {
    fn default() -> Self {
        Self {
            success_rate: Arc::new(RwLock::new(SuccessRateImcEphemeralStore {
                aggregates: FxHashMap::default(),
                current_blocks: FxHashMap::default(),
            })),
        }
    }
}

pub trait EphemeralStoreInterface: SuccessRateEphemeralStoreInterface + Sync + Send {}

impl EphemeralStoreInterface for RedisEphemeralStore {}

impl EphemeralStoreInterface for InMemoryEphemeralStore {}
