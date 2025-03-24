use crate::{error::IntoGrpcStatus, logger};
use tonic::Status;

#[derive(Debug, thiserror::Error)]
pub enum ContractRoutingError {
    #[error("Failed while executing redis command: {0}")]
    RedisError(&'static str),
    #[error("Failed to deserialize contract routing configs")]
    DeserializationFailed,
    #[error("Failed to serialize contract routing configs")]
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
    #[error("Contract was not found against the given parameters")]
    ContractNotFound,
}

impl IntoGrpcStatus for error_stack::Report<ContractRoutingError> {
    fn into_grpc_status(self) -> Status {
        logger::error!(error=?self);
        match self.current_context() {
            ContractRoutingError::RedisError(err) => Status::internal(err.to_string()),
            ContractRoutingError::DeserializationFailed => {
                Status::internal("Deserialization failure")
            }
            ContractRoutingError::SerializationFailed => Status::internal("Serialization failed"),
            ContractRoutingError::ConfigError(err) => Status::invalid_argument(err.to_string()),
            ContractRoutingError::TypeConversionError { .. }
            | ContractRoutingError::FailedToGetCurrentTime => {
                Status::internal("Internal server error")
            }
            ContractRoutingError::ContractNotFound => Status::not_found("Contract not found"),
        }
    }
}
