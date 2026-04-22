CREATE TABLE analytics_api_events_v1 (
    event_id UInt64,
    merchant_id Nullable(String),
    payment_id Nullable(String),
    api_flow LowCardinality(String),
    flow_type LowCardinality(String),
    created_at_timestamp Int64,
    created_at DateTime64(3, 'UTC') MATERIALIZED fromUnixTimestamp64Milli(created_at_timestamp),
    request_id String,
    global_request_id Nullable(String),
    trace_id Nullable(String),
    latency UInt64,
    status_code UInt16,
    auth_type Nullable(String),
    request String,
    user_agent Nullable(String),
    ip_addr Nullable(String),
    url_path String,
    response Nullable(String),
    error Nullable(String),
    http_method LowCardinality(String)
) ENGINE = ReplacingMergeTree(event_id)
PARTITION BY toYYYYMM(created_at)
ORDER BY (
    created_at_timestamp,
    api_flow,
    flow_type,
    status_code,
    request_id,
    event_id
)
TTL created_at + INTERVAL 18 MONTH;

CREATE TABLE analytics_api_events_queue (
    schema_version UInt8,
    produced_at_ms Int64,
    event_id UInt64,
    merchant_id Nullable(String),
    payment_id Nullable(String),
    api_flow LowCardinality(String),
    flow_type LowCardinality(String),
    created_at_timestamp Int64,
    request_id String,
    global_request_id Nullable(String),
    trace_id Nullable(String),
    latency UInt64,
    status_code UInt16,
    auth_type Nullable(String),
    request String,
    user_agent Nullable(String),
    ip_addr Nullable(String),
    url_path String,
    response Nullable(String),
    error Nullable(String),
    http_method LowCardinality(String)
) ENGINE = Kafka
SETTINGS
    kafka_broker_list = 'kafka:19092',
    kafka_topic_list = 'api',
    kafka_group_name = 'decision-engine-analytics-api',
    kafka_format = 'JSONEachRow',
    kafka_handle_error_mode = 'stream';

CREATE MATERIALIZED VIEW analytics_api_events_mv
TO analytics_api_events_v1 AS
SELECT
    event_id,
    merchant_id,
    payment_id,
    api_flow,
    flow_type,
    created_at_timestamp,
    request_id,
    global_request_id,
    trace_id,
    latency,
    status_code,
    auth_type,
    request,
    user_agent,
    ip_addr,
    url_path,
    response,
    error,
    http_method
FROM analytics_api_events_queue
WHERE length(_error) = 0;
