#!/bin/sh
set -eu

: "${ANALYTICS_KAFKA_BROKERS:?ANALYTICS_KAFKA_BROKERS must be set}"
: "${ANALYTICS_KAFKA_DOMAIN_TOPIC:?ANALYTICS_KAFKA_DOMAIN_TOPIC must be set}"

CLICKHOUSE_DATABASE="${CLICKHOUSE_DATABASE:-default}"
CLICKHOUSE_CLUSTER="${CLICKHOUSE_CLUSTER:-}"
CLICKHOUSE_USER="${CLICKHOUSE_USER:-default}"
CLICKHOUSE_PASSWORD="${CLICKHOUSE_PASSWORD:-}"
DOMAIN_GROUP_NAME="${ANALYTICS_KAFKA_DOMAIN_TOPIC}"

auth_args="--database=${CLICKHOUSE_DATABASE} --user=${CLICKHOUSE_USER}"
if [ -n "${CLICKHOUSE_PASSWORD}" ]; then
  auth_args="${auth_args} --password=${CLICKHOUSE_PASSWORD}"
fi

if [ -n "${CLICKHOUSE_CLUSTER}" ]; then
  clickhouse-client ${auth_args} --multiquery <<SQL
CREATE TABLE analytics_domain_events_local ON CLUSTER ${CLICKHOUSE_CLUSTER} (
    event_id UInt64,
    shard_key String,
    api_flow LowCardinality(String),
    flow_type LowCardinality(String),
    merchant_id Nullable(String),
    merchant_id_key String MATERIALIZED ifNull(merchant_id, ''),
    payment_id Nullable(String),
    payment_id_key String MATERIALIZED ifNull(payment_id, ''),
    request_id Nullable(String),
    request_id_key String MATERIALIZED ifNull(request_id, ''),
    global_request_id Nullable(String),
    trace_id Nullable(String),
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
    merchant_id_key,
    created_at_ms,
    api_flow,
    flow_type,
    request_id_key,
    payment_id_key,
    event_id
)
TTL created_at + INTERVAL 18 MONTH;

CREATE TABLE analytics_domain_events ON CLUSTER ${CLICKHOUSE_CLUSTER}
AS analytics_domain_events_local
ENGINE = Distributed('${CLICKHOUSE_CLUSTER}', '${CLICKHOUSE_DATABASE}', 'analytics_domain_events_local', cityHash64(shard_key));

DROP TABLE IF EXISTS analytics_domain_events_mv ON CLUSTER ${CLICKHOUSE_CLUSTER};
DROP TABLE IF EXISTS analytics_domain_events_queue ON CLUSTER ${CLICKHOUSE_CLUSTER};

CREATE TABLE analytics_domain_events_queue ON CLUSTER ${CLICKHOUSE_CLUSTER} (
    schema_version UInt8,
    produced_at_ms Int64,
    event_id UInt64,
    shard_key String,
    api_flow LowCardinality(String),
    flow_type LowCardinality(String),
    merchant_id Nullable(String),
    payment_id Nullable(String),
    request_id Nullable(String),
    global_request_id Nullable(String),
    trace_id Nullable(String),
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
    kafka_broker_list = '${ANALYTICS_KAFKA_BROKERS}',
    kafka_topic_list = '${ANALYTICS_KAFKA_DOMAIN_TOPIC}',
    kafka_group_name = '${DOMAIN_GROUP_NAME}',
    kafka_format = 'JSONEachRow',
    kafka_handle_error_mode = 'stream';

CREATE MATERIALIZED VIEW analytics_domain_events_mv ON CLUSTER ${CLICKHOUSE_CLUSTER}
TO analytics_domain_events AS
SELECT
    event_id,
    shard_key,
    api_flow,
    flow_type,
    merchant_id,
    payment_id,
    request_id,
    global_request_id,
    trace_id,
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
FROM analytics_domain_events_queue
WHERE length(_error) = 0;
SQL
else
  clickhouse-client ${auth_args} --multiquery <<SQL
CREATE TABLE analytics_domain_events (
    event_id UInt64,
    shard_key String,
    api_flow LowCardinality(String),
    flow_type LowCardinality(String),
    merchant_id Nullable(String),
    merchant_id_key String MATERIALIZED ifNull(merchant_id, ''),
    payment_id Nullable(String),
    payment_id_key String MATERIALIZED ifNull(payment_id, ''),
    request_id Nullable(String),
    request_id_key String MATERIALIZED ifNull(request_id, ''),
    global_request_id Nullable(String),
    trace_id Nullable(String),
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
    merchant_id_key,
    created_at_ms,
    api_flow,
    flow_type,
    request_id_key,
    payment_id_key,
    event_id
)
TTL created_at + INTERVAL 18 MONTH;

DROP TABLE IF EXISTS analytics_domain_events_mv;
DROP TABLE IF EXISTS analytics_domain_events_queue;

CREATE TABLE analytics_domain_events_queue (
    schema_version UInt8,
    produced_at_ms Int64,
    event_id UInt64,
    shard_key String,
    api_flow LowCardinality(String),
    flow_type LowCardinality(String),
    merchant_id Nullable(String),
    payment_id Nullable(String),
    request_id Nullable(String),
    global_request_id Nullable(String),
    trace_id Nullable(String),
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
    kafka_broker_list = '${ANALYTICS_KAFKA_BROKERS}',
    kafka_topic_list = '${ANALYTICS_KAFKA_DOMAIN_TOPIC}',
    kafka_group_name = '${DOMAIN_GROUP_NAME}',
    kafka_format = 'JSONEachRow',
    kafka_handle_error_mode = 'stream';

CREATE MATERIALIZED VIEW analytics_domain_events_mv
TO analytics_domain_events AS
SELECT
    event_id,
    shard_key,
    api_flow,
    flow_type,
    merchant_id,
    payment_id,
    request_id,
    global_request_id,
    trace_id,
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
FROM analytics_domain_events_queue
WHERE length(_error) = 0;
SQL
fi
