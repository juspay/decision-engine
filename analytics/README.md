# Decision Engine Analytics

This directory contains the analytics infrastructure for the Decision Engine project, implementing real-time event tracking and analytics using ClickHouse and Kafka.

## Architecture

The analytics system follows the Hyperswitch analytics pattern with the following components:

```
┌─────────────────────┐
│  Decision Engine    │
│   (Main Service)    │
└──────────┬──────────┘
           │ Events
           ▼
┌─────────────────────┐
│       Kafka         │
│ (Event Stream)      │
└──────────┬──────────┘
           │ Real-time
           ▼
┌─────────────────────┐
│     ClickHouse      │
│  ┌───────────────┐  │
│  │ Kafka Engine  │  │
│  │    Tables     │  │
│  └───────┬───────┘  │
│          │          │
│          ▼          │
│  ┌───────────────┐  │
│  │ Materialized  │  │
│  │    Views      │  │
│  └───────┬───────┘  │
│          │          │
│          ▼          │
│  ┌───────────────┐  │
│  │ Storage       │  │
│  │   Tables      │  │
│  └───────────────┘  │
└─────────────────────┘
```

## Components

### 1. Event Tracking
- **Endpoints Tracked**: `/routing/evaluate` and `/decide-gateway`
- **Event Data**: Request/response payloads, processing time, gateway selection, errors
- **Middleware**: Automatic event capture for tracked endpoints

### 2. Data Storage
- **Kafka Topics**: `decision-engine-routing-events`
- **ClickHouse Tables**: 
  - `routing_events_queue` (Kafka engine for ingestion)
  - `routing_events` (CollapsingMergeTree for storage)
  - `routing_events_hourly` (Hourly aggregations)
  - `routing_events_daily` (Daily aggregations)

### 3. Real-time Processing
- **Kafka Producer**: Batched event publishing
- **Materialized Views**: Real-time data transformation
- **Aggregations**: Automatic hourly and daily rollups

## Quick Start

### 1. Start Analytics Infrastructure

```bash
# Start with analytics profile
docker-compose --profile analytics up -d

# This will start:
# - Zookeeper
# - Kafka
# - ClickHouse
# - Analytics migrator (runs schema setup)
```

### 2. Enable Analytics in Configuration

Update your `config/development.toml`:

```toml
[analytics]
enabled = true

[analytics.kafka]
brokers = ["kafka:29092"]
topic_prefix = "decision-engine"
batch_size = 100
batch_timeout_ms = 1000

[analytics.clickhouse]
host = "http://clickhouse:8123"
username = "analytics_user"
password = "analytics_pass"
database = "decision_engine_analytics"
```

### 3. Start Decision Engine

```bash
# Start the main application
docker-compose up open-router-local
```

## Database Schema

### Routing Events Table

```sql
CREATE TABLE routing_events (
    event_id String,
    merchant_id LowCardinality(String),
    request_id String,
    endpoint LowCardinality(String),
    method LowCardinality(String),
    request_payload String,
    response_payload String,
    status_code UInt16,
    processing_time_ms UInt32,
    gateway_selected LowCardinality(Nullable(String)),
    routing_algorithm_id Nullable(String),
    error_message Nullable(String),
    user_agent Nullable(String),
    ip_address Nullable(String),
    created_at DateTime DEFAULT now(),
    inserted_at DateTime DEFAULT now(),
    sign_flag Int8
) ENGINE = CollapsingMergeTree(sign_flag)
PARTITION BY toStartOfDay(created_at)
ORDER BY (created_at, merchant_id, request_id, event_id);
```

### Aggregated Views

- **Hourly Aggregations**: Request counts, success/failure rates, performance metrics
- **Daily Aggregations**: Daily summaries with unique request tracking

## Querying Analytics Data

### Basic Queries

