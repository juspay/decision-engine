use crate::app::APP_STATE;
use crate::auth;
use crate::custom_extractors::TenantStateResolver;
use crate::error::{self, ContainerError};
use axum::body::Body;
use axum::response::Response;
use axum::{http::Request, http::StatusCode, middleware::Next};
use diesel::ExpressionMethods;
use masking::PeekInterface;

const API_KEY_CACHE_TTL: i64 = 300;

/// Path parameters that name the merchant a route acts on. Routing rules are owned by
/// `created_by`, which is the merchant id under another name.
const MERCHANT_PATH_PARAMS: &[&str] = &["merchant-id", "merchant_id", "created_by"];

/// Enforce that a per-merchant session may only act on the merchant named in the path.
///
/// Runs immediately *after* [`authenticate`], so an `AuthContext` is already in the request
/// extensions for the JWT and api-key lanes. Those lanes carry a concrete `merchant_id`; a route
/// addressed by a merchant names it in the path ([`MERCHANT_PATH_PARAMS`]). When the two disagree
/// the request is refused here, so no handler has to remember the check — and a path-addressed
/// route added later is covered the day it is written.
///
/// Routes that take the merchant from the request body bind it with
/// [`crate::custom_extractors::ScopedJson`] (or [`auth::ensure_merchant_scope`] where the handler
/// parses the body itself), checked against the typed request rather than raw JSON.
///
/// Service credentials (the shared admin secret, and the `api_key_auth_enabled = false`
/// backward-compat lane) insert *no* `AuthContext`, so they fall through unrestricted exactly as
/// before — they are trusted callers that scope requests off the path/body themselves.
pub async fn enforce_merchant_scope(
    req: Request<Body>,
    next: Next,
) -> Result<Response<Body>, ContainerError<error::ApiError>> {
    use axum::extract::{FromRequestParts, RawPathParams};

    // No context ⇒ admin-secret / compat lane ⇒ trusted service caller, nothing to scope.
    let Some(context) = req.extensions().get::<auth::AuthContext>().cloned() else {
        return Ok(next.run(req).await);
    };

    let (mut parts, body) = req.into_parts();
    // `RawPathParams` is populated during routing; this layer runs after it. Borrow the pattern's
    // captured values without consuming any typed `Path` extractor the handler still needs.
    let path_params = RawPathParams::from_request_parts(&mut parts, &())
        .await
        .ok();
    let req = Request::from_parts(parts, body);

    if let Some(path_params) = path_params {
        for (key, value) in &path_params {
            if MERCHANT_PATH_PARAMS.contains(&key) && value != context.merchant_id.as_str() {
                return Ok(axum::response::IntoResponse::into_response(
                    auth::MerchantScopeViolation,
                ));
            }
        }
    }

    Ok(next.run(req).await)
}

/// Middleware providing implementation to perform JWE + JWS encryption and decryption around the
/// card APIs
pub async fn middleware(
    TenantStateResolver(_tenant_state): TenantStateResolver,
    req: Request<Body>,
    next: Next,
) -> Result<Response<Body>, ContainerError<error::ApiError>> {
    let response = next.run(req).await;
    Ok(response)
}

