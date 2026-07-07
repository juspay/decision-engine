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
    sync::Arc,
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
    #[serde(default = "default_true")]
    pub api_key_auth_enabled: bool,
    #[serde(default)]
    pub user_auth: UserAuthConfig,
    #[serde(default)]
    pub admin_secret: AdminSecretConfig,
    #[serde(default)]
    pub email: EmailConfig,
    #[serde(default)]
    pub gsm: GsmConfig,
    #[serde(default)]
    pub mem_cache: MemCacheConfig,
    #[serde(default)]
    pub hypersense: HypersenseConfig,
    #[serde(default)]
    pub cost_ingestion: CostIngestionConfig,
    #[serde(default)]
    pub sr_auto_calibration: SrAutoCalibrationConfig,
}

/// Runtime auto-calibration of the SRv3 bucket size + hedging %.
#[derive(Clone, serde::Deserialize, Debug, Default)]
pub struct SrAutoCalibrationConfig {
    /// Recalc cadence in seconds. When unset (or 0), the job uses a built-in default (900s).
    #[serde(default)]
    pub interval_secs: Option<u64>,
    /// Smallest bucket size the calibrator will write. Default 100.
    #[serde(default)]
    pub min_bucket_size: Option<i32>,
    /// Largest bucket size the calibrator will write. Lower it (e.g. 25) for snappy demos —
    /// a smaller window flips scores faster. Default 2000.
    #[serde(default)]
    pub max_bucket_size: Option<i32>,
    /// Total hedging cap (%). Default 30.
    #[serde(default)]
    pub max_hedging_percent: Option<f64>,
    /// Horizon (seconds) hedging sizes window-refresh against. Default 3600 (production). Set it
    /// short (e.g. 60) for demos so laggards get enough exploration traffic to recover quickly.
    #[serde(default)]
    pub reaction_horizon_secs: Option<u64>,
    /// Trailing window (seconds) over which volume/dimensions are counted in ClickHouse. Default
    /// 3600. Keep it well above `interval_secs` so windows overlap and smooth; shorten (e.g. 300)
    /// for demos to react faster and shed stale events.
    #[serde(default)]
    pub lookback_secs: Option<u64>,
    /// Minimum per-cluster volume in the lookback window before a cluster is calibrated. Default
    /// 100. Lower it (e.g. 20) for demos, especially when splitting by dimension.
    #[serde(default)]
    pub min_volume: Option<i64>,
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

fn default_true() -> bool {
    true
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

/// Top-level email configuration block
#[derive(Clone, serde::Deserialize, Debug)]
#[serde(default)]
pub struct EmailConfig {
    /// From-address used in all outgoing emails
    pub sender_email: String,
    /// Public base URL of the application (used for verification links)
    pub base_url: String,
    /// Which email backend to use
    pub active_email_client: EmailClientChoice,
    /// SMTP settings — required when active_email_client = "smtp"
    pub smtp: Option<SmtpConfig>,
    /// AWS SES settings — required when active_email_client = "aws_ses"
    pub aws_ses: Option<AwsSesEmailConfig>,
}

impl Default for EmailConfig {
    fn default() -> Self {
        Self {
            sender_email: String::new(),
            base_url: "http://localhost:8080".to_string(),
            active_email_client: EmailClientChoice::default(),
            smtp: None,
            aws_ses: None,
        }
    }
}

impl EmailConfig {
    /// Returns true if a real email backend is configured
    pub fn is_active(&self) -> bool {
        !matches!(self.active_email_client, EmailClientChoice::NoEmailClient)
    }

    /// Construct the email client from this config
    pub async fn build_client(
        &self,
    ) -> error_stack::Result<crate::email::DynEmailClient, crate::email::EmailError> {
        use crate::email::EmailError;
        use error_stack::report;

        let client: Arc<dyn crate::email::EmailClient> = match &self.active_email_client {
            EmailClientChoice::NoEmailClient => Arc::new(crate::email::no_email::NoEmailClient),
            EmailClientChoice::Smtp => {
                let smtp_config = self
                    .smtp
                    .as_ref()
                    .ok_or_else(|| report!(EmailError::MissingConfig))?;
                Arc::new(crate::email::smtp::SmtpEmailClient::new(
                    smtp_config,
                    self.sender_email.clone(),
                )?)
            }
            EmailClientChoice::AwsSes => {
                let ses_config = self
                    .aws_ses
                    .as_ref()
                    .ok_or_else(|| report!(EmailError::MissingConfig))?;
                Arc::new(
                    crate::email::aws_ses::AwsSesEmailClient::new(
                        ses_config,
                        self.sender_email.clone(),
                    )
                    .await?,
                )
            }
        };

        Ok(client)
    }
}

/// Selects which email backend is active
#[derive(Clone, serde::Deserialize, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub enum EmailClientChoice {
    /// Emails are silently discarded — useful for development
    #[default]
    NoEmailClient,
    /// Send via SMTP (works with any SMTP-compatible service)
    Smtp,
    /// Send via AWS Simple Email Service v2
    AwsSes,
}

/// SMTP client configuration
#[derive(Clone, serde::Deserialize, Debug)]
pub struct SmtpConfig {
    /// SMTP server hostname
    pub host: String,
    /// SMTP server port — defaults to 587 (STARTTLS) or 465 (TLS), 1025 (None)
    pub port: Option<u16>,
    /// SMTP auth username
    pub username: String,
    /// SMTP auth password
    pub password: String,
    /// TLS mode — use "none" for local dev (Mailpit), "starttls" for most production SMTP servers
    #[serde(default)]
    pub tls: SmtpTls,
}

#[derive(Clone, serde::Deserialize, Debug, Default)]
#[serde(rename_all = "snake_case")]
pub enum SmtpTls {
    /// No TLS — plain SMTP. Use for local testing with Mailpit.
    None,
    /// Upgrade to TLS via STARTTLS after connecting. Standard for port 587.
    #[default]
    StartTls,
    /// Implicit TLS from the first byte. Standard for port 465.
    Tls,
}

/// AWS SES v2 client configuration
#[derive(Clone, serde::Deserialize, Debug)]
pub struct AwsSesEmailConfig {
    /// AWS region where SES is configured (e.g. "us-east-1")
    pub region: String,
    /// IAM role ARN to assume via STS before sending emails (cross-account use)
    pub email_role_arn: Option<String>,
    /// STS session name used when assuming the role
    pub sts_role_session_name: Option<String>,
    /// HTTP proxy URL for SES API calls (e.g. "http://squid-proxy:3128"). Use an `http://` scheme
    /// even when proxying HTTPS destinations — the proxy uses CONNECT tunneling. Required in private
    /// subnets without a SES VPC endpoint.
    pub proxy_url: Option<String>,
}

#[derive(Clone, Debug)]
pub struct TenantConfig {
    pub tenant_id: String,
    pub tenant_secrets: TenantSecrets,
    pub routing_config: Option<TomlConfig>,
    pub pm_filters: ConnectorFilters,
    pub debit_routing_config: network_decider::types::DebitRoutingConfig,
    pub cache_config: CacheConfig,
    pub hypersense: HypersenseConfig,
    pub cost_ingestion: CostIngestionConfig,
}

/// Configuration for the in-house cost-estimation settlement ingestion pipeline
/// (see `scratch/inhouse-cost-architecture.md` §7).
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(default)]
pub struct CostIngestionConfig {
    /// Key id used to encrypt *new* connector credentials. Must name a key present in
    /// `creds_encryption_keys`. Rotate by adding a new key there and pointing this at it —
    /// old keys stay in the ring so existing credentials still decrypt. Empty ⇒ storage disabled.
    pub creds_encryption_current: String,
    /// Keyring: key-id → hex-encoded 32-byte AES-256 key. Retaining old ids is what makes
    /// rotation non-destructive: a credential is stored tagged with the id that encrypted it,
    /// and decryption looks the id up here. Generate keys with `openssl rand -hex 32`.
    pub creds_encryption_keys: std::collections::HashMap<String, masking::Secret<String>>,
    /// Enable the background ingest worker that drains `report_ingest_queue`. Off by default so
    /// only the deployment(s) meant to run ingestion do.
    pub worker_enabled: bool,
    /// How often the ingest worker polls the queue, in seconds.
    pub worker_interval_secs: u64,
    /// Max jobs claimed per poll cycle.
    pub worker_batch_size: usize,
}

