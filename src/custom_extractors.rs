use std::sync::Arc;

use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
    Json,
};

use crate::{
    app::TenantAppState,
    auth::AuthContext,
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

#[derive(Debug, Clone)]
pub struct AuthenticatedAnalyticsContext(pub AuthContext);

#[async_trait]
impl<S> FromRequestParts<S> for AuthenticatedAnalyticsContext
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let auth_context = parts
            .extensions
            .get::<AuthContext>()
            .cloned()
            .ok_or_else(|| {
                (
                    StatusCode::UNAUTHORIZED,
                    Json(crate::error::ApiErrorResponse::new(
                        crate::error::error_codes::TE_03,
                        "Analytics access requires JWT or API key authentication".to_string(),
                        None,
                    )),
                )
                    .into_response()
            })?;

        Ok(Self(auth_context))
    }
}

#[cfg(test)]
mod tests {
    use super::AuthenticatedAnalyticsContext;
    use crate::auth::{AuthContext, AuthKind};
    use axum::{extract::FromRequestParts, http::Request};

    #[tokio::test]
    async fn analytics_context_returns_unauthorized_when_missing() {
        let (mut parts, _) = Request::builder()
            .uri("/analytics/overview")
            .body(())
            .unwrap()
            .into_parts();
        let response = AuthenticatedAnalyticsContext::from_request_parts(&mut parts, &())
            .await
            .unwrap_err();

        assert_eq!(response.status(), axum::http::StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn analytics_context_extracts_inserted_auth_context() {
        let expected = AuthContext {
            merchant_id: "m_123".to_string(),
            auth_kind: AuthKind::Jwt,
            user_id: Some("user_123".to_string()),
            email: Some("user@example.com".to_string()),
            role: Some("admin".to_string()),
        };
        let (mut parts, _) = Request::builder()
            .uri("/analytics/overview")
            .body(())
            .unwrap()
            .into_parts();
        parts.extensions.insert(expected.clone());

        let AuthenticatedAnalyticsContext(actual) =
            AuthenticatedAnalyticsContext::from_request_parts(&mut parts, &())
                .await
                .unwrap();

        assert_eq!(actual, expected);
    }
}
