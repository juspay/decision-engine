CREATE TABLE analytics_api_events (
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
