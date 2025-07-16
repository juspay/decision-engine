use crate::analytics::{AnalyticsResult, RoutingEventData};
use axum::extract::Request;
use axum::response::Response;
use serde_json::Value;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Clone)]
pub struct RoutingEvent {
    pub event_id: String,
    pub merchant_id: String,
    pub request_id: String,
    pub endpoint: String,
    pub method: String,
    pub request_payload: String,
    pub response_payload: String,
    pub status_code: u16,
    pub processing_time_ms: u32,
    pub gateway_selected: Option<String>,
    pub routing_algorithm_id: Option<String>,
    pub error_message: Option<String>,
    pub user_agent: Option<String>,
    pub ip_address: Option<String>,
    pub created_at: OffsetDateTime,
}

impl RoutingEvent {
    pub fn new(
        merchant_id: String,
        request_id: String,
        endpoint: String,
        method: String,
    ) -> Self {
        Self {
            event_id: Uuid::new_v4().to_string(),
            merchant_id,
            request_id,
            endpoint,
            method,
            request_payload: String::new(),
            response_payload: String::new(),
            status_code: 0,
            processing_time_ms: 0,
            gateway_selected: None,
            routing_algorithm_id: None,
            error_message: None,
            user_agent: None,
            ip_address: None,
            created_at: OffsetDateTime::now_utc(),
        }
    }

    pub fn from_request(request: &Request, merchant_id: String) -> Self {
        let request_id = extract_request_id(request);
        let endpoint = request.uri().path().to_string();
        let method = request.method().to_string();
        let user_agent = extract_user_agent(request);
        let ip_address = extract_ip_address(request);

        Self {
            event_id: Uuid::new_v4().to_string(),
            merchant_id,
            request_id,
            endpoint,
            method,
            request_payload: String::new(),
            response_payload: String::new(),
            status_code: 0,
            processing_time_ms: 0,
            gateway_selected: None,
            routing_algorithm_id: None,
            error_message: None,
            user_agent,
            ip_address,
            created_at: OffsetDateTime::now_utc(),
        }
    }

    pub fn with_request_payload(mut self, payload: &str) -> Self {
        self.request_payload = payload.to_string();
        self
    }

    pub fn with_response_payload(mut self, payload: &str) -> Self {
        self.response_payload = payload.to_string();
        self
    }

    pub fn with_status_code(mut self, status_code: u16) -> Self {
        self.status_code = status_code;
        self
    }

    pub fn with_processing_time(mut self, processing_time_ms: u32) -> Self {
        self.processing_time_ms = processing_time_ms;
        self
    }

    pub fn with_gateway_selected(mut self, gateway: Option<String>) -> Self {
        self.gateway_selected = gateway;
        self
    }

    pub fn with_routing_algorithm_id(mut self, algorithm_id: Option<String>) -> Self {
        self.routing_algorithm_id = algorithm_id;
        self
    }

    pub fn with_error_message(mut self, error: Option<String>) -> Self {
        self.error_message = error;
        self
    }

    pub fn to_event_data(&self) -> RoutingEventData {
        RoutingEventData {
            event_id: self.event_id.clone(),
            merchant_id: self.merchant_id.clone(),
            request_id: self.request_id.clone(),
            endpoint: self.endpoint.clone(),
            method: self.method.clone(),
            request_payload: self.request_payload.clone(),
            response_payload: self.response_payload.clone(),
            status_code: self.status_code,
            processing_time_ms: self.processing_time_ms,
            gateway_selected: self.gateway_selected.clone(),
            routing_algorithm_id: self.routing_algorithm_id.clone(),
            error_message: self.error_message.clone(),
            user_agent: self.user_agent.clone(),
            ip_address: self.ip_address.clone(),
            created_at: self.created_at,
            sign_flag: 1, // Always 1 for new events
        }
    }

