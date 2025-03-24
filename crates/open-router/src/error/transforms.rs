error_transform!(super::CryptoError => super::MerchantDBError);
impl<'a> From<&'a super::CryptoError> for super::MerchantDBError {
    fn from(value: &'a super::CryptoError) -> Self {
        match value {
            super::CryptoError::SerdeJsonError(_)
            | super::CryptoError::JWError(_)
            | super::CryptoError::InvalidData(_)
            | super::CryptoError::EncodingError(_)
            | super::CryptoError::NotImplemented => Self::UnknownError,
            super::CryptoError::EncryptionError => Self::DEKEncryptionError,
            super::CryptoError::DecryptionError => Self::DEKDecryptionError,
        }
    }
}

error_transform!(super::CryptoError => super::VaultDBError);
impl<'a> From<&'a super::CryptoError> for super::VaultDBError {
    fn from(value: &'a super::CryptoError) -> Self {
        match value {
            super::CryptoError::SerdeJsonError(_)
            | super::CryptoError::JWError(_)
            | super::CryptoError::InvalidData(_)
            | super::CryptoError::EncodingError(_)
            | super::CryptoError::NotImplemented => Self::UnknownError,
            super::CryptoError::EncryptionError => Self::DataEncryptionError,
            super::CryptoError::DecryptionError => Self::DataDecryptionError,
        }
    }
}

error_transform!(super::StorageError => super::MerchantDBError);
impl<'a> From<&'a super::StorageError> for super::MerchantDBError {
    fn from(value: &'a super::StorageError) -> Self {
        match value {
            super::StorageError::DBPoolError | super::StorageError::PoolClientFailure => {
                Self::DBError
            }
            super::StorageError::FindError => Self::DBFilterError,
            super::StorageError::DecryptionError
            | super::StorageError::EncryptionError
            | super::StorageError::DeleteError => Self::UnknownError,
            super::StorageError::InsertError => Self::DBInsertError,
            super::StorageError::NotFoundError => Self::NotFoundError,
            super::StorageError::InitializationError => Self::NotFoundError
        }
    }
}

error_transform!(super::StorageError => super::VaultDBError);
impl<'a> From<&'a super::StorageError> for super::VaultDBError {
    fn from(value: &'a super::StorageError) -> Self {
        match value {
            super::StorageError::DBPoolError | super::StorageError::PoolClientFailure => {
                Self::DBError
            }
            super::StorageError::FindError => Self::DBFilterError,
            super::StorageError::DecryptionError | super::StorageError::EncryptionError => {
                Self::UnknownError
            }
            super::StorageError::InsertError => Self::DBInsertError,
            super::StorageError::DeleteError => Self::DBDeleteError,
            super::StorageError::NotFoundError => Self::NotFoundError,
            super::StorageError::InitializationError => Self::NotFoundError,
            
        }
    }
}

error_transform!(super::StorageError => super::HashDBError);
impl<'a> From<&'a super::StorageError> for super::HashDBError {
    fn from(value: &'a super::StorageError) -> Self {
        match value {
            super::StorageError::DBPoolError | super::StorageError::PoolClientFailure => {
                Self::DBError
            }
            super::StorageError::FindError => Self::DBFilterError,
            super::StorageError::DecryptionError
            | super::StorageError::EncryptionError
            | super::StorageError::DeleteError => Self::UnknownError,
            super::StorageError::InsertError => Self::DBInsertError,
            super::StorageError::NotFoundError => Self::DBFilterError,
            super::StorageError::InitializationError => Self::DBFilterError,
        }
    }
}

error_transform!(super::StorageError => super::TestDBError);
impl<'a> From<&'a super::StorageError> for super::TestDBError {
    fn from(value: &'a super::StorageError) -> Self {
        match value {
            super::StorageError::DBPoolError | super::StorageError::PoolClientFailure => {
                Self::DBError
            }
            super::StorageError::FindError => Self::DBReadError,
            super::StorageError::InsertError => Self::DBWriteError,
            super::StorageError::DeleteError => Self::DBDeleteError,
            super::StorageError::DecryptionError
            | super::StorageError::EncryptionError
            | super::StorageError::NotFoundError => Self::UnknownError,
            | super::StorageError::InitializationError => Self::UnknownError,
        }
    }
}

