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

#[cfg(test)]
mod tests {
    // Assertions report test failures; Result propagates fixture and body collection errors.
    #![allow(clippy::panic_in_result_fn)]

    use serde_json::json;

    use super::{redact_body, OMITTED_BODY};

    #[test]
    fn redacts_nested_api_keys_and_preserves_other_fields() {
        let body = json!({
            "merchant_id": "merchant_test",
            "api_key": "test-secret-root",
            "accounts": [
                {"apiKey": "test-secret-camel", "connector": "stripe"},
                {"credentials": {"API-KEY": {"value": "test-secret-nested"}}}
            ],
            "api_key_prefix": "DE_example",
            "optional": {"api_key": null}
        });

        let redacted = redact_body(&body.to_string());
        let expected = json!({
            "merchant_id": "merchant_test",
            "api_key": "[REDACTED]",
            "accounts": [
                {"apiKey": "[REDACTED]", "connector": "stripe"},
                {"credentials": {"API-KEY": "[REDACTED]"}}
            ],
            "api_key_prefix": "DE_example",
            "optional": {"api_key": null}
        });

        assert_eq!(redacted, expected.to_string());
        assert!(!redacted.contains("test-secret"));
    }

    #[test]
    fn omits_unparsable_bodies_instead_of_leaking_keys() {
        for body in [
            r#"{"api_key":"test-secret-truncated""#,
            "api_key=test-secret-form-body",
            "\u{fffd}test-secret-invalid-utf8",
        ] {
            assert_eq!(redact_body(body), OMITTED_BODY);
        }
    }

    #[test]
    fn preserves_payment_data_and_null_keys() -> Result<(), serde_json::Error> {
        for body in [
            "{\n  \"payment_id\": \"pay_test\", \"amount\": 100\n}",
            r#"{"api_key":null,"api_key_prefix":"DE_example"}"#,
        ] {
            assert_eq!(
                serde_json::from_str::<serde_json::Value>(&redact_body(body))?,
                serde_json::from_str::<serde_json::Value>(body)?
            );
        }
        assert_eq!(redact_body(""), "");
        assert_eq!(redact_body(" \n"), " \n");
        Ok(())
    }

    #[test]
    fn duplicate_members_cannot_hide_a_key_from_redaction() {
        for body in [
            r#"{"api_key":"test-secret-shadowed","api_key":null}"#,
            r#"{"credentials":{"api_key":"test-secret-shadowed"},"credentials":null}"#,
            r#"{"api_key":null,"api_key":"test-secret-last"}"#,
        ] {
            assert!(!redact_body(body).contains("test-secret"));
        }
    }

    #[tokio::test]
    async fn analytics_redaction_does_not_change_the_delivered_body(
    ) -> Result<(), Box<dyn std::error::Error>> {
        use axum::body::Body;
        use http_body_util::BodyExt;

        let original = r#"{"merchant_id":"merchant_test","api_key":"test-secret-issued"}"#;
        let (body, handle) = crate::analytics::CaptureBody::new(Body::from(original));
        let delivered = body.collect().await?.to_bytes();
        let captured = handle.wait().await;
        let analytics_body = redact_body(std::str::from_utf8(&captured.bytes)?);

        assert_eq!(delivered.as_ref(), original.as_bytes());
        assert!(!analytics_body.contains("test-secret-issued"));
        assert!(analytics_body.contains("[REDACTED]"));
        Ok(())
    }
}
