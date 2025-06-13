use serde::{Deserialize, Serialize};
// use db::eulermeshimpl::mesh_config;
// use db::mesh::internal::*;
use crate::error::ApiError;
// use eulerhs::language::MonadFlow;
use crate::app::get_tenant_app_state;
use crate::storage::types::TenantConfig as DBTenantConfig;
// use types::utils::dbconfig::get_euler_db_conf;
// use eulerhs::extra::combinators::to_domain_all;
// use juspay::extra::parsing::{Parsed, Step, parse_field, around, project};
// use sequelize::{Clause::Is, Term::{Eq, In}};
// use ghc::generics::Generic;
// use ghc::typelits::KnownSymbol;
// use named::*;
#[cfg(feature = "mysql")]
use crate::storage::schema::tenant_config::dsl;
#[cfg(feature = "postgres")]
use crate::storage::schema_pg::tenant_config::dsl;
use diesel::associations::HasTable;
use diesel::*;
use std::option::Option;
use std::string::String;
use std::vec::Vec;
// use test::quickcheck::{Arbitrary, arbitrary};
// use test::quickcheck::arbitrary::generic::generic_arbitrary;
// use control::category::*;
// use types::tenant::tenantconfig::*;
// use crate::types::tenant;
use crate::types::country::country_iso::{text_db_to_country_iso, CountryISO};
use crate::types::tenant::tenant_config::{
    text_to_config_status, text_to_config_type, text_to_filter_dimension, text_to_module_name,
    text_to_tenant_config_id, ConfigStatus, ConfigType, FilterDimension, ModuleName,
    TenantConfigId,
};

use super::country::country_iso::country_iso_to_text;
use super::tenant::tenant_config::{
    config_status_to_text, config_type_to_text, module_name_to_text,
};

// use super::country::country_iso::text_db_to_country_iso;
// use super::tenant::tenant_config::{text_to_config_status, text_to_config_type, text_to_filter_dimension, text_to_module_name, text_to_tenant_config_id};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TenantConfig {
    #[serde(rename = "id")]
    pub id: TenantConfigId,
    #[serde(rename = "_type")]
    pub _type: ConfigType,
    #[serde(rename = "moduleKey")]
    pub moduleKey: String,
    #[serde(rename = "moduleName")]
    pub moduleName: ModuleName,
    #[serde(rename = "tenantAccountId")]
    pub tenantAccountId: String,
    #[serde(rename = "configValue")]
    pub configValue: String,
    #[serde(rename = "filterDimension")]
    pub filterDimension: Option<FilterDimension>,
    #[serde(rename = "filterGroupId")]
    pub filterGroupId: Option<String>,
    #[serde(rename = "status")]
    pub status: ConfigStatus,
    #[serde(rename = "countryCodeAlpha3")]
    pub countryCodeAlpha3: Option<CountryISO>,
}

impl TryFrom<DBTenantConfig> for TenantConfig {
    type Error = ApiError;

    fn try_from(db_tenant_config: DBTenantConfig) -> Result<Self, ApiError> {
        Ok(Self {
            id: text_to_tenant_config_id(db_tenant_config.id),
            _type: text_to_config_type(db_tenant_config._type)
                .map_err(|_| ApiError::ParsingError("Invalid Config Type"))?,
            moduleKey: db_tenant_config.module_key,
            moduleName: text_to_module_name(db_tenant_config.module_name)
                .map_err(|_| ApiError::ParsingError("Invalid Module Name"))?,
            tenantAccountId: db_tenant_config.tenant_account_id,
            configValue: db_tenant_config.config_value,
            filterDimension: db_tenant_config
                .filter_dimension
                .map(text_to_filter_dimension)
                .transpose()?,
            filterGroupId: db_tenant_config.filter_group_id,
            status: text_to_config_status(db_tenant_config.status)
                .map_err(|_| ApiError::ParsingError("Invalid Config Status"))?,
            countryCodeAlpha3: db_tenant_config
                .country_code_alpha3
                .map(|code| text_db_to_country_iso(code.as_str()))
                .transpose()?,
        })
    }
}

// #TOD implement db calls (only 1st function is needed)

