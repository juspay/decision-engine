use crate::{
    api_client::ApiClientConfig,
    crypto::secrets_manager::{
        secrets_interface::SecretManager, secrets_management::SecretsManagementConfig,
    },
    error,
    logger::config::Log,
};
use error_stack::ResultExt;
use masking::ExposeInterface;
use std::{
    collections::HashMap,
    ops::{Deref, DerefMut},
    path::PathBuf,
};
use redis_interface::RedisSettings;

#[derive(Clone, serde::Deserialize, Debug)]
pub struct GlobalConfig {
    pub server: Server,
    pub database: Database,
    pub secrets: Secrets,
    #[serde[default]]
    pub secrets_management: SecretsManagementConfig,
    pub log: Log,
    #[cfg(feature = "limit")]
    pub limit: Limit,
    pub redis: RedisSettings,
    pub tenant_secrets: TenantsSecrets,
    pub tls: Option<ServerTls>,
    #[serde(default)]
    pub api_client: ApiClientConfig,
}

#[derive(Clone, Debug)]
pub struct TenantConfig {
    pub tenant_id: String,
    pub open_router_secrets: Secrets,
    pub tenant_secrets: TenantSecrets,
}

impl TenantConfig {
    ///
    /// # Panics
    ///
    /// Never, as tenant_id would already be validated from [`crate::custom_extractors::TenantId`] custom extractor
    ///
    pub fn from_global_config(global_config: &GlobalConfig, tenant_id: String) -> Self {
        Self {
            tenant_id: tenant_id.clone(),
            open_router_secrets: global_config.secrets.clone(),
            #[allow(clippy::unwrap_used)]
            tenant_secrets: global_config
                .tenant_secrets
                .get(&tenant_id)
                .cloned()
                .unwrap(),
        }
    }
}

#[cfg(feature = "limit")]
#[derive(Clone, serde::Deserialize, Debug)]
pub struct Limit {
    pub request_count: u64,
    pub duration: u64, // in sec
    pub buffer_size: Option<usize>,
}

#[derive(Clone, serde::Deserialize, Debug)]
pub struct Server {
    pub host: String,
    pub port: u16,
}

#[derive(Clone, serde::Deserialize, Debug)]
pub struct Database {
    pub username: String,
    // KMS encrypted
    pub password: masking::Secret<String>,
    pub host: String,
    pub port: u16,
    pub dbname: String,
    pub pool_size: Option<usize>,
}


#[derive(Clone, serde::Deserialize, Debug)]
pub struct Secrets {
    // KMS encrypted
    #[cfg(feature = "middleware")]
    pub open_router_private_key: masking::Secret<String>,
}

#[derive(serde::Deserialize, Debug, Clone)]
pub struct TenantSecrets {
    #[serde(deserialize_with = "deserialize_hex")]
    pub master_key: Vec<u8>,
    #[cfg(feature = "middleware")]
    pub public_key: masking::Secret<String>,

    /// schema name for the tenant (defaults to tenant_id)
    pub schema: String,
}

fn deserialize_hex<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let deserialized_str: String = serde::Deserialize::deserialize(deserializer)?;

    let deserialized_str = deserialized_str.into_bytes();

    Ok(deserialized_str)
}

#[derive(serde::Deserialize, Debug, Clone)]
pub struct TenantsSecrets(HashMap<String, TenantSecrets>);

impl Deref for TenantsSecrets {
    type Target = HashMap<String, TenantSecrets>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for TenantsSecrets {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Debug, serde::Deserialize, Clone)]
pub struct ServerTls {
    /// certificate file associated with TLS (path to the certificate file (`pem` format))
    pub certificate: String,
    /// private key file path associated with TLS (path to the private key file (`pem` format))
    pub private_key: String,
}

impl Default for ApiClientConfig {
    fn default() -> Self {
        Self {
            client_idle_timeout: 90,
            pool_max_idle_per_host: 5,
        }
    }
}

/// Get the origin directory of the project
pub fn workspace_path() -> PathBuf {
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        PathBuf::from(manifest_dir)
    } else {
        PathBuf::from(".")
    }
}

impl GlobalConfig {
    /// Function to build the configuration by picking it from default locations
    pub fn new() -> Result<Self, config::ConfigError> {
        Self::new_with_config_path(None)
    }

    /// Function to build the configuration by picking it from default locations
    pub fn new_with_config_path(
        explicit_config_path: Option<PathBuf>,
    ) -> Result<Self, config::ConfigError> {
        let env = "dev";
        let config_path = Self::config_path(env, explicit_config_path);

        let config = Self::builder(env)?
            .add_source(config::File::from(config_path).required(false))
            .add_source(config::Environment::with_prefix("LOCKER").separator("__"))
            .build()?;

        serde_path_to_error::deserialize(config).map_err(|error| {
            eprintln!("Unable to deserialize application configuration: {error}");
            error.into_inner()
        })
    }

