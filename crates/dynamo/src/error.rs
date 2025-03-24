use redis_interface::errors::RedisError;
use tonic::Status;

use crate::logger;

pub trait IntoGrpcStatus {
    fn into_grpc_status(self) -> Status;
}

pub trait ResultExtGrpc<T> {
    fn into_grpc_status(self) -> Result<T, Status>;
}

impl<T, E> ResultExtGrpc<T> for error_stack::Result<T, E>
where
    error_stack::Report<E>: IntoGrpcStatus,
{
    fn into_grpc_status(self) -> Result<T, Status> {
        match self {
            Ok(x) => Ok(x),
            Err(err) => Err(err.into_grpc_status()),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigurationError {
    #[error("Invalid host for socket: {0}")]
    AddressError(#[from] std::net::AddrParseError),
    #[error("Failed while building grpc reflection service: {0}")]
    GrpcReflectionServiceError(#[from] tonic_reflection::server::Error),
    #[error("Error while creating metrics server")]
    MetricsServerError,
    #[error("Error while creating the server: {0}")]
    ServerError(#[from] tonic::transport::Error),
    #[error("Failed to create redis connection pool: {0:?}")]
    RedisConnectionError(error_stack::Report<RedisError>),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

#[derive(Debug, thiserror::Error)]
pub enum GrpcRequestError {
    #[error("Invalid header provided: {0}")]
    InvalidHeader(&'static str),
    #[error("Missing header: {0}")]
    MissingHeader(&'static str),
}

impl IntoGrpcStatus for error_stack::Report<GrpcRequestError> {
    fn into_grpc_status(self) -> Status {
        logger::error!(error=?self);
        match self.current_context() {
            GrpcRequestError::InvalidHeader(field) => {
                Status::invalid_argument(format!("Invalid header provided: {field}"))
            }
            GrpcRequestError::MissingHeader(field) => {
                Status::not_found(format!("Missing header: {field}"))
            }
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ParsingError {
    #[error("Failed to parse struct: {0}")]
    StructParseFailure(&'static str),
    #[error("Failed to serialize to {0} format")]
    EncodeError(&'static str),
}
