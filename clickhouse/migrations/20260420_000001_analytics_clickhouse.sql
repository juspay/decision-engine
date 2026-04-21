CREATE DATABASE IF NOT EXISTS decision_engine_analytics;

CREATE TABLE IF NOT EXISTS decision_engine_analytics.analytics_domain_events_v1 (
    event_id UInt64,
    tenant_id String,
    event_type LowCardinality(String),
    merchant_id Nullable(String),
    payment_id Nullable(String),
    request_id Nullable(String),
    payment_method_type Nullable(String),
    payment_method Nullable(String),
    card_network Nullable(String),
    card_is_in Nullable(String),
    currency Nullable(String),
    country Nullable(String),
    auth_type Nullable(String),
    gateway Nullable(String),
    event_stage Nullable(String),
    routing_approach Nullable(String),
    rule_name Nullable(String),
    status Nullable(String),
    error_code Nullable(String),
    error_message Nullable(String),
    score_value Nullable(Float64),
    sigma_factor Nullable(Float64),
    average_latency Nullable(Float64),
    tp99_latency Nullable(Float64),
    transaction_count Nullable(Int64),
    route Nullable(String),
    details Nullable(String),
    created_at_ms Int64
) ENGINE = ReplacingMergeTree(event_id)
PARTITION BY toYYYYMM(toDateTime(created_at_ms / 1000))
ORDER BY (
    tenant_id,
    ifNull(merchant_id, ''),
    created_at_ms,
    event_type,
    ifNull(route, ''),
    ifNull(request_id, ''),
    ifNull(payment_id, ''),
    event_id
)
TTL toDateTime(created_at_ms / 1000) + INTERVAL 90 DAY;

CREATE TABLE IF NOT EXISTS decision_engine_analytics.analytics_api_events_v1 (
    event_id UInt64,
    tenant_id String,
    merchant_id Nullable(String),
    payment_id Nullable(String),
    api_flow LowCardinality(String),
    created_at_timestamp Int64,
    request_id String,
    latency UInt64,
    status_code Int64,
    auth_type Nullable(String),
    request String,
    user_agent Nullable(String),
    ip_addr Nullable(String),
    url_path String,
    response Nullable(String),
    error Nullable(String),
    event_type LowCardinality(String),
    http_method LowCardinality(String),
    infra_components Nullable(String),
    request_truncated Bool,
    response_truncated Bool
) ENGINE = ReplacingMergeTree(event_id)
PARTITION BY toYYYYMM(toDateTime(created_at_timestamp / 1000))
ORDER BY (tenant_id, created_at_timestamp, api_flow, request_id, event_id)
TTL toDateTime(created_at_timestamp / 1000) + INTERVAL 90 DAY;
