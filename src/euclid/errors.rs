use crate::error::{error_codes, ApiErrorResponse};

#[derive(Debug, Clone, thiserror::Error)]
pub enum EuclidErrors {
    #[error("Failed to parse JSON input")]
    FailedToParseJsonInput,

    #[error("Failed to serialize to pretty-printed String of JSON")]
    FailedToSerializeToJson,

    #[error("Incorrect request received : {0}")]
    InvalidRequest(String),

    #[error("error parsing rules")]
    InvalidRuleConfiguration,

    #[error("Invalid parameter: {0}")]
    InvalidRequestParameter(String),

    #[error("Routing configuration not found")]
    GlobalRoutingConfigsUnavailable,

    #[error("Routing interpretation")]
    RoutingInterpretationFailed,

    #[error("Routing rule validation failed")]
    FailedToValidateRoutingRule,

    #[error("Storage error")]
    StorageError,
}

impl axum::response::IntoResponse for EuclidErrors {
    fn into_response(self) -> axum::response::Response {
        match self {
            EuclidErrors::InvalidRuleConfiguration => (
                hyper::StatusCode::BAD_REQUEST,
                axum::Json(ApiErrorResponse::new(
                    error_codes::TE_04,
                    "Invalid routing rule configuration".to_string(),
                    None,
                )),
            )
                .into_response(),

            EuclidErrors::InvalidRequestParameter(param) => (
                hyper::StatusCode::BAD_REQUEST,
                axum::Json(ApiErrorResponse::new(
                    error_codes::TE_04,
                    format!(
                    "Invalid parameter: {}",
                    param
                ),
                    None,
                )),
            )
                .into_response(),

            EuclidErrors::FailedToValidateRoutingRule => (
                hyper::StatusCode::BAD_REQUEST,
                axum::Json(ApiErrorResponse::new(
                    error_codes::TE_04,
                    "Failed to validate the provided routing rule".to_string(),
                    None,
                )),
            )
                .into_response(),

            EuclidErrors::InvalidRequest(msg) => (
                hyper::StatusCode::BAD_REQUEST,
                axum::Json(ApiErrorResponse::new(
                    error_codes::TE_04,
                    format!(
                    "Invalid request received : {}",
                    msg
                ),
                    None,
                )),
            )
                .into_response(),

            EuclidErrors::GlobalRoutingConfigsUnavailable => (
                hyper::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(ApiErrorResponse::new(
                    error_codes::TE_04,
                    "Routing configurations not found".to_string(),
                    None,
                )),
            )
                .into_response(),

            EuclidErrors::StorageError => (
                hyper::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(ApiErrorResponse::new(
                    error_codes::TE_04,
                    "Something went wrong".to_string(),
                    None,
                )),
            )
                .into_response(),

            EuclidErrors::FailedToParseJsonInput => (
                hyper::StatusCode::BAD_REQUEST,
                axum::Json(ApiErrorResponse::new(
                    error_codes::TE_04,
                    "Failed to parse Json received in request".to_string(),
                    None,
                )),
            )
                .into_response(),

            EuclidErrors::FailedToSerializeToJson => (
                hyper::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(ApiErrorResponse::new(
                    error_codes::TE_04,
                    "Failed to serialize Json ".to_string(),
                    None,
                )),
            )
                .into_response(),

            EuclidErrors::RoutingInterpretationFailed => (
                hyper::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(ApiErrorResponse::new(
                    error_codes::TE_04,
                    "unable to evaluate output for routing algorithm against the provided parameters".to_string(),
                    None,
                )),
            )
                .into_response(),
        }
    }
}
