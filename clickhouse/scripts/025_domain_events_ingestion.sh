#!/bin/sh
set -eu

: "${ANALYTICS_KAFKA_BROKERS:?ANALYTICS_KAFKA_BROKERS must be set}"
: "${ANALYTICS_KAFKA_DOMAIN_TOPIC:?ANALYTICS_KAFKA_DOMAIN_TOPIC must be set}"

CLICKHOUSE_DATABASE="${CLICKHOUSE_DATABASE:-default}"
CLICKHOUSE_USER="${CLICKHOUSE_USER:-default}"
CLICKHOUSE_PASSWORD="${CLICKHOUSE_PASSWORD:-}"
DOMAIN_GROUP_NAME="${ANALYTICS_KAFKA_DOMAIN_TOPIC}"
SUMMARY_BUCKET_GROUP_NAME="${ANALYTICS_KAFKA_DOMAIN_TOPIC}_payment_audit_summary_buckets"
LOOKUP_SUMMARY_GROUP_NAME="${ANALYTICS_KAFKA_DOMAIN_TOPIC}_payment_audit_lookup_summaries"

auth_args="--database=${CLICKHOUSE_DATABASE} --user=${CLICKHOUSE_USER}"
if [ -n "${CLICKHOUSE_PASSWORD}" ]; then
  auth_args="${auth_args} --password=${CLICKHOUSE_PASSWORD}"
fi

clickhouse-client ${auth_args} --multiquery <<SQL
CREATE TABLE analytics_domain_events (
    event_id String,
    api_flow LowCardinality(String),
    flow_type LowCardinality(String),
    summary_kind Nullable(String),
    merchant_id Nullable(String),
    payment_id Nullable(String),
    request_id Nullable(String),
    -- Populated by the analytics producer: payment_id first, else request_id.
    lookup_key Nullable(String),
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
) ENGINE = ReplacingMergeTree
PARTITION BY toYYYYMM(created_at)
ORDER BY (
    merchant_id,
    lookup_key,
    created_at_ms,
    api_flow,
    flow_type,
    event_id
)
TTL created_at + INTERVAL 18 MONTH
SETTINGS allow_nullable_key = 1;

DROP TABLE IF EXISTS analytics_payment_audit_summary_buckets_mv;
DROP TABLE IF EXISTS analytics_payment_audit_summary_buckets;
DROP TABLE IF EXISTS analytics_payment_audit_lookup_summaries_mv;
DROP TABLE IF EXISTS analytics_payment_audit_lookup_summaries;
DROP TABLE IF EXISTS analytics_domain_events_mv;
DROP TABLE IF EXISTS analytics_payment_audit_summary_buckets_queue;
DROP TABLE IF EXISTS analytics_payment_audit_lookup_summaries_queue;
DROP TABLE IF EXISTS analytics_domain_events_queue;

CREATE TABLE analytics_payment_audit_summary_buckets (
    merchant_id String,
    lookup_key String,
    summary_kind LowCardinality(String),
    bucket_start DateTime64(3),
    first_seen_ms_state AggregateFunction(min, Int64),
    last_seen_ms_state AggregateFunction(max, Int64),
    event_count_state AggregateFunction(sum, UInt64),
    payment_id_state AggregateFunction(argMax, Nullable(String), Int64),
    request_id_state AggregateFunction(argMax, Nullable(String), Int64),
    merchant_id_state AggregateFunction(argMax, Nullable(String), Int64),
    latest_status_state AggregateFunction(argMax, Nullable(String), Int64),
    latest_gateway_state AggregateFunction(argMax, Nullable(String), Int64),
    latest_stage_state AggregateFunction(argMax, Nullable(String), Int64),
    gateways_state AggregateFunction(groupUniqArray, String),
    routes_state AggregateFunction(groupUniqArray, String),
    statuses_state AggregateFunction(groupUniqArray, String),
    flow_types_state AggregateFunction(groupUniqArray, String),
    error_codes_state AggregateFunction(groupUniqArray, String)
) ENGINE = AggregatingMergeTree
PARTITION BY toYYYYMM(bucket_start)
ORDER BY (
    merchant_id,
    summary_kind,
    bucket_start,
    lookup_key
)
TTL bucket_start + INTERVAL 18 MONTH;

CREATE TABLE analytics_payment_audit_lookup_summaries (
    merchant_id String,
    lookup_key String,
    summary_kind LowCardinality(String),
    first_seen_ms_state AggregateFunction(min, Int64),
    last_seen_ms_state AggregateFunction(max, Int64),
    event_count_state AggregateFunction(sum, UInt64),
    payment_id_state AggregateFunction(argMax, Nullable(String), Int64),
    request_id_state AggregateFunction(argMax, Nullable(String), Int64),
    merchant_id_state AggregateFunction(argMax, Nullable(String), Int64),
    latest_status_state AggregateFunction(argMax, Nullable(String), Int64),
    latest_gateway_state AggregateFunction(argMax, Nullable(String), Int64),
    latest_stage_state AggregateFunction(argMax, Nullable(String), Int64),
    gateways_state AggregateFunction(groupUniqArray, String),
    routes_state AggregateFunction(groupUniqArray, String),
    statuses_state AggregateFunction(groupUniqArray, String),
    flow_types_state AggregateFunction(groupUniqArray, String),
    error_codes_state AggregateFunction(groupUniqArray, String)
) ENGINE = AggregatingMergeTree
ORDER BY (
    merchant_id,
    summary_kind,
    lookup_key
);

