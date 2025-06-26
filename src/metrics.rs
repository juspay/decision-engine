use error_stack::ResultExt;
use lazy_static::lazy_static;
use prometheus::{
    self, exponential_buckets, register_histogram_vec, register_int_counter_vec, Encoder,
    HistogramVec, IntCounterVec, TextEncoder,
};

lazy_static! {
    /// Total count of API requests by endpoint
    pub static ref API_REQUEST_TOTAL_COUNTER: IntCounterVec = register_int_counter_vec!(
        "api_requests_total",
        "Total Count of API requests by endpoint",
        &["endpoint"] // example: ("decide_gateway")
    )
    .unwrap();

    /// Count of API requests grouped by endpoint and result status
    pub static ref API_REQUEST_COUNTER: IntCounterVec = register_int_counter_vec!(
        "api_requests_by_status",
        "Count of API requests grouped by endpoint and result",
        &["endpoint", "status"] // example: ("decide_gateway", "success")
    ).unwrap();

    /// Latency of API calls grouped by endpoint
    pub static ref API_LATENCY_HISTOGRAM: HistogramVec = register_histogram_vec!(
        "api_latency_seconds",
        "Latency of API calls grouped by endpoint",
        &["endpoint"],
        exponential_buckets(0.0005, 2.0, 10).unwrap()
    ).unwrap();
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

// API endpoint constants for consistent labeling
pub mod endpoints {
    pub const DECIDE_GATEWAY: &str = "decide_gateway";
    pub const UPDATE_GATEWAY_SCORE: &str = "update_gateway_score";
    pub const RULE_CREATE: &str = "rule_create";
    pub const RULE_GET: &str = "rule_get";
    pub const RULE_UPDATE: &str = "rule_update";
    pub const RULE_DELETE: &str = "rule_delete";
    pub const MERCHANT_ACCOUNT_CREATE: &str = "merchant_account_create";
    pub const MERCHANT_ACCOUNT_GET: &str = "merchant_account_get";
    pub const MERCHANT_ACCOUNT_DELETE: &str = "merchant_account_delete";
    pub const ROUTING_CREATE: &str = "routing_create";
    pub const ROUTING_ACTIVATE: &str = "routing_activate";
    pub const ROUTING_EVALUATE: &str = "routing_evaluate";
    pub const ROUTING_LIST: &str = "routing_list";
    pub const ROUTING_LIST_ACTIVE: &str = "routing_list_active";
}

// Status constants for consistent labeling
pub mod status {
    pub const SUCCESS: &str = "success";
    pub const FAILURE: &str = "failure";
}
