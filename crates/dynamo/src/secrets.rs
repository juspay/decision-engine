pub mod kms;
pub mod no_encryption;

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
pub enum Config {
    AwsKms { region: String, key_id: String },
    NoEncryption,
}

#[async_trait::async_trait]
pub trait Secrets {
    async fn get_hash_key(
        &self,
        config: &crate::configs::Secrets,
    ) -> error_stack::Result<masking::Secret<[u8; 32]>, SecretError>;

    async fn get_database_password(
        &self,
        config: &crate::configs::DatabaseConfigs,
    ) -> error_stack::Result<String, SecretError>;

    async fn get_jwt_secret(
        &self,
        config: &crate::configs::Secrets,
    ) -> error_stack::Result<masking::Secret<String>, SecretError>;
}

#[derive(Debug, thiserror::Error)]
pub enum SecretError {
    #[error("AWS KMS error")]
    AwsKmsError,
    #[error("Hex decoding failed")]
    HexDecodingFailed,
    #[error("Invalid hash key length")]
    InvalidHashKeyLength,
}

impl Config {
    pub async fn create_client(&self) -> Box<dyn Secrets> {
        match self {
            Self::AwsKms { region, key_id } => {
                let output: Box<dyn Secrets> =
                    Box::new(kms::AwsKmsClient::new(key_id.clone(), region.to_string()).await);
                output
            }
            Self::NoEncryption => Box::new(no_encryption::NoEncryptionClient),
        }
    }
}
