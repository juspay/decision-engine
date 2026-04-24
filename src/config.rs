use crate::decider::network_decider;
use crate::{
    api_client::ApiClientConfig,
    crypto::secrets_manager::{
        secrets_interface::SecretManager, secrets_management::SecretsManagementConfig,
    },
    error,
    euclid::types::TomlConfig,
    logger,
    logger::config::Log,
};
use error_stack::ResultExt;
#[cfg(all(feature = "kms-hashicorp-vault", test))]
use masking::ExposeInterface;
use redis_interface::RedisSettings;
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    ops::{Deref, DerefMut},
    path::PathBuf,
};

#[derive(Clone, serde::Deserialize, Debug)]
pub struct GlobalConfig {
    pub server: Server,
    pub metrics: Server,
    #[cfg(feature = "mysql")]
    pub database: Database,
    #[cfg(feature = "postgres")]
    pub pg_database: PgDatabase,
    #[serde(default)]
    pub secrets_management: SecretsManagementConfig,
    pub log: Log,
    #[cfg(feature = "limit")]
    pub limit: Limit,
    pub redis: RedisSettings,
    pub cache_config: CacheConfig,
    pub tenant_secrets: TenantsSecrets,
    pub tls: Option<ServerTls>,
    #[serde(default)]
    pub api_client: ApiClientConfig,
    #[serde(default)]
    pub analytics: AnalyticsConfig,
    #[serde(default)]
    pub routing_config: Option<TomlConfig>,
    #[serde(default)]
    pub pm_filters: ConnectorFilters,
    #[serde(default)]
    pub debit_routing_config: network_decider::types::DebitRoutingConfig,
    pub compression_filepath: Option<CompressionFilepath>,
    #[serde(default)]
    pub api_key_auth_enabled: bool,
    #[serde(default)]
    pub user_auth: UserAuthConfig,
    #[serde(default)]
    pub admin_secret: AdminSecretConfig,
}

#[derive(Clone, serde::Deserialize, Debug)]
pub struct UserAuthConfig {
    /// Secret used to sign JWTs — set a strong random value in production
    pub jwt_secret: String,
    /// JWT expiry in seconds (default 24 hours)
    #[serde(default = "default_jwt_expiry")]
    pub jwt_expiry_seconds: u64,
    /// Send verification email on signup; block login until verified
    #[serde(default)]
    pub email_verification_enabled: bool,
}

fn default_jwt_expiry() -> u64 {
    86400
}

const DEFAULT_ADMIN_SECRET: &str = "test_admin";

#[derive(Clone, serde::Deserialize, Debug)]
pub struct AdminSecretConfig {
    /// Secret required in `x-admin-secret` header to call privileged endpoints (e.g. merchant create)
    pub secret: String,
}

impl Default for AdminSecretConfig {
    fn default() -> Self {
        Self {
            secret: DEFAULT_ADMIN_SECRET.to_string(),
        }
    }
}

impl AdminSecretConfig {
    pub fn is_default(&self) -> bool {
        self.secret == DEFAULT_ADMIN_SECRET
    }
}

impl Default for UserAuthConfig {
    fn default() -> Self {
        Self {
            jwt_secret: "change_me_in_production_use_32chars!!".to_string(),
            jwt_expiry_seconds: default_jwt_expiry(),
            email_verification_enabled: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct TenantConfig {
    pub tenant_id: String,
    pub tenant_secrets: TenantSecrets,
    pub routing_config: Option<TomlConfig>,
    pub pm_filters: ConnectorFilters,
    pub debit_routing_config: network_decider::types::DebitRoutingConfig,
    pub cache_config: CacheConfig,
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(default)]
pub struct AnalyticsConfig {
    pub capture: AnalyticsCaptureConfig,
    pub kafka: KafkaAnalyticsConfig,
    pub clickhouse: ClickHouseAnalyticsConfig,
}

impl Default for AnalyticsConfig {
    fn default() -> Self {
        Self {
            capture: AnalyticsCaptureConfig::default(),
            kafka: KafkaAnalyticsConfig::default(),
            clickhouse: ClickHouseAnalyticsConfig::default(),
        }
    }
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(default)]
pub struct AnalyticsCaptureConfig {
    pub details_max_bytes: usize,
}

impl Default for AnalyticsCaptureConfig {
    fn default() -> Self {
        Self {
            details_max_bytes: 65_536,
        }
    }
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(default)]
pub struct KafkaAnalyticsConfig {
    pub enabled: bool,
    pub brokers: String,
    pub client_id: String,
    pub api_topic: String,
    pub domain_topic: String,
    pub acks: String,
    pub compression: String,
    pub message_timeout_ms: u64,
    pub queue_capacity: usize,
    pub security_protocol: Option<String>,
    pub sasl_mechanism: Option<String>,
    pub sasl_username: Option<String>,
    pub sasl_password: Option<masking::Secret<String>>,
}

impl Default for KafkaAnalyticsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            brokers: "localhost:9092".to_string(),
            client_id: "decision-engine".to_string(),
            api_topic: "api".to_string(),
            domain_topic: "domain".to_string(),
            acks: "all".to_string(),
            compression: "lz4".to_string(),
            message_timeout_ms: 5_000,
            queue_capacity: 250,
            security_protocol: None,
            sasl_mechanism: None,
            sasl_username: None,
            sasl_password: None,
        }
    }
}

#[derive(Clone, Debug, serde::Deserialize)]
#[serde(default)]
pub struct ClickHouseAnalyticsConfig {
    pub enabled: bool,
    pub url: String,
    pub database: String,
    pub user: String,
    pub password: Option<masking::Secret<String>>,
}

