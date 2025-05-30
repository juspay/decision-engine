use diesel::associations::HasTable;
use serde::{Serialize, Deserialize};
use std::string::String;
use crate::{app::get_tenant_app_state, error::ApiError, storage::types::TenantConfigFilter as DBTenantConfigFilter};
use diesel::*;
#[cfg(feature = "mysql")]
use crate::storage::schema::tenant_config_filter::{dsl, filter_group_id, dimension_value};
#[cfg(feature = "postgres")]
use crate::storage::schema_pg::tenant_config_filter::{dsl, filter_group_id, dimension_value};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TenantConfigFilterId {
    pub tenantConfigFilterId: String,
}

pub fn text_to_tenant_config_filter_id(tenant_config_filter_id: String) -> TenantConfigFilterId {
    TenantConfigFilterId {
        tenantConfigFilterId: tenant_config_filter_id,
    }
}

pub fn tenant_config_filter_id_to_text(id: TenantConfigFilterId) -> String {
    id.tenantConfigFilterId
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TenantConfigFilter {
    #[serde(rename = "id")]
    pub id: TenantConfigFilterId,
    #[serde(rename = "filterGroupId")]
    pub filterGroupId: String,
    #[serde(rename = "dimensionValue")]
    pub dimensionValue: String,
    #[serde(rename = "configValue")]
    pub configValue: String,
    #[serde(rename = "tenantConfigId")]
    pub tenantConfigId: String
}


impl TryFrom<DBTenantConfigFilter> for TenantConfigFilter {
    type Error = ApiError;

    fn try_from(db_tenant_config_filter: DBTenantConfigFilter) -> Result<Self, ApiError> {
        Ok(TenantConfigFilter {
            id: text_to_tenant_config_filter_id(db_tenant_config_filter.id),
            filterGroupId: db_tenant_config_filter.filter_group_id,
            dimensionValue: db_tenant_config_filter.dimension_value,
            configValue: db_tenant_config_filter.config_value,
            tenantConfigId: db_tenant_config_filter.tenant_config_id
        })
    }
}


pub async fn get_tenant_config_filter_by_group_id_and_dimension_value(
    group_id: String,
    dimension_valu: String,
) -> Option<TenantConfigFilter> {
    let app_state = get_tenant_app_state().await;
    
    // Use Diesel's query builder for querying the database
    match crate::generics::generic_find_one_optional::<
        <DBTenantConfigFilter as HasTable>::Table,
        _,
        DBTenantConfigFilter
    >(
        &app_state.db,
        dsl::filter_group_id.eq(group_id)
            .and(dsl::dimension_value.eq(dimension_valu)),
    )
    .await {
        Ok(Some(db_tenant_config)) => TenantConfigFilter::try_from(db_tenant_config).ok(),
        _ => None,
    }
}
