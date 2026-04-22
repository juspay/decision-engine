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
    created_at_ms Int64,
    created_at DateTime64(3, 'UTC') MATERIALIZED fromUnixTimestamp64Milli(created_at_ms)
) ENGINE = ReplacingMergeTree(event_id)
PARTITION BY toYYYYMM(created_at)
ORDER BY (
    tenant_id,
    isNull(merchant_id),
    coalesce(merchant_id, ''),
    created_at_ms,
    event_type,
    isNull(route),
    coalesce(route, ''),
    isNull(request_id),
    coalesce(request_id, ''),
    isNull(payment_id),
    coalesce(payment_id, ''),
    event_id
)
TTL created_at + INTERVAL 18 MONTH;

CREATE TABLE IF NOT EXISTS decision_engine_analytics.analytics_domain_events_parse_errors (
    topic String,
    partition Int64,
    offset Int64,
    raw String,
    error String
) ENGINE = MergeTree
ORDER BY (topic, partition, offset);

CREATE TABLE IF NOT EXISTS decision_engine_analytics.analytics_domain_events_queue (
    schema_version UInt8,
    produced_at_ms Int64,
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
) ENGINE = Kafka
SETTINGS
    kafka_broker_list = 'kafka:19092',
    kafka_topic_list = 'decision-engine.analytics.domain.v1',
    kafka_group_name = 'decision-engine-analytics-domain-v1',
    kafka_format = 'JSONEachRow',
    kafka_num_consumers = 1,
    kafka_handle_error_mode = 'stream';

CREATE MATERIALIZED VIEW IF NOT EXISTS decision_engine_analytics.analytics_domain_events_parse_errors_mv
TO decision_engine_analytics.analytics_domain_events_parse_errors AS
SELECT
    _topic AS topic,
    _partition AS partition,
    _offset AS offset,
    _raw_message AS raw,
    _error AS error
FROM decision_engine_analytics.analytics_domain_events_queue
WHERE length(_error) > 0;

CREATE MATERIALIZED VIEW IF NOT EXISTS decision_engine_analytics.analytics_domain_events_mv
TO decision_engine_analytics.analytics_domain_events_v1 AS
SELECT
    event_id,
    tenant_id,
    event_type,
    merchant_id,
    payment_id,
    request_id,
    payment_method_type,
    payment_method,
    card_network,
    card_is_in,
    currency,
    country,
    auth_type,
    gateway,
    event_stage,
    routing_approach,
    rule_name,
    status,
    error_code,
    error_message,
    score_value,
    sigma_factor,
    average_latency,
    tp99_latency,
    transaction_count,
    route,
    details,
    created_at_ms
FROM decision_engine_analytics.analytics_domain_events_queue
WHERE length(_error) = 0;
