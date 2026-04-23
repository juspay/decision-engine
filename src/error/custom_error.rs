#[derive(Debug, thiserror::Error)]
pub enum MerchantDBError {
    #[error("Error while encrypting DEK before adding to DB")]
    DEKEncryptionError,
    #[error("Error while decrypting DEK from DB")]
    DEKDecryptionError,
    #[error("Error while connecting to database")]
    DBError,
    #[error("Error while finding merchant record in the database")]
    DBFilterError,
    #[error("Error while inserting merchant record in the database")]
    DBInsertError,
    #[error("Merchant record not found in database")]
    NotFoundError,
    #[error("Unpredictable error occurred")]
    UnknownError,
}

#[derive(Debug, thiserror::Error)]
pub enum VaultDBError {
    #[error("Error while encrypting data before adding to DB")]
    DataEncryptionError,
    #[error("Error while decrypting data from DB")]
    DataDecryptionError,
    #[error("Error while connecting to database")]
    DBError,
    #[error("Error while finding vault record in the database")]
    DBFilterError,
    #[error("Error while inserting vault record in the database")]
    DBInsertError,
    #[error("Error while deleting vault record in the database")]
    DBDeleteError,
    #[error("Vault record not found in database")]
    NotFoundError,
    #[error("Unpredictable error occurred")]
    UnknownError,
}

#[derive(Debug, thiserror::Error)]
pub enum HashDBError {
    #[error("Error while connecting to database")]
    DBError,
    #[error("Error while finding hash record in the database")]
    DBFilterError,
    #[error("Error while inserting hash record in the database")]
    DBInsertError,
    #[error("Unpredictable error occurred")]
    UnknownError,
}

#[derive(Debug, thiserror::Error)]
pub enum TestDBError {
    #[error("Error while connecting to database")]
    DBError,
    #[error("Error while writing to database")]
    DBWriteError,
    #[error("Error while reading element in the database")]
    DBReadError,
    #[error("Error while deleting element in the database")]
    DBDeleteError,
    #[error("Unpredictable error occurred")]
    UnknownError,
}

#[derive(Debug, thiserror::Error)]
pub enum FingerprintDBError {
    #[error("Error while connecting to database")]
    DBError,
    #[error("Error while finding fingerprint record in the database")]
    DBFilterError,
    #[error("Error while inserting fingerprint record in the database")]
    DBInsertError,
    #[error("Unpredictable error occurred")]
    UnknownError,
    #[error("Error while encoding data")]
    EncodingError,
}

