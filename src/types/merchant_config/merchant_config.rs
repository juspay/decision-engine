use diesel::*;
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use std::fmt::Debug;
use time::PrimitiveDateTime;

use crate::app::get_tenant_app_state;
use crate::error::ApiError;
#[cfg(feature = "mysql")]
use crate::storage::schema::merchant_config::dsl;
#[cfg(feature = "postgres")]
use crate::storage::schema_pg::merchant_config::dsl;
use crate::storage::types::MerchantConfig as DBMerchantConfig;
use crate::types::merchant::id::{merchant_pid_to_text, MerchantPId};
use crate::types::merchant_config::types::{ConfigCategory, ConfigName, ConfigStatus};
use diesel::associations::HasTable;
use diesel::ExpressionMethods;

use super::types::{
    to_config_category, to_config_name, to_config_status, to_merchant_config_pid, MerchantConfigPId,
};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MerchantConfig {
    pub id: MerchantConfigPId,
    pub merchant_account_id: MerchantPId,
    pub config_category: ConfigCategory,
    pub config_name: ConfigName,
    pub status: ConfigStatus,
    pub config_value: Option<String>,
    pub last_updated: PrimitiveDateTime,
}

impl TryFrom<DBMerchantConfig> for MerchantConfig {
    type Error = ApiError;

    fn try_from(db: DBMerchantConfig) -> Result<Self, ApiError> {
        Ok(Self {
            id: to_merchant_config_pid(db.id),
            merchant_account_id: MerchantPId(db.merchant_account_id),
            config_category: to_config_category(&db.config_category)
                .map_err(|_| ApiError::ParsingError("Invalid config category"))?,
            config_name: to_config_name(&db.config_name),
            status: to_config_status(&db.status)
                .map_err(|_| ApiError::ParsingError("Invalid config status"))?,
            config_value: db.config_value,
            last_updated: db.last_updated,
        })
    }
}

/// Get merchant config by merchant account id, category, name and status
pub async fn load_merchant_config_by_mpid_category_name_and_status(
    merchant_account_id: MerchantPId,
    category: String,
    name: String,
    status: String,
) -> Option<MerchantConfig> {
    let app_state = get_tenant_app_state().await;
    let merchant_account_id_c = merchant_pid_to_text(merchant_account_id);
    // Perform database query using Diesel's generic_find_one_optional
    match crate::generics::generic_find_one_optional::<
        <DBMerchantConfig as HasTable>::Table,
        _,
        DBMerchantConfig,
    >(
        &app_state.db,
        dsl::merchant_account_id
            .eq(merchant_account_id_c)
            .and(dsl::config_category.eq(category))
            .and(dsl::config_name.eq(name))
            .and(dsl::status.eq(status)),
    )
    .await
    {
        Ok(Some(db_record)) => MerchantConfig::try_from(db_record).ok(),
        Ok(None) | Err(_) => None, // Silently handle errors or no results by returning None
    }
}

pub async fn load_merchant_config_by_mpid_category_and_name(
    merchant_account_id: MerchantPId,
    category: String,
    name: String,
) -> Option<MerchantConfig> {
    // Perform query using Diesel with generic_find_one
    let app_state = get_tenant_app_state().await;
    match crate::generics::generic_find_one::<
        <DBMerchantConfig as HasTable>::Table,
        _,
        DBMerchantConfig,
    >(
        &app_state.db,
        dsl::merchant_account_id
            .eq(merchant_account_id.0)
            .and(dsl::config_category.eq(category))
            .and(dsl::config_name.eq(name)),
    )
    .await
    {
        Ok(db_record) => {
            MerchantConfig::try_from(db_record).and_then(parse_merchant_config).ok()
        }
        Err(_) => None,
    }
}

/// Get array of merchant configs by merchant account id, category and names
pub async fn load_arr_merchant_config_by_mpid_category_and_name(
    merchant_account_id: MerchantPId,
    category: ConfigCategory,
    names: Vec<ConfigName>,
) -> Vec<MerchantConfig> {
    let app_state = get_tenant_app_state().await;
    let name_strings: Vec<String> = names
        .into_iter()
        .map(|name| name.as_str().to_string())
        .collect();
    let merchant_account_id_c = merchant_pid_to_text(merchant_account_id);

    match crate::generics::generic_find_all::<
        <DBMerchantConfig as HasTable>::Table,
        _,
        DBMerchantConfig,
    >(
        &app_state.db,
        dsl::merchant_account_id
            .eq(merchant_account_id_c)
            .and(dsl::config_category.eq(category.to_string()))
            .and(dsl::config_name.eq_any(name_strings)),
    )
    .await
    {
        Ok(db_results) => db_results
            .into_iter()
            .filter_map(|db_merchant_config| MerchantConfig::try_from(db_merchant_config).ok())
            .collect(),
        Err(_) => vec![],
    }
}

pub fn parse_merchant_config(db_record: MerchantConfig) -> Result<MerchantConfig, ApiError> {
    Ok(db_record)
}
