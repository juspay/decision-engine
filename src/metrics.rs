//! Application metrics: OpenTelemetry instruments behind a Prometheus-shaped facade, pushed over OTLP to the collector when `[log.telemetry].metrics_enabled` is set; [`init`] must run before any metric is touched.

use std::sync::LazyLock;
use std::time::{Duration, Instant};

use error_stack::ResultExt;
use opentelemetry::{
    global,
    metrics::{Counter, Gauge, Histogram, Meter},
    KeyValue,
};
use opentelemetry_sdk::metrics::{PeriodicReader, SdkMeterProvider, Temporality};

use crate::logger::config::LogTelemetry;

/// Service name reported as the OpenTelemetry `service.name` resource attribute.
const SERVICE_NAME: &str = "decision-engine";
// Same push cadence and timeout as Hyperswitch's exporter.
const EXPORT_INTERVAL_SECS: u64 = 3;
const EXPORT_TIMEOUT_SECS: u64 = 10;

/// Meter every instrument is created from; resolved per instrument so anything created after [`init`] binds to the real provider.
fn meter() -> Meter {
    global::meter(SERVICE_NAME)
}

/// Total count of API requests by endpoint
pub static API_REQUEST_TOTAL_COUNTER: LazyLock<CounterVec> = LazyLock::new(|| {
    CounterVec::new(
        "api_requests_total",
        "Total Count of API requests by endpoint",
        &["endpoint"],
    )
});

/// Count of API requests grouped by endpoint and result status
pub static API_REQUEST_COUNTER: LazyLock<CounterVec> = LazyLock::new(|| {
    CounterVec::new(
        "api_requests_by_status",
        "Count of API requests grouped by endpoint and result",
        &["endpoint", "status"],
    )
});

/// Latency of API calls grouped by endpoint
pub static API_LATENCY_HISTOGRAM: LazyLock<HistogramVec> = LazyLock::new(|| {
    HistogramVec::new(
        "api_latency_seconds",
        "Latency of API calls grouped by endpoint",
        &["endpoint"],
        exponential_buckets(0.0005, 2.0, 10),
    )
});

/// Count of routing decisions grouped by routing approach and result status
pub static ROUTING_DECISION_COUNTER: LazyLock<CounterVec> = LazyLock::new(|| {
    CounterVec::new(
        "routing_decisions_total",
        "Count of routing decisions grouped by routing approach and result status",
        &["approach", "status"],
    )
});

/// Count of priority logic rule hits grouped by rule name
pub static ROUTING_RULE_HIT_COUNTER: LazyLock<CounterVec> = LazyLock::new(|| {
    CounterVec::new(
        "routing_rule_hits_total",
        "Count of priority logic rule hits grouped by rule name",
        &["rule_name"],
    )
});

/// Count of analytics events captured by flow type
pub static ANALYTICS_EVENT_COUNTER: LazyLock<CounterVec> = LazyLock::new(|| {
    CounterVec::new(
        "analytics_events_total",
        "Count of analytics events captured by flow type",
        &["flow_type"],
    )
});

pub static ANALYTICS_SINK_WRITES_TOTAL: LazyLock<CounterVec> = LazyLock::new(|| {
    CounterVec::new(
        "analytics_sink_writes_total",
        "Count of analytics sink write attempts grouped by sink, stream and result",
        &["sink", "stream", "result"],
    )
});

pub static ANALYTICS_SINK_WRITE_LATENCY_HISTOGRAM: LazyLock<HistogramVec> = LazyLock::new(|| {
    HistogramVec::new(
        "analytics_sink_write_latency_seconds",
        "Latency of analytics sink writes",
        &["sink", "stream"],
        exponential_buckets(0.001, 2.0, 12),
    )
});

pub static ANALYTICS_EVENTS_DROPPED_TOTAL: LazyLock<CounterVec> = LazyLock::new(|| {
    CounterVec::new(
        "analytics_events_dropped_total",
        "Count of dropped analytics events",
        &["stream", "reason"],
    )
});

pub static ANALYTICS_SINK_QUEUE_DEPTH: LazyLock<IntGaugeVec> = LazyLock::new(|| {
    IntGaugeVec::new(
        "analytics_sink_queue_depth",
        "Current analytics queue depth by stream",
        &["stream"],
    )
});