error_transform!(super::StorageError => super::FingerprintDBError);
impl<'a> From<&'a super::StorageError> for super::FingerprintDBError {
    fn from(value: &'a super::StorageError) -> Self {
        match value {
            super::StorageError::DBPoolError | super::StorageError::PoolClientFailure => {
                Self::DBError
            }
            super::StorageError::FindError | super::StorageError::NotFoundError => {
                Self::DBFilterError
            }
            super::StorageError::DecryptionError
            | super::StorageError::EncryptionError
            | super::StorageError::DeleteError => Self::UnknownError,
            super::StorageError::InsertError => Self::DBInsertError,
            super::StorageError::InitializationError => Self::DBInsertError,
        }
    }
}

error_transform!(super::CryptoError => super::FingerprintDBError);
impl<'a> From<&'a super::CryptoError> for super::FingerprintDBError {
    fn from(value: &'a super::CryptoError) -> Self {
        match value {
            super::CryptoError::SerdeJsonError(_)
            | super::CryptoError::JWError(_)
            | super::CryptoError::InvalidData(_)
            | super::CryptoError::NotImplemented
            | super::CryptoError::EncryptionError
            | super::CryptoError::DecryptionError => Self::UnknownError,
            super::CryptoError::EncodingError(_) => Self::EncodingError,
        }
    }
}

error_transform!(super::FingerprintDBError => super::ApiError);
impl<'a> From<&'a super::FingerprintDBError> for super::ApiError {
    fn from(value: &'a super::FingerprintDBError) -> Self {
        match value {
            super::FingerprintDBError::EncodingError => Self::EncodingError,
            super::FingerprintDBError::DBError => Self::DatabaseError,
            super::FingerprintDBError::DBFilterError => Self::RetrieveDataFailed("fingerprint"),
            super::FingerprintDBError::DBInsertError => Self::DatabaseInsertFailed("fingerprint"),
            super::FingerprintDBError::UnknownError => Self::UnknownError,
        }
    }
}

error_transform!(super::CryptoError => super::HashDBError);
impl<'a> From<&'a super::CryptoError> for super::HashDBError {
    fn from(value: &'a super::CryptoError) -> Self {
        match value {
            super::CryptoError::SerdeJsonError(_)
            | super::CryptoError::JWError(_)
            | super::CryptoError::InvalidData(_)
            | super::CryptoError::EncodingError(_)
            | super::CryptoError::NotImplemented
            | super::CryptoError::EncryptionError
            | super::CryptoError::DecryptionError => Self::UnknownError,
        }
    }
}

// -- API Error --

error_transform!(super::MerchantDBError => super::ApiError);
impl<'a> From<&'a super::MerchantDBError> for super::ApiError {
    fn from(value: &'a super::MerchantDBError) -> Self {
        match value {
            super::MerchantDBError::DEKEncryptionError |
            super::MerchantDBError::DEKDecryptionError | // This failure can also
                                                         // occur because of master key failure
            super::MerchantDBError::DBError |
            super::MerchantDBError::DBFilterError |
            super::MerchantDBError::DBInsertError=> Self::MerchantError,
            super::MerchantDBError::NotFoundError=> Self::NotFoundError,
            super::MerchantDBError::UnknownError => Self::UnknownError
        }
    }
}

error_transform!(super::VaultDBError => super::ApiError);
impl<'a> From<&'a super::VaultDBError> for super::ApiError {
    fn from(value: &'a super::VaultDBError) -> Self {
        match value {
            super::VaultDBError::DataEncryptionError | super::VaultDBError::DataDecryptionError => {
                Self::MerchantKeyError
            }
            super::VaultDBError::DBError => Self::DatabaseError,
            super::VaultDBError::DBFilterError => Self::RetrieveDataFailed("open_router"),
            super::VaultDBError::DBInsertError => Self::DatabaseInsertFailed("open_router"),
            super::VaultDBError::DBDeleteError => Self::DatabaseDeleteFailed("open_router"),
            super::VaultDBError::UnknownError => Self::UnknownError,
            super::VaultDBError::NotFoundError => Self::NotFoundError,
        }
    }
}

error_transform!(super::HashDBError => super::ApiError);
impl<'a> From<&'a super::HashDBError> for super::ApiError {
    fn from(value: &'a super::HashDBError) -> Self {
        match value {
            super::HashDBError::DBError => Self::DatabaseError,
            super::HashDBError::DBFilterError => Self::RetrieveDataFailed("hash table"),
            super::HashDBError::DBInsertError => Self::DatabaseInsertFailed("hash table"),
            super::HashDBError::UnknownError => Self::UnknownError,
        }
    }
}