/// Middleware to authenticate requests using either:
/// - `x-api-key` header (service-to-service / programmatic access)
/// - `Authorization: Bearer <jwt>` header (dashboard / user sessions)
///
/// When `api_key_auth_enabled` is false in config, all requests pass through (backward compat mode).
pub async fn authenticate(
    req: Request<Body>,
    next: Next,
) -> Result<Response<Body>, ContainerError<error::ApiError>> {
    let app_state = match APP_STATE.get() {
        Some(s) => s,
        None => {
            return Ok(crate::error::text_error_response(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Server not ready",
            ))
        }
    };
    let mut req = req;

    // Accept JWT Bearer token (dashboard sessions)
    if let Some(bearer) = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
    {
        use crate::routes::user_auth::verify_jwt_not_revoked;
        match verify_jwt_not_revoked(bearer, app_state.global_config.user_auth.jwt_secret.peek())
            .await
        {
            Ok(claims) => {
                let context = auth::AuthContext::from_jwt(
                    &claims,
                    app_state
                        .global_config
                        .user_auth
                        .require_explicit_permissions,
                );

                // Enforced here rather than per handler, so every route is covered at once. The
                // routed pattern is what gets classified — `MatchedPath` is set during routing, and
                // this layer runs after it — so a path parameter cannot be shaped to resemble a
                // route that needs less.
                let matched_path = req
                    .extensions()
                    .get::<axum::extract::MatchedPath>()
                    .map(|matched| matched.as_str().to_owned());
                let required =
                    auth::access::required_permission(req.method(), matched_path.as_deref());

                if !context.allows(&required) {
                    return Ok(crate::error::text_error_response(
                        StatusCode::FORBIDDEN,
                        format!("This session does not have {}", required.as_str()),
                    ));
                }

                req.extensions_mut().insert(context);
                return Ok(next.run(req).await);
            }
            Err(_) => {
                return Ok(crate::error::text_error_response(
                    StatusCode::UNAUTHORIZED,
                    "Invalid or expired token",
                ))
            }
        }
    }

    // Accept the shared admin secret (service-to-service callers such as Hyperswitch, which
    // has no merchant api-key or dashboard session). Checked before the api-key compat mode so
    // a wrong secret is always rejected. No AuthContext is inserted — like compat mode, these
    // callers scope requests off the body/path, and extractors that require an AuthContext
    // (e.g. analytics) keep rejecting them.
    if let Some(provided) = req
        .headers()
        .get("x-admin-secret")
        .and_then(|v| v.to_str().ok())
    {
        let expected = app_state.global_config.admin_secret.secret.peek();
        if !expected.is_empty() && provided == expected {
            return Ok(next.run(req).await);
        }
        return Ok(crate::error::text_error_response(
            StatusCode::UNAUTHORIZED,
            "Invalid admin secret",
        ));
    }

    if !app_state.global_config.api_key_auth_enabled {
        return Ok(next.run(req).await);
    }

    let api_key = match req.headers().get("x-api-key").and_then(|v| v.to_str().ok()) {
        Some(k) => k.to_owned(),
        None => {
            return Ok(crate::error::text_error_response(
                StatusCode::UNAUTHORIZED,
                "Missing authentication credentials",
            ))
        }
    };

    let key_hash = auth::hash_api_key(&api_key);
    let cache_key = format!("api_key:{}", key_hash);

    let tenant_state =
        match crate::tenant::GlobalAppState::get_app_state_of_tenant(app_state, "public").await {
            Ok(s) => s,
            Err(_) => {
                return Ok(crate::error::text_error_response(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Tenant not found",
                ))
            }
        };

    // Check Redis cache first
    if let Ok(cached) = tenant_state.redis_conn.get_key_string(&cache_key).await {
        if !cached.is_empty() {
            req.extensions_mut()
                .insert(auth::AuthContext::from_api_key(cached));
            return Ok(next.run(req).await);
        }
    }

    // Cache miss — query DB
    use crate::storage::types::MerchantApiKey;
    use diesel::associations::HasTable;

    #[cfg(feature = "mysql")]
    use crate::storage::schema::merchant_api_keys::dsl;
    #[cfg(feature = "postgres")]
    use crate::storage::schema_pg::merchant_api_keys::dsl;

    let results = crate::generics::generic_find_all::<
        <MerchantApiKey as HasTable>::Table,
        _,
        MerchantApiKey,
    >(&tenant_state.db, dsl::key_hash.eq(key_hash.clone()))
    .await;

    let key_record = match results {
        Ok(mut rows) => rows.pop(),
        Err(_) => None,
    };

    match key_record {
        Some(record) => {
            let is_active = {
                #[cfg(feature = "mysql")]
                {
                    record.is_active != 0
                }
                #[cfg(feature = "postgres")]
                {
                    record.is_active
                }
            };

            if !is_active {
                return Ok(crate::error::text_error_response(
                    StatusCode::UNAUTHORIZED,
                    "API key is revoked",
                ));
            }

            // Populate Redis cache
            let _ = tenant_state
                .redis_conn
                .set_key_with_ttl(&cache_key, &record.merchant_id, API_KEY_CACHE_TTL)
                .await;

            req.extensions_mut()
                .insert(auth::AuthContext::from_api_key(record.merchant_id));
            Ok(next.run(req).await)
        }
        None => Ok(crate::error::text_error_response(
            StatusCode::UNAUTHORIZED,
            "Invalid API key",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::middleware::from_fn;
    use axum::routing::post;
    use axum::Router;
    use tower::ServiceExt;

    const OWN: &str = "merchant_a";
    const FOREIGN: &str = "merchant_b";

    /// Stands in for `authenticate` on the api-key lane.
    async fn session_for_own_merchant(mut req: Request<Body>, next: Next) -> Response<Body> {
        req.extensions_mut()
            .insert(auth::AuthContext::from_api_key(OWN));
        next.run(req).await
    }

    fn app(with_session: bool) -> Router {
        let router = Router::new()
            .route(
                "/merchant-account/:merchant-id/seed-costs",
                post(|| async {}),
            )
            .route("/api-key/list/:merchant_id", post(|| async {}))
            .route("/routing/list/:created_by", post(|| async {}))
            .layer(from_fn(enforce_merchant_scope));
        if with_session {
            router.layer(from_fn(session_for_own_merchant))
        } else {
            router
        }
    }

    async fn status(with_session: bool, uri: &str) -> StatusCode {
        app(with_session)
            .oneshot(Request::post(uri).body(Body::empty()).expect("request"))
            .await
            .expect("response")
            .status()
    }

    #[tokio::test]
    async fn every_merchant_path_param_must_match_the_session() {
        for route in [
            "/merchant-account/{}/seed-costs",
            "/api-key/list/{}",
            "/routing/list/{}",
        ] {
            let own = route.replace("{}", OWN);
            let foreign = route.replace("{}", FOREIGN);
            assert_eq!(status(true, &own).await, StatusCode::OK, "{own}");
            assert_eq!(
                status(true, &foreign).await,
                StatusCode::FORBIDDEN,
                "{foreign}"
            );
        }
    }

    #[tokio::test]
    async fn service_credentials_are_not_scoped() {
        let foreign = format!("/merchant-account/{FOREIGN}/seed-costs");
        assert_eq!(status(false, &foreign).await, StatusCode::OK);
    }
}