pub static ANALYTICS_KAFKA_PRODUCE_TOTAL: LazyLock<CounterVec> = LazyLock::new(|| {
    CounterVec::new(
        "analytics_kafka_produce_total",
        "Count of Kafka analytics produce attempts grouped by stream and result",
        &["stream", "result"],
    )
});

pub static ANALYTICS_KAFKA_DELIVERY_LATENCY_HISTOGRAM: LazyLock<HistogramVec> =
    LazyLock::new(|| {
        HistogramVec::new(
            "analytics_kafka_delivery_latency_seconds",
            "Latency of Kafka analytics delivery acknowledgements",
            &["stream"],
            exponential_buckets(0.001, 2.0, 12),
        )
    });

/// `count` bucket boundaries starting at `start`, each `factor` times the previous one.
fn exponential_buckets(start: f64, factor: f64, count: i32) -> Vec<f64> {
    (0..count).map(|i| start * factor.powi(i)).collect()
}

/// Pairs label names with the values a call site passed, in order.
fn attributes(label_names: &[&'static str], values: &[&str]) -> Vec<KeyValue> {
    // Enforced in every build: a silent mismatch would corrupt series. Call sites pass literal label sets.
    assert_eq!(
        label_names.len(),
        values.len(),
        "metric label value count does not match its label names {label_names:?}"
    );
    label_names
        .iter()
        .zip(values)
        .map(|(name, value)| KeyValue::new(*name, (*value).to_owned()))
        .collect()
}

/// A monotonically increasing counter with a fixed set of label names.
pub struct CounterVec {
    counter: Counter<u64>,
    label_names: &'static [&'static str],
}

impl CounterVec {
    fn new(
        name: &'static str,
        description: &'static str,
        label_names: &'static [&'static str],
    ) -> Self {
        Self {
            counter: meter()
                .u64_counter(name)
                .with_description(description)
                .build(),
            label_names,
        }
    }

    pub fn with_label_values(&self, values: &[&str]) -> BoundCounter<'_> {
        BoundCounter {
            counter: &self.counter,
            attributes: attributes(self.label_names, values),
        }
    }
}

/// A [`CounterVec`] bound to one set of label values.
pub struct BoundCounter<'a> {
    counter: &'a Counter<u64>,
    attributes: Vec<KeyValue>,
}

impl BoundCounter<'_> {
    pub fn inc(&self) {
        self.counter.add(1, &self.attributes);
    }

    pub fn inc_by(&self, value: u64) {
        self.counter.add(value, &self.attributes);
    }
}

/// A distribution of `f64` observations with a fixed set of label names.
pub struct HistogramVec {
    histogram: Histogram<f64>,
    label_names: &'static [&'static str],
}

impl HistogramVec {
    fn new(
        name: &'static str,
        description: &'static str,
        label_names: &'static [&'static str],
        boundaries: Vec<f64>,
    ) -> Self {
        Self {
            histogram: meter()
                .f64_histogram(name)
                .with_description(description)
                .with_boundaries(boundaries)
                .build(),
            label_names,
        }
    }

    pub fn with_label_values(&self, values: &[&str]) -> BoundHistogram<'_> {
        BoundHistogram {
            histogram: &self.histogram,
            attributes: attributes(self.label_names, values),
        }
    }
}

/// A [`HistogramVec`] bound to one set of label values.
pub struct BoundHistogram<'a> {
    histogram: &'a Histogram<f64>,
    attributes: Vec<KeyValue>,
}

impl BoundHistogram<'_> {
    pub fn observe(&self, value: f64) {
        self.histogram.record(value, &self.attributes);
    }

    /// Starts a timer recorded by [`HistogramTimer::observe_duration`], or on drop if never observed explicitly.
    pub fn start_timer(&self) -> HistogramTimer {
        HistogramTimer {
            histogram: self.histogram.clone(),
            attributes: self.attributes.clone(),
            start: Instant::now(),
            observed: false,
        }
    }
}

/// Records the time elapsed since it was started into a histogram, exactly once.
pub struct HistogramTimer {
    histogram: Histogram<f64>,
    attributes: Vec<KeyValue>,
    start: Instant,
    observed: bool,
}

impl HistogramTimer {
    pub fn observe_duration(mut self) {
        self.observe_now();
    }

    /// Drops the timer without recording anything.
    pub fn stop_and_discard(mut self) {
        self.observed = true;
    }

    fn observe_now(&mut self) {
        if !self.observed {
            self.observed = true;
            self.histogram
                .record(self.start.elapsed().as_secs_f64(), &self.attributes);
        }
    }
}