    pub fn builder(
        environment: &str,
    ) -> Result<config::ConfigBuilder<config::builder::DefaultState>, config::ConfigError> {
        config::Config::builder()
            // Here, it should be `set_override()` not `set_default()`.
            // "env" can't be altered by config field.
            // Should be single source of truth.
            .set_override("env", environment)
    }

    /// Config path.
    pub fn config_path(environment: &str, explicit_config_path: Option<PathBuf>) -> PathBuf {
        let mut config_path = PathBuf::new();
        if let Some(explicit_config_path_val) = explicit_config_path {
            config_path.push(explicit_config_path_val);
        } else {
            let config_directory: String = "config".into();
            let config_file_name = match environment {
                "production" => "production.toml",
                "sandbox" => "sandbox.toml",
                _ => "development.toml",
            };

            config_path.push(workspace_path());
            config_path.push(config_directory);
            config_path.push(config_file_name);
        }
        config_path
    }

    /// # Panics
    ///
    /// - If secret management client cannot be constructed
    /// - If master key cannot be utf8 decoded to String
    /// - If master key cannot be hex decoded
    ///
    #[allow(clippy::expect_used)]
    pub async fn fetch_raw_secrets(
        &mut self,
    ) -> error_stack::Result<(), error::ConfigurationError> {
        let secret_management_client = self
            .secrets_management
            .get_secret_management_client()
            .await
            .expect("Failed to create secret management client");

        self.database.password = secret_management_client
            .get_secret(self.database.password.clone())
            .await
            .change_context(error::ConfigurationError::KmsDecryptError(
                "database_password",
            ))?;

        for tenant_secrets in self.tenant_secrets.values_mut() {
            tenant_secrets.master_key = hex::decode(
                secret_management_client
                    .get_secret(
                        String::from_utf8(tenant_secrets.master_key.clone())
                            .expect("Failed while converting master key to `String`")
                            .into(),
                    )
                    .await
                    .change_context(error::ConfigurationError::KmsDecryptError("master_key"))?
                    .expose(),
            )
            .expect("Failed to hex decode master key")
        }

        #[cfg(feature = "middleware")]
        {
            for tenant_secrets in self.tenant_secrets.values_mut() {
                tenant_secrets.public_key = secret_management_client
                    .get_secret(tenant_secrets.public_key.clone())
                    .await
                    .change_context(error::ConfigurationError::KmsDecryptError("master_key"))?;
            }

            self.secrets.open_router_private_key = secret_management_client
                .get_secret(self.secrets.open_router_private_key.clone())
                .await
                .change_context(error::ConfigurationError::KmsDecryptError(
                    "open_router_private_key",
                ))?;
        }

        Ok(())
    }

    pub fn validate(&self) -> error_stack::Result<(), error::ConfigurationError> {
        self.secrets_management.validate()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::assertions_on_constants
    )]
    use super::*;

    #[derive(Clone, serde::Deserialize, Debug)]
    struct TestDeser {
        #[serde(default)]
        pub secrets_management: SecretsManagementConfig,
    }

    #[test]
    fn test_non_case() {
        let data = r#"

        "#;
        let parsed: TestDeser = serde_path_to_error::deserialize(
            config::Config::builder()
                .add_source(config::File::from_str(data, config::FileFormat::Toml))
                .build()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            parsed.secrets_management,
            SecretsManagementConfig::NoEncryption
        )
    }

    #[cfg(feature = "kms-aws")]
    #[test]
    fn test_aws_kms_case() {
        let data = r#"
        [secrets_management]
        secrets_manager = "aws_kms"

        [secrets_management.aws_kms]
        key_id = "123"
        region = "abc"
        "#;
        let parsed: TestDeser = serde_path_to_error::deserialize(
            config::Config::builder()
                .add_source(config::File::from_str(data, config::FileFormat::Toml))
                .build()
                .unwrap(),
        )
        .unwrap();

        match parsed.secrets_management {
            SecretsManagementConfig::AwsKms { aws_kms } => {
                assert!(aws_kms.key_id == "123" && aws_kms.region == "abc")
            }
            _ => assert!(false),
        }
    }

    #[cfg(feature = "kms-hashicorp-vault")]
    #[test]
    fn test_hashicorp_case() {
        let data = r#"
        [secrets_management]
        secrets_manager = "hashi_corp_vault"

        [secrets_management.hashi_corp_vault]
        url = "123"
        token = "abc"
        "#;
        let parsed: TestDeser = serde_path_to_error::deserialize(
            config::Config::builder()
                .add_source(config::File::from_str(data, config::FileFormat::Toml))
                .build()
                .unwrap(),
        )
        .unwrap();

        match parsed.secrets_management {
            SecretsManagementConfig::HashiCorpVault { hashi_corp_vault } => {
                assert!(hashi_corp_vault.url == "123" && hashi_corp_vault.token.expose() == "abc")
            }
            _ => assert!(false),
        }
    }
}
