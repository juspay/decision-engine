use std::sync::{Arc, OnceLock};

use serde::Serialize;
use serde_json::Value;

/// A body without a typed snapshot must never fall back to its unmasked wire bytes.
pub(crate) fn omitted_payload() -> Value {
    Value::String("[OMITTED: no serializable typed analytics payload]".to_string())
}

/// Serialize while `Secret<T>` annotations still exist, before converting to JSON or bytes.
pub(crate) fn masked_payload<T: Serialize>(payload: &T) -> Value {
    masking::masked_serialize(payload).unwrap_or_else(|_| omitted_payload())
}

/// Shared by the capture middleware and the JSON request extractor.
#[derive(Clone, Default)]
pub(crate) struct RequestAnalyticsPayload(Arc<OnceLock<Value>>);

impl RequestAnalyticsPayload {
    pub(crate) fn record<T: Serialize>(&self, payload: &T) {
        let _ = self.0.set(masked_payload(payload));
    }

    pub(crate) fn snapshot(&self) -> Value {
        self.0.get().cloned().unwrap_or_else(omitted_payload)
    }
}

/// Created from a typed response by `custom_extractors::Json`, separately from its wire body.
#[derive(Clone)]
pub(crate) struct ResponseAnalyticsPayload(pub(crate) Value);
