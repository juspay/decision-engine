use std::collections::HashMap;

use axum::{body::Body, extract::Request};

use rand::{distributions::Alphanumeric, Rng};

use crate::{
    consts,
    handlers::simulate::types::{CsvRecord, Payment},
};

/// Record the header's fields in request's trace
pub fn record_fields_from_header(request: &Request<Body>) -> tracing::Span {
    let span = tracing::debug_span!(
        "request",
        method = %request.method(),
        uri = %request.uri(),
        version = ?request.version(),
        tenant_id = tracing::field::Empty,
        request_id = tracing::field::Empty,
        merchant_id = tracing::field::Empty,
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

    if let Some(path_and_query) = request.uri().path_and_query() {
        path_and_query
            .path()
            .trim_start_matches("/simulate/")
            .split('/')
            .collect::<Vec<_>>()
            .first()
            .map(|merchant_id| span.record("merchant_id", merchant_id));
    }

    span
}

pub fn to_percentage(num: f64) -> f64 {
    if num.is_nan() {
        0.0
    } else {
        (num * 100.0 * 100.0).round() / 100.0
    }
}

pub fn reorder_records(records: Vec<CsvRecord>) -> Vec<Payment> {
    let mut order_map: HashMap<String, Vec<CsvRecord>> = HashMap::new();

    for payment in records {
        order_map
            .entry(payment.payment_intent_id.clone())
            .or_default()
            .push(payment);
    }

    let mut order_groups = Vec::with_capacity(order_map.len());

    for (payment_intent_id, payment_attempts) in &mut order_map {
        let first_attempt_data = payment_attempts
            .iter()
            .min_by_key(|attempt| attempt.created_at)
            .map(|attempt| (attempt.created_at, attempt.payment_status));

        if let Some((first_attempt_created_at, first_attempt_payment_status)) = first_attempt_data {
            payment_attempts.sort_by_key(|payment| payment.created_at);

            order_groups.push(Payment {
                payment_intent_id: payment_intent_id.clone(),
                first_attempt_created_at,
                first_attempt_payment_status,
                payment_attempts: payment_attempts.clone(),
            });
        }
    }

    order_groups.sort_by(|payment_1, payment_2| {
        payment_1
            .first_attempt_created_at
            .cmp(&payment_2.first_attempt_created_at)
            .then(
                payment_1
                    .payment_intent_id
                    .cmp(&payment_2.payment_intent_id),
            )
    });

    order_groups
}

pub fn generate_random_id() -> String {
    let mut rng = rand::thread_rng();
    (0..7)
        .map(|_| rng.sample(Alphanumeric))
        .map(char::from)
        .collect()
}
