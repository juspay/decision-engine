use crate::error::{error_codes, ApiErrorResponse};

#[derive(Debug, Clone, thiserror::Error)]
pub enum EuclidErrors {
    #[error("Failed to parse JSON input")]
    FailedToParseJsonInput,

    #[error("Failed to serialize to pretty-printed String of JSON")]
    FailedToSerializeJsonToString,

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

    #[error("Failed to evaluate output for type : {0}")]
    FailedToEvaluateOutput(String),

    #[error("Active routing_algorithm not found for: {0}")]
    ActiveRoutingAlgorithmNotFound(String),

    #[error("routing_algorithm not found for: {0}")]
    RoutingAlgorithmNotFound(String),

    #[error("default fallback not found in evaluate request for: {0}")]
    DefaultFallbackNotFound(String),

    #[error("Storage error")]
    StorageError,

    #[error("Invalid Sr Dimension Configuration")]
    InvalidSrDimensionConfig(String),

    #[error("Field value out of range: {0}")]
    FieldValueOutOfRange(String),

    #[error("Field length invalid: {0}")]
    FieldLengthInvalid(String),

    #[error("Field pattern mismatch: {0}")]
    FieldPatternMismatch(String),

    #[error("Field validation failed: {0}")]
    FieldValidationFailed(String),
}

pub fn format_validation_error(
    context: &str,
    field: &str,
    error_type: &str,
    expected: &str,
    actual: &str,
) -> String {
    format!(
        "{}: Invalid field '{}': expected {}, got {}",
        context, field, expected, actual
    )
}

pub fn field_value_out_of_range(field: &str, value: i64, min: i64, max: i64) -> EuclidErrors {
    EuclidErrors::FieldValueOutOfRange(format!(
        "field '{}': value {} is outside valid range [{}, {}]",
        field, value, min, max
    ))
}

pub fn field_length_invalid(field: &str, expected: usize, actual: usize) -> EuclidErrors {
    EuclidErrors::FieldLengthInvalid(format!(
        "field '{}': expected {} characters, got {}",
        field, expected, actual
    ))
}

pub fn field_length_out_of_range(
    field: &str,
    actual: usize,
    min: usize,
    max: usize,
) -> EuclidErrors {
    EuclidErrors::FieldLengthInvalid(format!(
        "field '{}': length {} is outside valid range [{}, {}]",
        field, actual, min, max
    ))
}

pub fn field_pattern_mismatch(field: &str, value: &str, pattern: &str) -> EuclidErrors {
    EuclidErrors::FieldPatternMismatch(format!(
        "field '{}': value '{}' does not match pattern '{}'",
        field, value, pattern
    ))
}

pub fn field_validation_failed(field: &str, reason: &str) -> EuclidErrors {
    EuclidErrors::FieldValidationFailed(format!("field '{}': {}", field, reason))
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ValidationErrorDetails {
    pub field: String,
    pub error_type: String,
    pub message: String,
    pub expected: Option<String>,
    pub actual: Option<String>,
}

impl ValidationErrorDetails {
    pub fn new(
        field: impl Into<String>,
        error_type: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            field: field.into(),
            error_type: error_type.into(),
            message: message.into(),
            expected: None,
            actual: None,
        }
    }

    pub fn with_expected_actual(
        field: impl Into<String>,
        error_type: impl Into<String>,
        message: impl Into<String>,
        expected: impl Into<String>,
        actual: impl Into<String>,
    ) -> Self {
        Self {
            field: field.into(),
            error_type: error_type.into(),
            message: message.into(),
            expected: Some(expected.into()),
            actual: Some(actual.into()),
        }
    }
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

            EuclidErrors::FailedToSerializeJsonToString => (
                hyper::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(ApiErrorResponse::new(
                    error_codes::TE_04,
                    "Failed to serialize Json ".to_string(),
                    None,
                )),
            )
                .into_response(),

            EuclidErrors::ActiveRoutingAlgorithmNotFound(msg) => (
                hyper::StatusCode::BAD_REQUEST,
                axum::Json(ApiErrorResponse::new(
                    error_codes::TE_04,
                    format!(
                    "No active routing algorithm found for the created_by entity : {}",
                    msg
                ),
                    None,
                )),
            )
                .into_response(),

            EuclidErrors::RoutingAlgorithmNotFound(msg) => (
                hyper::StatusCode::BAD_REQUEST,
                axum::Json(ApiErrorResponse::new(
                    error_codes::TE_04,
                    format!(
                    "Routing algorithm not found for the provided id : {}",
                    msg
                ),
                    None,
                )),
            )
                .into_response(),

            EuclidErrors::DefaultFallbackNotFound(creator_by) => (
                hyper::StatusCode::BAD_REQUEST,
                axum::Json(ApiErrorResponse::new(
                    error_codes::TE_04,
                    format!(
                    "Default fallback not found for the provided created_by id: {}",
                    creator_by
                ),
                    None,
                )),
            )
                .into_response(),

            EuclidErrors::FailedToEvaluateOutput(msg) => (
                hyper::StatusCode::BAD_REQUEST,
                axum::Json(ApiErrorResponse::new(
                    error_codes::TE_04,
                    format!(
                    "Failed to evaluate output for algorithm kind : {}",
                    msg
                ),
                    None,
                )),
            ).into_response(),

            EuclidErrors::RoutingInterpretationFailed => (
                hyper::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(ApiErrorResponse::new(
                    error_codes::TE_04,
                    "unable to evaluate output for routing algorithm against the provided parameters".to_string(),
                    None,
                )),
            )
                .into_response(),

            EuclidErrors::InvalidSrDimensionConfig(msg) => (
                hyper::StatusCode::BAD_REQUEST,
                axum::Json(ApiErrorResponse::new(
                    error_codes::TE_04,
                    msg,
                    None,
                )),
            )
                .into_response(),

            EuclidErrors::FieldValueOutOfRange(msg) => (
                hyper::StatusCode::BAD_REQUEST,
                axum::Json(ApiErrorResponse::new(
                    error_codes::TE_04,
                    format!("Field value out of range: {}", msg),
                    None,
                )),
            )
                .into_response(),

            EuclidErrors::FieldLengthInvalid(msg) => (
                hyper::StatusCode::BAD_REQUEST,
                axum::Json(ApiErrorResponse::new(
                    error_codes::TE_04,
                    format!("Field length invalid: {}", msg),
                    None,
                )),
            )
                .into_response(),

            EuclidErrors::FieldPatternMismatch(msg) => (
                hyper::StatusCode::BAD_REQUEST,
                axum::Json(ApiErrorResponse::new(
                    error_codes::TE_04,
                    format!("Field pattern mismatch: {}", msg),
                    None,
                )),
            )
                .into_response(),

            EuclidErrors::FieldValidationFailed(msg) => (
                hyper::StatusCode::BAD_REQUEST,
                axum::Json(ApiErrorResponse::new(
                    error_codes::TE_04,
                    format!("Field validation failed: {}", msg),
                    None,
                )),
            )
                .into_response(),
        }
    }
}
