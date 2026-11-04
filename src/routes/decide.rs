use crate::custom_extractors::{ConnectedMerchant, RequestIds};
use crate::error::{error_codes, ApiErrorResponse, ContainerError};
use crate::euclid::errors::EuclidErrors;
use crate::types::decide::response::DecideResponse;
use crate::types::decide::DecideRequest;
use axum::body::Bytes;
use axum::extract::rejection::BytesRejection;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

pub mod evaluation;

/// The merchant is never in the body: it is the `x-connected-merchant-id` header, checked against
/// the caller's credential, or the credential's own merchant when the header is absent.
#[axum::debug_handler]
pub async fn decide(
    ids: RequestIds,
    ConnectedMerchant(merchant): ConnectedMerchant,
    body: Result<Bytes, BytesRejection>,
) -> Response {
    let body = match body {
        Ok(body) => body,
        Err(rejection) => {
            crate::logger::warn!(error = %rejection.body_text(), "/decide body could not be read");
            return DecideError::MalformedBody.into_response();
        }
    };
    let payload = match serde_json::from_slice::<DecideRequest>(&body) {
        Ok(payload) => payload.for_merchant(&merchant),
        Err(error) => {
            crate::logger::warn!(error = %error, "/decide body did not parse");
            return DecideError::from_unparsable(&body).into_response();
        }
    };
    match decide_payment(ids, payload).await {
        Ok(response) if response.status().is_success() => response,
        // A hybrid failure arrives already rendered in the inner engine's own format; only its
        // status class is kept. The evaluation has logged and recorded the detail.
        Ok(response) => DecideError::evaluation_failed(response.status()).into_response(),
        Err(error) => error.into_response(),
    }
}

async fn decide_payment(ids: RequestIds, payload: DecideRequest) -> Result<Response, DecideError> {
    let state = crate::app::get_tenant_app_state().await;
    let config = state.config.routing_config.as_ref().ok_or_else(|| {
        crate::logger::error!("routing configuration is unavailable for /decide");
        DecideError::Internal
    })?;
    let request = payload
        .to_hybrid_request(config)
        .map_err(DecideError::InvalidRequest)?;
    let invalid_inputs = payload.invalid_rule_inputs(config);
    let payment_id = payload.payment_id.clone();
    let merchant_reference_id = payload.merchant_reference_id.clone();
    evaluation::evaluate(
        ids,
        request,
        Some(payload),
        // The call's request id is the `x-request-id` response header, not part of the decision.
        move |evaluation, _request_id| {
            let decision = DecideResponse::from_evaluation(
                evaluation,
                invalid_inputs,
                payment_id,
                merchant_reference_id,
            );
            serde_json::to_value(decision)
                .map(Some)
                .map_err(|_| EuclidErrors::FailedToSerializeJsonToString.into())
        },
    )
    .await
    .map_err(DecideError::from)
}

/// Every error `/decide` answers with. Messages are written for the caller: a validation error
/// names the request field to fix, and anything else answers with a fixed message, so no
/// deserializer text, routing-key name or inner engine error reaches an external system. The
/// detail is logged instead.
#[derive(Debug)]
enum DecideError {
    /// The body is not JSON, or could not be read.
    MalformedBody,
    /// A field the contract does not have.
    UnknownField(String),
    /// A required field is absent.
    MissingField(String),
    /// A field of the wrong JSON type or out of range.
    InvalidField(String),
    /// A value the request validation rejected; the message is written by `/decide` itself.
    InvalidRequest(String),
    /// Routing could not be evaluated for an otherwise valid request.
    RoutingFailed,
    Internal,
}

impl DecideError {
    fn evaluation_failed(status: StatusCode) -> Self {
        if status.is_client_error() {
            Self::RoutingFailed
        } else {
            Self::Internal
        }
    }