impl Default for CostIngestionConfig {
    fn default() -> Self {
        Self {
            creds_encryption_current: String::new(),
            creds_encryption_keys: std::collections::HashMap::new(),
            worker_enabled: false,
            worker_interval_secs: 60,
            worker_batch_size: 20,
        }
    }
}

#[derive(Clone, Debug, Default, serde::Deserialize)]
#[serde(default)]
pub struct AnalyticsConfig {
    pub capture: AnalyticsCaptureConfig,
    pub kafka: KafkaAnalyticsConfig,
    pub clickhouse: ClickHouseAnalyticsConfig,
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
            hypersense: global_config.hypersense.clone(),
            cost_ingestion: global_config.cost_ingestion.clone(),
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
#[serde(default)]
pub struct HypersenseConfig {
    pub base_url: String,
    pub username: String,
    pub password: masking::Secret<String>,
    /// TTL (seconds) for the cached access token. Must stay below the token's own
    /// lifetime (the sign-in API issues 86400s tokens); defaults to 23h.
    pub token_ttl_secs: i64,
    /// TTL (seconds) for the in-process fee-rate-estimate cache that fronts the
    /// cost-observability endpoint. Identical lookup scenarios within this window
    /// are served from memory instead of re-hitting the API. `0` disables the
    /// cache. Defaults to 300s (5 min) — short, since this is a temporary cache
    /// and cost data should not go stale for long.
    pub cost_cache_ttl_secs: u64,
    /// When true, candidate PSP costs are served from the local, deterministic
    /// `seed_costs` table (realistic US IC++ for Adyen vs blended for Stripe) instead
    /// of the live Hypersense API. Intended for the Decision Simulator so the auth-vs-cost
    /// tradeoff is offline and repeatable. Defaults to `false` (production uses the API).
    pub use_seed_costs: bool,
    /// Per-PSP seed cost models consulted when `use_seed_costs` is true. Each entry has a
    /// `default` fee plus optional `tiers` overriding it by card network/program. Empty by
    /// default; populate it (see `config/development.toml`) to drive the simulator.
    pub seed_costs: Vec<SeedCostEntry>,
}

impl Default for HypersenseConfig {
    fn default() -> Self {
        Self {
            base_url: "https://eu.hyperswitch.io/cost-observability/router".to_string(),
            username: String::new(),
            password: masking::Secret::new(String::new()),
            token_ttl_secs: 82_800,
            cost_cache_ttl_secs: 300,
            use_seed_costs: false,
            seed_costs: Vec::new(),
        }
    }
}

/// An amount-independent fee split — `effective_cost_bps = pct_bps + fixed/amount·10_000`.
/// `fixed` is in the cluster's major currency unit (e.g. dollars).
#[derive(Clone, Debug, serde::Deserialize)]
pub struct SeedFeeModel {
    pub pct_bps: f64,
    pub fixed: f64,
}

/// A fee override scoped to a card scenario. Each field is matched against the same-named
/// `ClusterKey` field; a `None` field is a wildcard (matches any value). The most specific
/// matching tier wins (see `seed_costs`). Adding a network, currency, funding type, or
/// program is just appending another tier — no code change.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct SeedCostTier {
    /// Card network, e.g. "visa", "mastercard" (case-insensitive equality).
    /// Matches `ClusterKey::card_network`. `None` matches any.
    #[serde(default)]
    pub card_network: Option<String>,
    /// Funding type, "credit" or "debit" (case-insensitive equality).
    /// Matches `ClusterKey::payment_method_type`. `None` matches any.
    #[serde(default)]
    pub payment_method_type: Option<String>,
    /// Card program / tier, e.g. "standard", "premium", "ultra_premium", "commercial"
    /// (case-insensitive equality — exact, so "premium" and "ultra_premium" stay distinct).
    /// Matches `ClusterKey::card_type`. `None` matches any.
    #[serde(default)]
    pub card_type: Option<String>,
    /// Transaction currency, e.g. "USD", "EUR" (case-insensitive equality).
    /// Matches `ClusterKey::transaction_currency`. `None` matches any.
    #[serde(default)]
    pub transaction_currency: Option<String>,
    /// Issuer region bucket — "us", "eu", or "intl" (case-insensitive equality).
    /// Matches `ClusterKey::card_issuing_country` (which `derive_cluster_key` normalizes
    /// from the card's issuer country). This is the dimension that separates the otherwise
    /// indistinguishable USD scenarios at a US merchant — regulated US debit vs EU consumer
    /// vs international — since they share a currency but differ by who issued the card.
    /// `None` matches any.
    #[serde(default)]
    pub card_issuing_country: Option<String>,
    pub pct_bps: f64,
    pub fixed: f64,
}

