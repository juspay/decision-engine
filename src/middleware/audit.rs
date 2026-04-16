use axum::{
    body::{to_bytes, Body},
    extract::Request,
    middleware::Next,
    response::Response,
};
use diesel::prelude::*;
use diesel::sql_types::Text;
use diesel_async::RunQueryDsl;
use std::time::Instant;

use crate::app::APP_STATE;
use crate::logger;
use crate::storage::consts::X_REQUEST_ID;

const MAX_RESPONSE_BODY_BYTES: usize = 10 * 1024; // 10KB

const HEADERS_TO_SANITIZE: &[&str] = &[
    "authorization",
    "x-api-key",
    "cookie",
    "set-cookie",
    "x-auth-token",
];

fn sanitize_headers(headers: &axum::http::HeaderMap) -> serde_json::Value {
    let mut map = serde_json::Map::new();
    for (name, value) in headers.iter() {
        let key = name.as_str().to_lowercase();
        if HEADERS_TO_SANITIZE.contains(&key.as_str()) {
            map.insert(key, serde_json::Value::String("[REDACTED]".to_string()));
        } else if let Ok(v) = value.to_str() {
            map.insert(key, serde_json::Value::String(v.to_string()));
        }
    }
    serde_json::Value::Object(map)
}

fn extract_merchant_id(body: &serde_json::Value) -> Option<String> {
    body.get("merchant_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            body.get("merchantId")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
}

pub async fn audit_middleware(request: Request<Body>, next: Next) -> Response {
    let start = Instant::now();

    let endpoint = request.uri().path().to_string();
    let method = request.method().to_string();

    // Skip audit endpoints to avoid infinite recursion
    if endpoint.starts_with("/audit") || endpoint.starts_with("/health") {
        return next.run(request).await;
    }

    let request_id = request
        .headers()
        .get(X_REQUEST_ID)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("unknown")
        .to_string();

    let sanitized_headers = sanitize_headers(request.headers());

    // Extract the request body
    let (parts, body) = request.into_parts();
    let body_bytes = match to_bytes(body, 1024 * 1024).await {
        Ok(bytes) => bytes,
        Err(e) => {
            logger::error!("Failed to read request body for audit: {:?}", e);
            let request = Request::from_parts(parts, Body::empty());
            return next.run(request).await;
        }
    };

    let request_body: Option<serde_json::Value> = if body_bytes.is_empty() {
        None
    } else {
        serde_json::from_slice(&body_bytes).ok()
    };

    let merchant_id = request_body.as_ref().and_then(extract_merchant_id);

    // Reconstruct the request with the body
    let request = Request::from_parts(parts, Body::from(body_bytes));

    // Run the actual handler
    let response = next.run(request).await;

    let latency_ms = start.elapsed().as_millis() as i32;
    let response_status = response.status().as_u16() as i32;

    // Extract response body
    let (resp_parts, resp_body) = response.into_parts();
    let resp_bytes = match to_bytes(resp_body, 1024 * 1024).await {
        Ok(bytes) => bytes,
        Err(e) => {
            logger::error!("Failed to read response body for audit: {:?}", e);
            let response = Response::from_parts(resp_parts, Body::empty());
            return response;
        }
    };

    let response_body: Option<serde_json::Value> = if resp_bytes.is_empty() {
        None
    } else if resp_bytes.len() > MAX_RESPONSE_BODY_BYTES {
        Some(serde_json::json!({
            "_truncated": true,
            "_original_size": resp_bytes.len()
        }))
    } else {
        serde_json::from_slice(&resp_bytes).ok()
    };

    // Reconstruct the response
    let response = Response::from_parts(resp_parts, Body::from(resp_bytes));

    // Spawn a background task to insert the audit log
    let req_headers_json = serde_json::to_string(&sanitized_headers).unwrap_or_default();
    let req_body_json = request_body
        .as_ref()
        .map(|v| serde_json::to_string(v).unwrap_or_default());
    let resp_body_json = response_body
        .as_ref()
        .map(|v| serde_json::to_string(v).unwrap_or_default());

    tokio::spawn(async move {
        if let Err(e) = insert_audit_log(
            &endpoint,
            &method,
            &req_headers_json,
            req_body_json.as_deref(),
            response_status,
            resp_body_json.as_deref(),
            latency_ms,
            merchant_id.as_deref(),
            &request_id,
        )
        .await
        {
            logger::error!("Failed to insert audit log: {:?}", e);
        }
    });

    response
}

#[derive(Debug, QueryableByName)]
struct InsertResult {
    #[diesel(sql_type = Text)]
    id: String,
}

async fn insert_audit_log(
    endpoint: &str,
    method: &str,
    request_headers: &str,
    request_body: Option<&str>,
    response_status: i32,
    response_body: Option<&str>,
    latency_ms: i32,
    merchant_id: Option<&str>,
    request_id: &str,
) -> Result<(), String> {
    #[allow(clippy::expect_used)]
    let app_state = APP_STATE.get().expect("GlobalAppState not set");
    let tenant_state = app_state
        .get_app_state_of_tenant("public")
        .await
        .map_err(|e| format!("Failed to get tenant state: {:?}", e))?;

    let conn = tenant_state
        .db
        .get_conn()
        .await
        .map_err(|e| format!("Failed to get DB connection: {:?}", e))?;

    let id = uuid::Uuid::new_v4().to_string();

    diesel::sql_query(
        "INSERT INTO audit_log (id, endpoint, method, request_headers, request_body, \
         response_status, response_body, latency_ms, merchant_id, request_id) \
         VALUES ($1, $2, $3, $4::jsonb, $5::jsonb, $6, $7::jsonb, $8, $9, $10)",
    )
    .bind::<Text, _>(&id)
    .bind::<Text, _>(endpoint)
    .bind::<Text, _>(method)
    .bind::<Text, _>(request_headers)
    .bind::<diesel::sql_types::Nullable<Text>, _>(request_body)
    .bind::<diesel::sql_types::Integer, _>(response_status)
    .bind::<diesel::sql_types::Nullable<Text>, _>(response_body)
    .bind::<diesel::sql_types::Integer, _>(latency_ms)
    .bind::<diesel::sql_types::Nullable<Text>, _>(merchant_id)
    .bind::<Text, _>(request_id)
    .execute(&*conn)
    .await
    .map_err(|e| format!("Failed to execute audit insert: {:?}", e))?;

    Ok(())
}
