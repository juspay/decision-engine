//!
//! Formatting [layer](https://docs.rs/tracing-subscriber/0.3.15/tracing_subscriber/layer/trait.Layer.html) for Router.
//!

use std::{
    collections::{HashMap, HashSet},
    fmt,
    io::Write,
    sync::atomic::{AtomicU64, Ordering},
};

use chrono::{Datelike, Timelike, Utc};
use once_cell::sync::Lazy;
use serde::ser::{SerializeMap, Serializer};
use serde_json::{Map, Value};

use crate::logger;

use super::env::get_env_var;
use super::storage::Storage;
use time::format_description::well_known::Iso8601;
use tracing::{Event, Metadata, Subscriber};
use tracing_subscriber::{
    fmt::MakeWriter,
    layer::Context,
    registry::{LookupSpan, SpanRef},
    Layer,
};
use std::sync::Mutex;

// TODO: Documentation coverage for this crate

// Implicit keys

const HOSTNAME: &str = "hostname";
const PID: &str = "pid";
const LEVEL: &str = "level";
const TARGET: &str = "target";
const SERVICE: &str = "service";
const LINE: &str = "line";
const FILE: &str = "file";
const FN: &str = "fn";
const FULL_NAME: &str = "full_name";
const TIME: &str = "time";

/// Set of predefined implicit keys.
pub static IMPLICIT_KEYS: Lazy<rustc_hash::FxHashSet<&str>> = Lazy::new(|| {
    let mut set = rustc_hash::FxHashSet::default();

    set.insert(HOSTNAME);
    set.insert(PID);
    set.insert(LEVEL);
    set.insert(TARGET);
    set.insert(SERVICE);
    set.insert(LINE);
    set.insert(FILE);
    set.insert(FN);
    set.insert(FULL_NAME);
    set.insert(TIME);

    set
});

/// Global counter for auto-incrementing message numbers
pub static MESSAGE_NUMBER: AtomicU64 = AtomicU64::new(1);

/// Describe type of record: entering a span, exiting a span, an event.
#[derive(Clone, Debug)]
pub enum RecordType {
    /// Entering a span.
    EnterSpan,
    /// Exiting a span.
    ExitSpan,
    /// Event.
    Event,
}

impl fmt::Display for RecordType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let repr = match self {
            Self::EnterSpan => "START",
            Self::ExitSpan => "END",
            Self::Event => "EVENT",
        };
        write!(f, "{repr}")
    }
}

///
/// Format log records.
/// `FormattingLayer` relies on the `tracing_bunyan_formatter::JsonStorageLayer` which is storage of entries.
///
#[derive(Debug)]
#[allow(dead_code)]
pub struct FormattingLayer<W>
where
    W: for<'a> MakeWriter<'a> + 'static,
{
    dst_writer: W,
    pid: u32,
    hostname: String,
    service: String,
    default_fields: HashMap<String, Value>,
}

