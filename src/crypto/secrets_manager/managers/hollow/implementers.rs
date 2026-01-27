use error_stack::ResultExt;
use masking::{ExposeInterface, Secret};

use crate::crypto::secrets_manager::{
    managers::hollow::core::NoEncryption,
    secrets_interface::{SecretManager, SecretsManagementError},
};

#[async_trait::async_trait]
impl SecretManager for NoEncryption {
    async fn get_secret(
        &self,
        input: Secret<String>,
    ) -> error_stack::Result<Secret<String>, SecretsManagementError> {
        String::from_utf8(self.decrypt(input.expose()))
            .map(Into::into)
            .change_context(SecretsManagementError::FetchSecretFailed)
            .attach_printable("Failed to UTF-8 decode the secret")
    }
}
