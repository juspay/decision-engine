# Starts the local infrastructure for oneclick.sh as processes instead of containers.
# Mirrors the postgres-ghcr + analytics-clickhouse services in docker-compose.yaml: same ports,
# credentials, Kafka topics and ClickHouse init scripts. Packaged by `nix run .#infra`.

repo="$PWD"
if [ ! -f "$repo/clickhouse/scripts/025_domain_events_ingestion.sh" ]; then
  echo "Run this from the decision-engine repo root." >&2
  exit 1
fi

dir="$(realpath -m "${DE_INFRA_DIR:-$repo/.nix-infra}")"
mkdir -p "$dir/data" "$dir/conf"

cat >"$dir/conf/kafka.properties" <<EOF
process.roles=broker,controller
node.id=1
listeners=PLAINTEXT://localhost:9092,CONTROLLER://localhost:9093
advertised.listeners=PLAINTEXT://localhost:9092
controller.listener.names=CONTROLLER
listener.security.protocol.map=CONTROLLER:PLAINTEXT,PLAINTEXT:PLAINTEXT
controller.quorum.voters=1@localhost:9093
log.dirs=$dir/data/kafka
offsets.topic.replication.factor=1
transaction.state.log.replication.factor=1
transaction.state.log.min.isr=1
group.initial.rebalance.delay.ms=0
auto.create.topics.enable=false
EOF

cat >"$dir/conf/clickhouse.xml" <<EOF
<clickhouse>
  <logger><level>warning</level><console>1</console></logger>
  <http_port>8123</http_port>
  <tcp_port>9000</tcp_port>
  <listen_host>127.0.0.1</listen_host>
  <path>$dir/data/clickhouse/</path>
  <tmp_path>$dir/data/clickhouse/tmp/</tmp_path>
  <user_files_path>$dir/data/clickhouse/user_files/</user_files_path>
  <format_schema_path>$dir/data/clickhouse/format_schemas/</format_schema_path>
  <mark_cache_size>268435456</mark_cache_size>
  <user_directories>
    <users_xml><path>$dir/conf/clickhouse-users.xml</path></users_xml>
  </user_directories>
</clickhouse>
EOF

cat >"$dir/conf/clickhouse-users.xml" <<'EOF'
<clickhouse>
  <profiles><default/></profiles>
  <quotas><default/></quotas>
  <users>
    <default>
      <password></password>
      <networks><ip>127.0.0.1</ip><ip>::1</ip></networks>
      <profile>default</profile><quota>default</quota>
    </default>
    <decision_engine>
      <password>decision_engine</password>
      <networks><ip>127.0.0.1</ip><ip>::1</ip></networks>
      <profile>default</profile><quota>default</quota>
      <access_management>1</access_management>
    </decision_engine>
  </users>
</clickhouse>
EOF

cat >"$dir/process-compose.yaml" <<EOF
version: "0.5"
log_location: $dir/process-compose.log
processes:
  postgres-init:
    command: |
      [ -f $dir/data/pg/PG_VERSION ] && exit 0
      echo db_pass > $dir/data/pgpass
      initdb -D $dir/data/pg -U db_user --pwfile=$dir/data/pgpass -A scram-sha-256 >/dev/null
  postgres:
    command: postgres -D $dir/data/pg -h 127.0.0.1 -p 5432 -k $dir/data
    depends_on: { postgres-init: { condition: process_completed_successfully } }
    readiness_probe: { exec: { command: "pg_isready -h 127.0.0.1 -p 5432" }, period_seconds: 2 }
  postgres-db:
    command: |
      PGPASSWORD=db_pass psql -h 127.0.0.1 -U db_user -d postgres -tAc "SELECT 1 FROM pg_database WHERE datname='decision_engine_db'" | grep -q 1 \
        || PGPASSWORD=db_pass createdb -h 127.0.0.1 -U db_user decision_engine_db
    depends_on: { postgres: { condition: process_healthy } }
  redis:
    command: redis-server --port 6379 --bind 127.0.0.1 --dir $dir/data --save ""
    readiness_probe: { exec: { command: "redis-cli -p 6379 ping" }, period_seconds: 2 }
  kafka:
    command: |
      [ -f $dir/data/kafka/meta.properties ] || kafka-storage.sh format -t \$(kafka-storage.sh random-uuid) -c $dir/conf/kafka.properties
      exec kafka-server-start.sh $dir/conf/kafka.properties
    environment: [ "KAFKA_HEAP_OPTS=-Xmx512m -Xms256m", "LOG_DIR=$dir/data/kafka-logs" ]
    readiness_probe: { exec: { command: "kafka-topics.sh --bootstrap-server localhost:9092 --list" }, period_seconds: 5, timeout_seconds: 20, initial_delay_seconds: 5 }
  kafka-init:
    command: |
      kafka-topics.sh --bootstrap-server localhost:9092 --create --if-not-exists --topic api --partitions 4 --replication-factor 1
      kafka-topics.sh --bootstrap-server localhost:9092 --create --if-not-exists --topic domain --partitions 4 --replication-factor 1
    depends_on: { kafka: { condition: process_healthy } }
  clickhouse:
    command: clickhouse server --config-file=$dir/conf/clickhouse.xml
    readiness_probe: { http_get: { host: 127.0.0.1, port: 8123, path: /ping }, period_seconds: 2 }
  clickhouse-init:
    command: for s in $repo/clickhouse/scripts/*.sh; do echo "running \$s"; sh "\$s" || exit 1; done
    environment:
      - CLICKHOUSE_DATABASE=default
      - CLICKHOUSE_USER=decision_engine
      - CLICKHOUSE_PASSWORD=decision_engine
      - ANALYTICS_KAFKA_BROKERS=localhost:9092
      - ANALYTICS_KAFKA_API_TOPIC=api
      - ANALYTICS_KAFKA_DOMAIN_TOPIC=domain
    depends_on:
      clickhouse: { condition: process_healthy }
      kafka-init: { condition: process_completed_successfully }
  mailpit:
    command: mailpit --listen 127.0.0.1:8025 --smtp 127.0.0.1:1025 --max 500 --smtp-auth-accept-any --smtp-auth-allow-insecure
  otel-collector:
    command: otelcol-contrib --config=$repo/config/otel-collector.yaml
EOF

# process-compose's own API defaults to 8080, the backend's port.
exec process-compose up -f "$dir/process-compose.yaml" --port 18090 "$@"