impl<W> FormattingLayer<W>
where
    W: for<'a> MakeWriter<'a> + 'static,
{
    ///
    /// Constructor of `FormattingLayer`.
    ///
    /// A `name` will be attached to all records during formatting.
    /// A `dst_writer` to forward all records.
    ///
    /// ## Example
    /// ```rust,ignore
    /// let formatting_layer = router_env::FormattingLayer::new(env::service_name!(),std::io::stdout);
    /// ```
    ///
    pub fn new(service: &str, dst_writer: W) -> Self {
        Self::new_with_implicit_entries(service, dst_writer, HashMap::new())
    }

    /// Construct of `FormattingLayer with implicit default entries.
    pub fn new_with_implicit_entries(
        service: &str,
        dst_writer: W,
        mut default_fields: HashMap<String, Value>,
    ) -> Self {
        let pid = std::process::id();
        let hostname = gethostname::gethostname().to_string_lossy().into_owned();
        let service = service.to_string();
        default_fields.retain(|key, value| {
            if !IMPLICIT_KEYS.contains(key.as_str()) {
                true
            } else {
                logger::error!(
                    "Attempting to log a reserved entry. It won't be added to the logs. key: {:?}, value: {:?}",
                    key, value
                );
                false
            }
        });

        Self {
            dst_writer,
            pid,
            hostname,
            service,
            default_fields,
        }
    }

    pub fn normalize_json(value: Value) -> Value {
        match value {
            Value::Object(map) => {
                let new_map = map
                    .into_iter()
                    .map(|(k, v)| (k, Self::normalize_json(v)))
                    .collect::<Map<_, _>>();
                Value::Object(new_map)
            }
            Value::Array(arr) => {
                let new_arr = arr.into_iter().map(Self::normalize_json).collect();
                Value::Array(new_arr)
            }
            Value::String(s) => {
                // Try parsing string as JSON
                match serde_json::from_str::<Value>(&s) {
                    Ok(inner_json) => FormattingLayer::<W>::normalize_json(inner_json),
                    Err(_) => Value::String(s),
                }
            }
            other => other,
        }
    }

    /// Serialize common for both span and event entries.
    fn common_serialize<S>(
        &self,
        map_serializer: &mut impl SerializeMap<Error = serde_json::Error>,
        metadata: &Metadata<'_>,
        span: Option<&SpanRef<'_, S>>,
        storage: &Storage<'_>,
        name: &str,
    ) -> Result<(), std::io::Error>
    where
        S: Subscriber + for<'a> LookupSpan<'a>,
    {
        // Define specific keys for "Incoming_api" and "DOMAIN" categories.
        let incoming_api_keys = [
            "udf_order_id",
            "udf_customer_id",
            "udf_txn_uuid",
            "x-request-id",
            "x-global-request-id",
            "merchant_id",
            "sdk_session_span",
            "timestamp",
            "hostname",
            "action",
            "partitionKey",
            "category",
            "entity",
            "latency",
            "io_latency_metric",
            "request_cputime",
            "gc_time",
            "bytes_allocated",
            "schema_version",
            "tenant_name",
            "tenant_id",
            "resp_code",
            "url",
            "cell_selector",
            "gateway",
            "cell_id",
            "is_audit_trail_log",
            "is_art_enabled",
            "error_category",
            "error_code",
        ];

        let domain_keys = [
            "message_number",
            "error_category",
            "x-request-id",
            "env",
            "@timestamp",
            "udf_txn_uuid",
            "txn_uuid",
            "flow_guid",
            "action",
            "is_audit_trail_log",
            "euler-request-id",
            "tenant_name",
            "hostname",
            "cluster",
            "level",
            "merchant_id",
            "udf_order_id",
            "is_art_enabled",
            "schema_version",
            "message",
            "gateway",
            "error_reason",
            "config_version",
            "category",
            "tenant_id",
            "timestamp",
            "sdk_session_span",
            "tag",
        ];

        // Read the category from storage (default is "DOMAIN").
        let category = storage
            .values
            .get("category")
            .and_then(|v| v.as_str())
            .unwrap_or("DOMAIN")
            .to_string();

        // Initialize the explicit entries set.
        let mut explicit_entries_set: HashSet<&str> = HashSet::default();

        if category == "INCOMING_API" {
            // Serialize keys from the span that match the incoming_api keys array.
            if let Some(span) = span {
                let extensions = span.extensions();
                if let Some(visitor) = extensions.get::<Storage<'_>>() {
                    for key in &incoming_api_keys {
                        if let Some(value) = visitor.values.get(*key) {
                            map_serializer.serialize_entry(*key, value)?;
                            explicit_entries_set.insert(*key);
                        }
                    }
                }
            }

            // Serialize keys from storage that are not already in explicit_entries_set.
            for key in &incoming_api_keys {
                if !explicit_entries_set.contains(*key) {
                    if let Some(value) = storage.values.get(*key) {
                        map_serializer.serialize_entry(*key, value)?;
                        explicit_entries_set.insert(*key);
                    }
                }
            }

            // Construct the custom "message" object for Incoming_api logs.
            let mut message = serde_json::json!({
                "url": storage.values.get("url").unwrap_or(&Value::String("null".to_string())),
                "method": storage.values.get("method").unwrap_or(&Value::String("null".to_string())),
                "req_headers": storage.values.get("req_headers").unwrap_or(&Value::String("null".to_string())),
                "query_params": storage.values.get("query_params").unwrap_or(&Value::String("null".to_string())),
                "req_body": storage.values.get("req_body").unwrap_or(&Value::String("null".to_string())),
                "res_code": storage.values.get("res_code").unwrap_or(&Value::String("null".to_string())),
                "res_body": storage.values.get("res_body").unwrap_or(&Value::String("null".to_string())),
                "res_headers": storage.values.get("res_headers").unwrap_or(&Value::String("null".to_string())),
                "latency": storage.values.get("latency").unwrap_or(&Value::String("null".to_string())),
                "api_tag": storage.values.get("api_tag").unwrap_or(&Value::String("null".to_string())),
                "request_time": storage.values.get("request_time").unwrap_or(&Value::String("null".to_string())),
            });

            // Add error_info only for error level logs.
            if metadata.level() == &tracing::Level::ERROR {
                let error_info = serde_json::json!({
                    "error_code": storage.values.get("error_code").unwrap_or(&Value::String("null".to_string())),
                    "error_message": storage.values.get("error_message").unwrap_or(&Value::String("null".to_string())),
                    "jp_error_code": storage.values.get("jp_error_code").unwrap_or(&Value::String("null".to_string())),
                    "jp_error_message": storage.values.get("jp_error_message").unwrap_or(&Value::String("null".to_string())),
                    "source": storage.values.get("source").unwrap_or(&Value::String("null".to_string())),
                });
                message
                    .as_object_mut()
                    .unwrap()
                    .insert("error_info".to_string(), error_info);
            }

            if !explicit_entries_set.contains("message") {
                map_serializer.serialize_entry("message", &message)?;
                explicit_entries_set.insert("message");
            }
            if !explicit_entries_set.contains("latency") {
                map_serializer.serialize_entry(
                    "latency",
                    &storage
                        .values
                        .get("latency")
                        .unwrap_or(&Value::String("null".to_string())),
                )?;
                explicit_entries_set.insert("latency");
            }
            if !explicit_entries_set.contains("resp_code") {
                map_serializer.serialize_entry(
                    "resp_code",
                    &storage
                        .values
                        .get("res_code")
                        .unwrap_or(&Value::String("null".to_string())),
                )?;
                explicit_entries_set.insert("resp_code");
            }
            if !explicit_entries_set.contains("level") {
                if metadata.level() == &tracing::Level::ERROR {
                    map_serializer.serialize_entry("level", &Value::String("Error".to_string()))?;
                } else {
                    map_serializer.serialize_entry("level", &Value::String("Info".to_string()))?;
                }
                explicit_entries_set.insert("level");
            }
            if !explicit_entries_set.contains("cell_id") {
                map_serializer.serialize_entry("cell_id", &get_env_var("CELL_ID", "null"))?;
                explicit_entries_set.insert("cell_id");
            }
        } else {

            if let Some(span) = span {
                let extensions = span.extensions();
                if let Some(visitor) = extensions.get::<Storage<'_>>() {
                    for key in &domain_keys {
                        if let Some(value) = visitor.values.get(*key) {
                            map_serializer.serialize_entry(*key, value)?;
                            explicit_entries_set.insert(*key);
                        }
                    }
                }
            }

            // Serialize keys from storage that are not already in explicit_entries_set.
            for key in &domain_keys {
                if !explicit_entries_set.contains(*key) {
                    if let Some(value) = storage.values.get(*key) {
                        if key == &"message" {
                            let normalized_value = get_normalized_message::<W>(&storage);

                            map_serializer.serialize_entry("message", &normalized_value)?;
                            explicit_entries_set.insert("message");
                        } else {
                            map_serializer.serialize_entry(*key, value)?;
                            explicit_entries_set.insert(*key);
                        }
                    }
                }
            }

            // Set additional fields for DOMAIN logs.
            if metadata.level() == &tracing::Level::ERROR {
                if !explicit_entries_set.contains("category") {
                    map_serializer
                        .serialize_entry("category", &Value::String("ERROR".to_string()))?;
                    explicit_entries_set.insert("category");
                }
                if !explicit_entries_set.contains("error_category") {
                    map_serializer.serialize_entry(
                        "error_category",
                        &Value::String("DOMAIN_ERROR".to_string()),
                    )?;
                    explicit_entries_set.insert("error_category");
                }
                if !explicit_entries_set.contains("level") {
                    map_serializer.serialize_entry("level", &Value::String("Error".to_string()))?;
                    explicit_entries_set.insert("level");
                }
            } else {
                if !explicit_entries_set.contains("category") {
                    map_serializer
                        .serialize_entry("category", &Value::String("DOMAIN".to_string()))?;
                    explicit_entries_set.insert("category");
                }
                if !explicit_entries_set.contains("level") {
                    let level_str = {
                        let s = format!("{}", metadata.level()).to_lowercase();
                        let mut c = s.chars();
                        match c.next() {
                            Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                            None => String::new(),
                        }
                    };
                    map_serializer.serialize_entry("level", &Value::String(level_str))?;
                    explicit_entries_set.insert("level");
                }
            }

            if !explicit_entries_set.contains("message") {
                let normalized_value = get_normalized_message::<W>(&storage);

                map_serializer.serialize_entry("message", &normalized_value)?;
                explicit_entries_set.insert("message");
            }

            if !explicit_entries_set.contains("@timestamp") {
                map_serializer.serialize_entry("@timestamp", &format_time_custom())?;
                explicit_entries_set.insert("@timestamp");
            }
        }
        if !explicit_entries_set.contains("is_art_enabled") {
            map_serializer.serialize_entry("is_art_enabled", "false")?;
            explicit_entries_set.insert("is_art_enabled");
        }

        // Serialize other common fields.
        map_serializer.serialize_entry("timestamp", &format_time_custom())?;
        map_serializer.serialize_entry("app_framework", "Rust")?;
        map_serializer.serialize_entry("hostname", &self.hostname)?;
        map_serializer.serialize_entry("source_commit", &get_env_var("SOURCE_COMMIT", "NA"))?;
        if let Ok(env) = std::env::var("ENV") {
            map_serializer.serialize_entry("env", &env)?;
        }

        if let Ok(source_commit) = std::env::var("SOURCE_COMMIT") {
            map_serializer.serialize_entry("source_commit", &source_commit)?;
        }
        map_serializer.serialize_entry(TARGET, metadata.target())?;
        map_serializer.serialize_entry(SERVICE, &self.service)?;
        map_serializer.serialize_entry(LINE, &metadata.line())?;
        map_serializer.serialize_entry(FILE, &metadata.file())?;
        if name != "?" {
            map_serializer.serialize_entry(FN, name)?;
            map_serializer
                .serialize_entry(FULL_NAME, &format!("{}::{}", metadata.target(), name))?;
        }
        map_serializer.serialize_entry(
            "message_number",
            &MESSAGE_NUMBER.fetch_add(1, Ordering::SeqCst),
        )?;
        explicit_entries_set.insert("message_number");
        explicit_entries_set.insert("timestamp");
        explicit_entries_set.insert("app_framework");
        explicit_entries_set.insert("source_commit");
        explicit_entries_set.insert("env");
        explicit_entries_set.insert("hostname");
        explicit_entries_set.insert(TARGET);
        explicit_entries_set.insert(SERVICE);
        explicit_entries_set.insert(LINE);
        explicit_entries_set.insert(FILE);
        explicit_entries_set.insert(FN);
        explicit_entries_set.insert(FULL_NAME);

        if category == "INCOMING_API" {
            for key in &incoming_api_keys {
                if !explicit_entries_set.contains(*key) {
                    map_serializer.serialize_entry(*key, &Value::Null)?;
                    explicit_entries_set.insert(*key); // optional, not needed beyond this
                }
            }
        } else {
            for key in &domain_keys {
                if !explicit_entries_set.contains(*key) {
                    map_serializer.serialize_entry(*key, &Value::Null)?;
                    explicit_entries_set.insert(*key); // optional, not needed beyond this
                }
            }
        }

        if let Ok(time) = &time::OffsetDateTime::now_utc().format(&Iso8601::DEFAULT) {
            map_serializer.serialize_entry(TIME, time)?;
        }

        explicit_entries_set.clear();

        Ok(())
    }

    ///
    /// Flush memory buffer into an output stream trailing it with next line.
    ///
    /// Should be done by single `write_all` call to avoid fragmentation of log because of mutlithreading.
    ///
    fn flush(&self, mut buffer: Vec<u8>) -> Result<(), std::io::Error> {
        buffer.write_all(b"\n")?;
        self.dst_writer.make_writer().write_all(&buffer)
    }

    /// Serialize entries of span.
    fn span_serialize<S>(
        &self,
        span: &SpanRef<'_, S>,
        ty: RecordType,
    ) -> Result<Vec<u8>, std::io::Error>
    where
        S: Subscriber + for<'a> LookupSpan<'a>,
    {
        let mut buffer = Vec::new();
        let mut serializer = serde_json::Serializer::new(&mut buffer);
        let mut map_serializer = serializer.serialize_map(None)?;
        let mut storage = Storage::default();

        self.common_serialize(
            &mut map_serializer,
            span.metadata(),
            Some(span),
            &storage,
            span.name(),
        )?;

        map_serializer.end()?;
        Ok(buffer)
    }

    /// Serialize event into a buffer of bytes using parent span.
    pub fn event_serialize<S>(
        &self,
        span: &Option<&SpanRef<'_, S>>,
        event: &Event<'_>,
    ) -> std::io::Result<Vec<u8>>
    where
        S: Subscriber + for<'a> LookupSpan<'a>,
    {
        let mut buffer = Vec::new();
        let mut serializer = serde_json::Serializer::new(&mut buffer);
        let mut map_serializer = serializer.serialize_map(None)?;

        let mut storage = Storage::default();
        event.record(&mut storage);

        let name = span.map_or("?", SpanRef::name);

        self.common_serialize(&mut map_serializer, event.metadata(), *span, &storage, name)?;

        map_serializer.end()?;
        Ok(buffer)
    }
}

