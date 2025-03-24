use crate::success_rate::proto::types::{
    CalSuccessRateConfig, CurrentBlockThreshold as ProtoCurrentBlockThreshold,
    UpdateSuccessRateWindowConfig,
};

#[derive(thiserror::Error, Debug)]
pub enum ServerError {
    #[error("config error")]
    ConfigError,
    #[error("server error")]
    ServerError,
    #[error("io error")]
    IoError(#[from] std::io::Error),
    #[error("stopping server")]
    StoppingServer,
    #[error("error while decoding secrets")]
    SecretsError,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct ApiKeyInformation {
    pub tenant_id: String,
    pub merchant_id: String,
    pub key_id: String,
    pub expires_at: Option<time::PrimitiveDateTime>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct SuccessBasedRoutingConfig {
    pub params: Option<Vec<DynamicRoutingConfigParams>>,
    pub config: Option<SuccessBasedRoutingConfigBody>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, strum::Display)]
pub enum DynamicRoutingConfigParams {
    PaymentMethod,
    PaymentMethodType,
    AuthenticationType,
    Currency,
    Country,
    CardNetwork,
    CardBin,
}
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct SuccessBasedRoutingConfigBody {
    pub min_aggregates_size: Option<u32>,
    pub default_success_rate: Option<f64>,
    pub max_aggregates_size: Option<u32>,
    pub current_block_threshold: Option<CurrentBlockThreshold>,
    #[serde(default)]
    pub specificity_level: SuccessRateSpecificityLevel,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, Default)]
pub struct CurrentBlockThreshold {
    pub duration_in_mins: Option<u64>,
    pub max_total_count: Option<u64>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Default, Clone)]
#[serde(rename_all = "snake_case")]
pub enum SuccessRateSpecificityLevel {
    #[default]
    Merchant,
    Global,
}

impl From<SuccessRateSpecificityLevel> for i32 {
    fn from(level: SuccessRateSpecificityLevel) -> Self {
        match level {
            SuccessRateSpecificityLevel::Merchant => 0,
            SuccessRateSpecificityLevel::Global => 1,
        }
    }
}

impl From<SuccessBasedRoutingConfigBody> for CalSuccessRateConfig {
    fn from(value: SuccessBasedRoutingConfigBody) -> Self {
        Self {
            min_aggregates_size: value.min_aggregates_size.unwrap_or_default(),
            default_success_rate: value.default_success_rate.unwrap_or_default(),
            specificity_level: Some(value.specificity_level.into()),
        }
    }
}

impl From<SuccessBasedRoutingConfigBody> for UpdateSuccessRateWindowConfig {
    fn from(value: SuccessBasedRoutingConfigBody) -> Self {
        Self {
            max_aggregates_size: value.max_aggregates_size.unwrap_or_default(),
            current_block_threshold: Some(ProtoCurrentBlockThreshold::from(
                value.current_block_threshold.unwrap_or_default(),
            )),
        }
    }
}

impl From<CurrentBlockThreshold> for ProtoCurrentBlockThreshold {
    fn from(value: CurrentBlockThreshold) -> Self {
        Self {
            max_total_count: value.max_total_count.unwrap_or_default(),
            duration_in_mins: value.duration_in_mins,
        }
    }
}

pub struct Secret<T> {
    pub value: T,
}

impl<T> From<T> for Secret<T> {
    fn from(value: T) -> Self {
        Self { value }
    }
}

impl<T> std::fmt::Debug for Secret<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "***")
    }
}

impl<T> std::ops::Deref for Secret<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}
