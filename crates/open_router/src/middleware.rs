use crate::error::{self, ContainerError};
use crate::custom_extractors::TenantStateResolver;
use axum::body::Body;
use axum::response::Response;
use axum::{http::Request, middleware::Next};

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