fn get_normalized_message<W>(storage: &Storage<'_>) -> serde_json::Value
where
    W: for<'a> MakeWriter<'a> + 'static,
{
    let value = storage
        .values
        .get("message")
        .cloned()
        .unwrap_or(Value::String("null".to_string()));

    match &value {
        serde_json::Value::String(s) => match serde_json::from_str::<serde_json::Value>(s.as_str())
        {
            Ok(parsed) => FormattingLayer::<W>::normalize_json(parsed),
            Err(_) => value.clone(),
        },
        _ => value.clone(),
    }
}

/// Format the current time in a custom format: "YYYY-MM-DD HH:MM:SS.mmm"
fn format_time_custom() -> String {
    let now = Utc::now();
    let year = now.year();
    let month = now.month();
    let day = now.day();
    let hour = now.hour();
    let minute = now.minute();
    let second = now.second();
    let millisecond = now.timestamp_subsec_millis();

    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:03}",
        year, month, day, hour, minute, second, millisecond
    )
}

#[allow(clippy::expect_used)]
impl<S, W> Layer<S> for FormattingLayer<W>
where
    S: Subscriber + for<'a> LookupSpan<'a>,
    W: for<'a> MakeWriter<'a> + 'static,
{
    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, S>) {
        // Event could have no span.
        let span = ctx.lookup_current();

        let result: std::io::Result<Vec<u8>> = self.event_serialize(&span.as_ref(), event);
        if let Ok(formatted) = result {
            let _ = self.flush(formatted);
        }
    }

    fn on_enter(&self, id: &tracing::Id, ctx: Context<'_, S>) {
        let span = ctx.span(id).expect("No span");
        if let Ok(serialized) = self.span_serialize(&span, RecordType::EnterSpan) {
            let _ = self.flush(serialized);
        }
    }

    fn on_close(&self, id: tracing::Id, ctx: Context<'_, S>) {
        let span = ctx.span(&id).expect("No span");
        if let Ok(serialized) = self.span_serialize(&span, RecordType::ExitSpan) {
            let _ = self.flush(serialized);
        }
    }
}
