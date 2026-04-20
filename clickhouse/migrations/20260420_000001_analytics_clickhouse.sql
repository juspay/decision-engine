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

CREATE TABLE IF NOT EXISTS decision_engine_analytics.analytics_route_hits_5m_v1 (
    bucket_ms Int64,
    tenant_id String,
    merchant_id Nullable(String),
    route String,
    count UInt64
) ENGINE = SummingMergeTree
PARTITION BY toYYYYMM(toDateTime(bucket_ms / 1000))
ORDER BY (tenant_id, ifNull(merchant_id, ''), bucket_ms, route);

CREATE MATERIALIZED VIEW IF NOT EXISTS decision_engine_analytics.analytics_route_hits_5m_mv
TO decision_engine_analytics.analytics_route_hits_5m_v1 AS
SELECT
    intDiv(created_at_ms, 300000) * 300000 AS bucket_ms,
    tenant_id,
    merchant_id,
    ifNull(route, 'unknown') AS route,
    count() AS count
FROM decision_engine_analytics.analytics_domain_events_v1
WHERE event_type = 'request_hit'
GROUP BY bucket_ms, tenant_id, merchant_id, route;

CREATE TABLE IF NOT EXISTS decision_engine_analytics.analytics_decisions_5m_v1 (
    bucket_ms Int64,
    tenant_id String,
    merchant_id Nullable(String),
    routing_approach String,
    status String,
    count UInt64
) ENGINE = SummingMergeTree
PARTITION BY toYYYYMM(toDateTime(bucket_ms / 1000))
ORDER BY (tenant_id, merchant_id, bucket_ms, routing_approach, status);

CREATE MATERIALIZED VIEW IF NOT EXISTS decision_engine_analytics.analytics_decisions_5m_mv
TO decision_engine_analytics.analytics_decisions_5m_v1 AS
SELECT
    intDiv(created_at_ms, 300000) * 300000 AS bucket_ms,
    tenant_id,
    merchant_id,
    ifNull(routing_approach, 'UNKNOWN') AS routing_approach,
    ifNull(status, 'unknown') AS status,
    count() AS count
FROM decision_engine_analytics.analytics_domain_events_v1
WHERE event_type = 'decision'
GROUP BY bucket_ms, tenant_id, merchant_id, routing_approach, status;

CREATE TABLE IF NOT EXISTS decision_engine_analytics.analytics_gateway_share_5m_v1 (
    bucket_ms Int64,
    tenant_id String,
    merchant_id Nullable(String),
    gateway String,
    count UInt64
) ENGINE = SummingMergeTree
PARTITION BY toYYYYMM(toDateTime(bucket_ms / 1000))
ORDER BY (tenant_id, merchant_id, bucket_ms, gateway);

CREATE MATERIALIZED VIEW IF NOT EXISTS decision_engine_analytics.analytics_gateway_share_5m_mv
TO decision_engine_analytics.analytics_gateway_share_5m_v1 AS
SELECT
    intDiv(created_at_ms, 300000) * 300000 AS bucket_ms,
    tenant_id,
    merchant_id,
    ifNull(gateway, 'unknown') AS gateway,
    count() AS count
FROM decision_engine_analytics.analytics_domain_events_v1
WHERE event_type = 'decision'
GROUP BY bucket_ms, tenant_id, merchant_id, gateway;

CREATE TABLE IF NOT EXISTS decision_engine_analytics.analytics_rule_hits_5m_v1 (
    bucket_ms Int64,
    tenant_id String,
    merchant_id Nullable(String),
    rule_name String,
    count UInt64
) ENGINE = SummingMergeTree
PARTITION BY toYYYYMM(toDateTime(bucket_ms / 1000))
ORDER BY (tenant_id, merchant_id, bucket_ms, rule_name);

CREATE MATERIALIZED VIEW IF NOT EXISTS decision_engine_analytics.analytics_rule_hits_5m_mv
TO decision_engine_analytics.analytics_rule_hits_5m_v1 AS
SELECT
    intDiv(created_at_ms, 300000) * 300000 AS bucket_ms,
    tenant_id,
    merchant_id,
    ifNull(rule_name, 'unknown') AS rule_name,
    count() AS count
