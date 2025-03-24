#[allow(
    unused_qualifications,
    clippy::use_self,
    clippy::unwrap_used,
    clippy::as_conversions
)]
pub mod proto_items {
    tonic::include_proto!("grpc.health.v1");
}

pub use proto_items::{
    health_check_response::ServingStatus,
    health_server::{Health, HealthServer},
    HealthCheckRequest, HealthCheckResponse,
};
