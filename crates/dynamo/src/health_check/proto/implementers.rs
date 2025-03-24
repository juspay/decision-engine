use super::types as proto_types;
use crate::{health_check::types, logger};
use tonic::{Request, Response, Status};

#[tonic::async_trait]
impl proto_types::Health for types::HealthCheck {
    async fn check(
        &self,
        request: Request<proto_types::HealthCheckRequest>,
    ) -> Result<Response<proto_types::HealthCheckResponse>, Status> {
        logger::debug!(?request, "health_check request");

        let response = proto_types::HealthCheckResponse {
            status: proto_types::ServingStatus::Serving.into(),
        };
        logger::info!(?response, "health_check response");

        Ok(Response::new(response))
    }
}
