use async_trait::async_trait;
use error_stack::{report, ResultExt};
use masking::{PeekInterface, Secret};

use super::SecretError;

pub struct NoEncryptionClient;

#[async_trait]
impl super::Secrets for NoEncryptionClient {
    async fn get_hash_key(
        &self,
        config: &crate::configs::Secrets,
    ) -> error_stack::Result<Secret<[u8; 32]>, SecretError> {
        <[u8; 32]>::try_from(
            hex::decode(config.hash_key.peek()).change_context(SecretError::HexDecodingFailed)?,
        )
        .map_err(|_| report!(SecretError::InvalidHashKeyLength))
        .map(Secret::new)
    }

    async fn get_database_password(
        &self,
        config: &crate::configs::DatabaseConfigs,
    ) -> error_stack::Result<String, SecretError> {
        Ok(config.database_password.clone())
    }

    async fn get_jwt_secret(
        &self,
        config: &crate::configs::Secrets,
    ) -> error_stack::Result<Secret<String>, SecretError> {
        Ok(config.jwt_secret.clone())
    }
}
