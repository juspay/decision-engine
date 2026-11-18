//! Masked request and response copies for API analytics events.
//!
//! `capture_api_event` runs each captured request inside [`ApiPayloadCapture::scope`]. Within that
//! scope, the shared `Json<T>` and `ScopedJson<T>` extractors and the `Json<T>` response wrapper
//! record a masked copy of their typed value. Outside it (analytics writes disabled, or a route
//! the analytics flows do not classify) recording is a no-op, so those requests pay nothing for
//! masking.

use std::future::Future;
use std::sync::{Arc, Mutex, OnceLock};

use serde::Serialize;
use serde_json::Value;

tokio::task_local! {
    static CAPTURE: ApiPayloadCapture;
}

/// A body without a typed snapshot must never fall back to its unmasked wire bytes.
pub(crate) fn omitted_payload() -> Value {
    Value::String("[OMITTED: no serializable typed analytics payload]".to_string())
}

/// Serialize while `Secret<T>` annotations still exist, before converting to JSON or bytes.
pub(crate) fn masked_payload<T: Serialize>(payload: &T) -> Value {
    masking::masked_serialize(payload).unwrap_or_else(|_| omitted_payload())
}

/// The identifiers an API event is indexed by, read from the request body.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct RequestFields {
    pub merchant_id: Option<String>,
    pub payment_id: Option<String>,
    pub auth_type: Option<String>,
}

impl RequestFields {
    const MERCHANT_ID: &'static [&'static [&'static str]] = &[
        &["merchant_id"],
        &["merchantId"],
        &["created_by"],
        &["createdBy"],
    ];
    const PAYMENT_ID: &'static [&'static [&'static str]] = &[
        &["payment_id"],
        &["paymentId"],
        &["payment_info", "payment_id"],
        &["paymentInfo", "paymentId"],
    ];
    const AUTH_TYPE: &'static [&'static [&'static str]] = &[
        &["authentication_type"],
        &["authenticationType"],
        &["auth_type"],
        &["authType"],
        &["payment_info", "authentication_type"],
        &["payment_info", "auth_type"],
        &["paymentInfo", "authenticationType"],
        &["paymentInfo", "authType"],
    ];