CREATE TABLE analytics_domain_events_queue (
    schema_version UInt8,
    produced_at_ms Int64,
    event_id String,
    api_flow LowCardinality(String),
    flow_type LowCardinality(String),
    summary_kind Nullable(String),
    merchant_id Nullable(String),
    payment_id Nullable(String),
    request_id Nullable(String),
    lookup_key Nullable(String),
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

CREATE TABLE analytics_payment_audit_summary_buckets_queue AS analytics_domain_events_queue
ENGINE = Kafka
SETTINGS
    kafka_broker_list = '${ANALYTICS_KAFKA_BROKERS}',
    kafka_topic_list = '${ANALYTICS_KAFKA_DOMAIN_TOPIC}',
    kafka_group_name = '${SUMMARY_BUCKET_GROUP_NAME}',
    kafka_format = 'JSONEachRow',
    kafka_handle_error_mode = 'stream';

CREATE TABLE analytics_payment_audit_lookup_summaries_queue AS analytics_domain_events_queue
ENGINE = Kafka
SETTINGS
    kafka_broker_list = '${ANALYTICS_KAFKA_BROKERS}',
    kafka_topic_list = '${ANALYTICS_KAFKA_DOMAIN_TOPIC}',
    kafka_group_name = '${LOOKUP_SUMMARY_GROUP_NAME}',
    kafka_format = 'JSONEachRow',
    kafka_handle_error_mode = 'stream';

CREATE MATERIALIZED VIEW analytics_payment_audit_summary_buckets_mv
TO analytics_payment_audit_summary_buckets AS
SELECT
    merchant_id,
    effective_lookup_key AS lookup_key,
    summary_kind,
    bucket_start,
    minState(created_at_ms) AS first_seen_ms_state,
    maxState(created_at_ms) AS last_seen_ms_state,
    sumState(toUInt64(1)) AS event_count_state,
    argMaxState(payment_id, created_at_ms) AS payment_id_state,
    argMaxState(request_id, created_at_ms) AS request_id_state,
    argMaxState(merchant_id, created_at_ms) AS merchant_id_state,
    argMaxState(status, created_at_ms) AS latest_status_state,
    argMaxState(gateway, created_at_ms) AS latest_gateway_state,
    argMaxState(event_stage, created_at_ms) AS latest_stage_state,
    groupUniqArrayState(ifNull(gateway, '')) AS gateways_state,
    groupUniqArrayState(ifNull(route, '')) AS routes_state,
    groupUniqArrayState(ifNull(status, '')) AS statuses_state,
    groupUniqArrayState(flow_type) AS flow_types_state,
    groupUniqArrayState(ifNull(error_code, '')) AS error_codes_state
FROM (
    SELECT
        merchant_id,
        lookup_key AS effective_lookup_key,
        summary_kind,
        toStartOfFifteenMinutes(fromUnixTimestamp64Milli(created_at_ms)) AS bucket_start,
        created_at_ms,
        payment_id,
        request_id,
        status,
        gateway,
        event_stage,
        route,
        flow_type,
        error_code
    FROM analytics_payment_audit_summary_buckets_queue
    WHERE merchant_id IS NOT NULL
      AND merchant_id != ''
      AND lookup_key IS NOT NULL
      AND lookup_key != ''
) AS source
WHERE summary_kind != ''
GROUP BY merchant_id, effective_lookup_key, summary_kind, bucket_start;

CREATE MATERIALIZED VIEW analytics_payment_audit_lookup_summaries_mv
TO analytics_payment_audit_lookup_summaries AS
SELECT
    merchant_id,
    effective_lookup_key AS lookup_key,
    summary_kind,
    minState(created_at_ms) AS first_seen_ms_state,
    maxState(created_at_ms) AS last_seen_ms_state,
    sumState(toUInt64(1)) AS event_count_state,
    argMaxState(payment_id, created_at_ms) AS payment_id_state,
    argMaxState(request_id, created_at_ms) AS request_id_state,
    argMaxState(merchant_id, created_at_ms) AS merchant_id_state,
    argMaxState(status, created_at_ms) AS latest_status_state,
    argMaxState(gateway, created_at_ms) AS latest_gateway_state,
    argMaxState(event_stage, created_at_ms) AS latest_stage_state,
    groupUniqArrayState(ifNull(gateway, '')) AS gateways_state,
    groupUniqArrayState(ifNull(route, '')) AS routes_state,
    groupUniqArrayState(ifNull(status, '')) AS statuses_state,
    groupUniqArrayState(flow_type) AS flow_types_state,
    groupUniqArrayState(ifNull(error_code, '')) AS error_codes_state
FROM (
    SELECT
        merchant_id,
        lookup_key AS effective_lookup_key,
        summary_kind,
        created_at_ms,
        payment_id,
        request_id,
        status,
        gateway,
        event_stage,
        route,
        flow_type,
        error_code
    FROM analytics_payment_audit_lookup_summaries_queue
    WHERE merchant_id IS NOT NULL
      AND merchant_id != ''
      AND lookup_key IS NOT NULL
      AND lookup_key != ''
) AS source
WHERE summary_kind != ''
GROUP BY merchant_id, effective_lookup_key, summary_kind;

CREATE MATERIALIZED VIEW analytics_domain_events_mv
TO analytics_domain_events AS
SELECT
    event_id,
    api_flow,
    flow_type,
    summary_kind,
    merchant_id,
    payment_id,
    request_id,
    lookup_key,
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
