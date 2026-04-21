use std::error::Error as _;

use axum::body::{to_bytes, Body, Bytes};
use http_body_util::LengthLimitError;

use crate::decider::gatewaydecider::types::{ErrorResponse, UnifiedError};

#[derive(Debug)]
pub enum RequestBodyError {
    TooLarge,
    Read(axum::Error),
}

pub async fn read_request_body(body: Body) -> Result<Bytes, RequestBodyError> {
    let limit = crate::app::APP_STATE
        .get()
        .map(|state| state.analytics_runtime.request_body_limit_bytes())
        .unwrap_or_else(|| {
            crate::config::AnalyticsCaptureConfig::default().request_body_limit_bytes
        });

    read_request_body_with_limit(body, limit).await
}

pub fn observe_request_body_error(endpoint: &'static str, error: &RequestBodyError) {
    if matches!(error, RequestBodyError::TooLarge) {
        crate::metrics::ANALYTICS_REQUEST_BODY_REJECTIONS_TOTAL
            .with_label_values(&[endpoint])
            .inc();
    }
}

impl RequestBodyError {
    pub fn analytics_stage(&self) -> &'static str {
        match self {
            Self::TooLarge => "request_too_large",
            Self::Read(_) => "request_parse_failed",
        }
    }

    pub fn into_error_response(self) -> ErrorResponse {
        match self {
            Self::TooLarge => ErrorResponse {
                status: "413".to_string(),
                error_code: "413".to_string(),
                error_message: "Request body too large".to_string(),
                priority_logic_tag: None,
                routing_approach: None,
                filter_wise_gateways: None,
                error_info: UnifiedError {
                    code: "REQUEST_BODY_TOO_LARGE".to_string(),
                    user_message: "Request body exceeds the configured limit.".to_string(),
                    developer_message:
                        "Request body exceeded analytics.capture.request_body_limit_bytes"
                            .to_string(),
                },
                priority_logic_output: None,
                is_dynamic_mga_enabled: false,
            },
            Self::Read(error) => ErrorResponse {
                status: "400".to_string(),
                error_code: "400".to_string(),
                error_message: "Error parsing request".to_string(),
                priority_logic_tag: None,
                routing_approach: None,
                filter_wise_gateways: None,
                error_info: UnifiedError {
                    code: "INVALID_INPUT".to_string(),
                    user_message: "Invalid request params. Please verify your input.".to_string(),
                    developer_message: error.to_string(),
                },
                priority_logic_output: None,
                is_dynamic_mga_enabled: false,
            },
        }
    }

    pub fn analytics_code_and_message(&self) -> (&'static str, &'static str) {
        match self {
            Self::TooLarge => ("413", "Request body too large"),
            Self::Read(_) => ("400", "Error parsing request"),
        }
    }
}

async fn read_request_body_with_limit(body: Body, limit: usize) -> Result<Bytes, RequestBodyError> {
    to_bytes(body, limit).await.map_err(|error| {
        if error
            .source()
            .and_then(|source| source.downcast_ref::<LengthLimitError>())
            .is_some()
        {
            RequestBodyError::TooLarge
        } else {
            RequestBodyError::Read(error)
        }
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use axum::body::Body;

    use super::RequestBodyError;

    #[tokio::test]
    async fn limited_body_returns_too_large() {
        let body = Body::from("abcd");
        let error = super::read_request_body_with_limit(body, 2)
            .await
            .err()
            .unwrap();
        match error {
            RequestBodyError::TooLarge => {}
            RequestBodyError::Read(_) => panic!("expected body limit error"),
        }
    }
}
