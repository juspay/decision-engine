pub mod handler;
pub mod registry;
pub mod types;

pub use handler::*;
pub use registry::*;
pub use types::*;

use once_cell::sync::Lazy;

pub static GLOBAL_SHARD_QUEUE_HANDLER: Lazy<handler::ShardedQueueHandler> = 
    Lazy::new(|| handler::ShardedQueueHandler::new());
