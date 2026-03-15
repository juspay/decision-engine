//! Setup logging subsystem.

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{prelude::*, util::SubscriberInitExt, EnvFilter, Layer};

use super::{config, formatter::FormattingLayer, storage::StorageSubscription};

/// Contains guards necessary for logging and telemetry.
/// Dropping this guard flushes and shuts down OTel providers.
pub struct TelemetryGuard {
    _log_guards: Vec<WorkerGuard>,
    _tracer_provider: Option<opentelemetry_sdk::trace::TracerProvider>,
    _meter_provider: Option<opentelemetry_sdk::metrics::SdkMeterProvider>,
}

impl std::fmt::Debug for TelemetryGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TelemetryGuard")
            .field("_log_guards", &self._log_guards)
            .field(
                "_tracer_provider",
                &self._tracer_provider.as_ref().map(|_| "TracerProvider"),
            )
            .field(
                "_meter_provider",
                &self._meter_provider.as_ref().map(|_| "SdkMeterProvider"),
            )
            .finish()
    }
}

impl Drop for TelemetryGuard {
    fn drop(&mut self) {
        if let Some(tracer) = self._tracer_provider.take() {
            if let Err(err) = tracer.shutdown() {
                eprintln!("Failed to shutdown TracerProvider: {err}");
            }
        }
        if let Some(meter) = self._meter_provider.take() {
            if let Err(err) = meter.shutdown() {
                eprintln!("Failed to shutdown MeterProvider: {err}");
            }
        }
    }
}

/// Initialise the OTel TracerProvider when tracing is enabled.
#[allow(clippy::expect_used)]
fn init_tracer_provider(
    tracing_cfg: &config::TelemetryTracing,
    service_name: &str,
) -> opentelemetry_sdk::trace::TracerProvider {
    use opentelemetry::trace::TracerProvider as _;
    use opentelemetry::KeyValue;
    use opentelemetry_otlp::WithExportConfig;
    use opentelemetry_sdk::trace::{RandomIdGenerator, Sampler, TracerProvider};

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(&tracing_cfg.otlp_endpoint)
        .build()
        .expect("Failed to build OTLP span exporter");

    let sampler = if (tracing_cfg.sampling_ratio - 1.0_f64).abs() < f64::EPSILON {
        Sampler::AlwaysOn
    } else {
        Sampler::TraceIdRatioBased(tracing_cfg.sampling_ratio)
    };

    let resolved_name = tracing_cfg
        .service_name
        .as_deref()
        .unwrap_or(service_name);

    let provider = TracerProvider::builder()
        .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio)
        .with_sampler(sampler)
        .with_id_generator(RandomIdGenerator::default())
        .with_resource(opentelemetry_sdk::Resource::new(vec![
            KeyValue::new("service.name", resolved_name.to_owned()),
        ]))
        .build();

    // Register a tracer so the global API works
    let _ = provider.tracer("decision-engine");
    provider
}

/// Initialise the OTel MeterProvider when push metrics are enabled.
#[allow(clippy::expect_used)]
fn init_meter_provider(
    metrics_cfg: &config::TelemetryMetrics,
) -> opentelemetry_sdk::metrics::SdkMeterProvider {
    use opentelemetry_otlp::WithExportConfig;
    use opentelemetry_sdk::metrics::{PeriodicReader, SdkMeterProvider};

    let exporter = opentelemetry_otlp::MetricExporter::builder()
        .with_tonic()
        .with_endpoint(&metrics_cfg.otlp_endpoint)
        .build()
        .expect("Failed to build OTLP metric exporter");

    let reader = PeriodicReader::builder(exporter, opentelemetry_sdk::runtime::Tokio)
        .with_interval(std::time::Duration::from_secs(metrics_cfg.export_interval_secs))
        .with_timeout(std::time::Duration::from_secs(metrics_cfg.export_timeout_secs))
        .build();

    let provider = SdkMeterProvider::builder().with_reader(reader).build();

    opentelemetry::global::set_meter_provider(provider.clone());
    provider
}

/// Create an OTel tracing layer from a TracerProvider.
fn otel_layer<S>(
    provider: &opentelemetry_sdk::trace::TracerProvider,
) -> tracing_opentelemetry::OpenTelemetryLayer<S, opentelemetry_sdk::trace::Tracer>
where
    S: tracing::Subscriber + for<'span> tracing_subscriber::registry::LookupSpan<'span>,
{
    use opentelemetry::trace::TracerProvider as _;
    tracing_opentelemetry::layer().with_tracer(provider.tracer("decision-engine"))
}