error_transform!(super::TestDBError => super::ApiError);
impl<'a> From<&'a super::TestDBError> for super::ApiError {
    fn from(value: &'a super::TestDBError) -> Self {
        match value {
            super::TestDBError::DBError => Self::DatabaseError,
            super::TestDBError::UnknownError => Self::UnknownError,
            super::TestDBError::DBWriteError => Self::DatabaseInsertFailed("TestFailed"),
            super::TestDBError::DBReadError => Self::RetrieveDataFailed("Test Failed"),
            super::TestDBError::DBDeleteError => Self::DatabaseDeleteFailed("Test Failed"),
        }
    }
}

error_transform!(super::CryptoError => super::ApiError);
impl<'a> From<&'a super::CryptoError> for super::ApiError {
    fn from(value: &'a super::CryptoError) -> Self {
        match value {
            super::CryptoError::SerdeJsonError(_) => Self::DecodingError,
            super::CryptoError::DecryptionError | super::CryptoError::JWError(_) => {
                Self::RequestMiddlewareError("Failed while encrypting/decrypting")
            }
            super::CryptoError::InvalidData(_) => Self::DecodingError,
            super::CryptoError::EncodingError(_) => Self::EncodingError,
            super::CryptoError::EncryptionError => {
                Self::ResponseMiddlewareError("Failed while encrypting response")
            }
            super::CryptoError::NotImplemented => Self::UnknownError,
        }
    }
}

error_transform!(super::StorageError => super::EntityDBError);
impl<'a> From<&'a super::StorageError> for super::EntityDBError {
    fn from(value: &'a super::StorageError) -> Self {
        match value {
            super::StorageError::DBPoolError | super::StorageError::PoolClientFailure => {
                Self::DBError
            }
            super::StorageError::FindError => Self::DBFilterError,
            super::StorageError::NotFoundError => Self::NotFoundError,
            super::StorageError::InitializationError => Self::NotFoundError,
            
            super::StorageError::DecryptionError
            | super::StorageError::EncryptionError
            | super::StorageError::DeleteError => Self::UnknownError,
            super::StorageError::InsertError => Self::DBInsertError,
        }
    }
}

error_transform!(super::CryptoError => super::EntityDBError);
impl<'a> From<&'a super::CryptoError> for super::EntityDBError {
    fn from(value: &'a super::CryptoError) -> Self {
        match value {
            super::CryptoError::SerdeJsonError(_)
            | super::CryptoError::JWError(_)
            | super::CryptoError::InvalidData(_)
            | super::CryptoError::NotImplemented
            | super::CryptoError::EncryptionError
            | super::CryptoError::DecryptionError
            | super::CryptoError::EncodingError(_) => Self::UnknownError,
        }
    }
}

error_transform!(super::EntityDBError => super::ApiError);
impl<'a> From<&'a super::EntityDBError> for super::ApiError {
    fn from(value: &'a super::EntityDBError) -> Self {
        match value {
            super::EntityDBError::DBError => Self::DatabaseError,
            super::EntityDBError::DBFilterError => Self::RetrieveDataFailed("entity"),
            super::EntityDBError::DBInsertError => Self::DatabaseInsertFailed("entity"),
            super::EntityDBError::UnknownError => Self::UnknownError,
            super::EntityDBError::NotFoundError => Self::NotFoundError,
        }
    }
}

error_transform!(super::KeyManagerError => super::ApiError);
impl<'a> From<&'a super::KeyManagerError> for super::ApiError {
    fn from(value: &'a super::KeyManagerError) -> Self {
        match value {
            super::KeyManagerError::KeyAddFailed => {
                Self::KeyManagerError("Failed to add key to the Key manager")
            }
            super::KeyManagerError::KeyTransferFailed => {
                Self::KeyManagerError("Failed to transfer the key to the Key manager")
            }
            super::KeyManagerError::EncryptionFailed => {
                Self::KeyManagerError("Failed to encrypt the data in the Key manager")
            }
            super::KeyManagerError::DecryptionFailed => {
                Self::KeyManagerError("Failed to decrypt the data in the Key manager")
            }
            super::KeyManagerError::DbError => Self::KeyManagerError("Database error"),
            super::KeyManagerError::ResponseDecodingFailed => {
                Self::KeyManagerError("Failed to deserialize from bytes")
            }
            super::KeyManagerError::Unauthorized => Self::TenantError("Invalid master key"),
        }
    }
}

