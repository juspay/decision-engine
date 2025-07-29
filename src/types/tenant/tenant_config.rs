use serde::{Deserialize, Serialize};
use std::string::String;

use crate::error::ApiError;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct TenantConfigId {
    #[serde(rename = "tenantConfigId")]
    pub tenantConfigId: String,
}

pub fn text_to_tenant_config_id(tenant_config_id: String) -> TenantConfigId {
    TenantConfigId {
        tenantConfigId: tenant_config_id,
    }
}

pub fn tenant_config_id_to_text(id: TenantConfigId) -> String {
    id.tenantConfigId
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum ModuleName {
    #[serde(rename = "MERCHANT_ACCOUNT")]
    MERCHANT_ACCOUNT,
    #[serde(rename = "SURCHARGE_LOGIC")]
    SURCHARGE_LOGIC,
    #[serde(rename = "PRIORITY_LOGIC")]
    PRIORITY_LOGIC,
    #[serde(rename = "MERCHANT_CONFIG")]
    MERCHANT_CONFIG,
    #[serde(rename = "TENANT_FEATURE")]
    TENANT_FEATURE,
    #[serde(rename = "TENANT_ACCOUNT")]
    TENANT_ACCOUNT,
}

pub fn module_name_to_text(module_name: &ModuleName) -> String {
    match module_name {
        ModuleName::MERCHANT_ACCOUNT => "MERCHANT_ACCOUNT".to_string(),
        ModuleName::SURCHARGE_LOGIC => "SURCHARGE_LOGIC".to_string(),
        ModuleName::PRIORITY_LOGIC => "PRIORITY_LOGIC".to_string(),
        ModuleName::MERCHANT_CONFIG => "MERCHANT_CONFIG".to_string(),
        ModuleName::TENANT_FEATURE => "TENANT_FEATURE".to_string(),
        ModuleName::TENANT_ACCOUNT => "TENANT_ACCOUNT".to_string(),
    }
}

pub fn text_to_module_name(module_name: String) -> Result<ModuleName, ApiError> {
    match module_name.as_str() {
        "MERCHANT_ACCOUNT" => Ok(ModuleName::MERCHANT_ACCOUNT),
        "SURCHARGE_LOGIC" => Ok(ModuleName::SURCHARGE_LOGIC),
        "PRIORITY_LOGIC" => Ok(ModuleName::PRIORITY_LOGIC),
        "MERCHANT_CONFIG" => Ok(ModuleName::MERCHANT_CONFIG),
        "TENANT_FEATURE" => Ok(ModuleName::TENANT_FEATURE),
        "TENANT_ACCOUNT" => Ok(ModuleName::TENANT_ACCOUNT),
        _ => Err(ApiError::ParsingError("Invalid Module Name")),
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum ConfigType {
    #[serde(rename = "DEFAULT")]
    DEFAULT,
    #[serde(rename = "OVERRIDE")]
    OVERRIDE,
    #[serde(rename = "FALLBACK")]
    FALLBACK,
}

pub fn config_type_to_text(config_type: &ConfigType) -> String {
    match config_type {
        ConfigType::DEFAULT => "DEFAULT".to_string(),
        ConfigType::OVERRIDE => "OVERRIDE".to_string(),
        ConfigType::FALLBACK => "FALLBACK".to_string(),
    }
}

pub fn text_to_config_type(config_type: String) -> Result<ConfigType, ApiError> {
    match config_type.as_str() {
        "DEFAULT" => Ok(ConfigType::DEFAULT),
        "OVERRIDE" => Ok(ConfigType::OVERRIDE),
        "FALLBACK" => Ok(ConfigType::FALLBACK),
        _ => Err(ApiError::ParsingError("Invalid Config Type")),
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum FilterDimension {
    #[serde(rename = "MCC")]
    MCC,
    #[serde(rename = "TIER")]
    TIER,
    #[serde(rename = "TRACK")]
    TRACK,
}

pub fn filter_dimension_to_text(filter_dimension: &FilterDimension) -> String {
    match filter_dimension {
        FilterDimension::MCC => "MCC".to_string(),
        FilterDimension::TIER => "TIER".to_string(),
        FilterDimension::TRACK => "TRACK".to_string(),
    }
}

pub fn text_to_filter_dimension(filter_dimension: String) -> Result<FilterDimension, ApiError> {
    match filter_dimension.as_str() {
        "MCC" => Ok(FilterDimension::MCC),
        "TIER" => Ok(FilterDimension::TIER),
        "TRACK" => Ok(FilterDimension::TRACK),
        _ => Err(ApiError::ParsingError("Invalid Filter Dimension")),
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum TenantConfigStatus {
    #[serde(rename = "ENABLED")]
    ENABLED,
    #[serde(rename = "DISABLED")]
    DISABLED,
}

#[derive(Debug, Deserialize)]
pub struct TenantConfigValueType<T> {
    pub status: TenantConfigStatus,
    #[serde(rename = "configValue")]
    pub config_value: Option<T>,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub enum ConfigStatus {
    #[serde(rename = "ACTIVE")]
    ACTIVE,
    #[serde(rename = "INACTIVE")]
    INACTIVE,
}

pub fn config_status_to_text(config_status: &ConfigStatus) -> String {
    match config_status {
        ConfigStatus::ACTIVE => "ACTIVE".to_string(),
        ConfigStatus::INACTIVE => "INACTIVE".to_string(),
    }
}

pub fn text_to_config_status(config_status: String) -> Result<ConfigStatus, ApiError> {
    match config_status.as_str() {
        "ACTIVE" => Ok(ConfigStatus::ACTIVE),
        "INACTIVE" => Ok(ConfigStatus::INACTIVE),
        _ => Err(ApiError::ParsingError("Invalid Config Status")),
    }
}
