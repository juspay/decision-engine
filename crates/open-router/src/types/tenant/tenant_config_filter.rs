use serde::{Serialize, Deserialize};
use std::string::String;


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