error_transform!(super::EntityDBError => super::KeyManagerError);
impl<'a> From<&'a super::EntityDBError> for super::KeyManagerError {
    fn from(value: &'a super::EntityDBError) -> Self {
        match value {
            super::EntityDBError::DBError
            | super::EntityDBError::DBFilterError
            | super::EntityDBError::DBInsertError
            | super::EntityDBError::UnknownError
            | super::EntityDBError::NotFoundError => Self::DbError,
        }
    }
}

error_transform!(super::ApiClientError => super::DataKeyCreationError);
impl<'a> From<&'a super::ApiClientError> for super::DataKeyCreationError {
    fn from(value: &'a super::ApiClientError) -> Self {
        match value {
            super::ApiClientError::ClientConstructionFailed
            | super::ApiClientError::Unexpected { .. } => Self::Unexpected,
            super::ApiClientError::HeaderMapConstructionFailed
            | super::ApiClientError::UrlEncodingFailed => Self::RequestConstructionFailed,
            super::ApiClientError::IdentityParseFailed
            | super::ApiClientError::CertificateParseFailed { .. } => Self::CertificateParseFailed,
            super::ApiClientError::RequestNotSent { .. } => Self::RequestSendFailed,
            super::ApiClientError::ResponseDecodingFailed => Self::ResponseDecodingFailed,
            super::ApiClientError::BadRequest(_) => Self::BadRequest,
            super::ApiClientError::InternalServerError(_) => Self::InternalServerError,
            super::ApiClientError::Unauthorized(_) => Self::Unauthorized,
        }
    }
}

error_transform!(super::DataKeyCreationError => super::KeyManagerError);
impl<'a> From<&'a super::DataKeyCreationError> for super::KeyManagerError {
    fn from(value: &'a super::DataKeyCreationError) -> Self {
        match value {
            super::DataKeyCreationError::RequestSendFailed
            | super::DataKeyCreationError::ResponseDecodingFailed
            | super::DataKeyCreationError::InternalServerError
            | super::DataKeyCreationError::Unexpected
            | super::DataKeyCreationError::BadRequest
            | super::DataKeyCreationError::CertificateParseFailed
            | super::DataKeyCreationError::RequestConstructionFailed => Self::KeyAddFailed,
            super::DataKeyCreationError::Unauthorized => Self::Unauthorized,
        }
    }
}

error_transform!(super::ApiClientError => super::DataKeyTransferError);
impl<'a> From<&'a super::ApiClientError> for super::DataKeyTransferError {
    fn from(value: &'a super::ApiClientError) -> Self {
        match value {
            super::ApiClientError::ClientConstructionFailed
            | super::ApiClientError::Unexpected { .. } => Self::Unexpected,
            super::ApiClientError::HeaderMapConstructionFailed
            | super::ApiClientError::UrlEncodingFailed => Self::RequestConstructionFailed,
            super::ApiClientError::IdentityParseFailed
            | super::ApiClientError::CertificateParseFailed { .. } => Self::CertificateParseFailed,
            super::ApiClientError::RequestNotSent { .. } => Self::RequestSendFailed,
            super::ApiClientError::ResponseDecodingFailed => Self::ResponseDecodingFailed,
            super::ApiClientError::BadRequest(_) => Self::BadRequest,
            super::ApiClientError::InternalServerError(_) => Self::InternalServerError,
            super::ApiClientError::Unauthorized(_) => Self::Unauthorized,
        }
    }
}

error_transform!(super::DataKeyTransferError => super::KeyManagerError);
impl<'a> From<&'a super::DataKeyTransferError> for super::KeyManagerError {
    fn from(value: &'a super::DataKeyTransferError) -> Self {
        match value {
            super::DataKeyTransferError::RequestSendFailed
            | super::DataKeyTransferError::ResponseDecodingFailed
            | super::DataKeyTransferError::InternalServerError
            | super::DataKeyTransferError::Unexpected
            | super::DataKeyTransferError::BadRequest
            | super::DataKeyTransferError::CertificateParseFailed
            | super::DataKeyTransferError::RequestConstructionFailed => Self::KeyTransferFailed,
            super::DataKeyTransferError::Unauthorized => Self::Unauthorized,
        }
    }
}