    /// Reads each field from the first of its known paths that holds a string, in snake_case or
    /// camelCase.
    pub(crate) fn from_json(body: &Value) -> Self {
        let first = |paths: &[&[&str]]| {
            paths.iter().find_map(|path| {
                path.iter()
                    .try_fold(body, |value, key| value.get(*key))
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
        };
        Self {
            merchant_id: first(Self::MERCHANT_ID),
            payment_id: first(Self::PAYMENT_ID),
            auth_type: first(Self::AUTH_TYPE),
        }
    }
}

/// The request body as analytics recorded it.
#[derive(Debug, Clone)]
enum RecordedRequest {
    /// Extracted as its type: the masked copy, and the fields read from it.
    Typed {
        masked: Value,
        fields: RequestFields,
    },
    /// Read but not extracted (too large, or not a valid instance of the type). Only the fields
    /// are kept, read from the body when it is JSON.
    Unparsed { fields: RequestFields },
}

/// The masked request and response copies of one captured API call.
#[derive(Clone, Default)]
pub(crate) struct ApiPayloadCapture {
    request: Arc<OnceLock<RecordedRequest>>,
    response: Arc<Mutex<Option<Value>>>,
}

impl ApiPayloadCapture {
    /// Runs `future` with this capture as the current one.
    pub(crate) async fn scope<F: Future>(&self, future: F) -> F::Output {
        CAPTURE.scope(self.clone(), future).await
    }

    /// The masked request; the omission marker when a body was read but not extracted; empty
    /// when no body was read.
    pub(crate) fn request_body(&self) -> String {
        match self.request.get() {
            Some(RecordedRequest::Typed { masked, .. }) => masked.to_string(),
            Some(RecordedRequest::Unparsed { .. }) => omitted_payload().to_string(),
            None => String::new(),
        }
    }

    /// The identifiers read from the request body, if one was read.
    pub(crate) fn request_fields(&self) -> RequestFields {
        match self.request.get() {
            Some(RecordedRequest::Typed { fields, .. } | RecordedRequest::Unparsed { fields }) => {
                fields.clone()
            }
            None => RequestFields::default(),
        }
    }

    /// The masked response, or the omission marker when no typed response was written.
    pub(crate) fn response_snapshot(&self) -> Value {
        self.response_value().unwrap_or_else(omitted_payload)
    }

    fn response_value(&self) -> Option<Value> {
        self.response
            .lock()
            .ok()
            .and_then(|response| response.as_ref().cloned())
    }

    fn set_response(&self, value: Value) {
        if let Ok(mut response) = self.response.lock() {
            *response = Some(value);
        }
    }
}

fn record(request: impl FnOnce() -> RecordedRequest) {
    let _ = CAPTURE.try_with(|capture| {
        capture.request.get_or_init(request);
    });
}

/// Records the typed request body. The first recorded body is kept.
pub(crate) fn record_request<T: Serialize>(payload: &T) {
    record(|| {
        let masked = masked_payload(payload);
        let fields = RequestFields::from_json(&masked);
        RecordedRequest::Typed { masked, fields }
    });
}

/// Records a body that was read but did not extract as its type. Only its identifiers are kept.
/// An empty body records nothing, as if none was sent.
pub(crate) fn record_unparsed_request(body: &[u8]) {
    if body.is_empty() {
        return;
    }
    record(|| RecordedRequest::Unparsed {
        fields: serde_json::from_slice(body)
            .map(|value| RequestFields::from_json(&value))
            .unwrap_or_default(),
    });
}

/// Records a body that could not be read (over the route's limit, or the stream failed).
pub(crate) fn record_unread_request() {
    record(|| RecordedRequest::Unparsed {
        fields: RequestFields::default(),
    });
}

/// Records the typed response body. The response written last is the one sent, so it replaces any
/// earlier one.
pub(crate) fn record_response<T: Serialize>(payload: &T) {
    let _ = CAPTURE.try_with(|capture| capture.set_response(masked_payload(payload)));
}

/// Records a response that was masked from its typed value before being assembled as JSON.
pub(crate) fn record_masked_response(masked: Value) {
    let _ = CAPTURE.try_with(|capture| capture.set_response(masked));
}

/// The masked response recorded so far in the current capture, if any.
pub(crate) fn current_response_snapshot() -> Option<Value> {
    CAPTURE
        .try_with(ApiPayloadCapture::response_value)
        .ok()
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::{
        current_response_snapshot, masked_payload, omitted_payload, ApiPayloadCapture,
        RequestFields,
    };
    use crate::custom_extractors::Json;
    use axum::{body::Body, http::Request, response::IntoResponse, routing::post, Router};
    use masking::Secret;
    use serde::{Deserialize, Serialize};
    use tower::ServiceExt;

    const SECRET: &str = "sk_live_do_not_capture";

    #[derive(Serialize, Deserialize)]
    struct Credentials {
        user: String,
        password: Secret<String>,
    }

    #[derive(Serialize, Deserialize)]
    struct LoginRequest {
        credentials: Credentials,
        backup_codes: Vec<Secret<String>>,
        otp: Option<Secret<String>>,
    }

    #[derive(Serialize)]
    struct LoginResponse {
        token: Secret<String>,
        user: String,
    }

    fn assert_masked(value: &serde_json::Value) {
        let text = value.to_string();
        assert!(!text.contains(SECRET), "secret leaked into {text}");
    }

    fn login_router() -> Router {
        async fn login(Json(request): Json<LoginRequest>) -> Json<LoginResponse> {
            Json(LoginResponse {
                token: Secret::new(SECRET.to_string()),
                user: request.credentials.user,
            })
        }
        Router::new().route("/login", post(login))
    }

    fn login_request() -> Request<Body> {
        let body = serde_json::json!({
            "credentials": { "user": "ada", "password": SECRET },
            "backup_codes": [SECRET],
            "otp": null,
        });
        Request::builder()
            .method("POST")
            .uri("/login")
            .header("content-type", "application/json")
            .body(Body::from(body.to_string()))
            .unwrap()
    }

    async fn body_json(response: axum::response::Response) -> serde_json::Value {
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[test]
    fn masking_follows_the_type_through_nested_options_and_lists() {
        let request = LoginRequest {
            credentials: Credentials {
                user: "ada".to_string(),
                password: Secret::new(SECRET.to_string()),
            },
            backup_codes: vec![Secret::new(SECRET.to_string())],
            otp: None,
        };
        let masked = masked_payload(&request);
        assert_masked(&masked);
        assert_eq!(masked["credentials"]["user"], "ada");
        assert!(masked["otp"].is_null());
        assert!(serde_json::to_string(&request).unwrap().contains(SECRET));
    }

    #[tokio::test]
    async fn a_captured_call_records_masked_copies_and_sends_real_values() {
        let capture = ApiPayloadCapture::default();
        let response = capture
            .scope(login_router().oneshot(login_request()))
            .await
            .unwrap();

        assert_eq!(body_json(response).await["token"], SECRET);
        let request: serde_json::Value = serde_json::from_str(&capture.request_body()).unwrap();
        assert_masked(&request);
        assert_eq!(request["credentials"]["user"], "ada");
        let response = capture.response_snapshot();
        assert_masked(&response);
        assert_eq!(response["user"], "ada");
    }

    #[tokio::test]
    async fn an_uncaptured_call_records_nothing() {
        let response = login_router().oneshot(login_request()).await.unwrap();
        assert_eq!(body_json(response).await["token"], SECRET);
        assert!(current_response_snapshot().is_none());
    }

    #[tokio::test]
    async fn a_body_that_fails_extraction_is_omitted_not_copied() {
        let capture = ApiPayloadCapture::default();
        let request = Request::builder()
            .method("POST")
            .uri("/login")
            .header("content-type", "application/json")
            .body(Body::from(format!(
                r#"{{"credentials": {{"password": "{SECRET}"}}}}"#
            )))
            .unwrap();
        let response = capture
            .scope(login_router().oneshot(request))
            .await
            .unwrap();

        assert!(response.status().is_client_error());
        assert_eq!(capture.request_body(), omitted_payload().to_string());
        assert_eq!(capture.request_fields(), RequestFields::default());
        // The rejection is an `ApiErrorResponse`, captured from its own typed value.
        assert_masked(&capture.response_snapshot());
    }

    #[tokio::test]
    async fn a_scoped_body_is_recorded_masked() {
        use axum::extract::FromRequest;

        #[derive(Serialize, Deserialize, crate::auth::MerchantScoped)]
        struct ScopedCredentials {
            #[merchant]
            merchant_id: String,
            webhook_secret: Secret<String>,
        }

        let capture = ApiPayloadCapture::default();
        let mut request = Request::builder()
            .header("content-type", "application/json")
            .body(Body::from(format!(
                r#"{{"merchant_id": "m1", "webhook_secret": "{SECRET}"}}"#
            )))
            .unwrap();
        request
            .extensions_mut()
            .insert(crate::auth::AuthContext::from_api_key("m1"));
        let extracted = capture
            .scope(
                crate::custom_extractors::ScopedJson::<ScopedCredentials>::from_request(
                    request,
                    &(),
                ),
            )
            .await;

        assert!(extracted.is_ok());
        let snapshot: serde_json::Value = serde_json::from_str(&capture.request_body()).unwrap();
        assert_masked(&snapshot);
        assert_eq!(snapshot["merchant_id"], "m1");
        assert_eq!(capture.request_fields().merchant_id.as_deref(), Some("m1"));
    }

    #[tokio::test]
    async fn the_response_written_last_is_the_one_captured() {
        let capture = ApiPayloadCapture::default();
        capture
            .scope(async {
                let _ = Json(LoginResponse {
                    token: Secret::new(SECRET.to_string()),
                    user: "first".to_string(),
                })
                .into_response();
                let _ = crate::error::ApiErrorResponse::new("TE_00", "failed".to_string(), None)
                    .into_response();
            })
            .await;
        assert_eq!(capture.response_snapshot()["message"], "failed");
    }

    #[tokio::test]
    async fn an_assembled_response_records_its_masked_copy_not_its_wire_map() {
        let wire = serde_json::json!({ "token": SECRET });
        let masked = masked_payload(&LoginResponse {
            token: Secret::new(SECRET.to_string()),
            user: "ada".to_string(),
        });
        let capture = ApiPayloadCapture::default();
        let response = capture
            .scope(async {
                crate::custom_extractors::json_response_with_masked(wire, masked.clone())
            })
            .await;

        assert_eq!(body_json(response).await["token"], SECRET);
        assert_eq!(capture.response_snapshot(), masked);
    }

    /// Secrets in responses of captured routes, and in the request DTOs of other flows.
    #[test]
    fn issued_credentials_are_masked_in_their_dtos() {
        let merchant = crate::routes::merchant_account_config::MerchantAccountCreateResponse {
            message: "created".to_string(),
            merchant_id: "m1".to_string(),
            gateway_success_rate_based_decider_input: None,
            api_key: Some(Secret::new(SECRET.to_string())),
        };
        let api_key = crate::routes::api_key::CreateApiKeyResponse {
            key_id: "k1".to_string(),
            api_key: Secret::new(SECRET.to_string()),
            key_prefix: "sk_live".to_string(),
            merchant_id: "m1".to_string(),
            description: None,
            created_at: time::PrimitiveDateTime::MIN,
        };
        let invite = crate::routes::user_auth::InviteMemberResponse {
            email: "ada@example.com".to_string(),
            is_new_user: true,
            password: Some(Secret::new(SECRET.to_string())),
            role: "admin".to_string(),
        };
        assert_masked(&masked_payload(&merchant));
        assert_masked(&masked_payload(&api_key));
        assert_masked(&masked_payload(&invite));
        assert!(serde_json::to_string(&merchant).unwrap().contains(SECRET));
    }

    #[tokio::test]
    async fn a_body_that_fails_extraction_keeps_its_identifiers() {
        let capture = ApiPayloadCapture::default();
        let request = Request::builder()
            .method("POST")
            .uri("/login")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"merchant_id": "m1", "payment_id": "pay_1", "credentials": 7}"#,
            ))
            .unwrap();
        let response = capture
            .scope(login_router().oneshot(request))
            .await
            .unwrap();

        assert!(response.status().is_client_error());
        assert_eq!(capture.request_body(), omitted_payload().to_string());
        let fields = capture.request_fields();
        assert_eq!(fields.merchant_id.as_deref(), Some("m1"));
        assert_eq!(fields.payment_id.as_deref(), Some("pay_1"));
    }

