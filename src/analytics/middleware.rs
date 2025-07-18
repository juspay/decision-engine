use crate::analytics::RoutingEvent;
use crate::tenant::GlobalAppState;
use axum::{
    body::Body,
    extract::{Request, State},
    middleware::Next,
    response::Response,
};
use bytes::Bytes;
use http_body_util::BodyExt;
use std::{sync::Arc, time::Instant};
use tracing::{error, warn};

/// Analytics middleware to track routing events for specific endpoints
pub async fn analytics_middleware(
    State(global_app_state): State<Arc<GlobalAppState>>,
    request: Request,
    next: Next,
) -> Response {
    // Only track analytics for routing endpoints
    let path = request.uri().path();
    if !global_app_state.global_config.analytics.enabled || !should_track_endpoint(path) {
        return next.run(request).await;
    }

    let start_time = Instant::now();
    
    // Extract request information
    let method = request.method().to_string();
    let endpoint = path.to_string();
    
    // Extract merchant ID from request headers or body (simplified for now)
    let merchant_id = extract_merchant_id(&request).unwrap_or("public".to_string());
    
    // Get the tenant app state to access analytics client
    let tenant_app_state = match global_app_state.get_app_state_of_tenant(&merchant_id).await {
        Ok(state) => state,
        Err(_) => {
            // If tenant not found, try with default "public" tenant
            match global_app_state.get_app_state_of_tenant("public").await {
                Ok(state) => state,
                Err(_) => {
                    // If analytics client is not available, just proceed without analytics
                    return next.run(request).await;
                }
            }
        }
    };
    
    // Create routing event
    let mut routing_event = RoutingEvent::from_request(&request, merchant_id.clone());
    
    // Extract request body for logging
    let (request_parts, body) = request.into_parts();
    let body_bytes = match body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(_) => Bytes::new(),
    };
    
    let request_payload = String::from_utf8_lossy(&body_bytes).to_string();
    routing_event = routing_event.with_request_payload(&request_payload);
    
    // Reconstruct request with body
    let request = Request::from_parts(request_parts, Body::from(body_bytes));
    
    // Process the request
    let response = next.run(request).await;
    
    // Calculate processing time
    let processing_time = start_time.elapsed().as_millis() as u32;
    
    // Extract response information
    let status_code = response.status().as_u16();
    
    // Extract response body for logging. Note: This operation can lead to high memory usage 
    let (response_parts, body) = response.into_parts();
    let body_bytes = match body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(_) => Bytes::new(),
    };
    
    let response_payload = String::from_utf8_lossy(&body_bytes).to_string();
    
    // Complete the routing event
    routing_event = routing_event
        .with_response_payload(&response_payload)
        .with_status_code(status_code)
        .with_processing_time(processing_time);
    
    // Extract gateway information from response
    if let Err(e) = routing_event.extract_gateway_from_response() {
        warn!("Failed to extract gateway from response: {:?}", e);
    }
    
    // Extract error information if status indicates failure
    if let Err(e) = routing_event.extract_error_from_response() {
        warn!("Failed to extract error from response: {:?}", e);
    }
    
    // Send event to analytics (async, non-blocking)
    if let Err(e) = tenant_app_state.analytics_client.track_routing_event(routing_event).await {
        error!("Failed to track routing event: {:?}", e);
    }
    
    // Reconstruct response
    Response::from_parts(response_parts, Body::from(body_bytes))
}

/// Determine if an endpoint should be tracked for analytics
fn should_track_endpoint(path: &str) -> bool {
    matches!(path, "/routing/evaluate" | "/decide-gateway")
}

/// Extract merchant ID from request (simplified implementation)
fn extract_merchant_id(request: &Request) -> Option<String> {
    // Try to extract from headers first
    if let Some(merchant_id) = request.headers().get("x-merchant-id") {
        if let Ok(merchant_id_str) = merchant_id.to_str() {
            return Some(merchant_id_str.to_string());
        }
    }
    
    // Try x-tenant-id header as fallback
    if let Some(tenant_id) = request.headers().get("x-tenant-id") {
        if let Ok(tenant_id_str) = tenant_id.to_str() {
            return Some(tenant_id_str.to_string());
        }
    }
    
    // Default to "public" tenant
    Some("public".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_track_endpoint() {
        assert!(should_track_endpoint("/routing/evaluate"));
        assert!(should_track_endpoint("/decide-gateway"));
        assert!(!should_track_endpoint("/health"));
        assert!(!should_track_endpoint("/rule/create"));
    }

    #[test]
    fn test_extract_merchant_id_from_header() {
        use axum::http::{HeaderMap, HeaderValue};
        
        let mut headers = HeaderMap::new();
        headers.insert("x-merchant-id", HeaderValue::from_static("merchant-123"));
        
        let request = Request::builder()
            .uri("/routing/evaluate")
            .body(Body::empty())
            .unwrap();
        
        // Note: This test would need to be adjusted to work with the actual request structure
        // For now, it's a placeholder to show the testing approach
    }
}
