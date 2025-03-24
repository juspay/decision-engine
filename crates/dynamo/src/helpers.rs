use std::fmt::Display;

use crate::consts;
use error_stack::{Report, ResultExt};
use tonic::Request;

use crate::error::GrpcRequestError;

#[track_caller]
pub fn get_tenant_id_from_request<T>(
    request: &Request<T>,
    is_multi_tenancy_enabled: bool,
) -> Result<Option<String>, Report<GrpcRequestError>> {
    let tenant_id = if is_multi_tenancy_enabled {
        // TODO: handle missing x-tenant-id in header if multi-tenancy is enabled
        request
            .metadata()
            .get(consts::X_TENANT_ID)
            .map(|tenant_id| tenant_id.to_str())
            .transpose()
            .change_context(GrpcRequestError::InvalidHeader(consts::X_TENANT_ID))?
            .map(ToString::to_string)
    } else {
        None
    };

    Ok(tenant_id)
}

pub fn redis_key_create_with_suffix<I, P, L>(
    routing_type_prefix: &str,
    tenant_id: &Option<String>,
    id: &I,
    params: &P,
    label: &L,
    suffix: &str,
) -> String
where
    I: Display,
    P: Display,
    L: Display,
{
    let mut key = routing_type_prefix.to_string();
    let mut id = id.to_string();

    if let Some(tenant_id) = tenant_id {
        key.push(':');
        key.push_str(tenant_id);
        let possible_ids = id.split(':').collect::<Vec<_>>();
        let tenant_id = possible_ids.first();
        let actual_id = possible_ids.get(1);
        id = actual_id
            .or(tenant_id)
            .map(ToString::to_string)
            .unwrap_or_default()
    }

    format!("{}:{}:{}:{}:{}", key, id, params, label, suffix)
}

pub fn redis_key_create_without_suffix<I, P, L>(
    routing_type_prefix: &str,
    tenant_id: &Option<String>,
    id: &I,
    params: &P,
    label: &L,
) -> String
where
    I: Display,
    P: Display,
    L: Display,
{
    let mut key = routing_type_prefix.to_string();
    let mut id = id.to_string();

    if let Some(tenant_id) = tenant_id {
        key.push(':');
        key.push_str(tenant_id);
        let possible_ids = id.split(':').collect::<Vec<_>>();
        let tenant_id = possible_ids.first();
        let actual_id = possible_ids.get(1);
        id = actual_id
            .or(tenant_id)
            .map(ToString::to_string)
            .unwrap_or_default()
    }

    format!("{}:{}:{}:{}", key, id, params, label)
}

pub fn redis_key_create_for_metrics_invalidation<I>(
    routing_type_prefix: &str,
    tenant_id: &Option<String>,
    id: &I,
) -> String
where
    I: Display,
{
    let mut key = routing_type_prefix.to_string();
    let mut id = id.to_string();

    if let Some(tenant_id) = tenant_id {
        key.push(':');
        key.push_str(tenant_id);
        let possible_ids = id.split(':').collect::<Vec<_>>();
        let tenant_id = possible_ids.first();
        let actual_id = possible_ids.get(1);
        id = actual_id
            .or(tenant_id)
            .map(ToString::to_string)
            .unwrap_or_default()
    }

    format!("{}:{}", key, id)
}
