use error_stack::{Result, ResultExt};
use http::request::Request;
use serde::Serialize;

use crate::{consts, error};

/// Record the header's fields in request's trace
pub fn record_fields_from_header<B: hyper::body::Body>(request: &Request<B>) -> tracing::Span {
    let url_path = request.uri().path();

    let span = tracing::debug_span!(
        "request",
        uri = %url_path,
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

pub trait ValueExt {
    /// Convert `serde_json::Value` into type `<T>` by using `serde::Deserialize`
    fn parse_value<T>(self, type_name: &'static str) -> Result<T, error::ParsingError>
    where
        T: serde::de::DeserializeOwned;
}

impl ValueExt for serde_json::Value {
    fn parse_value<T>(self, type_name: &'static str) -> Result<T, error::ParsingError>
    where
        T: serde::de::DeserializeOwned,
    {
        let debug = format!(
            "Unable to parse {type_name} from serde_json::Value: {:?}",
            &self
        );
        serde_json::from_value::<T>(self)
            .change_context(error::ParsingError::StructParseFailure(type_name))
            .attach_printable_lazy(|| debug)
    }
}

pub trait Encode<'e>
where
    Self: 'e + std::fmt::Debug,
{
    fn encode_to_value(&'e self) -> Result<serde_json::Value, error::ParsingError>
    where
        Self: Serialize;
}

impl<'e, A> Encode<'e> for A
where
    Self: 'e + std::fmt::Debug,
{
    fn encode_to_value(&'e self) -> Result<serde_json::Value, error::ParsingError>
    where
        Self: Serialize,
    {
        serde_json::to_value(self)
            .change_context(error::ParsingError::EncodeError("json-value"))
            .attach_printable_lazy(|| format!("Unable to convert {self:?} to a value"))
    }
}
