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

    #[error("Routing algorithm not active for merchant: {0}")]
    RoutingAlgorithmNotActive(String),

    #[error("default fallback not found in evaluate request for: {0}")]
    DefaultFallbackNotFound(String),

    #[error("Storage error")]
    StorageError,

    #[error("Invalid Sr Dimension Configuration")]
    InvalidSrDimensionConfig(String),

    #[error("Field validation failed: {0}")]
    FieldValidationFailed(String),
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct ValidationErrorDetails {
    pub field: String,
    pub error_type: String,
    pub message: String,
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
        }
    }
}

impl EuclidErrors {
    pub fn status_code(&self) -> hyper::StatusCode {
        match self {
            Self::GlobalRoutingConfigsUnavailable
            | Self::StorageError
            | Self::FailedToSerializeJsonToString
            | Self::RoutingInterpretationFailed => hyper::StatusCode::INTERNAL_SERVER_ERROR,
            Self::RoutingAlgorithmNotActive(_) => hyper::StatusCode::NOT_FOUND,
            Self::InvalidRuleConfiguration
            | Self::InvalidRequestParameter(_)
            | Self::FailedToValidateRoutingRule
            | Self::InvalidRequest(_)
            | Self::FailedToParseJsonInput
            | Self::ActiveRoutingAlgorithmNotFound(_)
            | Self::RoutingAlgorithmNotFound(_)
            | Self::DefaultFallbackNotFound(_)
            | Self::FailedToEvaluateOutput(_)
            | Self::InvalidSrDimensionConfig(_)
            | Self::FieldValidationFailed(_) => hyper::StatusCode::BAD_REQUEST,
        }
    }

    /// The message the caller sees.
    pub fn message(&self) -> String {
        match self {
            Self::InvalidRuleConfiguration => "Invalid routing rule configuration".to_string(),
            Self::InvalidRequestParameter(param) => format!("Invalid parameter: {param}"),
            Self::FailedToValidateRoutingRule => {
                "Failed to validate the provided routing rule".to_string()
            }
            Self::InvalidRequest(msg) => format!("Invalid request received : {msg}"),
            Self::GlobalRoutingConfigsUnavailable => "Routing configurations not found".to_string(),
            Self::StorageError => "Something went wrong".to_string(),
            Self::FailedToParseJsonInput => "Failed to parse Json received in request".to_string(),
            Self::FailedToSerializeJsonToString => "Failed to serialize Json ".to_string(),
            Self::ActiveRoutingAlgorithmNotFound(msg) => {
                format!("No active routing algorithm found for the created_by entity : {msg}")
            }
            Self::RoutingAlgorithmNotFound(msg) => {
                format!("Routing algorithm not found for the provided id : {msg}")
            }
            Self::RoutingAlgorithmNotActive(msg) => {
                format!("Routing algorithm not active for merchant: {msg}")
            }
            Self::DefaultFallbackNotFound(created_by) => {
                format!("Default fallback not found for the provided created_by id: {created_by}")
            }
            Self::FailedToEvaluateOutput(msg) => {
                format!("Failed to evaluate output for algorithm kind : {msg}")
            }
            Self::RoutingInterpretationFailed => {
                "unable to evaluate output for routing algorithm against the provided parameters"
                    .to_string()
            }
            Self::InvalidSrDimensionConfig(msg) => msg.clone(),
            Self::FieldValidationFailed(msg) => format!("Field validation failed: {msg}"),
        }
    }
}

impl axum::response::IntoResponse for EuclidErrors {
    fn into_response(self) -> axum::response::Response {
        (
            self.status_code(),
            ApiErrorResponse::new(error_codes::TE_04, self.message(), None),
        )
            .into_response()
    }
}
