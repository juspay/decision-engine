use axum::body::{to_bytes, Body, Bytes};

use crate::decider::gatewaydecider::types::{ErrorResponse, UnifiedError};

#[derive(Debug)]
pub enum RequestBodyError {
    Read(axum::Error),
}

pub async fn read_request_body(body: Body) -> Result<Bytes, RequestBodyError> {
    to_bytes(body, usize::MAX)
        .await
        .map_err(RequestBodyError::Read)
}

impl RequestBodyError {
    pub fn analytics_stage(&self) -> &'static str {
        match self {
            Self::Read(_) => "request_parse_failed",
        }
    }

    pub fn into_error_response(self) -> ErrorResponse {
        match self {
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
            Self::Read(_) => ("400", "Error parsing request"),
        }
    }
}
