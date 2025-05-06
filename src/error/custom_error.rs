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
    #[error(" Rule Configuration not found")]
    ConfigurationNotFound,
    #[error(" Rule Configuration already exists")]
    ConfigurationAlreadyExists,
}

impl axum::response::IntoResponse for RuleConfigurationError {
    fn into_response(self) -> axum::response::Response {
        match self {
            RuleConfigurationError::StorageError => (
                hyper::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(crate::error::ApiErrorResponse::new(
                    crate::error::error_codes::TE_04,
                    "Storage error".to_string(),
                    None,
                )),
            )
                .into_response(),
            RuleConfigurationError::InvalidRuleConfiguration => (
                hyper::StatusCode::BAD_REQUEST,
                axum::Json(crate::error::ApiErrorResponse::new(
                    crate::error::error_codes::TE_04,
                    "Invalid routing rule configuration".to_string(),
                    None,
                )),
            )
                .into_response(),
            RuleConfigurationError::MerchantNotFound => (
                hyper::StatusCode::NOT_FOUND,
                axum::Json(crate::error::ApiErrorResponse::new(
                    crate::error::error_codes::TE_04,
                    "MerchantId not found".to_string(),
                    None,
                )),
            )
                .into_response(),
            RuleConfigurationError::ConfigurationNotFound => (
                hyper::StatusCode::NOT_FOUND,
                axum::Json(crate::error::ApiErrorResponse::new(
                    crate::error::error_codes::TE_04,
                    "Rule configuration not found".to_string(),
                    None,
                )),
            )
                .into_response(),
            RuleConfigurationError::ConfigurationAlreadyExists => (
                hyper::StatusCode::BAD_REQUEST,
                axum::Json(crate::error::ApiErrorResponse::new(
                    crate::error::error_codes::TE_04,
                    "Rule configuration already exists".to_string(),
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
    #[error(" Merchant account already exists")]
    MerchantAlreadyExists,
    #[error(" Merchant account deletion failed")]
    MerchantDeletionFailed,
    #[error(" Merchant account insertion failed")]
    MerchantInsertionFailed,
}

impl axum::response::IntoResponse for MerchantAccountConfigurationError {
    fn into_response(self) -> axum::response::Response {
        match self {
            MerchantAccountConfigurationError::StorageError => (
                hyper::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(crate::error::ApiErrorResponse::new(
                    crate::error::error_codes::TE_04,
                    "Storage error".to_string(),
                    None,
                )),
            )
                .into_response(),
            MerchantAccountConfigurationError::InvalidConfiguration => (
                hyper::StatusCode::BAD_REQUEST,
                axum::Json(crate::error::ApiErrorResponse::new(
                    crate::error::error_codes::TE_04,
                    "Invalid merchant account configuration".to_string(),
                    None,
                )),
            )
                .into_response(),
            MerchantAccountConfigurationError::MerchantNotFound => (
                hyper::StatusCode::NOT_FOUND,
                axum::Json(crate::error::ApiErrorResponse::new(
                    crate::error::error_codes::TE_04,
                    "MerchantId not found".to_string(),
                    None,
                )),
            )
                .into_response(),
            MerchantAccountConfigurationError::MerchantAlreadyExists => (
                hyper::StatusCode::BAD_REQUEST,
                axum::Json(crate::error::ApiErrorResponse::new(
                    crate::error::error_codes::TE_04,
                    "Merchant account already exists".to_string(),
                    None,
                )),
            )
                .into_response(),
            MerchantAccountConfigurationError::MerchantDeletionFailed => (
                hyper::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(crate::error::ApiErrorResponse::new(
                    crate::error::error_codes::TE_04,
                    "Merchant account deletion failed".to_string(),
                    None,
                )),
            )
                .into_response(),
            MerchantAccountConfigurationError::MerchantInsertionFailed => (
                hyper::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(crate::error::ApiErrorResponse::new(
                    crate::error::error_codes::TE_04,
                    "Merchant account insertion failed".to_string(),
                    None,
                )),
            )
                .into_response(),
        }
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
