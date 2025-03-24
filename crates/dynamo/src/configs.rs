use crate::{
    consts, elimination::configs::BucketSettings, error::ConfigurationError, logger::config::Log,
};
use redis_interface::RedisSettings;
use std::path::PathBuf;

#[derive(Clone, serde::Deserialize, Debug)]
pub struct Config {
    pub server: Server,
    pub metrics: MetricsServer,
    pub log: Log,
    pub redis: RedisSettings,
    pub ttl_for_keys: KeysTtl,
    pub global_routing_configs: GlobalRoutingConfigs,
    pub multi_tenancy: MultiTenancy,
    pub secret_config: crate::secrets::Config,
    pub secrets: Secrets,
    pub database: DatabaseConfigs,
    pub cache: CacheConfigs,
}

#[derive(Clone, serde::Deserialize, Debug)]
pub struct GlobalRoutingConfigs {
    pub success_rate: GlobalSrConfig,
    pub elimination_rate: BucketSettings,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct DatabaseConfigs {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub database: String,
    pub database_password: String,
    pub max_connections: u32,
    pub tenants: Vec<String>,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct CacheConfigs {
    pub max_cache_size: u64,
    pub ttl_in_seconds: u64,
    pub tti_in_seconds: u64,
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Secrets {
    pub hash_key: masking::Secret<String>,
    pub jwt_secret: masking::Secret<String>,
}

#[derive(Clone, serde::Deserialize, Debug)]
pub struct Server {
    pub host: String,
    pub port: u16,
    #[serde(rename = "type", default)]
    pub type_: ServiceType,
}

#[derive(Clone, serde::Deserialize, Debug)]
pub struct MetricsServer {
    pub host: String,
    pub port: u16,
}

#[derive(Clone, serde::Deserialize, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub enum ServiceType {
    #[default]
    Grpc,
    Http,
}

#[derive(Clone, Copy, serde::Deserialize, serde::Serialize, Debug, Default)]
pub struct GlobalSrConfig {
    pub min_aggregates_size: usize,
    pub default_success_rate: f64,
    pub max_aggregates_size: usize,
    pub current_block_threshold: GlobalSrCurrentBlockThreshold,
}

#[derive(Debug, Clone, Copy, serde::Deserialize, serde::Serialize, Default)]
pub struct GlobalSrCurrentBlockThreshold {
    pub duration_in_mins: Option<u64>,
    pub max_total_count: u64,
}

#[derive(Clone, Copy, serde::Deserialize, Debug)]
pub struct KeysTtl {
    pub aggregates: i64,
    pub current_block: i64,
    pub elimination_bucket: i64,
    pub contract_ttl: i64,
}

#[derive(Clone, serde::Deserialize, Debug)]
pub struct MultiTenancy {
    pub enabled: bool,
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
                config::Environment::with_prefix("DYNAMO")
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

impl Server {
    pub async fn tcp_listener(&self) -> Result<tokio::net::TcpListener, ConfigurationError> {
        let loc = format!("{}:{}", self.host, self.port);

        tracing::info!(loc = %loc, "binding the server");

        Ok(tokio::net::TcpListener::bind(loc).await?)
    }
}

impl MetricsServer {
    pub async fn tcp_listener(&self) -> Result<tokio::net::TcpListener, ConfigurationError> {
        let loc = format!("{}:{}", self.host, self.port);

        tracing::info!(loc = %loc, "binding the server");

        Ok(tokio::net::TcpListener::bind(loc).await?)
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
