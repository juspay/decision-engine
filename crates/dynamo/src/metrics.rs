use error_stack::ResultExt;
use lazy_static::lazy_static;
use prometheus::{
    self, exponential_buckets, register_histogram, register_int_counter, Encoder, Histogram,
    IntCounter, TextEncoder,
};

const MICROS_500: f64 = 0.0001;

lazy_static! {
    pub static ref SUCCESS_BASED_ROUTING_METRICS_REQUEST: IntCounter = register_int_counter!(
        "success_based_routing_metrics_request",
        "total success based routing request received"
    )
    .unwrap();
    pub static ref SUCCESS_BASED_ROUTING__METRICS_SUCCESSFUL_RESPONSE_COUNT: IntCounter =
        register_int_counter!(
            "success_based_routing_metrics_successful_response",
            "total success based successful routing response sent count"
        )
        .unwrap();
    pub static ref SUCCESS_BASED_ROUTING_METRICS_UNSUCCESSFUL_RESPONSE_COUNT: IntCounter =
        register_int_counter!(
            "success_based_routing_metrics_unsuccessful_response",
            "total success based unsuccessful routing response sent count"
        )
        .unwrap();
    pub static ref SUCCESS_BASED_ROUTING_METRICS_DECISION_REQUEST_TIME: Histogram =
        register_histogram!(
            "success_based_routing_metrics_decision_request_time",
            "Time taken to process success based routing request (in seconds)",
            #[allow(clippy::expect_used)]
            exponential_buckets(MICROS_500, 2.0, 10).expect("failed to create histogram")
        )
        .unwrap();
    pub static ref SUCCESS_BASED_ROUTING_UPDATE_WINDOW_COUNT: IntCounter = register_int_counter!(
        "success_based_routing_update_window_count",
        "total success based routing update window count"
    )
    .unwrap();
    pub static ref SUCCESS_BASED_ROUTING_UPDATE_WINDOW_DECISION_REQUEST_TIME: Histogram =
        register_histogram!(
            "success_based_routing_update_window_decision_request_time",
            "Time taken to process success based routing update window request (in seconds)",
            #[allow(clippy::expect_used)]
            exponential_buckets(MICROS_500, 2.0, 10).expect("failed to create histogram")
        )
        .unwrap();
    pub static ref SUCCESS_BASED_ROUTING_REQUEST: IntCounter = register_int_counter!(
        "success_based_routing_requests",
        "total success based routing request received"
    )
    .unwrap();
    pub static ref SUCCESS_BASED_ROUTING_SUCCESSFUL_RESPONSE_COUNT: IntCounter =
        register_int_counter!(
            "success_based_routing_successful_response",
            "total success based successful routing response sent count"
        )
        .unwrap();
    pub static ref SUCCESS_BASED_ROUTING_UNSUCCESSFUL_RESPONSE_COUNT: IntCounter =
        register_int_counter!(
            "success_based_routing_unsuccessful_response",
            "total success based unsuccessful routing response sent count"
        )
        .unwrap();
    pub static ref SUCCESS_BASED_ROUTING_DECISION_REQUEST_TIME: Histogram = register_histogram!(
        "success_based_routing_decision_request_time",
        "Time taken to process success based routing request (in seconds)",
        #[allow(clippy::expect_used)]
        exponential_buckets(MICROS_500, 2.0, 10).expect("failed to create histogram")
    )
    .unwrap();
}

pub async fn metrics_handler() -> error_stack::Result<String, MetricsError> {
    let mut buffer = Vec::new();
    let encoder = TextEncoder::new();
    let metric_families = prometheus::gather();
    encoder
        .encode(&metric_families, &mut buffer)
        .change_context(MetricsError::EncodingError)?;
    String::from_utf8(buffer).change_context(MetricsError::Utf8Error)
}

#[derive(Debug, thiserror::Error)]
pub enum MetricsError {
    #[error("Error encoding metrics")]
    EncodingError,
    #[error("Error converting metrics to utf8")]
    Utf8Error,
}
