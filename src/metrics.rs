use error_stack::ResultExt;
use lazy_static::lazy_static;
use prometheus::{
    self, exponential_buckets, register_histogram, register_int_counter, Encoder, Histogram,
    IntCounter, TextEncoder,
};

const MICROS_500: f64 = 0.0001;

lazy_static! {
    // Decide Gateway API metrics
    pub static ref DECIDE_GATEWAY_METRICS_REQUEST: IntCounter = register_int_counter!(
        "decide_gateway_metrics_request",
        "total decide gateway request received"
    )
    .unwrap();
    pub static ref DECIDE_GATEWAY_SUCCESSFUL_RESPONSE_COUNT: IntCounter = register_int_counter!(
        "decide_gateway_successful_response",
        "total decide gateway successful response sent count"
    )
    .unwrap();
    pub static ref DECIDE_GATEWAY_UNSUCCESSFUL_RESPONSE_COUNT: IntCounter = register_int_counter!(
        "decide_gateway_unsuccessful_response",
        "total decide gateway unsuccessful response sent count"
    )
    .unwrap();
    pub static ref DECIDE_GATEWAY_METRICS_DECISION_REQUEST_TIME: Histogram = register_histogram!(
        "decide_gateway_metrics_decision_request_time",
        "Time taken to process decide gateway request (in seconds)",
        exponential_buckets(MICROS_500, 2.0, 10).unwrap()
    )
    .unwrap();

    // Decision Gateway API metrics
    pub static ref DECISION_GATEWAY_METRICS_REQUEST: IntCounter = register_int_counter!(
        "decision_gateway_metrics_request",
        "total decision gateway request received"
    )
    .unwrap();
    pub static ref DECISION_GATEWAY_SUCCESSFUL_RESPONSE_COUNT: IntCounter = register_int_counter!(
        "decision_gateway_successful_response",
        "total decision gateway successful response sent count"
    )
    .unwrap();
    pub static ref DECISION_GATEWAY_UNSUCCESSFUL_RESPONSE_COUNT: IntCounter = register_int_counter!(
        "decision_gateway_unsuccessful_response",
        "total decision gateway unsuccessful response sent count"
    )
    .unwrap();
    pub static ref DECISION_GATEWAY_METRICS_DECISION_REQUEST_TIME: Histogram = register_histogram!(
        "decision_gateway_metrics_decision_request_time",
        "Time taken to process decision gateway request (in seconds)",
        exponential_buckets(MICROS_500, 2.0, 10).unwrap()
    )
    .unwrap();

    // Merchant Account Config GET API metrics
    pub static ref MERCHANT_CONFIG_GET_METRICS_REQUEST: IntCounter = register_int_counter!(
        "merchant_config_get_metrics_request",
        "total merchant config get request received"
    )
    .unwrap();
    pub static ref MERCHANT_CONFIG_GET_SUCCESSFUL_RESPONSE_COUNT: IntCounter = register_int_counter!(
        "merchant_config_get_successful_response",
        "total merchant config get successful response sent count"
    )
    .unwrap();
    pub static ref MERCHANT_CONFIG_GET_UNSUCCESSFUL_RESPONSE_COUNT: IntCounter = register_int_counter!(
        "merchant_config_get_unsuccessful_response",
        "total merchant config get unsuccessful response sent count"
    )
    .unwrap();
    pub static ref MERCHANT_CONFIG_GET_METRICS_DECISION_REQUEST_TIME: Histogram = register_histogram!(
        "merchant_config_get_metrics_decision_request_time",
        "Time taken to process merchant config get request (in seconds)",
        exponential_buckets(MICROS_500, 2.0, 10).unwrap()
    )
    .unwrap();

    // Merchant Account Config CREATE API metrics
    pub static ref MERCHANT_CONFIG_CREATE_METRICS_REQUEST: IntCounter = register_int_counter!(
        "merchant_config_create_metrics_request",
        "total merchant config create request received"
    )
    .unwrap();
    pub static ref MERCHANT_CONFIG_CREATE_SUCCESSFUL_RESPONSE_COUNT: IntCounter = register_int_counter!(
        "merchant_config_create_successful_response",
        "total merchant config create successful response sent count"
    )
    .unwrap();
    pub static ref MERCHANT_CONFIG_CREATE_UNSUCCESSFUL_RESPONSE_COUNT: IntCounter = register_int_counter!(
        "merchant_config_create_unsuccessful_response",
        "total merchant config create unsuccessful response sent count"
    )
    .unwrap();
    pub static ref MERCHANT_CONFIG_CREATE_METRICS_DECISION_REQUEST_TIME: Histogram = register_histogram!(
        "merchant_config_create_metrics_decision_request_time",
        "Time taken to process merchant config create request (in seconds)",
        exponential_buckets(MICROS_500, 2.0, 10).unwrap()
    )
    .unwrap();

    // Merchant Account Config DELETE API metrics
    pub static ref MERCHANT_CONFIG_DELETE_METRICS_REQUEST: IntCounter = register_int_counter!(
        "merchant_config_delete_metrics_request",
        "total merchant config delete request received"
    )
    .unwrap();
    pub static ref MERCHANT_CONFIG_DELETE_SUCCESSFUL_RESPONSE_COUNT: IntCounter = register_int_counter!(
        "merchant_config_delete_successful_response",
        "total merchant config delete successful response sent count"
    )
    .unwrap();
    pub static ref MERCHANT_CONFIG_DELETE_UNSUCCESSFUL_RESPONSE_COUNT: IntCounter = register_int_counter!(
        "merchant_config_delete_unsuccessful_response",
        "total merchant config delete unsuccessful response sent count"
    )
    .unwrap();
    pub static ref MERCHANT_CONFIG_DELETE_METRICS_DECISION_REQUEST_TIME: Histogram = register_histogram!(
        "merchant_config_delete_metrics_decision_request_time",
        "Time taken to process merchant config delete request (in seconds)",
        exponential_buckets(MICROS_500, 2.0, 10).unwrap()
    )
    .unwrap();

    // Rule Configuration CREATE API metrics
    pub static ref RULE_CONFIG_CREATE_METRICS_REQUEST: IntCounter = register_int_counter!(
        "rule_config_create_metrics_request",
        "total rule config create request received"
    )
    .unwrap();
    pub static ref RULE_CONFIG_CREATE_SUCCESSFUL_RESPONSE_COUNT: IntCounter = register_int_counter!(
        "rule_config_create_successful_response",
        "total rule config create successful response sent count"
    )
    .unwrap();
    pub static ref RULE_CONFIG_CREATE_UNSUCCESSFUL_RESPONSE_COUNT: IntCounter = register_int_counter!(
        "rule_config_create_unsuccessful_response",
        "total rule config create unsuccessful response sent count"
    )
    .unwrap();
    pub static ref RULE_CONFIG_CREATE_METRICS_DECISION_REQUEST_TIME: Histogram = register_histogram!(
        "rule_config_create_metrics_decision_request_time",
        "Time taken to process rule config create request (in seconds)",
        exponential_buckets(MICROS_500, 2.0, 10).unwrap()
    )
    .unwrap();

    // Rule Configuration GET API metrics
    pub static ref RULE_CONFIG_GET_METRICS_REQUEST: IntCounter = register_int_counter!(
        "rule_config_get_metrics_request",
        "total rule config get request received"
    )
    .unwrap();
    pub static ref RULE_CONFIG_GET_SUCCESSFUL_RESPONSE_COUNT: IntCounter = register_int_counter!(
        "rule_config_get_successful_response",
        "total rule config get successful response sent count"
    )
    .unwrap();
    pub static ref RULE_CONFIG_GET_UNSUCCESSFUL_RESPONSE_COUNT: IntCounter = register_int_counter!(
        "rule_config_get_unsuccessful_response",
        "total rule config get unsuccessful response sent count"
    )
    .unwrap();
    pub static ref RULE_CONFIG_GET_METRICS_DECISION_REQUEST_TIME: Histogram = register_histogram!(
        "rule_config_get_metrics_decision_request_time",
        "Time taken to process rule config get request (in seconds)",
        exponential_buckets(MICROS_500, 2.0, 10).unwrap()
    )
    .unwrap();

    // Rule Configuration UPDATE API metrics
    pub static ref RULE_CONFIG_UPDATE_METRICS_REQUEST: IntCounter = register_int_counter!(
        "rule_config_update_metrics_request",
        "total rule config update request received"
    )
    .unwrap();
    pub static ref RULE_CONFIG_UPDATE_SUCCESSFUL_RESPONSE_COUNT: IntCounter = register_int_counter!(
        "rule_config_update_successful_response",
        "total rule config update successful response sent count"
    )
    .unwrap();
    pub static ref RULE_CONFIG_UPDATE_UNSUCCESSFUL_RESPONSE_COUNT: IntCounter = register_int_counter!(
        "rule_config_update_unsuccessful_response",
        "total rule config update unsuccessful response sent count"
    )
    .unwrap();
    pub static ref RULE_CONFIG_UPDATE_METRICS_DECISION_REQUEST_TIME: Histogram = register_histogram!(
        "rule_config_update_metrics_decision_request_time",
        "Time taken to process rule config update request (in seconds)",
        exponential_buckets(MICROS_500, 2.0, 10).unwrap()
    )
    .unwrap();

    // Rule Configuration DELETE API metrics
    pub static ref RULE_CONFIG_DELETE_METRICS_REQUEST: IntCounter = register_int_counter!(
        "rule_config_delete_metrics_request",
        "total rule config delete request received"
    )
    .unwrap();
    pub static ref RULE_CONFIG_DELETE_SUCCESSFUL_RESPONSE_COUNT: IntCounter = register_int_counter!(
        "rule_config_delete_successful_response",
        "total rule config delete successful response sent count"
    )
    .unwrap();
    pub static ref RULE_CONFIG_DELETE_UNSUCCESSFUL_RESPONSE_COUNT: IntCounter = register_int_counter!(
        "rule_config_delete_unsuccessful_response",
        "total rule config delete unsuccessful response sent count"
    )
    .unwrap();
    pub static ref RULE_CONFIG_DELETE_METRICS_DECISION_REQUEST_TIME: Histogram = register_histogram!(
        "rule_config_delete_metrics_decision_request_time",
        "Time taken to process rule config delete request (in seconds)",
        exponential_buckets(MICROS_500, 2.0, 10).unwrap()
    )
    .unwrap();

    // Update Gateway Score API metrics
    pub static ref UPDATE_GATEWAY_SCORE_METRICS_REQUEST: IntCounter = register_int_counter!(
        "update_gateway_score_metrics_request",
        "total update gateway score request received"
    )
    .unwrap();
    pub static ref UPDATE_GATEWAY_SCORE_SUCCESSFUL_RESPONSE_COUNT: IntCounter = register_int_counter!(
        "update_gateway_score_successful_response",
        "total update gateway score successful response sent count"
    )
    .unwrap();
    pub static ref UPDATE_GATEWAY_SCORE_UNSUCCESSFUL_RESPONSE_COUNT: IntCounter = register_int_counter!(
        "update_gateway_score_unsuccessful_response",
        "total update gateway score unsuccessful response sent count"
    )
    .unwrap();
    pub static ref UPDATE_GATEWAY_SCORE_METRICS_DECISION_REQUEST_TIME: Histogram = register_histogram!(
        "update_gateway_score_metrics_decision_request_time",
        "Time taken to process update gateway score request (in seconds)",
        exponential_buckets(MICROS_500, 2.0, 10).unwrap()
    )
    .unwrap();

    // Update Score API metrics
    pub static ref UPDATE_SCORE_METRICS_REQUEST: IntCounter = register_int_counter!(
        "update_score_metrics_request",
        "total update score request received"
    )
    .unwrap();
    pub static ref UPDATE_SCORE_SUCCESSFUL_RESPONSE_COUNT: IntCounter = register_int_counter!(
        "update_score_successful_response",
        "total update score successful response sent count"
    )
    .unwrap();
    pub static ref UPDATE_SCORE_UNSUCCESSFUL_RESPONSE_COUNT: IntCounter = register_int_counter!(
        "update_score_unsuccessful_response",
        "total update score unsuccessful response sent count"
    )
    .unwrap();
    pub static ref UPDATE_SCORE_METRICS_DECISION_REQUEST_TIME: Histogram = register_histogram!(
        "update_score_metrics_decision_request_time",
        "Time taken to process update score request (in seconds)",
        exponential_buckets(MICROS_500, 2.0, 10).unwrap()
    )
    .unwrap();

    // Routing Create API metrics
    pub static ref ROUTING_CREATE_METRICS_REQUEST: IntCounter = register_int_counter!(
        "routing_create_metrics_request",
        "total routing create request received"
    )
    .unwrap();
    pub static ref ROUTING_CREATE_SUCCESSFUL_RESPONSE_COUNT: IntCounter = register_int_counter!(
        "routing_create_successful_response",
        "total routing create successful response sent count"
    )
    .unwrap();
    pub static ref ROUTING_CREATE_UNSUCCESSFUL_RESPONSE_COUNT: IntCounter = register_int_counter!(
        "routing_create_unsuccessful_response",
        "total routing create unsuccessful response sent count"
    )
    .unwrap();
    pub static ref ROUTING_CREATE_METRICS_DECISION_REQUEST_TIME: Histogram = register_histogram!(
        "routing_create_metrics_decision_request_time",
        "Time taken to process routing create request (in seconds)",
        exponential_buckets(MICROS_500, 2.0, 10).unwrap()
    )
    .unwrap();

    // Routing Activate API metrics
    pub static ref ROUTING_ACTIVATE_METRICS_REQUEST: IntCounter = register_int_counter!(
        "routing_activate_metrics_request",
        "total routing activate request received"
    )
    .unwrap();
    pub static ref ROUTING_ACTIVATE_SUCCESSFUL_RESPONSE_COUNT: IntCounter = register_int_counter!(
        "routing_activate_successful_response",
        "total routing activate successful response sent count"
    )
    .unwrap();
    pub static ref ROUTING_ACTIVATE_UNSUCCESSFUL_RESPONSE_COUNT: IntCounter = register_int_counter!(
        "routing_activate_unsuccessful_response",
        "total routing activate unsuccessful response sent count"
    )
    .unwrap();
    pub static ref ROUTING_ACTIVATE_METRICS_DECISION_REQUEST_TIME: Histogram = register_histogram!(
        "routing_activate_metrics_decision_request_time",
        "Time taken to process routing activate request (in seconds)",
        exponential_buckets(MICROS_500, 2.0, 10).unwrap()
    )
    .unwrap();

    // Routing List API metrics
    pub static ref ROUTING_LIST_METRICS_REQUEST: IntCounter = register_int_counter!(
        "routing_list_metrics_request",
        "total routing list request received"
    )
    .unwrap();
    pub static ref ROUTING_LIST_SUCCESSFUL_RESPONSE_COUNT: IntCounter = register_int_counter!(
        "routing_list_successful_response",
        "total routing list successful response sent count"
    )
    .unwrap();
    pub static ref ROUTING_LIST_UNSUCCESSFUL_RESPONSE_COUNT: IntCounter = register_int_counter!(
        "routing_list_unsuccessful_response",
        "total routing list unsuccessful response sent count"
    )
    .unwrap();
    pub static ref ROUTING_LIST_METRICS_DECISION_REQUEST_TIME: Histogram = register_histogram!(
        "routing_list_metrics_decision_request_time",
        "Time taken to process routing list request (in seconds)",
        exponential_buckets(MICROS_500, 2.0, 10).unwrap()
    )
    .unwrap();

    // Routing List Active API metrics
    pub static ref ROUTING_LIST_ACTIVE_METRICS_REQUEST: IntCounter = register_int_counter!(
        "routing_list_active_metrics_request",
        "total routing list active request received"
    )
    .unwrap();
    pub static ref ROUTING_LIST_ACTIVE_SUCCESSFUL_RESPONSE_COUNT: IntCounter = register_int_counter!(
        "routing_list_active_successful_response",
        "total routing list active successful response sent count"
    )
    .unwrap();
    pub static ref ROUTING_LIST_ACTIVE_UNSUCCESSFUL_RESPONSE_COUNT: IntCounter = register_int_counter!(
        "routing_list_active_unsuccessful_response",
        "total routing list active unsuccessful response sent count"
    )
    .unwrap();
    pub static ref ROUTING_LIST_ACTIVE_METRICS_DECISION_REQUEST_TIME: Histogram = register_histogram!(
        "routing_list_active_metrics_decision_request_time",
        "Time taken to process routing list active request (in seconds)",
        exponential_buckets(MICROS_500, 2.0, 10).unwrap()
    )
    .unwrap();

    // Routing Evaluate API metrics
    pub static ref ROUTING_EVALUATE_METRICS_REQUEST: IntCounter = register_int_counter!(
        "routing_evaluate_metrics_request",
        "total routing evaluate request received"
    )
    .unwrap();
    pub static ref ROUTING_EVALUATE_SUCCESSFUL_RESPONSE_COUNT: IntCounter = register_int_counter!(
        "routing_evaluate_successful_response",
        "total routing evaluate successful response sent count"
    )
    .unwrap();
    pub static ref ROUTING_EVALUATE_UNSUCCESSFUL_RESPONSE_COUNT: IntCounter = register_int_counter!(
        "routing_evaluate_unsuccessful_response",
        "total routing evaluate unsuccessful response sent count"
    )
    .unwrap();
    pub static ref ROUTING_EVALUATE_METRICS_DECISION_REQUEST_TIME: Histogram = register_histogram!(
        "routing_evaluate_metrics_decision_request_time",
        "Time taken to process routing evaluate request (in seconds)",
        exponential_buckets(MICROS_500, 2.0, 10).unwrap()
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

#[derive(Debug, thiserror::Error)]
pub enum ConfigurationError {
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}

pub async fn metrics_server_builder(
    config: crate::config::GlobalConfig,
) -> Result<(), ConfigurationError> {
    let listener = config.metrics.tcp_listener().await?;

    let router = axum::Router::new().route(
        "/metrics",
        axum::routing::get(|| async {
            let output = metrics_handler().await;
            match output {
                Ok(metrics) => Ok(metrics),
                Err(error) => {
                    tracing::error!(?error, "Error fetching metrics");

                    Err((
                        axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                        "Error fetching metrics".to_string(),
                    ))
                }
            }
        }),
    );

    axum::serve(listener, router.into_make_service())
        .with_graceful_shutdown(async {
            let output = tokio::signal::ctrl_c().await;
            tracing::error!("shutting down: {:?}", output);
        })
        .await?;

    Ok(())
}

impl crate::config::Server {
    pub async fn tcp_listener(&self) -> Result<tokio::net::TcpListener, ConfigurationError> {
        let loc = format!("{}:{}", self.host, self.port);

        tracing::info!(tag = "SERVER", "binding the metrics server at {}", loc);

        Ok(tokio::net::TcpListener::bind(loc).await?)
    }
}
