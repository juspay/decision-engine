use error_stack::ResultExt;
use lazy_static::lazy_static;
use prometheus::{
    self, exponential_buckets, register_histogram_vec, register_int_counter_vec,
    register_int_gauge_vec, Encoder, HistogramVec, IntCounterVec, IntGaugeVec, TextEncoder,
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

    /// Count of routing decisions grouped by routing approach and result status
    pub static ref ROUTING_DECISION_COUNTER: IntCounterVec = register_int_counter_vec!(
        "routing_decisions_total",
        "Count of routing decisions grouped by routing approach and result status",
        &["approach", "status"]
    ).unwrap();

    /// Count of priority logic rule hits grouped by rule name
    pub static ref ROUTING_RULE_HIT_COUNTER: IntCounterVec = register_int_counter_vec!(
        "routing_rule_hits_total",
        "Count of priority logic rule hits grouped by rule name",
        &["rule_name"]
    ).unwrap();

    /// Count of analytics events captured by flow type
    pub static ref ANALYTICS_EVENT_COUNTER: IntCounterVec = register_int_counter_vec!(
        "analytics_events_total",
        "Count of analytics events captured by flow type",
        &["flow_type"]
    ).unwrap();

    pub static ref ANALYTICS_SINK_WRITES_TOTAL: IntCounterVec = register_int_counter_vec!(
        "analytics_sink_writes_total",
        "Count of analytics sink write attempts grouped by sink, stream and result",
        &["sink", "stream", "result"]
    ).unwrap();

    pub static ref ANALYTICS_SINK_WRITE_LATENCY_HISTOGRAM: HistogramVec = register_histogram_vec!(
        "analytics_sink_write_latency_seconds",
        "Latency of analytics sink writes",
        &["sink", "stream"],
        exponential_buckets(0.001, 2.0, 12).unwrap()
    ).unwrap();

    pub static ref ANALYTICS_EVENTS_DROPPED_TOTAL: IntCounterVec = register_int_counter_vec!(
        "analytics_events_dropped_total",
        "Count of dropped analytics events",
        &["stream", "reason"]
    ).unwrap();

    pub static ref ANALYTICS_SINK_QUEUE_DEPTH: IntGaugeVec = register_int_gauge_vec!(
        "analytics_sink_queue_depth",
        "Current analytics queue depth by stream",
        &["stream"]
    ).unwrap();

    pub static ref ANALYTICS_KAFKA_PRODUCE_TOTAL: IntCounterVec = register_int_counter_vec!(
        "analytics_kafka_produce_total",
        "Count of Kafka analytics produce attempts grouped by stream and result",
        &["stream", "result"]
    ).unwrap();

    pub static ref ANALYTICS_KAFKA_DELIVERY_LATENCY_HISTOGRAM: HistogramVec = register_histogram_vec!(
        "analytics_kafka_delivery_latency_seconds",
        "Latency of Kafka analytics delivery acknowledgements",
        &["stream"],
        exponential_buckets(0.001, 2.0, 12).unwrap()
    ).unwrap();

    pub static ref ANALYTICS_CAPTURE_TRUNCATIONS_TOTAL: IntCounterVec = register_int_counter_vec!(
        "analytics_capture_truncations_total",
        "Count of truncated captured HTTP bodies grouped by direction",
        &["direction"]
    ).unwrap();

    pub static ref ANALYTICS_REQUEST_BODY_REJECTIONS_TOTAL: IntCounterVec = register_int_counter_vec!(
        "analytics_request_body_rejections_total",
        "Count of request body limit rejections grouped by endpoint",
        &["endpoint"]
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
            tracing::debug!("Metrics server shutting down gracefully");
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
            action = "metrics_server_startup",
            server,
            bind_address = %loc,
            "Metrics server listening"
        );

        Ok(tokio::net::TcpListener::bind(loc).await?)
    }
}