/// The seed pricing for one PSP: a `default` fee used when no tier matches, plus optional
/// per-(network, program) `tiers`. A blended PSP needs only `default`; an IC++ PSP lists a
/// tier per card type it prices differently.
#[derive(Clone, Debug, serde::Deserialize)]
pub struct SeedCostEntry {
    /// PSP / gateway name (case-insensitive), e.g. "adyen", "stripe".
    pub psp: String,
    pub default: SeedFeeModel,
    #[serde(default)]
    pub tiers: Vec<SeedCostTier>,
}

/// TTL configuration for the in-process memory caches that sit in front of
/// Redis / DB on the routing hot-path.
#[derive(Clone, serde::Deserialize, Debug)]
pub struct MemCacheConfig {
    /// SR score in-process cache TTL in milliseconds (default: 75).
    #[serde(default = "MemCacheConfig::default_sr_score_ttl_ms")]
    pub sr_score_ttl_ms: u64,
    /// Gateway outage list cache TTL in milliseconds (default: 30 000 = 30s).
    #[serde(default = "MemCacheConfig::default_gw_outage_ttl_ms")]
    pub gw_outage_ttl_ms: u64,
    /// Per-merchant payment-flow config cache TTL in milliseconds (default: 60 000 = 60s).
    #[serde(default = "MemCacheConfig::default_payment_flow_ttl_ms")]
    pub payment_flow_ttl_ms: u64,
}

impl MemCacheConfig {
    fn default_sr_score_ttl_ms() -> u64 {
        75
    }
    fn default_gw_outage_ttl_ms() -> u64 {
        30_000
    }
    fn default_payment_flow_ttl_ms() -> u64 {
        60_000
    }
}

impl Default for MemCacheConfig {
    fn default() -> Self {
        Self {
            sr_score_ttl_ms: Self::default_sr_score_ttl_ms(),
            gw_outage_ttl_ms: Self::default_gw_outage_ttl_ms(),
            payment_flow_ttl_ms: Self::default_payment_flow_ttl_ms(),
        }
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

pub use gsm::{GsmConfig, GsmSourceKind};

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
