#!/bin/sh
set -eu

: "${ANALYTICS_KAFKA_BROKERS:?ANALYTICS_KAFKA_BROKERS must be set}"
: "${ANALYTICS_KAFKA_API_TOPIC:?ANALYTICS_KAFKA_API_TOPIC must be set}"

CLICKHOUSE_DATABASE="${CLICKHOUSE_DATABASE:-default}"
CLICKHOUSE_CLUSTER="${CLICKHOUSE_CLUSTER:-}"
CLICKHOUSE_USER="${CLICKHOUSE_USER:-default}"
CLICKHOUSE_PASSWORD="${CLICKHOUSE_PASSWORD:-}"
API_GROUP_NAME="${ANALYTICS_KAFKA_API_TOPIC}"

auth_args="--database=${CLICKHOUSE_DATABASE} --user=${CLICKHOUSE_USER}"
if [ -n "${CLICKHOUSE_PASSWORD}" ]; then
  auth_args="${auth_args} --password=${CLICKHOUSE_PASSWORD}"
fi

if [ -n "${CLICKHOUSE_CLUSTER}" ]; then
  clickhouse-client ${auth_args} --multiquery <<SQL
CREATE TABLE analytics_api_events_local ON CLUSTER ${CLICKHOUSE_CLUSTER} (
    event_id UInt64,
    shard_key String,
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

CREATE TABLE analytics_api_events ON CLUSTER ${CLICKHOUSE_CLUSTER}
AS analytics_api_events_local
ENGINE = Distributed('${CLICKHOUSE_CLUSTER}', '${CLICKHOUSE_DATABASE}', 'analytics_api_events_local', cityHash64(shard_key));

DROP TABLE IF EXISTS analytics_api_events_mv ON CLUSTER ${CLICKHOUSE_CLUSTER};
DROP TABLE IF EXISTS analytics_api_events_queue ON CLUSTER ${CLICKHOUSE_CLUSTER};

CREATE TABLE analytics_api_events_queue ON CLUSTER ${CLICKHOUSE_CLUSTER} (
    schema_version UInt8,
    produced_at_ms Int64,
    event_id UInt64,
    shard_key String,
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
    kafka_broker_list = '${ANALYTICS_KAFKA_BROKERS}',
    kafka_topic_list = '${ANALYTICS_KAFKA_API_TOPIC}',
    kafka_group_name = '${API_GROUP_NAME}',
    kafka_format = 'JSONEachRow',
    kafka_handle_error_mode = 'stream';

CREATE MATERIALIZED VIEW analytics_api_events_mv ON CLUSTER ${CLICKHOUSE_CLUSTER}
TO analytics_api_events AS
SELECT
    event_id,
    shard_key,
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
SQL
else
  clickhouse-client ${auth_args} --multiquery <<SQL
CREATE TABLE analytics_api_events (
    event_id UInt64,
    shard_key String,
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

DROP TABLE IF EXISTS analytics_api_events_mv;
DROP TABLE IF EXISTS analytics_api_events_queue;

CREATE TABLE analytics_api_events_queue (
    schema_version UInt8,
    produced_at_ms Int64,
    event_id UInt64,
    shard_key String,
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
    kafka_broker_list = '${ANALYTICS_KAFKA_BROKERS}',
    kafka_topic_list = '${ANALYTICS_KAFKA_API_TOPIC}',
    kafka_group_name = '${API_GROUP_NAME}',
    kafka_format = 'JSONEachRow',
    kafka_handle_error_mode = 'stream';

CREATE MATERIALIZED VIEW analytics_api_events_mv
TO analytics_api_events AS
SELECT
    event_id,
    shard_key,
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
SQL
fi
