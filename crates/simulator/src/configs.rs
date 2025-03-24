use dynamo::{
    configs::{CacheConfigs, DatabaseConfigs, KeysTtl, MultiTenancy, Secrets},
    logger::{self, config::Log},
    secrets,
};
use redis_interface::RedisSettings;

use crate::{consts, errors, file_storage::FileStorageConfig, handlers::simulate::types::SrConfig};
use std::path::PathBuf;

#[derive(Clone, serde::Deserialize, Debug)]
pub struct Config {
    pub log: Log,
    pub server: Server,
    #[serde(default)]
    pub file_storage: FileStorageConfig,
    pub redis: RedisSettings,
    pub ttl_for_keys: KeysTtl,
    pub model_configs: Vec<SrConfig>,
    pub parameters: ParameterSettings,
    pub multi_tenancy: MultiTenancy,
    pub redis_simulation_keys: RedisSimulationKeys,
    pub baseline_static_data: BaselineStaticData,
    pub secrets: Secrets,
    pub secret_config: secrets::Config,
    pub database: DatabaseConfigs,
    pub cache: CacheConfigs,
    #[serde(default)]
    pub environment: Environment,
}

#[derive(Clone, serde::Deserialize, Default, Debug, strum::Display, strum::EnumString)]
#[serde(tag = "env")]
#[strum(serialize_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum Environment {
    #[default]
    Development,
    Integration,
    Sandbox,
}

#[derive(Clone, serde::Deserialize, Debug)]
pub struct BaselineStaticData {
    pub file_name: String,
}

#[derive(Clone, serde::Deserialize, Debug)]
pub struct Server {
    pub host: String,
    pub port: u16,
}

#[derive(Clone, serde::Deserialize, Debug)]
pub struct RedisSimulationKeys {
    pub ttl: i64,
}
#[derive(Clone, serde::Deserialize, Debug)]
pub struct ParameterSettings {
    pub max_default_body_limit_in_bytes: usize,
    pub total_chunks: usize,
    pub total_records_per_json: usize,
}

impl Server {
    pub async fn tcp_listener(
        &self,
    ) -> Result<tokio::net::TcpListener, errors::ConfigurationError> {
        let loc = format!("{}:{}", self.host, self.port);
        logger::info!(host = %self.host, port = %self.port, "starting simulator");

        tokio::net::TcpListener::bind(loc).await.map_err(|err| {
            errors::ConfigurationError::ServerError {
                msg: err.to_string(),
            }
        })
    }
}

impl Config {
    /// Function to build the configuration by picking it from default locations
    pub fn new() -> Result<Self, config::ConfigError> {
        Self::new_with_config_path(None)
    }

    /// Function to build the configuration by picking it from default locations
    pub fn new_with_config_path(
        explicit_config_path: Option<PathBuf>,
    ) -> Result<Self, config::ConfigError> {
        let env = consts::Env::current_env();
        let config_path = Self::config_path(&env, explicit_config_path);

        let config = Self::builder(&env)?
            .add_source(config::File::from(config_path).required(false))
            .add_source(
                config::Environment::with_prefix("SIMULATOR")
                    .try_parsing(true)
                    .separator("__")
                    .list_separator(",")
                    .with_list_parse_key("redis.cluster_urls")
                    .with_list_parse_key("database.tenants"),
            )
            .build()?;

        #[allow(clippy::print_stderr)]
        serde_path_to_error::deserialize(config).map_err(|error| {
            eprintln!("Unable to deserialize application configuration: {error}");
            error.into_inner()
        })
    }

    pub fn builder(
        environment: &consts::Env,
    ) -> Result<config::ConfigBuilder<config::builder::DefaultState>, config::ConfigError> {
        config::Config::builder()
            // Here, it should be `set_override()` not `set_default()`.
            // "env" can't be altered by config field.
            // Should be single source of truth.
            .set_override("env", environment.to_string())
    }

    /// Config path.
    pub fn config_path(
        environment: &consts::Env,
        explicit_config_path: Option<PathBuf>,
    ) -> PathBuf {
        let mut config_path = PathBuf::new();
        if let Some(explicit_config_path_val) = explicit_config_path {
            config_path.push(explicit_config_path_val);
        } else {
            let config_directory: String = "config".into();
            let config_file_name = environment.config_path();

            config_path.push(workspace_path());
            config_path.push(config_directory);
            config_path.push(config_file_name);
        }
        config_path
    }
}

/// Get the origin directory of the project
pub fn workspace_path() -> PathBuf {
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let mut path = PathBuf::from(manifest_dir);
        path.pop();
        path.pop();
        path
    } else {
        PathBuf::from(".")
    }
}
