use crate::{error::IntoGrpcStatus, logger};
use tonic::Status;

#[derive(Debug, thiserror::Error)]
pub enum AuthenticationError {
    #[error("The request source is un-authenticated")]
    UnAuthenticated,
    #[error("The api-key is expired")]
    ApiKeyExpired,
    #[error("Missing header: {0}")]
    MissingHeader(&'static str),
    #[error("Missing config: {0}")]
    RoutingConfigsNotFound(String),
    #[error("Storage not constructed")]
    StorageNotFound,
}

impl IntoGrpcStatus for error_stack::Report<AuthenticationError> {
    fn into_grpc_status(self) -> Status {
        logger::error!(error=?self);
        match self.current_context() {
            AuthenticationError::UnAuthenticated => {
                Status::unauthenticated("Unauthenticated caller")
            }
            AuthenticationError::ApiKeyExpired => Status::unauthenticated("Api-key expired"),
            AuthenticationError::MissingHeader(e) => Status::not_found(e.to_string()),
            AuthenticationError::RoutingConfigsNotFound(e) => Status::not_found(e),
            AuthenticationError::StorageNotFound => Status::internal("Internal server error"),
        }
    }
}
