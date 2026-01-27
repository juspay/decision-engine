//! Interactions with the AWS KMS SDK

use aws_config::meta::region::RegionProviderChain;
use aws_sdk_kms::{config::Region, primitives::Blob, Client};
use base64::Engine;
use error_stack::{report, ResultExt};

use crate::{crypto::consts::BASE64_ENGINE, error::ConfigurationError, logger};

/// Configuration parameters required for constructing a [`AwsKmsClient`].
#[derive(Clone, Debug, Default, serde::Deserialize, Eq, PartialEq)]
#[serde(default)]
pub struct AwsKmsConfig {
    /// The AWS key identifier of the KMS key used to encrypt or decrypt data.
    pub key_id: String,

    /// The AWS region to send KMS requests to.
    pub region: String,
}

/// Client for AWS KMS operations.
#[derive(Debug, Clone)]
pub struct AwsKmsClient {
    inner_client: Client,
    key_id: String,
}

impl AwsKmsClient {
    /// Constructs a new AWS KMS client.
    pub async fn new(config: &AwsKmsConfig) -> Self {
        let region_provider = RegionProviderChain::first_try(Region::new(config.region.clone()));
        let sdk_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .region(region_provider)
            .load()
            .await;

        Self {
            inner_client: Client::new(&sdk_config),
            key_id: config.key_id.clone(),
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

        logger::debug!(ciphertext_blob=?ciphertext_blob, "Ciphertext blob to be decrypted");

        let decrypt_output = self
            .inner_client
            .decrypt()
            .key_id(&self.key_id)
            .ciphertext_blob(ciphertext_blob)
            .send()
            .await
            .map_err(|error| {
                // Logging using `Debug` representation of the error as the `Display`
                // representation does not hold sufficient information.
                logger::error!(aws_kms_sdk_error=?error, "Failed to AWS KMS decrypt data");
                error
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
    /// An error occurred when base64 encoding input data.
    #[error("Failed to base64 encode input data")]
    Base64EncodingFailed,

    /// An error occurred when base64 decoding input data.
    #[error("Failed to base64 decode input data")]
    Base64DecodingFailed,

    /// An error occurred when AWS KMS decrypting input data.
    #[error("Failed to AWS KMS decrypt input data")]
    DecryptionFailed,

    /// An error occurred when AWS KMS encrypting input data.
    #[error("Failed to AWS KMS encrypt input data")]
    EncryptionFailed,

    /// The AWS KMS decrypted output does not include a plaintext output.
    #[error("Missing plaintext AWS KMS decryption output")]
    MissingPlaintextDecryptionOutput,

    /// The AWS KMS encrypted output does not include a ciphertext output.
    #[error("Missing ciphertext AWS KMS encryption output")]
    MissingCiphertextEncryptionOutput,

    /// An error occurred UTF-8 decoding AWS KMS decrypted output.
    #[error("Failed to UTF-8 decode decryption output")]
    Utf8DecodingFailed,

    /// The AWS KMS client has not been initialized.
    #[error("The AWS KMS client has not been initialized")]
    AwsKmsClientNotInitialized,
}

impl AwsKmsConfig {
    /// Verifies that the [`AwsKmsClient`] configuration is usable.
    pub fn validate(&self) -> Result<(), ConfigurationError> {
        if self.key_id.trim().is_empty() {
            return Err(ConfigurationError::InvalidConfigurationValueError(
                "AWS KMS key ID must not be empty".into(),
            ));
        }

        if self.region.trim().is_empty() {
            return Err(ConfigurationError::InvalidConfigurationValueError(
                "AWS KMS region must not be empty".into(),
            ));
        }

        Ok(())
    }
}
