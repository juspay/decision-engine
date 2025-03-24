use dynamo::logger;
use redis_interface::errors::RedisError;

#[derive(Debug, thiserror::Error)]
pub enum ConfigurationError {
    #[error("Invalid host for socket: {0}")]
    AddressError(#[from] std::net::AddrParseError),
    #[error("Error while creating the server: {msg}")]
    ServerError { msg: String },
    #[error("Failed to create redis connection pool: {0:?}")]
    RedisConnectionError(error_stack::Report<RedisError>),
}

#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum ApiError {
    #[error("CSV file not sent")]
    CsvFileNotSent,

    #[error("Failed to deserialize CSV record")]
    FailedToDeserializeCsv,

    #[error("Failed to parse multipart stream")]
    FailedToParseMultiPartStream,

    #[error("Failed to convert field in multipart stream to bytes")]
    FailedToConvertToBytes,

    #[error("Failed to parse JSON input")]
    FailedToParseJsonInput,

    #[error("Failed to fetch success rate from dynamo")]
    FailedToFetchSr,

    #[error("Failed to update window in dynamo")]
    FailedToUpdateWindow,

    #[error("Failed to serialize to pretty-printed String of JSON")]
    FailedToSerializeToJson,

    #[error("Failed to upload simulation summary")]
    FailedToUploadSimulationSummary,

    #[error("Failed to upload simulation report")]
    FailedToUploadSimulationReport,

    #[error("Failed to parse headers: {0}")]
    HeadersError(&'static str),

    #[error("Failed to invalidate report chunks in: {0}")]
    ChunkInvalidationError(&'static str),

    #[error("Failed to retrieve baseline data file")]
    FailedToRetrieveBaselineData,

    #[error("Multipart error: {0}")]
    MultipartError(&'static str),

    #[error("Unexpected error: {0}")]
    Unexpected(&'static str),

    #[error("Storage not constructed")]
    StorageNotFound,

    #[error("Failed to retrieve simulation summary")]
    FailedToRetrieveSimulationSummary,

    #[error("Failed to deserialize simulation summary")]
    FailedToDeserializeSimulationSummary,

    #[error("Failed to retrieve simulation report")]
    FailedToRetrieveSimulationReport,

    #[error("The request source is un-authenticated")]
    UnAuthenticated,

    #[error("Incorrect request received : {0}")]
    InvalidRequest(&'static str),
}

impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        match self {
            ApiError::CsvFileNotSent => (
                hyper::StatusCode::BAD_REQUEST,
                axum::Json(ApiErrorResponse::new(
                    "No CSV file found in request".to_string(),
                )),
            )
                .into_response(),

            ApiError::FailedToParseJsonInput => (
                hyper::StatusCode::BAD_REQUEST,
                axum::Json(ApiErrorResponse::new(
                    "Failed to parse JSON input".to_string(),
                )),
            )
                .into_response(),

            ApiError::InvalidRequest(msg) => (
                hyper::StatusCode::BAD_REQUEST,
                axum::Json(ApiErrorResponse::new(format!(
                    "Invalid request received : {}",
                    msg
                ))),
            )
                .into_response(),

            ApiError::FailedToDeserializeCsv => (
                hyper::StatusCode::BAD_REQUEST,
                axum::Json(ApiErrorResponse::new(
                    "Failed to parse CSV data".to_string(),
                )),
            )
                .into_response(),

            ApiError::HeadersError(msg) => (
                hyper::StatusCode::BAD_REQUEST,
                axum::Json(ApiErrorResponse::new(format!(
                    "Failed to parse headers: {}",
                    msg
                ))),
            )
                .into_response(),

            ApiError::FailedToParseMultiPartStream => (
                hyper::StatusCode::BAD_REQUEST,
                axum::Json(ApiErrorResponse::new(
                    "Failed to parse multipart form data".to_string(),
                )),
            )
                .into_response(),

            ApiError::FailedToConvertToBytes => (
                hyper::StatusCode::BAD_REQUEST,
                axum::Json(ApiErrorResponse::new(
                    "Failed to process uploaded file".to_string(),
                )),
            )
                .into_response(),

            ApiError::FailedToSerializeToJson => (
                hyper::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(ApiErrorResponse::new(
                    "Failed to generate JSON output".to_string(),
                )),
            )
                .into_response(),

            ApiError::FailedToUploadSimulationSummary => (
                hyper::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(ApiErrorResponse::new(
                    "Failed to save simulation summary".to_string(),
                )),
            )
                .into_response(),

            ApiError::FailedToRetrieveSimulationSummary => (
                hyper::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(ApiErrorResponse::new(
                    "Failed to fetch simulation summary".to_string(),
                )),
            )
                .into_response(),

            ApiError::FailedToUploadSimulationReport => (
                hyper::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(ApiErrorResponse::new(
                    "Failed to save detailed simulation report".to_string(),
                )),
            )
                .into_response(),

            ApiError::FailedToRetrieveSimulationReport => (
                hyper::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(ApiErrorResponse::new(
                    "Failed to fetch detailed simulation report".to_string(),
                )),
            )
                .into_response(),

            ApiError::Unexpected(msg) => (
                hyper::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(ApiErrorResponse::new(format!(
                    "An unexpected error occurred: {}",
                    msg
                ))),
            )
                .into_response(),

            ApiError::FailedToDeserializeSimulationSummary => (
                hyper::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(ApiErrorResponse::new(
                    "Failed to deserialize simulation summary".to_string(),
                )),
            )
                .into_response(),

            ApiError::UnAuthenticated => (
                hyper::StatusCode::UNAUTHORIZED,
                axum::Json(ApiErrorResponse::new("Unauthenticated caller".to_string())),
            )
                .into_response(),

            ApiError::FailedToFetchSr
            | ApiError::FailedToUpdateWindow
            | ApiError::ChunkInvalidationError(_)
            | ApiError::FailedToRetrieveBaselineData
            | ApiError::MultipartError(_)
            | ApiError::StorageNotFound => (
                hyper::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(ApiErrorResponse::new("Something went wrong".to_string())),
            )
                .into_response(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ApiErrorResponse {
    message: String,
}

impl ApiErrorResponse {
    pub fn new(message: String) -> Self {
        Self { message }
    }
}

#[derive(Debug)]
pub struct ContainerError<E> {
    pub(crate) error: error_stack::Report<E>,
}

impl<T: axum::response::IntoResponse + error_stack::Context + Copy> axum::response::IntoResponse
    for ContainerError<T>
{
    fn into_response(self) -> axum::response::Response {
        logger::error!(error=?self.error);
        (*self.error.current_context()).into_response()
    }
}

impl<T> From<error_stack::Report<T>> for ContainerError<T> {
    fn from(error: error_stack::Report<T>) -> Self {
        Self { error }
    }
}

impl<T> From<T> for ContainerError<T>
where
    error_stack::Report<T>: From<T>,
{
    fn from(value: T) -> Self {
        Self {
            error: error_stack::Report::from(value),
        }
    }
}
