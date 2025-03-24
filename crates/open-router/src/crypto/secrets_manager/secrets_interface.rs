//! Secrets management interface

use masking::Secret;

/// Trait defining the interface for managing application secrets
#[async_trait::async_trait]
pub trait SecretManager: Send + Sync {
    /// Given an input, decrypt/retrieve the secret
    async fn get_secret(
        &self,
        input: Secret<String>,
    ) -> error_stack::Result<Secret<String>, SecretsManagementError>;
}

/// Errors that may occur during secret management
#[derive(Debug, thiserror::Error)]
pub enum SecretsManagementError {
    /// An error occurred when retrieving raw data.
    #[error("Failed to fetch the raw data")]
    FetchSecretFailed,

    /// Failed while creating kms client
    #[error("Failed while creating a secrets management client")]
    ClientCreationFailed,
}
