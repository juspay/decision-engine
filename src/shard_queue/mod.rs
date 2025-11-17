//! Sharded Queue System
//! 
//! This module implements a sharded queue system with 10 shards, each containing
//! a queue for processing custom objects. A polling thread runs every 10 seconds
//! to process new entries from all shards.

pub mod handler;
pub mod registry;
pub mod types;

pub use handler::*;
pub use registry::*;
pub use types::*;

use once_cell::sync::Lazy;

/// Global singleton instance of ShardedQueueHandler
/// This ensures all parts of the application use the same queue instance
pub static GLOBAL_SHARD_QUEUE_HANDLER: Lazy<handler::ShardedQueueHandler> = 
    Lazy::new(|| handler::ShardedQueueHandler::new());
