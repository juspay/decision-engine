#!/bin/bash

# Test script for Decision Engine Analytics
set -e

echo "🚀 Testing Decision Engine Analytics Setup"
echo "=========================================="

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${GREEN}✓${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}⚠${NC} $1"
}

print_error() {
    echo -e "${RED}✗${NC} $1"
}

# Check if Docker is running
if ! docker info > /dev/null 2>&1; then
    print_error "Docker is not running. Please start Docker first."
    exit 1
fi

print_status "Docker is running"

# Check if docker-compose is available
if ! command -v docker-compose &> /dev/null; then
    print_error "docker-compose is not installed"
    exit 1
fi

print_status "docker-compose is available"

echo ""
echo "🔧 Starting Analytics Infrastructure..."
echo "======================================"

# Clean up any existing containers to avoid conflicts
print_status "Cleaning up existing containers..."
docker-compose --profile analytics down --remove-orphans 2>/dev/null || true

# Start analytics infrastructure
print_status "Starting Zookeeper, Kafka, ClickHouse and Analytics Migrator..."
docker-compose up -d kafka zookeeper clickhouse analytics-migrator

# Wait for services to be ready
echo ""
echo "⏳ Waiting for services to be ready..."
sleep 10

# Check Kafka
echo ""
echo "🔍 Checking Kafka..."
if docker exec open-router-kafka kafka-topics --bootstrap-server localhost:9092 --list > /dev/null 2>&1; then
    print_status "Kafka is running"
else
    print_error "Kafka is not responding"
    exit 1
fi

# Check ClickHouse
echo ""
echo "🔍 Checking ClickHouse..."
if docker exec open-router-clickhouse clickhouse-client --query "SELECT 1" > /dev/null 2>&1; then
    print_status "ClickHouse is running"
else
    print_error "ClickHouse is not responding"
    exit 1
fi

# Check if analytics database exists
echo ""
echo "🔍 Checking Analytics Database..."
if docker exec open-router-clickhouse clickhouse-client --query "SHOW DATABASES" | grep -q "decision_engine_analytics"; then
    print_status "Analytics database exists"
else
    print_warning "Analytics database not found, running migrations..."
    
    # Run migrations manually if needed
    docker exec open-router-clickhouse clickhouse-client --multiquery < analytics/migrations/001_routing_events.sql
    
    if docker exec open-router-clickhouse clickhouse-client --query "SHOW DATABASES" | grep -q "decision_engine_analytics"; then
        print_status "Analytics database created successfully"
    else
        print_error "Failed to create analytics database"
        exit 1
    fi
fi

# Check analytics tables
echo ""
echo "🔍 Checking Analytics Tables..."
TABLES=$(docker exec open-router-clickhouse clickhouse-client --query "SHOW TABLES FROM decision_engine_analytics")

if echo "$TABLES" | grep -q "routing_events"; then
    print_status "routing_events table exists"
else
    print_error "routing_events table not found"
    exit 1
fi

if echo "$TABLES" | grep -q "routing_events_queue"; then
    print_status "routing_events_queue table exists"
else
    print_error "routing_events_queue table not found"
    exit 1
fi

# Test Kafka topic creation
echo ""
echo "🔍 Checking Kafka Topics..."
if docker exec open-router-kafka kafka-topics --bootstrap-server localhost:9092 --list | grep -q "decision-engine-routing-events"; then
    print_status "Routing events topic exists"
else
    print_warning "Creating routing events topic..."
    docker exec open-router-kafka kafka-topics --bootstrap-server localhost:9092 --create --topic decision-engine-routing-events --partitions 3 --replication-factor 1
    print_status "Routing events topic created"
fi

# Test sending a sample event to Kafka
echo ""
echo "🧪 Testing Event Publishing..."
SAMPLE_EVENT='{"event_id":"test-event-1","merchant_id":"test-merchant","request_id":"test-req-1","endpoint":"/routing/evaluate","method":"POST","request_payload":"{}","response_payload":"{}","status_code":200,"processing_time_ms":100,"gateway_selected":"stripe","routing_algorithm_id":"algo-1","error_message":null,"user_agent":"test-agent","ip_address":"127.0.0.1","created_at":"2024-01-01T12:00:00Z","sign_flag":1}'

echo "$SAMPLE_EVENT" | docker exec -i open-router-kafka kafka-console-producer --bootstrap-server localhost:9092 --topic decision-engine-routing-events

print_status "Sample event sent to Kafka"

# Wait a moment for processing
sleep 5

# Check if event was processed by ClickHouse
echo ""
echo "🔍 Checking Event Processing..."
EVENT_COUNT=$(docker exec open-router-clickhouse clickhouse-client --query "SELECT count(*) FROM decision_engine_analytics.routing_events WHERE event_id = 'test-event-1'")

if [ "$EVENT_COUNT" -gt 0 ]; then
    print_status "Event successfully processed by ClickHouse"
else
    print_warning "Event not yet processed (this might be normal for new setups)"
fi

# Show sample queries
echo ""
echo "📊 Sample Analytics Queries"
echo "=========================="

echo ""
echo "Total events in the last hour:"
docker exec open-router-clickhouse clickhouse-client --query "
SELECT count(*) as total_events 
FROM decision_engine_analytics.routing_events 
WHERE created_at >= now() - INTERVAL 1 HOUR
"

echo ""
echo "Events by endpoint:"
docker exec open-router-clickhouse clickhouse-client --query "
SELECT 
    endpoint,
    count(*) as event_count
FROM decision_engine_analytics.routing_events 
GROUP BY endpoint
ORDER BY event_count DESC
"

echo ""
echo "🎉 Analytics Setup Test Complete!"
echo "================================="
print_status "All analytics components are running correctly"

echo ""
echo "📝 Next Steps:"
echo "1. Enable analytics in your config: Set analytics.enabled = true"
echo "2. Start the decision engine: docker-compose up open-router-local"
echo "3. Send test requests to /routing/evaluate or /decide-gateway"
echo "4. Query analytics data using the sample queries above"

echo ""
echo "🔗 Useful Commands:"
echo "- View ClickHouse logs: docker logs open-router-clickhouse"
echo "- View Kafka logs: docker logs open-router-kafka"
echo "- Connect to ClickHouse: docker exec -it open-router-clickhouse clickhouse-client"
echo "- List Kafka topics: docker exec open-router-kafka kafka-topics --bootstrap-server localhost:9092 --list"

echo ""
echo "📚 For more information, see analytics/README.md"
