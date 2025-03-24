use error_stack::ResultExt;
use masking::{PeekInterface, Secret};

use crate::crypto::secrets_manager::{
    managers::aws_kms::core::AwsKmsClient,
    secrets_interface::{SecretManager, SecretsManagementError},
};

#[async_trait::async_trait]
impl SecretManager for AwsKmsClient {
    async fn get_secret(
        &self,
        input: Secret<String>,
    ) -> error_stack::Result<Secret<String>, SecretsManagementError> {
        self.decrypt(input.peek())
            .await
            .change_context(SecretsManagementError::FetchSecretFailed)
            .map(Into::into)
    }
}
