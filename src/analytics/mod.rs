pub mod client;
pub mod events;
pub mod kafka_producer;
pub mod middleware;
pub mod models;
pub mod service;
pub mod types;

pub use client::AnalyticsClient;
pub use events::RoutingEvent;
pub use kafka_producer::KafkaProducer;
pub use middleware::analytics_middleware;
pub use models::*;
pub use service::*;
pub use types::*;