// pub async fn get_tenant_config_by_tenant_id_and_module_name_and_module_key_and_type(
//     t_id: String,
//     m_name: ModuleName,
//     m_key: String,
//     config_type: ConfigType,
// ) -> Option<TenantConfig> {
//     let db_conf = get_euler_db_conf::<DB::TenantConfigT>().await;
//     let res = find_one_row(
//         db_conf,
//         mesh_config,
//         vec![
//             Is(DB::tenant_account_id, Eq(t_id)),
//             Is(DB::module_name, Eq(module_name_to_text(m_name))),
//             Is(DB::module_key, Eq(m_key)),
//             Is(DB::_type, Eq(config_type_to_text(config_type))),
//             Is(DB::status, Eq(config_status_to_text(ConfigStatus::ACTIVE))),
//         ],
//     )
//     .await;
//     to_domain_all(
//         res,
//         parse_tenant_config,
//         named!("#function_name", "getTenantConfigByTenantIdAndModuleNameAndModuleKeyAndType"),
//         named!("#parser_name", "parseTenantConfig"),
//     )
//     .await
// }

pub async fn get_arr_active_tenant_config_by_tenant_id_module_name_module_key_and_arr_type(
    t_id: String,
    m_name: ModuleName,
    m_key: String,
    arr_config_type: Vec<ConfigType>,
) -> Vec<TenantConfig> {
    // Convert ModuleName and ConfigType to strings for database query
    let app_state = get_tenant_app_state().await;
    let module_name_str = module_name_to_text(&m_name);
    let config_type_strs: Vec<String> = arr_config_type.iter().map(config_type_to_text).collect();
    let active_status_str = config_status_to_text(&ConfigStatus::ACTIVE);

    // Use Diesel's query builder with multiple conditions
    match crate::generics::generic_find_all::<
        <DBTenantConfig as HasTable>::Table,
        _,
        DBTenantConfig
    >(
        &app_state.db,
        dsl::tenant_account_id.eq(t_id)
            .and(dsl::module_name.eq(module_name_str))
            .and(dsl::module_key.eq(m_key))
            .and(dsl::tenant_type.eq_any(config_type_strs))
            .and(dsl::status.eq(active_status_str)),
    ).await {
        Ok(db_results) => db_results.into_iter()
                                    .filter_map(|db_record| TenantConfig::try_from(db_record).ok())
                                    .collect(),
        Err(_) => Vec::new(), // Silently handle any errors by returning an empty vec
    }
}

pub async fn get_arr_active_tenant_config_by_tenant_id_module_name_module_key_and_arr_type_and_country(
    t_id: String,
    m_name: ModuleName,
    m_key: String,
    arr_config_type: Vec<ConfigType>,
    country_code: CountryISO,
) -> Vec<TenantConfig> {
    // Convert input types to string for database query
    let app_state = get_tenant_app_state().await;
    let module_name_str = module_name_to_text(&m_name);
    let config_type_strs: Vec<String> = arr_config_type.iter().map(config_type_to_text).collect();
    let country_code_str = country_iso_to_text(country_code);

    // Perform query using Diesel's generic_find_all
    match crate::generics::generic_find_all::<
        <DBTenantConfig as HasTable>::Table,
        _,
        DBTenantConfig
    >(
        &app_state.db,
        dsl::tenant_account_id.eq(t_id)
            .and(dsl::module_name.eq(module_name_str))
            .and(dsl::module_key.eq(m_key))
            .and(dsl::tenant_type.eq_any(config_type_strs))
            .and(dsl::status.eq(config_status_to_text(&ConfigStatus::ACTIVE)))
            .and(dsl::country_code_alpha3.eq(Some(country_code_str))),
    ).await {
        Ok(db_results) => db_results
            .into_iter()
            .filter_map(|db_record| TenantConfig::try_from(db_record).ok())
            .collect(),
        Err(_) => Vec::new(), // Silently handle any errors by returning empty vec
    }
}

pub async fn get_tenant_config_by_tenant_id_and_module_name_and_module_key_and_type(
    t_id: String,
    m_name: ModuleName,
    m_key: String,
    config_type: ConfigType,
) -> Option<TenantConfig> {
    // Convert to string representations for query
    let module_name_str = module_name_to_text(&m_name);
    let config_type_str = config_type_to_text(&config_type);
    let config_status_str = config_status_to_text(&ConfigStatus::ACTIVE);
    let app_state = get_tenant_app_state().await;
    // Use Diesel's query builder for querying the database
    match crate::generics::generic_find_one_optional::<
        <DBTenantConfig as HasTable>::Table,
        _,
        DBTenantConfig,
    >(
        &app_state.db,
        dsl::tenant_account_id
            .eq(t_id)
            .and(dsl::module_name.eq(module_name_str))
            .and(dsl::module_key.eq(m_key))
            .and(dsl::tenant_type.eq(config_type_str))
            .and(dsl::status.eq(config_status_str)),
    )
    .await
    {
        Ok(Some(db_tenant_config)) => TenantConfig::try_from(db_tenant_config).ok(),
        _ => None, // Silently return None on error or no result
    }
}
