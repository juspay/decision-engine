use crate::logger;
use std::sync::Arc;

use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
    response::{IntoResponse, Response},
};

use crate::{
    app::TenantAppState,
    auth::AuthContext,
    error::{ApiError, ContainerError},
    storage::consts,
    tenant::GlobalAppState,
};

/// JSON extraction with one diagnostic event for framework rejections.
/// Axum parses the body; rejections use the Decision Engine validation response.
#[derive(Debug, Clone, Copy, Default)]
pub struct Json<T>(pub T);

#[async_trait]
impl<S, T> axum::extract::FromRequest<S> for Json<T>
where
    S: Send + Sync,
    T: serde::de::DeserializeOwned,
{
    type Rejection = Response;

    async fn from_request(
        request: axum::extract::Request,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        match axum::Json::<T>::from_request(request, state).await {
            Ok(axum::Json(payload)) => Ok(Self(payload)),
            Err(rejection) => {
                let message = rejection.body_text();
                logger::error!(
                    error = ?rejection,
                    status_code = StatusCode::BAD_REQUEST.as_u16(),
                    error_code = crate::error::error_codes::TE_03,
                    error_message = %message,
                    "Request JSON extraction failed"
                );
                Err((
                    StatusCode::BAD_REQUEST,
                    crate::error::ApiErrorResponse::new(
                        crate::error::error_codes::TE_03,
                        message,
                        None,
                    ),
                )
                    .into_response())
            }
        }
    }
}

impl<T: serde::Serialize> IntoResponse for Json<T> {
    fn into_response(self) -> Response {
        axum::Json(self.0).into_response()
    }
}

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
                logger::error!(
                    error_message = "Analytics access requires JWT or API key authentication",
                    error_code = crate::error::error_codes::TE_03,
                    "Analytics authentication failed"
                );
                (
                    StatusCode::UNAUTHORIZED,
                    crate::error::ApiErrorResponse::new(
                        crate::error::error_codes::TE_03,
                        "Analytics access requires JWT or API key authentication".to_string(),
                        None,
                    ),
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

    #[derive(Debug, serde::Deserialize, serde::Serialize, PartialEq)]
    struct TestPayload {
        count: u32,
    }

    #[derive(Clone)]
    struct ErrorCounter(std::sync::Arc<std::sync::atomic::AtomicUsize>);

    impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for ErrorCounter {
        fn on_event(
            &self,
            event: &tracing::Event<'_>,
            _context: tracing_subscriber::layer::Context<'_, S>,
        ) {
            if *event.metadata().level() == tracing::Level::ERROR {
                self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
        }
    }

    #[tokio::test]
    async fn json_rejections_return_invalid_request_and_log_once() {
        use axum::{body::Body, extract::FromRequest};
        use tracing::instrument::WithSubscriber;
        use tracing_subscriber::prelude::*;

        for (content_type, body) in [
            (Some("application/json"), r#"{"count": 1,}"#.to_string()),
            (
                Some("application/json"),
                r#"{"count": "invalid"}"#.to_string(),
            ),
            (None, r#"{"count": 1}"#.to_string()),
            (Some("text/plain"), r#"{"count": 1}"#.to_string()),
            (Some("application/json"), " ".repeat(2 * 1024 * 1024 + 1)),
        ] {
            let request = || {
                let mut req = Request::new(Body::from(body.clone()));
                if let Some(content_type) = content_type {
                    req.headers_mut().insert(
                        axum::http::header::CONTENT_TYPE,
                        axum::http::HeaderValue::from_static(content_type),
                    );
                }
                req
            };
            let errors = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
            let subscriber = tracing_subscriber::registry().with(ErrorCounter(errors.clone()));
            let actual = super::Json::<TestPayload>::from_request(request(), &())
                .with_subscriber(subscriber)
                .await
                .unwrap_err();
            let expected = axum::Json::<TestPayload>::from_request(request(), &())
                .await
                .unwrap_err();
            assert_eq!(errors.load(std::sync::atomic::Ordering::Relaxed), 1);
            assert_eq!(actual.status(), axum::http::StatusCode::BAD_REQUEST);
            assert_eq!(actual.headers()["content-type"], "application/json");
            let body = axum::body::to_bytes(actual.into_body(), usize::MAX)
                .await
                .unwrap();
            let actual: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(
                actual,
                serde_json::json!({
                    "code": crate::error::error_codes::TE_03,
                    "message": expected.body_text(),
                    "data": null,
                })
            );
        }
    }

    #[tokio::test]
    async fn valid_json_and_response_serialization_match_axum() {
        use axum::{body::Body, extract::FromRequest, response::IntoResponse};
        use tracing::instrument::WithSubscriber;
        use tracing_subscriber::prelude::*;

        let errors = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let subscriber = tracing_subscriber::registry().with(ErrorCounter(errors.clone()));
        let request = Request::builder()
            .header("content-type", "application/json")
            .body(Body::from(r#"{"count": 7}"#))
            .unwrap();
        let actual = super::Json::<TestPayload>::from_request(request, &())
            .with_subscriber(subscriber)
            .await
            .unwrap();
        assert_eq!(actual.0, TestPayload { count: 7 });
        assert_eq!(errors.load(std::sync::atomic::Ordering::Relaxed), 0);
        let actual = actual.into_response();
        let expected = axum::Json(TestPayload { count: 7 }).into_response();
        assert_eq!(actual.status(), expected.status());
        assert_eq!(actual.headers(), expected.headers());
        assert_eq!(
            axum::body::to_bytes(actual.into_body(), usize::MAX)
                .await
                .unwrap(),
            axum::body::to_bytes(expected.into_body(), usize::MAX)
                .await
                .unwrap()
        );
    }

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
            permissions: None,
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
