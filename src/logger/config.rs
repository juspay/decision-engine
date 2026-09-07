//!
//! Logger-specific config.
//!

use serde::Deserialize;

/// Log config settings.
#[derive(Debug, Deserialize, Clone)]
pub struct Log {
    /// Logging to a console.
    pub console: LogConsole,
    /// Telemetry export.
    #[serde(default)]
    pub telemetry: LogTelemetry,
}

/// Telemetry export: metrics pushed to an OpenTelemetry collector over OTLP/gRPC.
#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
pub struct LogTelemetry {
    /// Whether metrics are pushed to the collector; when off, every instrument is a no-op.
    pub metrics_enabled: bool,
    /// Whether a failure to build the exporter is logged and ignored instead of failing startup.
    pub ignore_errors: bool,
    /// OTLP/gRPC endpoint of the collector, e.g. `http://otel-collector:4317`.
    pub otel_exporter_otlp_endpoint: Option<String>,
    /// Timeout (in milliseconds) for one export.
    pub otel_exporter_otlp_timeout: Option<u64>,
    /// Seconds between two exports. Defaults to 5.
    pub metrics_export_interval_secs: Option<u64>,
}

/// Logging to a console.
#[derive(Debug, Deserialize, Clone)]
pub struct LogConsole {
    /// Whether you want to see log in your terminal.
    pub enabled: bool,
    /// What you see in your terminal.
    pub level: Level,
    /// Log format
    pub log_format: LogFormat,
    /// Directive which sets the log level for one or more crates/modules.
    pub filtering_directive: Option<String>,
}

/// Describes the level of verbosity of a span or event.
#[derive(Debug, Clone, Copy)]
pub struct Level(pub(super) tracing::Level);

impl Level {
    /// Returns the most verbose [`tracing::Level`]
    pub fn into_level(&self) -> tracing::Level {
        self.0
    }
}

impl<'de> Deserialize<'de> for Level {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use std::str::FromStr as _;

        let s = String::deserialize(deserializer)?;
        tracing::Level::from_str(&s)
            .map(Level)
            .map_err(serde::de::Error::custom)
    }
}

/// Telemetry / tracing.
#[derive(Default, Debug, Deserialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    /// Default pretty log format
    Default,
    /// JSON based structured logging
    #[default]
    Json,
}
