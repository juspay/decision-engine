#!/bin/sh
set -eu

: "${ANALYTICS_KAFKA_BROKERS:?ANALYTICS_KAFKA_BROKERS must be set}"

ANALYTICS_KAFKA_API_TOPIC="${ANALYTICS_KAFKA_API_TOPIC:-api}"
ANALYTICS_KAFKA_DOMAIN_TOPIC="${ANALYTICS_KAFKA_DOMAIN_TOPIC:-domain}"
ANALYTICS_KAFKA_TOPIC_PARTITIONS="${ANALYTICS_KAFKA_TOPIC_PARTITIONS:-4}"
ANALYTICS_KAFKA_TOPIC_REPLICATION_FACTOR="${ANALYTICS_KAFKA_TOPIC_REPLICATION_FACTOR:-1}"

if ! command -v kafka-topics.sh >/dev/null 2>&1; then
  echo "kafka-topics.sh must be available on PATH"
  exit 1
fi

create_topic() {
  kafka-topics.sh \
    --bootstrap-server "${ANALYTICS_KAFKA_BROKERS}" \
    --create \
    --if-not-exists \
    --topic "$1" \
    --partitions "${ANALYTICS_KAFKA_TOPIC_PARTITIONS}" \
    --replication-factor "${ANALYTICS_KAFKA_TOPIC_REPLICATION_FACTOR}"
}

topic_partition_count() {
  kafka-topics.sh \
    --bootstrap-server "${ANALYTICS_KAFKA_BROKERS}" \
    --describe \
    --topic "$1" \
    | sed -n 's/.*PartitionCount:\([0-9][0-9]*\).*/\1/p' \
    | head -n 1
}

create_topic "${ANALYTICS_KAFKA_API_TOPIC}"
create_topic "${ANALYTICS_KAFKA_DOMAIN_TOPIC}"

api_partitions="$(topic_partition_count "${ANALYTICS_KAFKA_API_TOPIC}")"
domain_partitions="$(topic_partition_count "${ANALYTICS_KAFKA_DOMAIN_TOPIC}")"

if [ -z "${api_partitions}" ] || [ -z "${domain_partitions}" ]; then
  echo "Unable to determine analytics topic partition counts"
  exit 1
fi

if [ "${api_partitions}" != "${domain_partitions}" ]; then
  echo "Analytics topics must have the same partition count: api=${api_partitions}, domain=${domain_partitions}"
  exit 1
fi

if [ "${api_partitions}" != "${ANALYTICS_KAFKA_TOPIC_PARTITIONS}" ]; then
  echo "Analytics topics must use ${ANALYTICS_KAFKA_TOPIC_PARTITIONS} partitions: api=${api_partitions}"
  exit 1
fi

echo "Analytics topics ready: ${ANALYTICS_KAFKA_API_TOPIC}=${api_partitions}, ${ANALYTICS_KAFKA_DOMAIN_TOPIC}=${domain_partitions}"
