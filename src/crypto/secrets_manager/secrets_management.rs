#[cfg(feature = "kms-hashicorp-vault")]
use error_stack::ResultExt;
use masking::Secret;

#[cfg(feature = "kms-aws")]
use crate::crypto::secrets_manager::managers::aws_kms::core::{AwsKmsClient, AwsKmsConfig};
#[cfg(feature = "kms-hashicorp-vault")]
use crate::crypto::secrets_manager::managers::hcvault::core::{
    HashiCorpVault, HashiCorpVaultConfig,
};

use crate::{
    crypto::secrets_manager::{
        managers::hollow::core::NoEncryption,
        secrets_interface::{SecretManager, SecretsManagementError},
    },
    error::ConfigurationError,
};

/// Enum representing configuration options for secrets management.
#[derive(Debug, Clone, Default, serde::Deserialize, Eq, PartialEq)]
#[serde(tag = "secrets_manager")]
#[serde(rename_all = "snake_case")]
pub enum SecretsManagementConfig {
    /// AWS KMS configuration
    #[cfg(feature = "kms-aws")]
    AwsKms {
        /// AWS KMS config
        aws_kms: AwsKmsConfig,
    },

    /// HashiCorp-Vault configuration
    #[cfg(feature = "kms-hashicorp-vault")]
    HashiCorpVault {
        /// HC-Vault config
        hashi_corp_vault: HashiCorpVaultConfig,
    },

    /// Variant representing no encryption
    #[default]
    NoEncryption,
}

enum SecretsManagerClient {
    #[cfg(feature = "kms-aws")]
    AwsKms(AwsKmsClient),
    #[cfg(feature = "kms-hashicorp-vault")]
    HashiCorp(HashiCorpVault),
    NoEncryption(NoEncryption),
}

#[async_trait::async_trait]
impl SecretManager for SecretsManagerClient {
    async fn get_secret(
        &self,
        input: Secret<String>,
    ) -> error_stack::Result<Secret<String>, SecretsManagementError> {
        match self {
            #[cfg(feature = "kms-aws")]
            Self::AwsKms(config) => config.get_secret(input).await,
            #[cfg(feature = "kms-hashicorp-vault")]
            Self::HashiCorp(config) => config.get_secret(input).await,
            Self::NoEncryption(config) => config.get_secret(input).await,
        }
    }
}

impl SecretsManagementConfig {
    /// Verifies that the client configuration is usable
    pub fn validate(&self) -> Result<(), ConfigurationError> {
        match self {
            #[cfg(feature = "kms-aws")]
            Self::AwsKms { aws_kms } => aws_kms.validate(),
            #[cfg(feature = "kms-hashicorp-vault")]
            Self::HashiCorpVault { hashi_corp_vault } => hashi_corp_vault.validate(),
            Self::NoEncryption => Ok(()),
        }
    }

    /// Retrieves the appropriate secret management client based on the configuration.
    pub async fn get_secret_management_client(
        &self,
    ) -> error_stack::Result<impl SecretManager, SecretsManagementError> {
        match self {
            #[cfg(feature = "kms-aws")]
            Self::AwsKms { aws_kms } => Ok(SecretsManagerClient::AwsKms(
                AwsKmsClient::new(aws_kms).await,
            )),
            #[cfg(feature = "kms-hashicorp-vault")]
            Self::HashiCorpVault { hashi_corp_vault } => HashiCorpVault::new(hashi_corp_vault)
                .change_context(SecretsManagementError::ClientCreationFailed)
                .map(SecretsManagerClient::HashiCorp),
            Self::NoEncryption => Ok(SecretsManagerClient::NoEncryption(NoEncryption)),
        }
    }
}
