use error_stack::ResultExt;
use masking::{ExposeInterface, Secret};

use crate::crypto::secrets_manager::{
    managers::hcvault::core::{HashiCorpVault, Kv2},
    secrets_interface::{SecretManager, SecretsManagementError},
};

#[async_trait::async_trait]
impl SecretManager for HashiCorpVault {
    async fn get_secret(
        &self,
        input: Secret<String>,
    ) -> error_stack::Result<Secret<String>, SecretsManagementError> {
        self.fetch::<Kv2, Secret<String>>(input.expose())
            .await
            .change_context(SecretsManagementError::FetchSecretFailed)
    }
}
