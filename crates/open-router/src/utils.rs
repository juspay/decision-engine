use axum::{body::Body, extract::Request};
use serde::Deserialize;
use error_stack::ResultExt;
use crate::{decider::gatewaydecider::runner::ResponseBody, error::ApiClientError, storage::consts};
use reqwest::{Client, header::{HeaderMap, HeaderValue, CONTENT_TYPE}};
use serde_json::Value;
use hyper::StatusCode;

use rand::Rng;

use std::time::{SystemTime, UNIX_EPOCH};
/// Date-time utilities.
pub mod date_time {
    use time::{OffsetDateTime, PrimitiveDateTime};

    /// Create a new [`PrimitiveDateTime`] with the current date and time in UTC.
    pub fn now() -> PrimitiveDateTime {
        let utc_date_time = OffsetDateTime::now_utc();
        PrimitiveDateTime::new(utc_date_time.date(), utc_date_time.time())
    }
}

/// Record the header's fields in request's trace
pub fn record_fields_from_header(request: &Request<Body>) -> tracing::Span {
    let span = tracing::debug_span!(
        "request",
        method = %request.method(),
        uri = %request.uri(),
        version = ?request.version(),
        tenant_id = tracing::field::Empty,
        request_id = tracing::field::Empty,
    );
    request
        .headers()
        .get(consts::X_TENANT_ID)
        .and_then(|value| value.to_str().ok())
        .map(|tenant_id| span.record("tenant_id", tenant_id));

    request
        .headers()
        .get(consts::X_REQUEST_ID)
        .and_then(|value| value.to_str().ok())
        .map(|request_id| span.record("request_id", request_id));

    span
}

/// Effectively, equivalent to `Result<T, error_stack::Report<E>>`
pub type CustomResult<T, E> = error_stack::Result<T, E>;

/// Parsing Errors
#[allow(missing_docs)] // Only to prevent warnings about struct fields not being documented
#[derive(Debug, thiserror::Error)]
pub enum ParsingError {
    ///Failed to parse enum
    #[error("Failed to parse enum: {0}")]
    EnumParseFailure(&'static str),
    ///Failed to parse struct
    #[error("Failed to parse struct: {0}")]
    StructParseFailure(&'static str),
    /// Failed to encode data to given format
    #[error("Failed to serialize to {0} format")]
    EncodeError(&'static str),
    /// Failed to parse data
    #[error("Unknown error while parsing")]
    UnknownError,
    /// Failed to parse datetime
    #[error("Failed to parse datetime")]
    DateTimeParsingError,
    /// Failed to parse email
    #[error("Failed to parse email")]
    EmailParsingError,
    /// Failed to parse phone number
    #[error("Failed to parse phone number")]
    PhoneNumberParsingError,
    /// Failed to parse Float value for converting to decimal points
    #[error("Failed to parse Float value for converting to decimal points")]
    FloatToDecimalConversionFailure,
    /// Failed to parse Decimal value for i64 value conversion
    #[error("Failed to parse Decimal value for i64 value conversion")]
    DecimalToI64ConversionFailure,
    /// Failed to parse string value for f64 value conversion
    #[error("Failed to parse string value for f64 value conversion")]
    StringToFloatConversionFailure,
    /// Failed to parse i64 value for f64 value conversion
    #[error("Failed to parse i64 value for f64 value conversion")]
    I64ToDecimalConversionFailure,
    /// Failed to parse String value to Decimal value conversion because `error`
    #[error("Failed to parse String value to Decimal value conversion because {error}")]
    StringToDecimalConversionFailure { error: String },
    /// Failed to convert the given integer because of integer overflow error
    #[error("Integer Overflow error")]
    IntegerOverflow,
}

/// Extending functionalities of `String` for performing parsing
pub trait StringExt<T> {
    /// Convert `String` into type `<T>` (which being an `enum`)
    fn parse_enum(self, enum_name: &'static str) -> CustomResult<T, ParsingError>
    where
        T: std::str::FromStr,
        // Requirement for converting the `Err` variant of `FromStr` to `Report<Err>`
        <T as std::str::FromStr>::Err: std::error::Error + Send + Sync + 'static;

    /// Convert `serde_json::Value` into type `<T>` by using `serde::Deserialize`
    fn parse_struct<'de>(
        &'de self,
        type_name: &'static str,
    ) -> CustomResult<T, ParsingError>
    where
        T: Deserialize<'de>;
}

impl<T> StringExt<T> for String {
    fn parse_enum(self, enum_name: &'static str) -> CustomResult<T, ParsingError>
    where
        T: std::str::FromStr,
        <T as std::str::FromStr>::Err: std::error::Error + Send + Sync + 'static,
    {
        T::from_str(&self)
            .change_context(ParsingError::EnumParseFailure(enum_name))
            .attach_printable_lazy(|| format!("Invalid enum variant {self:?} for enum {enum_name}"))
    }

    fn parse_struct<'de>(
        &'de self,
        type_name: &'static str,
    ) -> CustomResult<T, ParsingError>
    where
        T: Deserialize<'de>,
    {
        serde_json::from_str::<T>(self)
            .change_context(ParsingError::StructParseFailure(type_name))
            .attach_printable_lazy(|| {
                format!("Unable to parse {type_name} from string {:?}", &self)
            })
    }
}

pub async fn call_api(url: &str, body: &serde_json::Value) -> Result<ResponseBody, ApiClientError> {
    let client = Client::new();
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    // Send the request
    let response = client
        .post(url)
        .headers(headers)
        .body(body.to_string())
        .send()
        .await
        .map_err(|_| ApiClientError::RequestNotSent)?;

    let status_code = response.status();

    // Parse the response body
    let body_bytes = response.bytes().await.map_err(|_| ApiClientError::ResponseDecodingFailed)?;

    if status_code.is_success() {
        // Deserialize the response body into `ResponseBody`
        let body: ResponseBody = serde_json::from_slice(&body_bytes)
            .map_err(|_| ApiClientError::ResponseDecodingFailed)?;
        Ok(body)
    } else {
        // Map errors based on the status code
        match status_code {
            StatusCode::BAD_REQUEST => Err(ApiClientError::BadRequest(body_bytes)),
            StatusCode::UNAUTHORIZED => Err(ApiClientError::Unauthorized(body_bytes)),
            StatusCode::INTERNAL_SERVER_ERROR => Err(ApiClientError::InternalServerError(body_bytes)),
            _ => Err(ApiClientError::Unexpected {
                status_code,
                message: body_bytes,
            }),
        }
    }
}

pub fn generate_random_number(tag: String, range: (f64, f64)) -> f64 {
    let (min, max) = range;

    // Create a random number generator
    let mut rng = rand::thread_rng();
    
    // Handle invalid range
    if min > max {
        return rng.gen_range(max..=min)
    }
    rng.gen_range(min..=max)
    // Generate a random number in the range (inclusive)
}




pub fn get_current_date_in_millis() -> u128 {
    let since_epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    
    since_epoch.as_millis()
}

