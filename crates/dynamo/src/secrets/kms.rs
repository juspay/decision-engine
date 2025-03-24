use crate::logger::error;
use aws_config::meta::region::RegionProviderChain;
use aws_config::Region;
use aws_sdk_kms::primitives::Blob;
use aws_sdk_kms::Client;
use base64::Engine;
use error_stack::{report, ResultExt};
use masking::{PeekInterface, Secret};

use crate::consts::BASE64_ENGINE;

use super::{SecretError, Secrets};

/// Client for AWS KMS operations.
#[derive(Debug, Clone)]
pub struct AwsKmsClient {
    inner_client: Client,
    key_id: String,
}

impl AwsKmsClient {
    /// Constructs a new AWS KMS client.
    pub async fn new(key_id: String, region: String) -> Self {
        let region_provider = RegionProviderChain::first_try(Region::new(region.clone()));
        let sdk_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .region(region_provider)
            .load()
            .await;

        Self {
            inner_client: Client::new(&sdk_config),
            key_id: key_id.clone(),
        }
    }

    /// Decrypts the provided base64-encoded encrypted data using the AWS KMS SDK. We assume that
    /// the SDK has the values required to interact with the AWS KMS APIs (`AWS_ACCESS_KEY_ID` and
    /// `AWS_SECRET_ACCESS_KEY`) either set in environment variables, or that the SDK is running in
    /// a machine that is able to assume an IAM role.
    pub async fn decrypt(
        &self,
        data: impl AsRef<[u8]>,
    ) -> error_stack::Result<String, AwsKmsError> {
        let data = BASE64_ENGINE
            .decode(data)
            .change_context(AwsKmsError::Base64DecodingFailed)?;
        let ciphertext_blob = Blob::new(data);

        let decrypt_output = self
            .inner_client
            .decrypt()
            .key_id(&self.key_id)
            .ciphertext_blob(ciphertext_blob)
            .send()
            .await
            .inspect_err(|e| {
                // Logging using `Debug` representation of the error as the `Display`
                // representation does not hold sufficient information.
                error!(aws_kms_sdk_error=?e, "Failed to AWS KMS decrypt data");
            })
            .change_context(AwsKmsError::DecryptionFailed)?;

        let output = decrypt_output
            .plaintext
            .ok_or(report!(AwsKmsError::MissingPlaintextDecryptionOutput))
            .and_then(|blob| {
                String::from_utf8(blob.into_inner()).change_context(AwsKmsError::Utf8DecodingFailed)
            })?;

        Ok(output)
    }
}

/// Errors that could occur during KMS operations.
#[derive(Debug, thiserror::Error)]
pub enum AwsKmsError {
    /// An error occurred when base64 decoding input data.
    #[error("Failed to base64 decode input data")]
    Base64DecodingFailed,

    /// An error occurred when AWS KMS decrypting input data.
    #[error("Failed to AWS KMS decrypt input data")]
    DecryptionFailed,

    /// The AWS KMS decrypted output does not include a plaintext output.
    #[error("Missing plaintext AWS KMS decryption output")]
    MissingPlaintextDecryptionOutput,

    /// An error occurred UTF-8 decoding AWS KMS decrypted output.
    #[error("Failed to UTF-8 decode decryption output")]
    Utf8DecodingFailed,
}

#[async_trait::async_trait]
impl Secrets for AwsKmsClient {
    async fn get_hash_key(
        &self,
        config: &crate::configs::Secrets,
    ) -> error_stack::Result<Secret<[u8; 32]>, SecretError> {
        let hash_key = self
            .decrypt(config.hash_key.peek().to_string())
            .await
            .change_context(SecretError::AwsKmsError)?;

        <[u8; 32]>::try_from(hex::decode(hash_key).change_context(SecretError::HexDecodingFailed)?)
            .map_err(|_| report!(SecretError::InvalidHashKeyLength))
            .map(Secret::new)
    }

    async fn get_database_password(
        &self,
        config: &crate::configs::DatabaseConfigs,
    ) -> error_stack::Result<String, SecretError> {
        self.decrypt(config.database_password.to_string())
            .await
            .change_context(SecretError::AwsKmsError)
    }

    async fn get_jwt_secret(
        &self,
        config: &crate::configs::Secrets,
    ) -> error_stack::Result<Secret<String>, SecretError> {
        Ok(Secret::new(
            self.decrypt(config.jwt_secret.peek())
                .await
                .change_context(SecretError::AwsKmsError)?,
        ))
    }
}
