use crate::custom_extractors::TenantStateResolver;
use crate::error::{self, ContainerError};
use axum::body::Body;
use axum::http::header::HeaderValue;
use axum::response::{IntoResponse, Response};
use axum::{http::Request, http::StatusCode, middleware::Next};

const VALID_API_KEY: &str = "your_valid_api_key"; // Replace with your actual API key

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

/// Middleware to authenticate requests using the x-api-key header
pub async fn authenticate(
    req: Request<Body>,
    next: Next,
) -> Result<Response<Body>, ContainerError<error::ApiError>> {
    if let Some(api_key) = req.headers().get("x-api-key") {
        if api_key == HeaderValue::from_static(VALID_API_KEY) {
            return Ok(next.run(req).await);
        }
    }

    let unauthorized_response = (StatusCode::UNAUTHORIZED, "Unauthorized").into_response();
    Ok(unauthorized_response)
}
