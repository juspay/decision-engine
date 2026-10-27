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
            Err(rejection) => Err(invalid_json_response(rejection.body_text())),
        }
    }
}

/// The standard answer to a body that could not be read or parsed, logged once.
fn invalid_json_response(message: String) -> Response {
    crate::logger::error!(
        status_code = StatusCode::BAD_REQUEST.as_u16(),
        error_code = crate::error::error_codes::TE_03,
        error_message = %message,
        "Request JSON extraction failed"
    );
    (
        StatusCode::BAD_REQUEST,
        crate::error::ApiErrorResponse::new(crate::error::error_codes::TE_03, message, None),
    )
        .into_response()
}

impl<T: serde::Serialize> IntoResponse for Json<T> {
    fn into_response(self) -> Response {
        axum::Json(self.0).into_response()
    }
}

/// The request body of a route that names its merchant in the body, parsed as `T` and authorized
/// with [`crate::auth::authorize`] before the handler runs: a per-merchant session naming another
/// merchant is refused with `403`, and a service credential acts for the merchant the body names.
/// `T` derives [`crate::auth::MerchantScoped`], marking each field that names a merchant.
///
/// Failures answer in the standard invalid-request format, as [`Json`] does. A handler with its
/// own error format (the decider endpoints, `routing/create`) takes
/// `Result<ScopedJson<T>, ScopedJsonRejection>` and answers each [`ScopedJsonRejection`] itself.
#[derive(Debug, Clone)]
pub struct ScopedJson<T> {
    pub merchant: crate::auth::AuthorizedMerchant,
    pub payload: T,
    /// The body exactly as received, for request logs and analytics.
    pub body: axum::body::Bytes,
}

/// Why a [`ScopedJson`] could not be extracted.
#[derive(Debug)]
pub enum ScopedJsonRejection {
    /// The body could not be read, or is over the route's body limit.
    Read(crate::routes::body::RequestBodyError),
    /// The body is not a valid `T`. The message names the JSON path of the field that failed.
    Parse {
        body: axum::body::Bytes,
        error: axum::extract::rejection::JsonRejection,
    },
    /// The body parsed but names a merchant the caller may not act on.
    Scope(crate::auth::MerchantScopeViolation),
}

impl IntoResponse for ScopedJsonRejection {
    fn into_response(self) -> Response {
        match self {
            Self::Read(error) => invalid_json_response(error.to_string()),
            Self::Parse { error, .. } => invalid_json_response(error.body_text()),
            Self::Scope(violation) => violation.into_response(),
        }
    }
}

#[async_trait]
impl<S, T> axum::extract::FromRequest<S> for ScopedJson<T>
where
    S: Send + Sync,
    T: serde::de::DeserializeOwned + crate::auth::MerchantScoped,
{
    type Rejection = ScopedJsonRejection;

    async fn from_request(
        request: axum::extract::Request,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let session = request.extensions().get::<AuthContext>().cloned();
        // Read through `Bytes` so the route's `DefaultBodyLimit` applies.
        let body = axum::body::Bytes::from_request(request, state)
            .await
            .map_err(|rejection| {
                // `body_text` keeps the cause ("length limit exceeded"), as `Json` reports it.
                ScopedJsonRejection::Read(crate::routes::body::RequestBodyError::Read(
                    axum::Error::new(rejection.body_text()),
                ))
            })?;
        let payload = match axum::Json::<T>::from_bytes(&body) {
            Ok(axum::Json(payload)) => payload,
            Err(error) => return Err(ScopedJsonRejection::Parse { body, error }),
        };
        let merchant = crate::auth::authorize(session.as_ref(), &payload)
            .map_err(ScopedJsonRejection::Scope)?;
        Ok(Self {
            merchant,
            payload,
            body,
        })
    }
}

/// The merchant a path-addressed route acts on, read from its merchant path parameter (see
/// [`crate::auth::scope::MERCHANT_PATH_PARAM`]) and authorized against the caller's session.
#[derive(Debug, Clone)]
pub struct MerchantFromPath(pub crate::auth::AuthorizedMerchant);