impl Drop for HistogramTimer {
    fn drop(&mut self) {
        self.observe_now();
    }
}

/// A last-value gauge with a fixed set of label names.
pub struct IntGaugeVec {
    gauge: Gauge<i64>,
    label_names: &'static [&'static str],
}

impl IntGaugeVec {
    fn new(
        name: &'static str,
        description: &'static str,
        label_names: &'static [&'static str],
    ) -> Self {
        Self {
            gauge: meter()
                .i64_gauge(name)
                .with_description(description)
                .build(),
            label_names,
        }
    }

    pub fn with_label_values(&self, values: &[&str]) -> BoundGauge<'_> {
        BoundGauge {
            gauge: &self.gauge,
            attributes: attributes(self.label_names, values),
        }
    }
}

/// An [`IntGaugeVec`] bound to one set of label values.
pub struct BoundGauge<'a> {
    gauge: &'a Gauge<i64>,
    attributes: Vec<KeyValue>,
}

impl BoundGauge<'_> {
    pub fn set(&self, value: i64) {
        self.gauge.record(value, &self.attributes);
    }
}

/// Keeps the meter provider alive and flushes it on shutdown.
#[derive(Debug)]
pub struct MetricsGuard {
    provider: Option<SdkMeterProvider>,
}

impl Drop for MetricsGuard {
    fn drop(&mut self) {
        if let Some(Err(error)) = self.provider.as_ref().map(SdkMeterProvider::shutdown) {
            tracing::warn!(?error, "Failed to shut down the metrics provider cleanly");
        }
    }
}

/// Installs the global meter provider with the OTLP push reader; call inside the Tokio runtime, before any metric is used.
pub fn init(config: &LogTelemetry) -> error_stack::Result<MetricsGuard, MetricsError> {
    if !config.metrics_enabled {
        tracing::info!(
            category = "SERVER",
            action = "metrics_pipeline_setup",
            "Metrics export is disabled"
        );
        return Ok(MetricsGuard { provider: None });
    }

    let reader = match build_otlp_reader(config) {
        Ok(reader) => reader,
        Err(error) if config.ignore_errors => {
            tracing::warn!(
                ?error,
                "Failed to build the OTLP metrics exporter, continuing without metrics"
            );
            return Ok(MetricsGuard { provider: None });
        }
        Err(error) => return Err(error),
    };

    let provider = SdkMeterProvider::builder()
        .with_reader(reader)
        .with_resource(resource())
        .build();
    global::set_meter_provider(provider.clone());

    tracing::info!(
        category = "SERVER",
        action = "metrics_pipeline_setup",
        otlp_endpoint = config
            .otel_exporter_otlp_endpoint
            .as_deref()
            .unwrap_or("default"),
        "Metrics pipeline initialised"
    );

    Ok(MetricsGuard {
        provider: Some(provider),
    })
}

fn resource() -> opentelemetry_sdk::Resource {
    let mut attributes = vec![KeyValue::new("service.name", SERVICE_NAME)];
    if let Ok(pod) = std::env::var("POD_NAME") {
        attributes.push(KeyValue::new("pod", pod));
    }
    opentelemetry_sdk::Resource::new(attributes)
}

fn build_otlp_reader(config: &LogTelemetry) -> error_stack::Result<PeriodicReader, MetricsError> {
    use opentelemetry_otlp::WithExportConfig;

    let mut export_config = opentelemetry_otlp::ExportConfig {
        protocol: opentelemetry_otlp::Protocol::Grpc,
        endpoint: config.otel_exporter_otlp_endpoint.clone(),
        ..Default::default()
    };
    if let Some(timeout_ms) = config.otel_exporter_otlp_timeout {
        export_config.timeout = Duration::from_millis(timeout_ms);
    }

    let exporter = opentelemetry_otlp::MetricExporter::builder()
        .with_tonic()
        // The collector's prometheus exporter needs cumulative sums.
        .with_temporality(Temporality::Cumulative)
        .with_export_config(export_config)
        .build()
        .change_context(MetricsError::ExporterSetup)?;

    Ok(
        PeriodicReader::builder(exporter, opentelemetry_sdk::runtime::Tokio)
            .with_interval(Duration::from_secs(EXPORT_INTERVAL_SECS))
            .with_timeout(Duration::from_secs(EXPORT_TIMEOUT_SECS))
            .build(),
    )
}

