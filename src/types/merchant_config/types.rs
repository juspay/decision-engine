use crate::error::ApiError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::{Debug, Display};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MerchantConfigPId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConfigName(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConfigCategory {
    PaymentFlow,
    GeneralConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ConfigStatus {
    ENABLED,
    DISABLED,
}

pub struct TenantConfigValueType {
    pub status: ConfigStatus,
    pub configValue: Option<Box<TenantConfigValueType>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FeatureConf {
    pub enableAll: bool,
    pub enableAllRollout: Option<i32>,
    pub disableAny: Option<Vec<String>>,
    pub merchants: Option<Vec<MerchantFeature>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MerchantFeature {
    pub merchantId: String,
    pub rollout: i32,
}

pub type PfMcConfig = HashMap<String, FeatureConf>;

// Conversion functions
pub fn to_merchant_config_pid(id: String) -> MerchantConfigPId {
    MerchantConfigPId(id)
}

pub fn config_category_to_text(category: ConfigCategory) -> String {
    match category {
        ConfigCategory::PaymentFlow => "PAYMENT_FLOW".to_string(),
        ConfigCategory::GeneralConfig => "GENERAL_CONFIG".to_string(),
    }
}

pub fn to_config_name(name: &str) -> ConfigName {
    ConfigName(name.to_string())
}

pub fn to_config_category(category: &str) -> Result<ConfigCategory, ApiError> {
    match category {
        "PAYMENT_FLOW" => Ok(ConfigCategory::PaymentFlow),
        "GENERAL_CONFIG" => Ok(ConfigCategory::GeneralConfig),
        _ => Err(ApiError::ParsingError("Invalid ConfigCategory")),
    }
}

pub fn to_config_status(status: &str) -> Result<ConfigStatus, ApiError> {
    match status {
        "ENABLED" => Ok(ConfigStatus::ENABLED),
        "DISABLED" => Ok(ConfigStatus::DISABLED),
        _ => Err(ApiError::ParsingError("Invalid ConfigStatus")),
    }
}

pub fn config_status_to_text(status: ConfigStatus) -> String {
    match status {
        ConfigStatus::ENABLED => "ENABLED".to_string(),
        ConfigStatus::DISABLED => "DISABLED".to_string(),
    }
}

// Implement Display for string representation
impl Display for ConfigCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::PaymentFlow => write!(f, "PAYMENT_FLOW"),
            Self::GeneralConfig => write!(f, "GENERAL_CONFIG"),
        }
    }
}

impl Display for ConfigStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ENABLED => write!(f, "ENABLED"),
            Self::DISABLED => write!(f, "DISABLED"),
        }
    }
}

// Methods for ConfigName
impl ConfigName {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
