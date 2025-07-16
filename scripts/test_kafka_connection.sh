#!/bin/bash

echo "Testing Kafka connection from application container..."

# Test 1: Check if the application can reach Kafka
echo "1. Testing network connectivity to Kafka..."
docker exec open-router-kafka sh -c "nc -z kafka 29092 && echo 'Kafka is reachable' || echo 'Kafka is NOT reachable'"

# Test 2: Send a test message to the topic using kafka tools from within the kafka container
echo "2. Sending test message to decision-engine-routing-events topic..."
docker exec open-router-kafka kafka-console-producer --bootstrap-server localhost:9092 --topic decision-engine-routing-events <<EOF
{"test": "kafka_connection_test", "timestamp": "$(date -u +%Y-%m-%dT%H:%M:%SZ)"}
EOF

# Test 3: Verify the message was received
echo "3. Checking if message was received..."
docker exec open-router-kafka kafka-console-consumer --bootstrap-server localhost:9092 --topic decision-engine-routing-events --from-beginning --timeout-ms 3000 --max-messages 1

echo "Kafka connection test completed."