#[derive(Debug, thiserror::Error)]
pub enum MetricsError {
    #[error("Error setting up the OTLP metrics exporter")]
    ExporterSetup,
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use opentelemetry_sdk::metrics::data;
    use opentelemetry_sdk::testing::metrics::InMemoryMetricExporter;

    use super::*;

    fn find<'a>(metrics: &'a [data::ResourceMetrics], name: &str) -> &'a data::Metric {
        metrics
            .iter()
            .flat_map(|resource| resource.scope_metrics.iter())
            .flat_map(|scope| scope.metrics.iter())
            .find(|metric| metric.name == name)
            .unwrap_or_else(|| panic!("metric {name} was not exported"))
    }

    /// The facade must export the same names, labels and values the call sites expect.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn facade_exports_plain_names_labels_and_values() {
        let exporter = InMemoryMetricExporter::default();
        let reader = PeriodicReader::builder(exporter.clone(), opentelemetry_sdk::runtime::Tokio)
            .with_interval(Duration::from_secs(3600))
            .build();
        let provider = SdkMeterProvider::builder()
            .with_reader(reader)
            .with_resource(resource())
            .build();
        global::set_meter_provider(provider.clone());

        let counter = CounterVec::new("metrics_facade_test_total", "test counter", &["endpoint"]);
        counter.with_label_values(&["decide"]).inc();
        counter.with_label_values(&["decide"]).inc_by(2);

        let histogram = HistogramVec::new(
            "metrics_facade_test_seconds",
            "test histogram",
            &["endpoint"],
            exponential_buckets(0.0005, 2.0, 10),
        );
        histogram
            .with_label_values(&["decide"])
            .start_timer()
            .observe_duration();
        histogram.with_label_values(&["decide"]).observe(0.25);

        let gauge = IntGaugeVec::new("metrics_facade_test_depth", "test gauge", &["stream"]);
        gauge.with_label_values(&["api"]).set(7);

        provider.force_flush().expect("flush");
        let exported = exporter.get_finished_metrics().expect("exported metrics");

        let sum = find(&exported, "metrics_facade_test_total")
            .data
            .as_any()
            .downcast_ref::<data::Sum<u64>>()
            .expect("counter is a u64 sum");
        assert!(sum.is_monotonic);
        assert_eq!(sum.data_points.len(), 1);
        assert_eq!(sum.data_points[0].value, 3);
        assert_eq!(
            sum.data_points[0].attributes,
            vec![KeyValue::new("endpoint", "decide")]
        );

        let hist = find(&exported, "metrics_facade_test_seconds")
            .data
            .as_any()
            .downcast_ref::<data::Histogram<f64>>()
            .expect("histogram is f64");
        assert_eq!(hist.data_points.len(), 1);
        assert_eq!(hist.data_points[0].count, 2);
        assert_eq!(hist.data_points[0].bounds.len(), 10);
        assert_eq!(hist.data_points[0].bounds[0], 0.0005);

        let gauge = find(&exported, "metrics_facade_test_depth")
            .data
            .as_any()
            .downcast_ref::<data::Gauge<i64>>()
            .expect("gauge is i64");
        assert_eq!(gauge.data_points[0].value, 7);
        assert_eq!(
            gauge.data_points[0].attributes,
            vec![KeyValue::new("stream", "api")]
        );
    }

    #[test]
    #[should_panic(expected = "metric label value count does not match")]
    fn label_arity_mismatch_panics_in_every_build() {
        attributes(&["endpoint", "status"], &["decide"]);
    }

    /// The push reader must build and run without a reachable collector; failed exports never take the process down.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn otlp_reader_survives_an_unreachable_collector() {
        use opentelemetry::metrics::MeterProvider as _;

        let config = LogTelemetry {
            metrics_enabled: true,
            ignore_errors: false,
            otel_exporter_otlp_endpoint: Some("http://127.0.0.1:1".to_owned()),
            otel_exporter_otlp_timeout: Some(200),
        };
        let reader = build_otlp_reader(&config).expect("otlp reader");
        let provider = SdkMeterProvider::builder()
            .with_reader(reader)
            .with_resource(resource())
            .build();

        provider
            .meter("test")
            .u64_counter("otlp_test_total")
            .build()
            .add(1, &[]);

        // Both fail because nothing listens on the endpoint; the point is that they return.
        let _ = provider.force_flush();
        let _ = provider.shutdown();
    }
}