    #[tokio::test]
    async fn a_request_without_a_body_records_an_empty_request() {
        let capture = ApiPayloadCapture::default();
        let request = Request::builder()
            .method("POST")
            .uri("/login")
            .header("content-type", "application/json")
            .body(Body::empty())
            .unwrap();
        let _ = capture.scope(login_router().oneshot(request)).await;
        assert_eq!(capture.request_body(), "");
    }

    #[tokio::test]
    async fn a_body_without_a_json_content_type_is_not_read() {
        let capture = ApiPayloadCapture::default();
        let request = Request::builder()
            .method("POST")
            .uri("/login")
            .body(Body::from(r#"{"merchant_id": "m1"}"#))
            .unwrap();
        let response = capture
            .scope(login_router().oneshot(request))
            .await
            .unwrap();
        assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
        assert_eq!(capture.request_body(), "");
    }

    #[test]
    fn identifiers_are_read_in_either_case() {
        let fields = |body: &str| RequestFields::from_json(&serde_json::from_str(body).unwrap());
        assert_eq!(
            fields(
                r#"{"payment_id":"pay_decide","merchant_id":"pro_decide","routing_config":{"algorithm_for":"payment"},"authentication_type":"three_ds"}"#
            ),
            RequestFields {
                merchant_id: Some("pro_decide".to_string()),
                payment_id: Some("pay_decide".to_string()),
                auth_type: Some("three_ds".to_string()),
            }
        );
        assert_eq!(
            fields(
                r#"{"merchantId":"demo_merchant","paymentInfo":{"paymentId":"pay_001","authType":"THREE_DS"}}"#
            ),
            RequestFields {
                merchant_id: Some("demo_merchant".to_string()),
                payment_id: Some("pay_001".to_string()),
                auth_type: Some("THREE_DS".to_string()),
            }
        );
    }

    /// `cargo test --lib response_capture_cost -- --ignored --nocapture`, ideally with `--release`.
    /// Compares writing the documented `/decide` response with and without an active capture.
    #[tokio::test]
    #[ignore = "micro-benchmark"]
    async fn response_capture_cost() {
        let docs = include_str!("../../docs/api-reference/endpoint/decide.mdx");
        let start = docs
            .find("## Response")
            .and_then(|at| docs[at..].find("```json").map(|offset| at + offset + 7))
            .unwrap();
        let end = start + docs[start..].find("```").unwrap();
        let body: serde_json::Value = serde_json::from_str(&docs[start..end]).unwrap();

        const ROUNDS: u32 = 20_000;
        let write = |rounds: u32| {
            let started = std::time::Instant::now();
            for _ in 0..rounds {
                std::hint::black_box(Json(body.clone()).into_response());
            }
            started.elapsed() / rounds
        };
        write(1_000);
        let uncaptured = write(ROUNDS);
        let capture = ApiPayloadCapture::default();
        let captured = capture.scope(async { write(ROUNDS) }).await;
        println!(
            "decide response ({} bytes): uncaptured {uncaptured:?}/response, captured {captured:?}/response",
            end - start
        );
    }
}
