-- Merged SQL Migration File for Decision Engine Analytics
-- This file combines all migration steps into a single comprehensive migration
-- Created by merging: 001_routing_events.sql, 002_fix_datetime_parsing.sql, 002_fix_datetime_parsing_alternative.sql, 003_unix_timestamp_fix.sql

-- =============================================================================
-- STEP 1: Database Setup
-- =============================================================================

-- Create database if not exists
CREATE DATABASE IF NOT EXISTS decision_engine_analytics;

-- Use the analytics database
USE decision_engine_analytics;

-- =============================================================================
-- STEP 2: Create Storage Tables (Final Schema)
-- =============================================================================

-- Create storage table with CollapsingMergeTree for routing events
CREATE TABLE IF NOT EXISTS routing_events (
    `event_id` String,
    `merchant_id` LowCardinality(String),
    `request_id` String,
    `endpoint` LowCardinality(String),
    `method` LowCardinality(String),
    `request_payload` String,
    `response_payload` String,
    `status_code` UInt16,
    `processing_time_ms` UInt32,
    `gateway_selected` LowCardinality(Nullable(String)),
    `routing_algorithm_id` Nullable(String),
    `error_message` Nullable(String),
    `user_agent` Nullable(String),
    `ip_address` Nullable(String),
    `created_at` DateTime DEFAULT now() CODEC(T64, LZ4),
    `inserted_at` DateTime DEFAULT now() CODEC(T64, LZ4),
    `sign_flag` Int8,
    INDEX endpointIndex endpoint TYPE bloom_filter GRANULARITY 1,
    INDEX gatewayIndex gateway_selected TYPE bloom_filter GRANULARITY 1,
    INDEX statusIndex status_code TYPE bloom_filter GRANULARITY 1,
    INDEX merchantIndex merchant_id TYPE bloom_filter GRANULARITY 1
) ENGINE = CollapsingMergeTree(sign_flag) 
PARTITION BY toStartOfDay(created_at)
ORDER BY (created_at, merchant_id, request_id, event_id) 
TTL created_at + toIntervalMonth(18) 
SETTINGS index_granularity = 8192;

-- =============================================================================
-- STEP 3: Create Kafka Integration (Final Version with Unix Timestamp Support)
-- =============================================================================

-- Drop existing views and tables if they exist (for clean migration)
DROP VIEW IF EXISTS routing_events_mv;
DROP TABLE IF EXISTS routing_events_queue;

-- Create Kafka engine table with Unix timestamp handling (final version)
CREATE TABLE IF NOT EXISTS routing_events_queue (
    `event_id` String,
    `merchant_id` String,
    `request_id` String,
    `endpoint` LowCardinality(String),
    `method` LowCardinality(String),
    `request_payload` String,
    `response_payload` String,
    `status_code` UInt16,
    `processing_time_ms` UInt32,
    `gateway_selected` LowCardinality(Nullable(String)),
    `routing_algorithm_id` Nullable(String),
    `error_message` Nullable(String),
    `user_agent` Nullable(String),
    `ip_address` Nullable(String),
    `created_at` Int64,  -- Unix timestamp (final fix)
    `sign_flag` Int8
) ENGINE = Kafka 
SETTINGS 
    kafka_broker_list = 'open-router-kafka:29092',
    kafka_topic_list = 'decision-engine-routing-events',
    kafka_group_name = 'decision-engine-analytics',
    kafka_format = 'JSONEachRow',
    kafka_handle_error_mode = 'stream';

-- Create materialized view with Unix timestamp conversion (final version)
CREATE MATERIALIZED VIEW IF NOT EXISTS routing_events_mv TO routing_events AS
SELECT
    event_id,
    merchant_id,
    request_id,
    endpoint,
    method,
    request_payload,
    response_payload,
    status_code,
    processing_time_ms,
    gateway_selected,
    routing_algorithm_id,
    error_message,
    user_agent,
    ip_address,
    -- Convert Unix timestamp to DateTime (final fix)
    CASE 
        WHEN created_at > 0
        THEN toDateTime(created_at)
        ELSE now()
    END AS created_at,
    now() AS inserted_at,
    sign_flag
FROM routing_events_queue
WHERE length(_error) = 0;
