//!
//! Logger-specific config.
//!

use serde::Deserialize;

/// Log config settings.
#[derive(Debug, Deserialize, Clone)]
pub struct Log {
    /// Logging to a console.
    pub console: LogConsole,
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

/// OpenTelemetry configuration.
#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct Telemetry {
    pub tracing: TelemetryTracing,
    pub metrics: TelemetryMetrics,
}

impl Default for Telemetry {
    fn default() -> Self {
        Self {
            tracing: TelemetryTracing::default(),
            metrics: TelemetryMetrics::default(),
        }
    }
}

/// OpenTelemetry distributed tracing configuration.
#[derive(Debug, Deserialize, Clone)]
pub struct TelemetryTracing {
    pub enabled: bool,
    pub otlp_endpoint: String,
    pub service_name: Option<String>,
    pub sampling_ratio: f64,
}

impl Default for TelemetryTracing {
    fn default() -> Self {
        Self {
            enabled: false,
            otlp_endpoint: "http://localhost:4317".to_owned(),
            service_name: None,
            sampling_ratio: 1.0,
        }
    }
}

/// OpenTelemetry push metrics configuration.
#[derive(Debug, Deserialize, Clone)]
pub struct TelemetryMetrics {
    pub enabled: bool,
    pub otlp_endpoint: String,
    pub export_interval_secs: u64,
    pub export_timeout_secs: u64,
}

impl Default for TelemetryMetrics {
    fn default() -> Self {
        Self {
            enabled: false,
            otlp_endpoint: "http://localhost:4317".to_owned(),
            export_interval_secs: 60,
            export_timeout_secs: 30,
        }
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
