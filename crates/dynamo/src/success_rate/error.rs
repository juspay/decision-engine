use crate::{error::IntoGrpcStatus, logger};
use tonic::Status;

#[derive(Debug, thiserror::Error)]
pub enum SuccessRateError {
    #[error("Failed while executing redis command: {0}")]
    RedisError(&'static str),
    #[error("Failed to deserialize success rate configs")]
    DeserializationFailed,
    #[error("Failed to serialize from success rate configs")]
    SerializationFailed,
    #[error("Failed to get current unix timestamp")]
    FailedToGetCurrentTime,
    #[error("Failed while building config: {0}")]
    ConfigError(&'static str),
    #[error("Failed to convert {field} from {from} to {to}")]
    TypeConversionError {
        field: &'static str,
        from: &'static str,
        to: &'static str,
    },
}

impl IntoGrpcStatus for error_stack::Report<SuccessRateError> {
    fn into_grpc_status(self) -> Status {
        logger::error!(error=?self);
        match self.current_context() {
            SuccessRateError::RedisError(err) => Status::internal(err.to_string()),
            SuccessRateError::DeserializationFailed => Status::internal("Deserialization failure"),
            SuccessRateError::SerializationFailed => Status::internal("Serialization failed"),
            SuccessRateError::ConfigError(err) => Status::invalid_argument(err.to_string()),
            SuccessRateError::TypeConversionError { .. }
            | SuccessRateError::FailedToGetCurrentTime => Status::internal("Internal server error"),
        }
    }
}
