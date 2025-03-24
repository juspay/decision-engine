use crate::{error::IntoGrpcStatus, logger};
use tonic::Status;

#[derive(Debug, thiserror::Error)]
pub enum EliminationError {
    #[error("Failed while executing redis command: {0}")]
    RedisError(&'static str),
    #[error("Failed to serialize elimination routing configs")]
    SerializationFailed,
    #[error("Failed to deserialize elimination routing configs")]
    DeserializationFailed,
    #[error("Failed while building config: {0}")]
    ConfigError(&'static str),
    #[error("Failed to convert {field} from {from} to {to}")]
    TypeConversionError {
        field: &'static str,
        from: &'static str,
        to: &'static str,
    },
    #[error("Failed to compute elapsed time since earlier is later than self")]
    FailedToGetElapsedTime,
}

impl IntoGrpcStatus for error_stack::Report<EliminationError> {
    fn into_grpc_status(self) -> Status {
        logger::error!(error=?self);
        match self.current_context() {
            EliminationError::SerializationFailed => Status::internal("Serialization failed"),
            EliminationError::DeserializationFailed => Status::internal("Deserialization failure"),
            EliminationError::RedisError(err) => Status::internal(err.to_string()),
            EliminationError::ConfigError(err) => Status::invalid_argument(err.to_string()),
            EliminationError::TypeConversionError { .. }
            | EliminationError::FailedToGetElapsedTime => Status::internal("Internal server error"),
        }
    }
}