error_transform!(super::DataEncryptionError => super::KeyManagerError);
impl<'a> From<&'a super::DataEncryptionError> for super::KeyManagerError {
    fn from(value: &'a super::DataEncryptionError) -> Self {
        match value {
            super::DataEncryptionError::RequestSendFailed
            | super::DataEncryptionError::ResponseDecodingFailed
            | super::DataEncryptionError::InternalServerError
            | super::DataEncryptionError::Unexpected
            | super::DataEncryptionError::BadRequest
            | super::DataEncryptionError::CertificateParseFailed
            | super::DataEncryptionError::RequestConstructionFailed => Self::EncryptionFailed,
            super::DataEncryptionError::Unauthorized => Self::Unauthorized,
        }
    }
}

error_transform!(super::ApiClientError => super::DataEncryptionError);
impl<'a> From<&'a super::ApiClientError> for super::DataEncryptionError {
    fn from(value: &'a super::ApiClientError) -> Self {
        match value {
            super::ApiClientError::ClientConstructionFailed
            | super::ApiClientError::Unexpected { .. } => Self::Unexpected,
            super::ApiClientError::HeaderMapConstructionFailed
            | super::ApiClientError::UrlEncodingFailed => Self::RequestConstructionFailed,
            super::ApiClientError::IdentityParseFailed
            | super::ApiClientError::CertificateParseFailed { .. } => Self::CertificateParseFailed,
            super::ApiClientError::RequestNotSent { .. } => Self::RequestSendFailed,
            super::ApiClientError::ResponseDecodingFailed => Self::ResponseDecodingFailed,
            super::ApiClientError::BadRequest(_) => Self::BadRequest,
            super::ApiClientError::InternalServerError(_) => Self::InternalServerError,
            super::ApiClientError::Unauthorized(_) => Self::Unauthorized,
        }
    }
}

error_transform!(super::DataDecryptionError => super::KeyManagerError);
impl<'a> From<&'a super::DataDecryptionError> for super::KeyManagerError {
    fn from(value: &'a super::DataDecryptionError) -> Self {
        match value {
            super::DataDecryptionError::RequestSendFailed
            | super::DataDecryptionError::ResponseDecodingFailed
            | super::DataDecryptionError::InternalServerError
            | super::DataDecryptionError::Unexpected
            | super::DataDecryptionError::BadRequest
            | super::DataDecryptionError::CertificateParseFailed
            | super::DataDecryptionError::RequestConstructionFailed => Self::DecryptionFailed,
            super::DataDecryptionError::Unauthorized => Self::Unauthorized,
        }
    }
}

error_transform!(super::ApiClientError => super::DataDecryptionError);
impl<'a> From<&'a super::ApiClientError> for super::DataDecryptionError {
    fn from(value: &'a super::ApiClientError) -> Self {
        match value {
            super::ApiClientError::ClientConstructionFailed
            | super::ApiClientError::Unexpected { .. } => Self::Unexpected,
            super::ApiClientError::HeaderMapConstructionFailed
            | super::ApiClientError::UrlEncodingFailed => Self::RequestConstructionFailed,
            super::ApiClientError::IdentityParseFailed
            | super::ApiClientError::CertificateParseFailed { .. } => Self::CertificateParseFailed,
            super::ApiClientError::RequestNotSent { .. } => Self::RequestSendFailed,
            super::ApiClientError::ResponseDecodingFailed => Self::ResponseDecodingFailed,
            super::ApiClientError::BadRequest(_) => Self::BadRequest,
            super::ApiClientError::InternalServerError(_) => Self::InternalServerError,
            super::ApiClientError::Unauthorized(_) => Self::Unauthorized,
        }
    }
}

error_transform!(super::ApiClientError => super::KeyManagerHealthCheckError);
impl<'a> From<&'a super::ApiClientError> for super::KeyManagerHealthCheckError {
    fn from(value: &'a super::ApiClientError) -> Self {
        match value {
            super::ApiClientError::ClientConstructionFailed
            | super::ApiClientError::HeaderMapConstructionFailed
            | super::ApiClientError::IdentityParseFailed
            | super::ApiClientError::CertificateParseFailed { .. }
            | super::ApiClientError::UrlEncodingFailed
            | super::ApiClientError::RequestNotSent
            | super::ApiClientError::ResponseDecodingFailed
            | super::ApiClientError::BadRequest(_)
            | super::ApiClientError::Unexpected { .. }
            | super::ApiClientError::InternalServerError(_)
            | super::ApiClientError::Unauthorized(_) => Self::FailedToConnect,
        }
    }
}
