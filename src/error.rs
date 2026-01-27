use std::string::FromUtf8Error;

#[macro_use]
pub mod container;

mod custom_error;
mod transforms;

pub use container::*;
pub use custom_error::*;
use reqwest::StatusCode;

#[derive(Debug, thiserror::Error)]
pub enum ConfigurationError {
    #[error("error while creating the webserver")]
    ServerError(#[from] hyper::Error),
    #[error("invalid host for socket")]
    AddressError(#[from] std::net::AddrParseError),
    #[error("invalid host for socket")]
    IOError(#[from] std::io::Error),
    #[error("Error while connecting/creating database pool")]
    DatabaseError,
    #[error("Failed to KMS decrypt: {0}")]
    KmsDecryptError(&'static str),
    #[error("Failed while building Vault Client")]
    VaultClientError,
    #[error("Invalid configuration value provided: {0}")]
    InvalidConfigurationValueError(String),
}

#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("failed while serializing with serde_json")]
    SerdeJsonError(#[from] serde_json::Error),
    #[error("error while performing jwe operation")]
    JWError(#[from] josekit::JoseError),
    #[error("invalid data received: {0}")]
    InvalidData(&'static str),
    #[error("error while parsing utf-8")]
    EncodingError(#[from] FromUtf8Error),
    #[error("Failed while encrypting payload")]
    EncryptionError,
    #[error("Failed while decrypting payload")]
    DecryptionError,
    #[error("Not implemented")]
    NotImplemented,
}

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("failed to inittialize database pool")]
    InitializationError,
    #[error("failed to construct database pool")]
    DBPoolError,
    #[error("failed to construct database pool")]
    PoolClientFailure,
    #[error("Error while finding element in database")]
    FindError,
    #[error("Error while inserting data in database")]
    InsertError,
    #[error("Error while deleting data in database")]
    DeleteError,
    #[error("Error while decrypting the payload")]
    DecryptionError,
    #[error("Error while encrypting the payload")]
    EncryptionError,
    #[error("Element not found in storage")]
    NotFoundError,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum ApiError {
    #[error("failed while retrieving stored data")]
    RetrieveDataFailed(&'static str),

    #[error("failed to decrypt two custodian keys: {0}")]
    DecryptingKeysFailed(&'static str),

    #[error("failed in request middleware: {0}")]
    RequestMiddlewareError(&'static str),

    #[error("failed in response middleware: {0}")]
    ResponseMiddlewareError(&'static str),

    #[error("Missing required param: {0}")]
    MissingRequiredField(&'static str),

    #[error("Error while encoding data")]
    EncodingError,

    #[error("Failed while decoding data")]
    DecodingError,

    #[error("Failed while inserting data into {0}")]
    DatabaseInsertFailed(&'static str),

    #[error("failed while deleting data from {0}")]
    DatabaseDeleteFailed(&'static str),

    #[error("Failed while getting merchant from DB")]
    MerchantError,

    #[error("Something went wrong")]
    UnknownError,

    #[error("Error while encrypting with merchant key")]
    MerchantKeyError,

    #[error("Failed while connecting to database")]
    DatabaseError,

    #[error("Failed while validation: {0}")]
    ValidationError(&'static str),

    #[error("Requested resource not found")]
    NotFoundError,

    #[error("TTL is invalid")]
    InvalidTtl,

    #[error("Custodian is locked")]
    CustodianLocked,

    #[error("Custodian is already unlocked")]
    CustodianUnlocked,

    #[error("Tenant error: {0}")]
    TenantError(&'static str),

    #[error("Key manager error: {0}")]
    KeyManagerError(&'static str),

    #[error("Parsing error: {0}")]
    ParsingError(&'static str),
}

/// Errors that could occur during KMS operations.
#[derive(Debug, thiserror::Error)]
pub enum KmsError {
    /// An error occurred when base64 decoding input data.
    #[error("Failed to base64 decode input data")]
    Base64DecodingFailed,

    /// An error occurred when hex decoding input data.
    #[error("Failed to hex decode input data")]
    HexDecodingFailed,

    /// An error occurred when KMS decrypting input data.
    #[error("Failed to KMS decrypt input data")]
    DecryptionFailed,

    /// The KMS decrypted output does not include a plaintext output.
    #[error("Missing plaintext KMS decryption output")]
    MissingPlaintextDecryptionOutput,

    /// An error occurred UTF-8 decoding KMS decrypted output.
    #[error("Failed to UTF-8 decode decryption output")]
    Utf8DecodingFailed,

    /// The KMS client has not been initialized.
    #[error("The KMS client has not been initialized")]
    KmsClientNotInitialized,

    #[error("This KMS flow is not implemented")]
    KmsNotImplemented,

    #[error("Provided information about the value is incomplete")]
    IncompleteData,

    #[error("Failed while fetching data from the server")]
    FetchFailed,

    #[error("Failed while parsing the response")]
    ParseError,
}

#[derive(Debug, thiserror::Error)]
pub enum ApiClientError {
    #[error("Failed to construct api client")]
    ClientConstructionFailed,
    #[error("Failed to construct Header map")]
    HeaderMapConstructionFailed,
    #[error("Failed to parse Identity")]
    IdentityParseFailed,
    #[error("Failed to parse Certificate of {service}")]
    CertificateParseFailed { service: &'static str },
    #[error("Failed to encode request URL")]
    UrlEncodingFailed,
    #[error("Failed to send api request")]
    RequestNotSent,
    #[error("Failed to decode response")]
    ResponseDecodingFailed,
    #[error("Received bad request: {0:?}")]
    BadRequest(bytes::Bytes),
    #[error("Tenant authentication failed: {0:?}")]
    Unauthorized(bytes::Bytes),
    #[error("Unexpected error occurred: status_code-{status_code:?} message-{message:?}")]
    Unexpected {
        status_code: StatusCode,
        message: bytes::Bytes,
    },
    #[error("Received internal server error {0:?}")]
    InternalServerError(bytes::Bytes),
}

#[derive(Debug, thiserror::Error)]
pub enum KeyManagerError {
    #[error("Failed to add key to the Key manager")]
    KeyAddFailed,
    #[error("Failed to transfer the key to the Key manager")]
    KeyTransferFailed,
    #[error("Failed to encrypt the data in the Key manager")]
    EncryptionFailed,
    #[error("Failed to decrypt the data in the Key manager")]
    DecryptionFailed,
    #[error("Failed while performing db operation on entity")]
    DbError,
    #[error("Response decoding failed")]
    ResponseDecodingFailed,
    #[error("Authentication failed")]
    Unauthorized,
}

#[derive(Debug, thiserror::Error)]
pub enum DataKeyCreationError {
    #[error("Failed to send the request to Key manager")]
    RequestSendFailed,
    #[error("Response decoding failed")]
    ResponseDecodingFailed,
    #[error("Received internal server error")]
    InternalServerError,
    #[error("Unexpected error occurred while calling the Key manager")]
    Unexpected,
    #[error("Received bad request")]
    BadRequest,
    #[error("Failed while parsing certificates")]
    CertificateParseFailed,
    #[error("Failed while constructing client request")]
    RequestConstructionFailed,
    #[error("Authentication failed")]
    Unauthorized,
}

#[derive(Debug, thiserror::Error)]
pub enum DataKeyTransferError {
    #[error("Failed to send the request to Key manager")]
    RequestSendFailed,
    #[error("Response decoding failed")]
    ResponseDecodingFailed,
    #[error("Received internal server error")]
    InternalServerError,
    #[error("Unexpected error occurred while calling the Key manager")]
    Unexpected,
    #[error("Bad request received")]
    BadRequest,
    #[error("Failed while parsing certificates")]
    CertificateParseFailed,
    #[error("Failed while constructing client request")]
    RequestConstructionFailed,
    #[error("Authentication failed")]
    Unauthorized,
}

#[derive(Debug, thiserror::Error)]
pub enum DataEncryptionError {
    #[error("Failed to send the request to Key manager")]
    RequestSendFailed,
    #[error("Response decoding failed")]
    ResponseDecodingFailed,
    #[error("Received internal server error")]
    InternalServerError,
    #[error("Unexpected error occurred while calling the Key manager")]
    Unexpected,
    #[error("Received bad request")]
    BadRequest,
    #[error("Failed while parsing certificates")]
    CertificateParseFailed,
    #[error("Failed while constructing client request")]
    RequestConstructionFailed,
    #[error("Authentication failed")]
    Unauthorized,
}

#[derive(Debug, thiserror::Error)]
pub enum DataDecryptionError {
    #[error("Failed to send the request to Key manager")]
    RequestSendFailed,
    #[error("Response decoding failed")]
    ResponseDecodingFailed,
    #[error("Received internal server error")]
    InternalServerError,
    #[error("Unexpected error occurred while calling the Key manager")]
    Unexpected,
    #[error("Received bad request")]
    BadRequest,
    #[error("Failed while parsing certificates")]
    CertificateParseFailed,
    #[error("Failed while constructing client request")]
    RequestConstructionFailed,
    #[error("Authentication failed")]
    Unauthorized,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum KeyManagerHealthCheckError {
    #[error("Failed to establish Key manager connection")]
    FailedToConnect,
}

/// Error code constants.
pub mod error_codes {
    /// Processing error: Indicates an error that occurred during processing of a task or operation.
    pub const TE_00: &str = "TE_00";

    /// Database error: Denotes an error related to database operations or connectivity.
    pub const TE_01: &str = "TE_01";

    /// Resource not found: Signifies that the requested resource could not be located.
    pub const TE_02: &str = "TE_02";

    /// Validation error: Represents an error occurring during data validation or integrity checks.
    pub const TE_03: &str = "TE_03";

    /// Validation error: Represents an error occurring in euclid codebase.
    pub const TE_04: &str = "TE_04";
}

impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        match self {
            Self::CustodianLocked => (
                hyper::StatusCode::UNAUTHORIZED,
                axum::Json(ApiErrorResponse::new(
                    error_codes::TE_00,
                    "Custodian is locked".into(),
                    None,
                )),
            )
                .into_response(),

            Self::MissingRequiredField(field_name) => (
                hyper::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(ApiErrorResponse::new(
                    error_codes::TE_00,
                    format!("Missing required param: {field_name}"),
                    None,
                )),
            )
                .into_response(),

            Self::DecryptingKeysFailed(err) => (
                hyper::StatusCode::UNAUTHORIZED,
                axum::Json(ApiErrorResponse::new(
                    error_codes::TE_00,
                    format!("Failed while decrypting two custodian keys: {err}"),
                    None,
                )),
            )
                .into_response(),
            data @ Self::EncodingError => (
                hyper::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(ApiErrorResponse::new(
                    error_codes::TE_00,
                    format!("{}", data),
                    None,
                )),
            )
                .into_response(),
            data @ Self::ResponseMiddlewareError(_)
            | data @ Self::UnknownError
            | data @ Self::MerchantKeyError
            | data @ Self::KeyManagerError(_) => (
                hyper::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(ApiErrorResponse::new(
                    error_codes::TE_00,
                    format!("{}", data),
                    None,
                )),
            )
                .into_response(),

            data @ Self::DatabaseInsertFailed(_)
            | data @ Self::DatabaseError
            | data @ Self::DatabaseDeleteFailed(_)
            | data @ Self::RetrieveDataFailed(_) => (
                hyper::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(ApiErrorResponse::new(
                    error_codes::TE_01,
                    format!("{}", data),
                    None,
                )),
            )
                .into_response(),
            data @ Self::RequestMiddlewareError(_)
            | data @ Self::DecodingError
            | data @ Self::ValidationError(_)
            | data @ Self::InvalidTtl
            | data @ Self::CustodianUnlocked
            | data @ Self::TenantError(_) => (
                hyper::StatusCode::BAD_REQUEST,
                axum::Json(ApiErrorResponse::new(
                    error_codes::TE_03,
                    format!("{}", data),
                    None,
                )),
            )
                .into_response(),
            data @ Self::NotFoundError => (
                hyper::StatusCode::NOT_FOUND,
                axum::Json(ApiErrorResponse::new(
                    error_codes::TE_02,
                    format!("{}", data),
                    None,
                )),
            )
                .into_response(),
            data @ Self::MerchantError => (
                hyper::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(ApiErrorResponse::new(
                    error_codes::TE_02,
                    format!("{}", data),
                    None,
                )),
            )
                .into_response(),
            data @ Self::ParsingError(..) => (
                hyper::StatusCode::BAD_REQUEST,
                axum::Json(ApiErrorResponse::new(
                    error_codes::TE_03,
                    format!("{}", data),
                    None,
                )),
            )
                .into_response(),
        }
    }
}

impl<T: axum::response::IntoResponse + error_stack::Context + Clone> axum::response::IntoResponse
    for ContainerError<T>
{
    fn into_response(self) -> axum::response::Response {
        crate::logger::error!(error=?self.error);
        self.error.current_context().clone().into_response()
    }
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ApiErrorResponse {
    code: &'static str,
    message: String,
    data: Option<serde_json::Value>,
}

impl ApiErrorResponse {
    pub fn new(code: &'static str, message: String, data: Option<serde_json::Value>) -> Self {
        Self {
            code,
            message,
            data,
        }
    }
}

// pub trait LogReport<T, E> {
//     fn report_unwrap(self) -> Result<T, E>;
// }

// impl<T, E1, E2> LogReport<T, E1> for Result<T, Report<E2>>
// where
//     E1: Send + Sync + std::error::Error + Copy + 'static,
//     E2: Send + Sync + std::error::Error + Copy + 'static,
//     E1: From<E2>,
// {
//     #[track_caller]
//     fn report_unwrap(self) -> Result<T, E1> {
//         let output = match self {
//             Ok(inner_val) => Ok(inner_val),
//             Err(inner_err) => {
//                 let new_error: E1 = (*inner_err.current_context()).into();
//                 crate::logger::error!(?inner_err);
//                 Err(inner_err.change_context(new_error))
//             }
//         };

//         output.map_err(|err| (*err.current_context()))
//     }
// }