impl Default for ClickHouseAnalyticsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            url: String::new(),
            database: "default".to_string(),
            user: "default".to_string(),
            password: None,
        }
    }
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
            routing_config: global_config.routing_config.clone(),
            pm_filters: global_config.pm_filters.clone(),
            #[allow(clippy::unwrap_used)]
            tenant_secrets: global_config
                .tenant_secrets
                .get(&tenant_id)
                .cloned()
                .unwrap(),
            debit_routing_config: global_config.debit_routing_config.clone(),
            cache_config: global_config.cache_config.clone(),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(transparent)]
pub struct ConnectorFilters(pub HashMap<String, PaymentMethodFilters>);

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(transparent)]
pub struct PaymentMethodFilters(pub HashMap<String, CurrencyCountryFlowFilter>);

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct CurrencyCountryFlowFilter {
    #[serde(default, deserialize_with = "deserialize_optional_hashset")]
    pub country: Option<HashSet<String>>,
    #[serde(default, deserialize_with = "deserialize_optional_hashset")]
    pub currency: Option<HashSet<String>>,
    pub not_available_flows: Option<NotAvailableFlows>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(default)]
pub struct NotAvailableFlows {
    pub capture_method: Option<String>,
}

fn deserialize_optional_hashset<'a, D>(deserializer: D) -> Result<Option<HashSet<String>>, D::Error>
where
    D: serde::Deserializer<'a>,
{
    let raw = Option::<String>::deserialize(deserializer)?;
    Ok(raw.map(|value| {
        value
            .trim()
            .split(',')
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(ToOwned::to_owned)
            .collect()
    }))
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
pub struct PgDatabase {
    pub pg_username: String,
    pub pg_password: masking::Secret<String>,
    pub pg_host: String,
    pub pg_port: u16,
    pub pg_dbname: String,
    pub pg_pool_size: Option<usize>,
}

#[derive(Clone, serde::Deserialize, Debug)]
pub struct CacheConfig {
    pub service_config_redis_prefix: String,
    pub service_config_ttl: i64,
}

impl CacheConfig {
    pub fn add_prefix(&self, key: &str) -> String {
        format!("{}{}", self.service_config_redis_prefix, key)
    }
}

#[derive(Clone, serde::Deserialize, Debug)]
pub struct CompressionFilepath {
    pub zstd_compression_filepath: String,
}

#[derive(serde::Deserialize, Debug, Clone)]
pub struct TenantSecrets {
    /// schema name for the tenant (defaults to tenant_id)
    pub schema: String,
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
        let env = std::env::var("APP_ENV").unwrap_or_else(|_| "dev".to_string());
        let config_path = Self::config_path(&env, explicit_config_path);

        let config = Self::builder(&env)?
            .add_source(config::File::from(config_path).required(false))
            .add_source(
                config::Environment::with_prefix("DECISION_ENGINE")
                    .separator("__")
                    .list_separator(",")
                    .with_list_parse_key("redis.cluster_urls"),
            )
            .build()?;

        serde_path_to_error::deserialize(config).map_err(|error| {
            logger::error!("Unable to deserialize application configuration: {error}");
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

        #[cfg(feature = "mysql")]
        {
            self.database.password = secret_management_client
                .get_secret(self.database.password.clone())
                .await
                .change_context(error::ConfigurationError::KmsDecryptError(
                    "database_password",
                ))?;
        }
        #[cfg(feature = "postgres")]
        {
            self.pg_database.pg_password = secret_management_client
                .get_secret(self.pg_database.pg_password.clone())
                .await
                .change_context(error::ConfigurationError::KmsDecryptError(
                    "pg_database_password",
                ))?;
        }

        if let Some(password) = self.analytics.clickhouse.password.clone() {
            self.analytics.clickhouse.password = Some(
                secret_management_client
                    .get_secret(password)
                    .await
                    .change_context(error::ConfigurationError::KmsDecryptError(
                        "analytics_clickhouse_password",
                    ))?,
            );
        }

        if let Some(password) = self.analytics.kafka.sasl_password.clone() {
            self.analytics.kafka.sasl_password = Some(
                secret_management_client
                    .get_secret(password)
                    .await
                    .change_context(error::ConfigurationError::KmsDecryptError(
                        "analytics_kafka_sasl_password",
                    ))?,
            );
        }

        Ok(())
    }

    pub fn validate(&self) -> error_stack::Result<(), error::ConfigurationError> {
        self.secrets_management.validate()?;
        if self.analytics.capture.details_max_bytes == 0 {
            return Err(error_stack::report!(
                error::ConfigurationError::InvalidConfigurationValueError(
                    "analytics.capture.details_max_bytes".to_string(),
                )
            ));
        }
        if self.analytics.clickhouse.enabled && self.analytics.clickhouse.url.trim().is_empty() {
            return Err(error_stack::report!(
                error::ConfigurationError::InvalidConfigurationValueError(
                    "analytics.clickhouse.url".to_string(),
                )
            ));
        }
        if self.analytics.kafka.enabled && self.analytics.kafka.queue_capacity == 0 {
            return Err(error_stack::report!(
                error::ConfigurationError::InvalidConfigurationValueError(
                    "analytics.kafka.queue_capacity".to_string(),
                )
            ));
        }
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

/// Represents a key configuration in the TOML file
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct KeyConfig {
    #[serde(rename = "type")]
    pub data_type: String,
    #[serde(default)]
    pub values: Option<String>,
}

/// Structure for the [keys] section in the TOML
#[derive(Clone, Debug, serde::Deserialize, serde::Serialize)]
pub struct KeysConfig {
    #[serde(flatten)]
    pub keys: HashMap<String, KeyConfig>,
}