```sql
-- Total requests by endpoint
SELECT 
    endpoint,
    sum(sign_flag) as total_requests
FROM routing_events 
WHERE created_at >= now() - INTERVAL 1 DAY
GROUP BY endpoint;

-- Success rate by gateway
SELECT 
    gateway_selected,
    sum(if(status_code < 400, sign_flag, 0)) as successful_requests,
    sum(sign_flag) as total_requests,
    (successful_requests / total_requests) * 100 as success_rate
FROM routing_events 
WHERE created_at >= now() - INTERVAL 1 DAY
  AND gateway_selected IS NOT NULL
GROUP BY gateway_selected;

-- Performance metrics
SELECT 
    endpoint,
    avg(processing_time_ms) as avg_processing_time,
    quantile(0.95)(processing_time_ms) as p95_processing_time,
    quantile(0.99)(processing_time_ms) as p99_processing_time
FROM routing_events 
WHERE created_at >= now() - INTERVAL 1 DAY
GROUP BY endpoint;
```

### Using Aggregated Tables

```sql
-- Hourly trends
SELECT 
    hour,
    endpoint,
    total_requests,
    successful_requests,
    (successful_requests / total_requests) * 100 as success_rate,
    avg_processing_time_ms
FROM routing_events_hourly 
WHERE hour >= now() - INTERVAL 24 HOUR
ORDER BY hour DESC;
```

## Configuration Options

### Analytics Configuration

```toml
[analytics]
enabled = true  # Enable/disable analytics

[analytics.kafka]
brokers = ["kafka:29092"]  # Kafka broker addresses
topic_prefix = "decision-engine"  # Topic prefix for events
batch_size = 100  # Events per batch
batch_timeout_ms = 1000  # Batch timeout in milliseconds

[analytics.clickhouse]
host = "http://clickhouse:8123"  # ClickHouse HTTP endpoint
username = "analytics_user"  # ClickHouse username
password = "analytics_pass"  # ClickHouse password
database = "decision_engine_analytics"  # Database name
```

## Monitoring and Maintenance

### Health Checks

The analytics client provides health check functionality:

```rust
// Check analytics connectivity
analytics_client.health_check().await?;
```

### Data Retention

- **Raw Events**: 18 months (configurable via TTL)
- **Hourly Aggregations**: 12 months
- **Daily Aggregations**: 24 months

### Performance Considerations

1. **Batch Processing**: Events are batched for efficient Kafka publishing
2. **Async Processing**: Analytics don't block request processing
3. **Fallback Handling**: Graceful degradation when analytics are unavailable
4. **Partitioning**: Data partitioned by day for efficient queries

## Troubleshooting

### Common Issues

1. **Kafka Connection Failed**
   - Check Kafka broker connectivity
   - Verify topic creation
   - Check network configuration

2. **ClickHouse Connection Failed**
   - Verify ClickHouse is running
   - Check credentials and database existence
   - Verify network connectivity

3. **Missing Events**
   - Check analytics middleware is enabled
   - Verify endpoint tracking configuration
   - Check Kafka topic consumption

### Debugging

Enable debug logging for analytics:

```toml
[log.console]
level = "DEBUG"
```

Check analytics client status:

```bash
# Check if analytics is enabled
curl http://localhost:8080/health

# Check Kafka topics
docker exec -it open-router-kafka kafka-topics --bootstrap-server localhost:9092 --list

# Check ClickHouse tables
docker exec -it open-router-clickhouse clickhouse-client --query "SHOW TABLES FROM decision_engine_analytics"
```

## Development

### Adding New Event Types

1. Extend `RoutingEventData` struct in `src/analytics/types.rs`
2. Update ClickHouse schema in `analytics/migrations/`
3. Modify event extraction logic in middleware
4. Update aggregation queries if needed

### Testing

```bash
# Run analytics tests
cargo test analytics

# Test with sample events
curl -X POST http://localhost:8080/routing/evaluate \
  -H "Content-Type: application/json" \
  -H "x-merchant-id: test-merchant" \
  -d '{"test": "data"}'
```

## Security Considerations

1. **Data Privacy**: Ensure sensitive data is properly masked
2. **Access Control**: Restrict ClickHouse access to authorized users
3. **Network Security**: Use proper network isolation
4. **Data Retention**: Implement appropriate data retention policies