#[derive(Debug, thiserror::Error)]
pub enum EntityDBError {
    #[error("Error while connecting to database")]
    DBError,
    #[error("Error while finding entity record in the database")]
    DBFilterError,
    #[error("Error while inserting entity record in the database")]
    DBInsertError,
    #[error("Unpredictable error occurred")]
    UnknownError,
    #[error("Entity record not found in database")]
    NotFoundError,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum RuleConfigurationError {
    #[error("Storage error")]
    StorageError,
    #[error("Invalid Rule Configuration error")]
    InvalidRuleConfiguration,
    #[error("Merchant not found")]
    MerchantNotFound,
    #[error("Rule Configuration not found")]
    ConfigurationNotFound,
    #[error(" Rule Configuration already exists")]
    ConfigurationAlreadyExists,
    #[error("Failed to deserialize configuration")]
    DeserializationError,
    #[error("Debit routing not enabled for merchant")]
    DebitRoutingNotEnabled,
}

impl axum::response::IntoResponse for RuleConfigurationError {
    fn into_response(self) -> axum::response::Response {
        match self {
            Self::StorageError => (
                hyper::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(crate::error::ApiErrorResponse::new(
                    crate::error::error_codes::TE_04,
                    "Storage error".to_string(),
                    None,
                )),
            )
                .into_response(),
            Self::DeserializationError => (
                hyper::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(crate::error::ApiErrorResponse::new(
                    crate::error::error_codes::TE_04,
                    "Failed to deserialize configuration".to_string(),
                    None,
                )),
            )
                .into_response(),
            Self::InvalidRuleConfiguration => (
                hyper::StatusCode::BAD_REQUEST,
                axum::Json(crate::error::ApiErrorResponse::new(
                    crate::error::error_codes::TE_04,
                    "Invalid routing rule configuration".to_string(),
                    None,
                )),
            )
                .into_response(),
            Self::MerchantNotFound => (
                hyper::StatusCode::NOT_FOUND,
                axum::Json(crate::error::ApiErrorResponse::new(
                    crate::error::error_codes::TE_04,
                    "MerchantId not found".to_string(),
                    None,
                )),
            )
                .into_response(),
            Self::ConfigurationNotFound => (
                hyper::StatusCode::NOT_FOUND,
                axum::Json(crate::error::ApiErrorResponse::new(
                    crate::error::error_codes::TE_04,
                    "Rule configuration not found".to_string(),
                    None,
                )),
            )
                .into_response(),
            Self::ConfigurationAlreadyExists => (
                hyper::StatusCode::BAD_REQUEST,
                axum::Json(crate::error::ApiErrorResponse::new(
                    crate::error::error_codes::TE_04,
                    "Rule configuration already exists".to_string(),
                    None,
                )),
            )
                .into_response(),
            Self::DebitRoutingNotEnabled => (
                hyper::StatusCode::FORBIDDEN,
                axum::Json(crate::error::ApiErrorResponse::new(
                    crate::error::error_codes::TE_05,
                    "Debit routing not enabled for merchant".to_string(),
                    None,
                )),
            )
                .into_response(),
        }
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum MerchantAccountConfigurationError {
    #[error("Storage error")]
    StorageError,
    #[error("Invalid Configuration error")]
    InvalidConfiguration,
    #[error("Merchant account not found")]
    MerchantNotFound,
    #[error("Merchant account already exists")]
    MerchantAlreadyExists,
    #[error("Merchant account deletion failed")]
    MerchantDeletionFailed,
    #[error("Merchant account insertion failed")]
    MerchantInsertionFailed,
    #[error("Unauthorized")]
    Unauthorized,
}

impl axum::response::IntoResponse for MerchantAccountConfigurationError {
    fn into_response(self) -> axum::response::Response {
        match self {
            Self::StorageError => (
                hyper::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(crate::error::ApiErrorResponse::new(
                    crate::error::error_codes::TE_04,
                    "Storage error".to_string(),
                    None,
                )),
            )
                .into_response(),
            Self::InvalidConfiguration => (
                hyper::StatusCode::BAD_REQUEST,
                axum::Json(crate::error::ApiErrorResponse::new(
                    crate::error::error_codes::TE_04,
                    "Invalid merchant account configuration".to_string(),
                    None,
                )),
            )
                .into_response(),
            Self::MerchantNotFound => (
                hyper::StatusCode::NOT_FOUND,
                axum::Json(crate::error::ApiErrorResponse::new(
                    crate::error::error_codes::TE_04,
                    "MerchantId not found".to_string(),
                    None,
                )),
            )
                .into_response(),
            Self::MerchantAlreadyExists => (
                hyper::StatusCode::BAD_REQUEST,
                axum::Json(crate::error::ApiErrorResponse::new(
                    crate::error::error_codes::TE_04,
                    "Merchant account already exists".to_string(),
                    None,
                )),
            )
                .into_response(),
            Self::MerchantDeletionFailed => (
                hyper::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(crate::error::ApiErrorResponse::new(
                    crate::error::error_codes::TE_04,
                    "Merchant account deletion failed".to_string(),
                    None,
                )),
            )
                .into_response(),
            Self::MerchantInsertionFailed => (
                hyper::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(crate::error::ApiErrorResponse::new(
                    crate::error::error_codes::TE_04,
                    "Merchant account insertion failed".to_string(),
                    None,
                )),
            )
                .into_response(),
            Self::Unauthorized => (
                hyper::StatusCode::UNAUTHORIZED,
                axum::Json(crate::error::ApiErrorResponse::new(
                    crate::error::error_codes::TE_04,
                    "Invalid or missing x-admin-secret header".to_string(),
                    None,
                )),
            )
                .into_response(),
        }
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum ApiKeyError {
    #[error("API key not found")]
    NotFound,
    #[error("API key creation failed")]
    CreationFailed,
    #[error("API key revocation failed")]
    RevocationFailed,
    #[error("Merchant not found")]
    MerchantNotFound,
    #[error("Storage error")]
    StorageError,
}

impl axum::response::IntoResponse for ApiKeyError {
    fn into_response(self) -> axum::response::Response {
        match self {
            Self::NotFound => (
                hyper::StatusCode::NOT_FOUND,
                axum::Json(crate::error::ApiErrorResponse::new(
                    crate::error::error_codes::TE_04,
                    "API key not found".to_string(),
                    None,
                )),
            )
                .into_response(),
            Self::MerchantNotFound => (
                hyper::StatusCode::NOT_FOUND,
                axum::Json(crate::error::ApiErrorResponse::new(
                    crate::error::error_codes::TE_04,
                    "Merchant not found".to_string(),
                    None,
                )),
            )
                .into_response(),
            Self::CreationFailed | Self::RevocationFailed | Self::StorageError => (
                hyper::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(crate::error::ApiErrorResponse::new(
                    crate::error::error_codes::TE_04,
                    self.to_string(),
                    None,
                )),
            )
                .into_response(),
        }
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum UserAuthError {
    #[error("Email already registered")]
    EmailAlreadyExists,
    #[error("User not found")]
    UserNotFound,
    #[error("Invalid password")]
    InvalidPassword,
    #[error("Account is inactive")]
    AccountInactive,
    #[error("Email not verified")]
    EmailNotVerified,
    #[error("Invalid or expired token")]
    InvalidToken,
    #[error("Storage error")]
    StorageError,
    #[error("Token generation failed")]
    TokenGenerationFailed,
    #[error("Password hashing failed")]
    PasswordHashingFailed,
    #[error("Merchant not found")]
    MerchantNotFound,
}

impl axum::response::IntoResponse for UserAuthError {
    fn into_response(self) -> axum::response::Response {
        let (status, message) = match &self {
            Self::EmailAlreadyExists => (hyper::StatusCode::CONFLICT, self.to_string()),
            Self::UserNotFound | Self::InvalidPassword => (
                hyper::StatusCode::UNAUTHORIZED,
                "Invalid email or password".to_string(),
            ),
            Self::AccountInactive => (hyper::StatusCode::FORBIDDEN, self.to_string()),
            Self::EmailNotVerified => (hyper::StatusCode::FORBIDDEN, self.to_string()),
            Self::InvalidToken => (hyper::StatusCode::UNAUTHORIZED, self.to_string()),
            Self::MerchantNotFound => (hyper::StatusCode::NOT_FOUND, self.to_string()),
            Self::StorageError | Self::TokenGenerationFailed | Self::PasswordHashingFailed => {
                (hyper::StatusCode::INTERNAL_SERVER_ERROR, self.to_string())
            }
        };
        (
            status,
            axum::Json(crate::error::ApiErrorResponse::new(
                crate::error::error_codes::TE_04,
                message,
                None,
            )),
        )
            .into_response()
    }
}

pub trait NotFoundError {
    fn is_not_found(&self) -> bool;
}

impl NotFoundError for super::ContainerError<MerchantDBError> {
    fn is_not_found(&self) -> bool {
        matches!(self.error.current_context(), MerchantDBError::NotFoundError)
    }
}

impl NotFoundError for super::ContainerError<EntityDBError> {
    fn is_not_found(&self) -> bool {
        matches!(self.error.current_context(), EntityDBError::NotFoundError)
    }
}