    /// Extract gateway information from response payload
    pub fn extract_gateway_from_response(&mut self) -> AnalyticsResult<()> {
        if !self.response_payload.is_empty() {
            if let Ok(response_json) = serde_json::from_str::<Value>(&self.response_payload) {
                // Try to extract gateway from various possible response structures
                if let Some(gateway) = response_json.get("gateway")
                    .or_else(|| response_json.get("selected_gateway"))
                    .or_else(|| response_json.get("connector"))
                    .and_then(|v| v.as_str()) {
                    self.gateway_selected = Some(gateway.to_string());
                }

                // Try to extract routing algorithm ID
                if let Some(algo_id) = response_json.get("routing_algorithm_id")
                    .or_else(|| response_json.get("algorithm_id"))
                    .and_then(|v| v.as_str()) {
                    self.routing_algorithm_id = Some(algo_id.to_string());
                }
            }
        }
        Ok(())
    }

    /// Extract error information from response payload
    pub fn extract_error_from_response(&mut self) -> AnalyticsResult<()> {
        if self.status_code >= 400 && !self.response_payload.is_empty() {
            if let Ok(response_json) = serde_json::from_str::<Value>(&self.response_payload) {
                if let Some(error) = response_json.get("error")
                    .or_else(|| response_json.get("message"))
                    .or_else(|| response_json.get("error_message"))
                    .and_then(|v| v.as_str()) {
                    self.error_message = Some(error.to_string());
                }
            }
        }
        Ok(())
    }
}

fn extract_request_id(request: &Request) -> String {
    request
        .headers()
        .get("x-request-id")
        .or_else(|| request.headers().get("request-id"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| Uuid::new_v4().to_string())
}

fn extract_user_agent(request: &Request) -> Option<String> {
    request
        .headers()
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

fn extract_ip_address(request: &Request) -> Option<String> {
    // Try various headers for IP address
    request
        .headers()
        .get("x-forwarded-for")
        .or_else(|| request.headers().get("x-real-ip"))
        .or_else(|| request.headers().get("cf-connecting-ip"))
        .and_then(|v| v.to_str().ok())
        .map(|s| {
            // Take the first IP if there are multiple (comma-separated)
            s.split(',').next().unwrap_or(s).trim().to_string()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::{HeaderMap, HeaderValue, Method, Uri};

    #[test]
    fn test_routing_event_creation() {
        let event = RoutingEvent::new(
            "merchant-123".to_string(),
            "req-456".to_string(),
            "/routing/evaluate".to_string(),
            "POST".to_string(),
        );

        assert_eq!(event.merchant_id, "merchant-123");
        assert_eq!(event.request_id, "req-456");
        assert_eq!(event.endpoint, "/routing/evaluate");
        assert_eq!(event.method, "POST");
        assert!(!event.event_id.is_empty());
    }

    #[test]
    fn test_event_builder_pattern() {
        let event = RoutingEvent::new(
            "merchant-123".to_string(),
            "req-456".to_string(),
            "/routing/evaluate".to_string(),
            "POST".to_string(),
        )
        .with_request_payload(r#"{"test": "data"}"#)
        .with_response_payload(r#"{"gateway": "stripe"}"#)
        .with_status_code(200)
        .with_processing_time(150)
        .with_gateway_selected(Some("stripe".to_string()));

        assert_eq!(event.request_payload, r#"{"test": "data"}"#);
        assert_eq!(event.response_payload, r#"{"gateway": "stripe"}"#);
        assert_eq!(event.status_code, 200);
        assert_eq!(event.processing_time_ms, 150);
        assert_eq!(event.gateway_selected, Some("stripe".to_string()));
    }

    #[test]
    fn test_extract_gateway_from_response() {
        let mut event = RoutingEvent::new(
            "merchant-123".to_string(),
            "req-456".to_string(),
            "/routing/evaluate".to_string(),
            "POST".to_string(),
        )
        .with_response_payload(r#"{"gateway": "stripe", "routing_algorithm_id": "algo-123"}"#);

        event.extract_gateway_from_response().unwrap();

        assert_eq!(event.gateway_selected, Some("stripe".to_string()));
        assert_eq!(event.routing_algorithm_id, Some("algo-123".to_string()));
    }

    #[test]
    fn test_extract_error_from_response() {
        let mut event = RoutingEvent::new(
            "merchant-123".to_string(),
            "req-456".to_string(),
            "/routing/evaluate".to_string(),
            "POST".to_string(),
        )
        .with_response_payload(r#"{"error": "Gateway not available"}"#)
        .with_status_code(500);

        event.extract_error_from_response().unwrap();

        assert_eq!(event.error_message, Some("Gateway not available".to_string()));
    }
}
