use axum::body::{to_bytes, Body, Bytes};
use axum::http::StatusCode;

use crate::decider::gatewaydecider::types::{ErrorResponse, UnifiedError};

#[derive(Debug, Clone, Copy)]
enum RequestBodyFailureKind {
    ReadFailed,
}

#[derive(Debug, Clone, Copy)]
struct RequestBodyFailureSpec {
    analytics_stage: &'static str,
    status: StatusCode,
    error_code: &'static str,
    error_message: &'static str,
    developer_error_code: &'static str,
    user_message: &'static str,
}

impl RequestBodyFailureKind {
    fn spec(self) -> RequestBodyFailureSpec {
        match self {
            Self::ReadFailed => RequestBodyFailureSpec {
                analytics_stage: "request_read_failed",
                status: StatusCode::BAD_REQUEST,
                error_code: "400",
                error_message: "Error reading request body",
                developer_error_code: "INVALID_INPUT",
                user_message: "Invalid request params. Please verify your input.",
            },
        }
    }
}

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
    fn kind(&self) -> RequestBodyFailureKind {
        match self {
            Self::Read(_) => RequestBodyFailureKind::ReadFailed,
        }
    }

    pub fn analytics_stage(&self) -> &'static str {
        self.kind().spec().analytics_stage
    }

    pub fn into_error_response(self) -> ErrorResponse {
        let spec = self.kind().spec();
        match self {
            Self::Read(error) => ErrorResponse {
                status: spec.status.as_u16().to_string(),
                error_code: spec.error_code.to_string(),
                error_message: spec.error_message.to_string(),
                priority_logic_tag: None,
                routing_approach: None,
                filter_wise_gateways: None,
                error_info: UnifiedError {
                    code: spec.developer_error_code.to_string(),
                    user_message: spec.user_message.to_string(),
                    developer_message: error.to_string(),
                },
                priority_logic_output: None,
                is_dynamic_mga_enabled: false,
            },
        }
    }

    pub fn analytics_code_and_message(&self) -> (&'static str, &'static str) {
        let spec = self.kind().spec();
        (spec.error_code, spec.error_message)
    }
}
