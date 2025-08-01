use error_stack::ResultExt;
use lazy_static::lazy_static;
use prometheus::{
    self, exponential_buckets, register_histogram_vec, register_int_counter_vec, Encoder,
    HistogramVec, IntCounterVec, TextEncoder,
};
use tokio::signal::unix::{signal, SignalKind};
lazy_static! {
    /// Total count of API requests by endpoint
    pub static ref API_REQUEST_TOTAL_COUNTER: IntCounterVec = register_int_counter_vec!(
        "api_requests_total",
        "Total Count of API requests by endpoint",
        &["endpoint"]
    )
    .unwrap();

    /// Count of API requests grouped by endpoint and result status
    pub static ref API_REQUEST_COUNTER: IntCounterVec = register_int_counter_vec!(
        "api_requests_by_status",
        "Count of API requests grouped by endpoint and result",
        &["endpoint", "status"]
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
    let listener = config.tcp_listener("Metrics").await?;

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

    // Create a signal stream for SIGTERM
    let mut sigterm = signal(SignalKind::terminate()).expect("Failed to create SIGTERM handler");

    axum::serve(listener, router.into_make_service())
        .with_graceful_shutdown(async move {
            let _ = sigterm.recv().await;
            tracing::info!("Metrics server shutting down gracefully");
        })
        .await?;

    Ok(())
}

impl crate::config::GlobalConfig {
    pub async fn tcp_listener(
        &self,
        server: &str,
    ) -> Result<tokio::net::TcpListener, ConfigurationError> {
        let loc = format!("{}:{}", self.metrics.host, self.metrics.port);

        tracing::info!(
            category = "SERVER",
            "{} started [{:?}] [{:?}]",
            server,
            self.metrics,
            self.log
        );

        Ok(tokio::net::TcpListener::bind(loc).await?)
    }
}
