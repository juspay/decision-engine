use serde_json::Value;

use super::ApiEvent;

const REDACTED: &str = "[REDACTED]";
const OMITTED_BODY: &str = r#""[REDACTED: non-JSON body]""#;

/// Sanitize only the analytics copy, before it enters the publisher queue.
pub(super) fn redact_api_event(event: &mut ApiEvent) {
    event.request = redact_body(&event.request);
    event.response = event.response.as_deref().map(redact_body);
    if let Some(error) = event.error.as_mut() {
        redact_value(error);
    }
}

fn redact_body(body: &str) -> String {
    if body.trim().is_empty() {
        return body.to_string();
    }

    let Ok(mut value) = serde_json::from_str::<Value>(body) else {
        // Never fall back to raw bytes: malformed or truncated JSON can still contain keys.
        return OMITTED_BODY.to_string();
    };
    redact_value(&mut value);
    // Always re-encode: duplicate JSON members can shadow a key in the parsed value while
    // leaving the original credential in the raw body.
    serde_json::to_string(&value).unwrap_or_else(|_| OMITTED_BODY.to_string())
}

fn redact_value(value: &mut Value) {
    match value {
        Value::Object(fields) => {
            for (key, value) in fields {
                if key.eq_ignore_ascii_case("api_key")
                    || key.eq_ignore_ascii_case("apiKey")
                    || key.eq_ignore_ascii_case("api-key")
                {
                    if !value.is_null() {
                        *value = Value::String(REDACTED.to_string());
                    }
                } else {
                    redact_value(value);
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                redact_value(value);
            }
        }
        _ => {}
    }
}