#[async_trait]
impl<S> FromRequestParts<S> for MerchantFromPath
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let path_params = axum::extract::RawPathParams::from_request_parts(parts, state)
            .await
            .map_err(IntoResponse::into_response)?;
        let Some(merchant_id) = path_params
            .iter()
            .find(|(key, _)| *key == crate::auth::scope::MERCHANT_PATH_PARAM)
            .map(|(_, value)| value.to_string())
        else {
            // A route wired with this extractor but no merchant parameter: a programming error.
            return Err(crate::error::text_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Route has no merchant path parameter",
            ));
        };
        let session = parts.extensions.get::<AuthContext>();
        crate::auth::authorize_merchant_id(session, &merchant_id)
            .map(Self)
            .map_err(IntoResponse::into_response)
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
                crate::logger::error!(
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

    #[derive(Debug, serde::Deserialize, crate::auth::MerchantScoped)]
    struct ScopedTestPayload {
        #[merchant]
        #[serde(default)]
        merchant_id: Option<String>,
        #[allow(dead_code)]
        count: u32,
    }

    thread_local! {
        static ERRORS_LOGGED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }

    /// Counts ERROR events on the thread that emits them. Installed once as the global subscriber:
    /// a subscriber scoped to one test can miss events while other tests register callsites
    /// concurrently, and each `#[tokio::test]` runs on its own thread.
    struct ErrorCounter;

    impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for ErrorCounter {
        fn on_event(
            &self,
            event: &tracing::Event<'_>,
            _context: tracing_subscriber::layer::Context<'_, S>,
        ) {
            if *event.metadata().level() == tracing::Level::ERROR {
                ERRORS_LOGGED.with(|count| count.set(count.get() + 1));
            }
        }
    }

    /// ERROR events logged so far on this thread.
    fn errors_logged() -> usize {
        static INSTALL: std::sync::Once = std::sync::Once::new();
        INSTALL.call_once(|| {
            use tracing_subscriber::prelude::*;
            let _ = tracing::subscriber::set_global_default(
                tracing_subscriber::registry().with(ErrorCounter),
            );
        });
        ERRORS_LOGGED.with(std::cell::Cell::get)
    }

    #[tokio::test]
    async fn json_rejections_return_invalid_request_and_log_once() {
        use axum::{body::Body, extract::FromRequest, response::IntoResponse};

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
            let before = errors_logged();
            let actual = super::Json::<TestPayload>::from_request(request(), &())
                .await
                .unwrap_err();
            assert_eq!(errors_logged() - before, 1);
            let expected = axum::Json::<TestPayload>::from_request(request(), &())
                .await
                .unwrap_err();
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

            // `ScopedJson` answers an unreadable or unparsable JSON body exactly as `Json` does.
            // It does not require the content type, so those cases parse there.
            if content_type != Some("application/json") {
                continue;
            }
            let before = errors_logged();
            let actual = super::ScopedJson::<ScopedTestPayload>::from_request(request(), &())
                .await
                .unwrap_err()
                .into_response();
            assert_eq!(errors_logged() - before, 1);
            let expected = axum::Json::<ScopedTestPayload>::from_request(request(), &())
                .await
                .unwrap_err();
            assert_eq!(actual.status(), axum::http::StatusCode::BAD_REQUEST);
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

        let before = errors_logged();
        let request = Request::builder()
            .header("content-type", "application/json")
            .body(Body::from(r#"{"count": 7}"#))
            .unwrap();
        let actual = super::Json::<TestPayload>::from_request(request, &())
            .await
            .unwrap();
        assert_eq!(actual.0, TestPayload { count: 7 });
        assert_eq!(errors_logged(), before);
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

    async fn scoped_status<T>(session: Option<&str>, body: &str) -> axum::http::StatusCode
    where
        T: serde::de::DeserializeOwned + crate::auth::MerchantScoped,
    {
        use axum::{body::Body, extract::FromRequest};

        let mut request = Request::builder()
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap();
        if let Some(merchant_id) = session {
            request
                .extensions_mut()
                .insert(AuthContext::from_api_key(merchant_id));
        }
        match super::ScopedJson::<T>::from_request(request, &()).await {
            Ok(_) => axum::http::StatusCode::OK,
            Err(rejection) => axum::response::IntoResponse::into_response(rejection).status(),
        }
    }

    #[tokio::test]
    async fn a_parse_failure_names_the_field_that_failed() {
        use crate::routes::api_key::CreateApiKeyRequest;
        use axum::{body::Body, extract::FromRequest};

        let request = Request::builder()
            .body(Body::from(r#"{"merchant_id": "m", "description": 7}"#))
            .unwrap();
        match super::ScopedJson::<CreateApiKeyRequest>::from_request(request, &()).await {
            Err(super::ScopedJsonRejection::Parse { error, .. }) => {
                assert!(
                    error.body_text().contains("description"),
                    "{}",
                    error.body_text()
                )
            }
            other => panic!("expected a parse failure, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn scoped_json_binds_the_body_merchant_to_the_session() {
        use crate::routes::api_key::CreateApiKeyRequest;
        use axum::http::StatusCode;

        let own = r#"{"merchant_id":"merchant_a"}"#;
        let foreign = r#"{"merchant_id":"merchant_b"}"#;
        assert_eq!(
            scoped_status::<CreateApiKeyRequest>(Some("merchant_a"), own).await,
            StatusCode::OK
        );
        assert_eq!(
            scoped_status::<CreateApiKeyRequest>(Some("merchant_a"), foreign).await,
            StatusCode::FORBIDDEN
        );
        // A service credential carries no session and may name any merchant.
        assert_eq!(
            scoped_status::<CreateApiKeyRequest>(None, foreign).await,
            StatusCode::OK
        );
        // A body that does not parse is rejected as `Json` rejects it, before any scope check.
        assert_eq!(
            scoped_status::<CreateApiKeyRequest>(Some("merchant_a"), "{").await,
            StatusCode::BAD_REQUEST
        );
    }

    #[tokio::test]
    async fn scoped_json_checks_the_merchant_after_serde_aliases() {
        use crate::routes::gateway_score::ResetGatewayScoresRequest;

        // `merchantId` is an alias the handler accepts, so it is checked like `merchant_id`.
        assert_eq!(
            scoped_status::<ResetGatewayScoresRequest>(
                Some("merchant_a"),
                r#"{"merchantId":"merchant_b"}"#
            )
            .await,
            axum::http::StatusCode::FORBIDDEN
        );
    }

    #[tokio::test]
    async fn scoped_json_hands_the_handler_the_authorized_merchant() {
        use crate::routes::api_key::CreateApiKeyRequest;
        use axum::{body::Body, extract::FromRequest};

        let request = |session: Option<&str>| {
            let mut request = Request::builder()
                .header("content-type", "application/json")
                .body(Body::from(r#"{"merchant_id":"merchant_b"}"#))
                .unwrap();
            if let Some(merchant_id) = session {
                request
                    .extensions_mut()
                    .insert(AuthContext::from_api_key(merchant_id));
            }
            request
        };
        // A service credential acts on the merchant the request names.
        let scoped = super::ScopedJson::<CreateApiKeyRequest>::from_request(request(None), &())
            .await
            .unwrap();
        assert_eq!(scoped.merchant.as_str(), "merchant_b");
        // The session's own merchant.
        let scoped = super::ScopedJson::<CreateApiKeyRequest>::from_request(
            request(Some("merchant_b")),
            &(),
        )
        .await
        .unwrap();
        assert_eq!(scoped.merchant.as_str(), "merchant_b");
    }

    #[tokio::test]
    async fn merchant_from_path_authorizes_the_path_merchant() {
        use axum::{body::Body, routing::get, Router};
        use tower::ServiceExt;

        async fn echo(super::MerchantFromPath(merchant): super::MerchantFromPath) -> String {
            merchant.as_str().to_string()
        }
        let app = |session: Option<&'static str>| {
            Router::new()
                .route("/merchant-account/:merchant_id/x", get(echo))
                .route("/routing/list/:merchant_id", get(echo))
                .layer(axum::middleware::from_fn(
                    move |mut req: axum::extract::Request, next: axum::middleware::Next| async move {
                        if let Some(merchant_id) = session {
                            req.extensions_mut()
                                .insert(AuthContext::from_api_key(merchant_id));
                        }
                        next.run(req).await
                    },
                ))
        };
        let call = |session, uri: &'static str| async move {
            let response = app(session)
                .oneshot(Request::get(uri).body(Body::empty()).unwrap())
                .await
                .unwrap();
            let status = response.status();
            let body = axum::body::to_bytes(response.into_body(), usize::MAX)
                .await
                .unwrap();
            (status, String::from_utf8_lossy(&body).into_owned())
        };

        use axum::http::StatusCode;
        assert_eq!(
            call(Some("m_a"), "/merchant-account/m_a/x").await,
            (StatusCode::OK, "m_a".to_string())
        );
        assert_eq!(
            call(Some("m_a"), "/routing/list/m_b").await.0,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            call(None, "/routing/list/m_b").await,
            (StatusCode::OK, "m_b".to_string())
        );
    }
}
