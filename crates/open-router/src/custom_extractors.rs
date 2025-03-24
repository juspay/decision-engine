use std::sync::Arc;

use axum::{async_trait, extract::FromRequestParts, http::request::Parts};

use crate::{
    app::TenantAppState,
    error::{ApiError, ContainerError},
    storage::consts,
    tenant::GlobalAppState,
};

#[derive(Clone)]
pub struct TenantStateResolver(pub Arc<TenantAppState>);

#[async_trait]
impl FromRequestParts<Arc<GlobalAppState>> for TenantStateResolver {
    type Rejection = ContainerError<ApiError>;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<GlobalAppState>,
    ) -> Result<Self, Self::Rejection> {
        let tenant_id = parts
            .headers
            .get(consts::X_TENANT_ID)
            .and_then(|h| h.to_str().ok())
            .ok_or(ApiError::TenantError("x-tenant-id not found in headers"))?;

        state.is_known_tenant(tenant_id)?;
        Ok(Self(state.get_app_state_of_tenant(tenant_id).await?))
    }
}