/// Setup logging sub-system specifying the logging configuration, service (binary) name, and a
/// list of external crates for which a more verbose logging must be enabled. All crates within the
/// current cargo workspace are automatically considered for verbose logging.
pub fn setup(
    config: &config::Log,
    telemetry_config: &config::Telemetry,
    service_name: &str,
    crates_to_filter: impl AsRef<[&'static str]>,
) -> TelemetryGuard {
    let mut guards = Vec::new();

    // --- OTel providers ---
    let tracer_provider = if telemetry_config.tracing.enabled {
        // Register W3C TraceContext propagator so incoming `traceparent` headers
        // are extracted and outgoing requests carry the trace context forward.
        opentelemetry::global::set_text_map_propagator(
            opentelemetry_sdk::propagation::TraceContextPropagator::new(),
        );
        Some(init_tracer_provider(&telemetry_config.tracing, service_name))
    } else {
        None
    };

    let meter_provider = if telemetry_config.metrics.enabled {
        Some(init_meter_provider(&telemetry_config.metrics))
    } else {
        None
    };

    if config.console.enabled {
        let (console_writer, guard) = tracing_appender::non_blocking(std::io::stdout());
        guards.push(guard);

        let console_filter = get_envfilter(
            config.console.filtering_directive.as_ref(),
            config::Level(tracing::Level::WARN),
            config.console.level,
            &crates_to_filter,
        );

        match config.console.log_format {
            config::LogFormat::Default => {
                let formatting_layer = tracing_subscriber::fmt::layer()
                    .with_target(false)
                    .with_level(true)
                    .pretty()
                    .with_filter(console_filter);

                let subscriber = tracing_subscriber::registry()
                    .with(StorageSubscription)
                    .with(formatting_layer);

                if let Some(tp) = tracer_provider.as_ref() {
                    let subscriber = subscriber.with(otel_layer(tp));
                    #[allow(clippy::expect_used)]
                    subscriber
                        .try_init()
                        .expect("Unable to set global subscriber");
                } else {
                    #[allow(clippy::expect_used)]
                    subscriber
                        .try_init()
                        .expect("Unable to set global subscriber");
                }
            }

            config::LogFormat::Json => {
                let formatting_layer =
                    FormattingLayer::new(service_name, console_writer).with_filter(console_filter);

                let subscriber = tracing_subscriber::registry()
                    .with(StorageSubscription)
                    .with(formatting_layer);

                if let Some(tp) = tracer_provider.as_ref() {
                    subscriber.with(otel_layer(tp)).init();
                } else {
                    subscriber.init();
                }
            }
        }
    } else {
        let subscriber = tracing_subscriber::registry().with(StorageSubscription);
        if let Some(tp) = tracer_provider.as_ref() {
            subscriber.with(otel_layer(tp)).init();
        } else {
            subscriber.init();
        }
    }

    TelemetryGuard {
        _log_guards: guards,
        _tracer_provider: tracer_provider,
        _meter_provider: meter_provider,
    }
}

fn get_envfilter(
    filtering_directive: Option<&String>,
    default_log_level: config::Level,
    filter_log_level: config::Level,
    crates_to_filter: impl AsRef<[&'static str]>,
) -> EnvFilter {
    filtering_directive
        .map(|filter| {
            // Try to create target filter from specified filtering directive, if set

            // Safety: If user is overriding the default filtering directive, then we need to panic
            // for invalid directives.
            #[allow(clippy::expect_used)]
            EnvFilter::builder()
                .with_default_directive(default_log_level.into_level().into())
                .parse(filter)
                .expect("Invalid EnvFilter filtering directive")
        })
        .unwrap_or_else(|| {
            // Construct a default target filter otherwise
            let mut workspace_members = crate::cargo_workspace_members!();
            workspace_members.extend(crates_to_filter.as_ref());

            workspace_members
                .drain()
                .zip(std::iter::repeat(filter_log_level.into_level()))
                .fold(
                    EnvFilter::default().add_directive(default_log_level.into_level().into()),
                    |env_filter, (target, level)| {
                        // Safety: This is a hardcoded basic filtering directive. If even the basic
                        // filter is wrong, it's better to panic.
                        #[allow(clippy::expect_used)]
                        env_filter.add_directive(
                            format!("{target}={level}")
                                .parse()
                                .expect("Invalid EnvFilter directive format"),
                        )
                    },
                )
        })
}