    /// Reads which field failed, and how, from the body the extractor could not parse.
    fn from_unparsable(body: &[u8]) -> Self {
        let deserializer = &mut serde_json::Deserializer::from_slice(body);
        let Err(error) = serde_path_to_error::deserialize::<_, DecideRequest>(deserializer) else {
            return Self::MalformedBody;
        };
        let path = error.path().to_string();
        let inner = error.into_inner();
        if !inner.is_data() {
            return Self::MalformedBody;
        }
        let message = inner.to_string();
        if message.starts_with("unknown field") {
            Self::UnknownField(path)
        } else if let Some(field) = message
            .strip_prefix("missing field `")
            .and_then(|rest| rest.split('`').next())
        {
            Self::MissingField(match path.as_str() {
                "." | "" => field.to_string(),
                parent => format!("{parent}.{field}"),
            })
        } else {
            Self::InvalidField(path)
        }
    }

    fn status_and_message(&self) -> (StatusCode, &'static str, String) {
        let invalid = |message: String| (StatusCode::BAD_REQUEST, error_codes::TE_03, message);
        match self {
            Self::MalformedBody => invalid("Request body must be a valid JSON object".to_string()),
            Self::UnknownField(field) => invalid(format!("Unknown field `{field}`")),
            Self::MissingField(field) => invalid(format!("Missing required field `{field}`")),
            Self::InvalidField(field) => invalid(format!("Invalid value for field `{field}`")),
            Self::InvalidRequest(message) => invalid(message.clone()),
            Self::RoutingFailed => (
                StatusCode::BAD_REQUEST,
                error_codes::TE_04,
                "Unable to decide a connector for this payment".to_string(),
            ),
            Self::Internal => (
                StatusCode::INTERNAL_SERVER_ERROR,
                error_codes::TE_00,
                "Something went wrong".to_string(),
            ),
        }
    }
}

impl From<ContainerError<EuclidErrors>> for DecideError {
    fn from(error: ContainerError<EuclidErrors>) -> Self {
        crate::logger::error!(error = %error.get_inner(), "/decide evaluation failed");
        Self::Internal
    }
}

impl IntoResponse for DecideError {
    fn into_response(self) -> Response {
        let (status, code, message) = self.status_and_message();
        (status, ApiErrorResponse::new(code, message, None)).into_response()
    }
}

#[cfg(test)]
#[allow(clippy::panic_in_result_fn)]
mod tests {
    use super::*;
    use serde_json::json;

    fn parse_error(body: serde_json::Value) -> String {
        let body = serde_json::to_vec(&body).unwrap_or_default();
        DecideError::from_unparsable(&body).status_and_message().2
    }

    fn valid() -> serde_json::Value {
        serde_json::from_str(include_str!(
            "../../tests/fixtures/contracts/decide-request.json"
        ))
        .unwrap_or_default()
    }

    #[test]
    fn parse_errors_name_the_field_without_deserializer_text() {
        let mut unknown = valid();
        unknown["mandate_type"] = json!(null);
        assert_eq!(parse_error(unknown), "Unknown field `mandate_type`");

        let mut nested = valid();
        nested["routing_config"]["eligible_connectors"][0]["gateway_id"] = json!("mca_1");
        assert_eq!(
            parse_error(nested),
            "Unknown field `routing_config.eligible_connectors[0].gateway_id`"
        );

        let mut missing = valid();
        if let Some(body) = missing.as_object_mut() {
            body.remove("amount");
        }
        assert_eq!(parse_error(missing), "Missing required field `amount`");

        let mut missing_nested = valid();
        if let Some(config) = missing_nested["routing_config"].as_object_mut() {
            config.remove("algorithm_for");
        }
        assert_eq!(
            parse_error(missing_nested),
            "Missing required field `routing_config.algorithm_for`"
        );

        for amount in [json!(-1), json!("100"), json!(1.5)] {
            let mut wrong = valid();
            wrong["amount"] = amount;
            assert_eq!(parse_error(wrong), "Invalid value for field `amount`");
        }
    }

    #[test]
    fn malformed_json_is_named_as_such() {
        let message = DecideError::from_unparsable(b"{\"amount\": ")
            .status_and_message()
            .2;
        assert_eq!(message, "Request body must be a valid JSON object");
    }

    #[test]
    fn evaluation_failures_answer_with_fixed_messages() {
        let (status, _, message) =
            DecideError::evaluation_failed(StatusCode::NOT_FOUND).status_and_message();
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(message, "Unable to decide a connector for this payment");
        let (status, _, message) =
            DecideError::evaluation_failed(StatusCode::BAD_GATEWAY).status_and_message();
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(message, "Something went wrong");
    }
}