FROM decision_engine_analytics.analytics_domain_events_v1
WHERE event_type = 'rule_hit'
GROUP BY bucket_ms, tenant_id, merchant_id, rule_name;

CREATE TABLE IF NOT EXISTS decision_engine_analytics.analytics_errors_5m_v1 (
    bucket_ms Int64,
    tenant_id String,
    merchant_id Nullable(String),
    route String,
    error_code String,
    error_message String,
    count UInt64
) ENGINE = SummingMergeTree
PARTITION BY toYYYYMM(toDateTime(bucket_ms / 1000))
ORDER BY (tenant_id, merchant_id, bucket_ms, route, error_code);

CREATE MATERIALIZED VIEW IF NOT EXISTS decision_engine_analytics.analytics_errors_5m_mv
TO decision_engine_analytics.analytics_errors_5m_v1 AS
SELECT
    intDiv(created_at_ms, 300000) * 300000 AS bucket_ms,
    tenant_id,
    merchant_id,
    ifNull(route, 'unknown') AS route,
    ifNull(error_code, 'unknown') AS error_code,
    ifNull(error_message, 'unknown') AS error_message,
    count() AS count
FROM decision_engine_analytics.analytics_domain_events_v1
WHERE event_type = 'error'
GROUP BY bucket_ms, tenant_id, merchant_id, route, error_code, error_message;

CREATE TABLE IF NOT EXISTS decision_engine_analytics.analytics_score_latest_v1 (
    tenant_id String,
    merchant_id Nullable(String),
    payment_method_type Nullable(String),
    payment_method Nullable(String),
    gateway Nullable(String),
    score_value Nullable(Float64),
    sigma_factor Nullable(Float64),
    average_latency Nullable(Float64),
    tp99_latency Nullable(Float64),
    transaction_count Nullable(Int64),
    created_at_ms Int64,
    event_id UInt64
) ENGINE = ReplacingMergeTree(created_at_ms)
PARTITION BY toYYYYMM(toDateTime(created_at_ms / 1000))
ORDER BY (tenant_id, merchant_id, payment_method_type, payment_method, gateway, created_at_ms, event_id);

CREATE MATERIALIZED VIEW IF NOT EXISTS decision_engine_analytics.analytics_score_latest_mv
TO decision_engine_analytics.analytics_score_latest_v1 AS
SELECT
    tenant_id,
    merchant_id,
    payment_method_type,
    payment_method,
    gateway,
    score_value,
    sigma_factor,
    average_latency,
    tp99_latency,
    transaction_count,
    created_at_ms,
    event_id
FROM decision_engine_analytics.analytics_domain_events_v1
WHERE event_type = 'score_snapshot';

CREATE TABLE IF NOT EXISTS decision_engine_analytics.analytics_payment_lookup_v1 (
    tenant_id String,
    lookup_key String,
    payment_id Nullable(String),
    request_id Nullable(String),
    merchant_id Nullable(String),
    gateway Nullable(String),
    route Nullable(String),
    status Nullable(String),
    event_stage Nullable(String),
    created_at_ms Int64,
    event_id UInt64
) ENGINE = ReplacingMergeTree(created_at_ms)
PARTITION BY toYYYYMM(toDateTime(created_at_ms / 1000))
ORDER BY (tenant_id, lookup_key, created_at_ms, event_id);

CREATE MATERIALIZED VIEW IF NOT EXISTS decision_engine_analytics.analytics_payment_lookup_mv
TO decision_engine_analytics.analytics_payment_lookup_v1 AS
SELECT
    tenant_id,
    if(payment_id != '' AND payment_id IS NOT NULL, payment_id, request_id) AS lookup_key,
    payment_id,
    request_id,
    merchant_id,
    gateway,
    route,
    status,
    event_stage,
    created_at_ms,
    event_id
FROM decision_engine_analytics.analytics_domain_events_v1
WHERE event_type IN ('decision', 'gateway_update', 'rule_hit', 'rule_evaluation_preview', 'error')
  AND if(payment_id != '' AND payment_id IS NOT NULL, payment_id, request_id) != '';
