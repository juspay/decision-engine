#!/bin/bash

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

OPENAPI_PATH="$SCRIPT_DIR/docs/openapi.json"
DOCS_PORT="${DOCS_PORT:-3000}"
DOCS_URL="http://localhost:${DOCS_PORT}"
DOCS_HOME_URL="${DOCS_URL}/introduction"
API_REF_URL="${DOCS_URL}/api-reference"
API_EXAMPLES_URL="${DOCS_URL}/api-refs/api-ref"
DOCS_LOG_PATH="$SCRIPT_DIR/.mintlify-dev.log"

POSTGRES_HOST="${POSTGRES_HOST:-localhost}"
POSTGRES_PORT="${POSTGRES_PORT:-5432}"
POSTGRES_USER="${POSTGRES_USER:-db_user}"
POSTGRES_DB="${POSTGRES_DB:-decision_engine_db}"
REDIS_HOST="${REDIS_HOST:-localhost}"
REDIS_PORT="${REDIS_PORT:-6379}"
CLICKHOUSE_HTTP_URL="${CLICKHOUSE_HTTP_URL:-http://localhost:8123}"
CLICKHOUSE_USER="${CLICKHOUSE_USER:-decision_engine}"
CLICKHOUSE_PASSWORD="${CLICKHOUSE_PASSWORD:-decision_engine}"
KAFKA_HOST="${KAFKA_HOST:-localhost}"
KAFKA_PORT="${KAFKA_PORT:-9092}"

PORTS=(8080 5173 "$DOCS_PORT" 9094)
EXPECTED_CLICKHOUSE_TABLES=(
    analytics_api_events_queue
    analytics_domain_events_queue
    analytics_api_events_v1
    analytics_domain_events_v1
)

check_and_kill_ports() {
    local pids_to_kill=()
    local ports_in_use=()

    echo "Checking for processes on ports ${PORTS[*]}..."
    echo ""

    for port in "${PORTS[@]}"; do
        local pids
        pids=$(lsof -t -iTCP:$port -sTCP:LISTEN 2>/dev/null || true)
        if [ -n "$pids" ]; then
            ports_in_use+=("$port")
            while IFS= read -r pid; do
                [ -z "$pid" ] && continue
                local cmd
                cmd=$(ps -p "$pid" -o command= 2>/dev/null || echo "unknown process")
                pids_to_kill+=("$pid")
                echo "  [!] Port $port is in use by PID $pid"
                echo "      Command: $cmd"
            done <<< "$pids"
        fi
    done

    if [ ${#pids_to_kill[@]} -gt 0 ]; then
        echo ""
        echo "=========================================="
        echo "  WARNING: Found processes on ports ${ports_in_use[*]}"
        echo "  These processes will be killed to proceed."
        echo "=========================================="
        echo ""
        echo "Press Enter to continue and kill these processes, or Ctrl+C to abort..."
        read -r

        echo ""
        echo "Killing processes..."
        for pid in "${pids_to_kill[@]}"; do
            kill "$pid" 2>/dev/null || true
            echo "  Killed PID $pid"
        done

        sleep 1

        for port in "${PORTS[@]}"; do
            local pid
            pid=$(lsof -t -iTCP:$port -sTCP:LISTEN 2>/dev/null || true)
            if [ -n "$pid" ]; then
                kill -9 "$pid" 2>/dev/null || true
                echo "  Force killed PID $pid on port $port"
            fi
        done

        echo "Done. All ports cleared."
        echo ""
    else
        echo "No processes found on ports ${PORTS[*]}."
        echo ""
    fi
}

cleanup() {
    local exit_code="${1:-0}"
    echo ""
    echo "Stopping services..."
    if [ -n "$SERVER_PID" ]; then
        kill "$SERVER_PID" 2>/dev/null || true
    fi
    if [ -n "$DASHBOARD_PID" ]; then
        kill "$DASHBOARD_PID" 2>/dev/null || true
    fi
    if [ -n "$DOCS_PID" ]; then
        kill "$DOCS_PID" 2>/dev/null || true
    fi
    exit "$exit_code"
}

trap cleanup SIGINT SIGTERM

command_exists() {
    command -v "$1" >/dev/null 2>&1
}

check_docker_daemon() {
    if ! command_exists docker; then
        return 1
    fi

    docker info >/dev/null 2>&1
}

check_postgres() {
    if command_exists pg_isready; then
        pg_isready -h "$POSTGRES_HOST" -p "$POSTGRES_PORT" -U "$POSTGRES_USER" -d "$POSTGRES_DB" >/dev/null 2>&1
    elif command_exists nc; then
        nc -z "$POSTGRES_HOST" "$POSTGRES_PORT" >/dev/null 2>&1
    else
        lsof -t -iTCP:"$POSTGRES_PORT" -sTCP:LISTEN >/dev/null 2>&1
    fi
}

check_redis() {
    if command_exists redis-cli; then
        [ "$(redis-cli -h "$REDIS_HOST" -p "$REDIS_PORT" ping 2>/dev/null)" = "PONG" ]
    elif command_exists nc; then
        nc -z "$REDIS_HOST" "$REDIS_PORT" >/dev/null 2>&1
    else
        lsof -t -iTCP:"$REDIS_PORT" -sTCP:LISTEN >/dev/null 2>&1
    fi
}

check_clickhouse() {
    curl -fsS \
        --user "${CLICKHOUSE_USER}:${CLICKHOUSE_PASSWORD}" \
        "${CLICKHOUSE_HTTP_URL}/?query=SELECT%201" >/dev/null 2>&1
}

check_kafka() {
    if command_exists nc; then
        nc -z "$KAFKA_HOST" "$KAFKA_PORT" >/dev/null 2>&1
    else
        lsof -t -iTCP:"$KAFKA_PORT" -sTCP:LISTEN >/dev/null 2>&1
    fi
}

check_clickhouse_schema() {
    local missing=0

    for table_name in "${EXPECTED_CLICKHOUSE_TABLES[@]}"; do
        local query
        query="SELECT%20count()%20FROM%20system.tables%20WHERE%20database%20%3D%20'decision_engine_analytics'%20AND%20name%20%3D%20'${table_name}'"
        local result
        result=$(curl -fsS \
            --user "${CLICKHOUSE_USER}:${CLICKHOUSE_PASSWORD}" \
            "${CLICKHOUSE_HTTP_URL}/?query=${query}" 2>/dev/null || echo "0")
        if [ "$result" != "1" ]; then
            echo "  [missing] ClickHouse table ${table_name}"
            missing=1
        fi
    done

    return "$missing"
}

print_service_status() {
    local service_name="$1"
    local status="$2"

    if [ "$status" -eq 1 ]; then
        echo "  [ok] $service_name"
    else
        echo "  [missing] $service_name"
    fi
}

run_infra_checklist() {
    echo "Running infrastructure checklist..."

    if check_docker_daemon; then
        DOCKER_READY=1
    else
        DOCKER_READY=0
    fi

    if check_postgres; then
        POSTGRES_READY=1
    else
        POSTGRES_READY=0
    fi

    if check_redis; then
        REDIS_READY=1
    else
        REDIS_READY=0
    fi

    if check_kafka; then
        KAFKA_READY=1
    else
        KAFKA_READY=0
    fi

    if check_clickhouse; then
        CLICKHOUSE_READY=1
    else
        CLICKHOUSE_READY=0
    fi

    print_service_status "Docker daemon" "$DOCKER_READY"
    print_service_status "Postgres (${POSTGRES_HOST}:${POSTGRES_PORT})" "$POSTGRES_READY"
    print_service_status "Redis (${REDIS_HOST}:${REDIS_PORT})" "$REDIS_READY"
    print_service_status "Kafka (${KAFKA_HOST}:${KAFKA_PORT})" "$KAFKA_READY"
    print_service_status "ClickHouse (${CLICKHOUSE_HTTP_URL})" "$CLICKHOUSE_READY"
    echo ""
}

wait_for_postgres() {
    local attempts=0
    local max_attempts=60

    echo "Waiting for Postgres on ${POSTGRES_HOST}:${POSTGRES_PORT}..."

    while [ $attempts -lt $max_attempts ]; do
        if check_postgres; then
            echo "Postgres is healthy."
            echo ""
            return 0
        fi

        attempts=$((attempts + 1))
        sleep 1
    done

    echo "Postgres did not become healthy within ${max_attempts}s."
    return 1
}

wait_for_redis() {
    local attempts=0
    local max_attempts=60

    echo "Waiting for Redis on ${REDIS_HOST}:${REDIS_PORT}..."

    while [ $attempts -lt $max_attempts ]; do
        if check_redis; then
            echo "Redis is healthy."
            echo ""
            return 0
        fi

        attempts=$((attempts + 1))
        sleep 1
    done

    echo "Redis did not become healthy within ${max_attempts}s."
    return 1
}

wait_for_kafka() {
    local attempts=0
    local max_attempts=60

    echo "Waiting for Kafka on ${KAFKA_HOST}:${KAFKA_PORT}..."

    while [ $attempts -lt $max_attempts ]; do
        if check_kafka; then
            echo "Kafka is healthy."
            echo ""
            return 0
        fi

        attempts=$((attempts + 1))
        sleep 1
    done

    echo "Kafka did not become healthy within ${max_attempts}s."
    return 1
}

wait_for_clickhouse() {
    local attempts=0
    local max_attempts=60

    echo "Waiting for ClickHouse on ${CLICKHOUSE_HTTP_URL}..."

    while [ $attempts -lt $max_attempts ]; do
        if check_clickhouse; then
            echo "ClickHouse is healthy."
            echo ""
            return 0
        fi

        attempts=$((attempts + 1))
        sleep 1
    done

    echo "ClickHouse did not become healthy within ${max_attempts}s."
    return 1
}

wait_for_backend() {
    local attempts=0
    local max_attempts=90

    echo "Waiting for Decision Engine API on http://localhost:8080/health..."

    while [ $attempts -lt $max_attempts ]; do
        if curl -fsS http://localhost:8080/health >/dev/null 2>&1; then
            echo "Decision Engine API is healthy."
            echo ""
            return 0
        fi

        if ! kill -0 "$SERVER_PID" 2>/dev/null; then
            echo "Decision Engine server exited before becoming healthy."
            return 1
        fi

        attempts=$((attempts + 1))
        sleep 1
    done

    echo "Decision Engine API did not become healthy within ${max_attempts}s."
    return 1
}

wait_for_docs() {
    local attempts=0
    local max_attempts=120

    echo "Waiting for docs preview on ${DOCS_HOME_URL}..."

    while [ $attempts -lt $max_attempts ]; do
        if curl -fsS "${DOCS_HOME_URL}" >/dev/null 2>&1; then
            echo "Docs preview is healthy."
            echo ""
            return 0
        fi

        if ! kill -0 "$DOCS_PID" 2>/dev/null; then
            echo "Docs preview exited before becoming healthy."
            echo "Check ${DOCS_LOG_PATH} for details."
            return 1
        fi

        attempts=$((attempts + 1))
        sleep 1
    done

    echo "Docs preview did not become healthy within ${max_attempts}s."
    echo "Check ${DOCS_LOG_PATH} for details."
    return 1
}

check_and_kill_ports
run_infra_checklist

if [ "${DOCKER_READY}" -eq 0 ] && ([ "${POSTGRES_READY}" -eq 0 ] || [ "${REDIS_READY}" -eq 0 ] || [ "${KAFKA_READY}" -eq 0 ] || [ "${CLICKHOUSE_READY}" -eq 0 ]); then
    echo "Cannot start missing infrastructure services because Docker is not available."
    echo "Start Docker/OrbStack first, then rerun ./oneclick.sh."
    cleanup 1
fi

if [ "${POSTGRES_READY}" -eq 0 ] || [ "${REDIS_READY}" -eq 0 ] || [ "${KAFKA_READY}" -eq 0 ] || [ "${CLICKHOUSE_READY}" -eq 0 ]; then
    echo "Starting infrastructure services..."
    docker compose --profile postgres-ghcr --profile analytics-clickhouse up -d postgresql redis kafka kafka-init clickhouse
    echo ""
fi

if [ "${POSTGRES_READY}" -eq 0 ] && ! wait_for_postgres; then
    cleanup 1
fi

if [ "${REDIS_READY}" -eq 0 ] && ! wait_for_redis; then
    cleanup 1
fi

if [ "${KAFKA_READY}" -eq 0 ] && ! wait_for_kafka; then
    cleanup 1
fi

if [ "${CLICKHOUSE_READY}" -eq 0 ] && ! wait_for_clickhouse; then
    cleanup 1
fi

echo "Infrastructure checklist after bring-up:"
run_infra_checklist

if ! check_clickhouse_schema; then
    echo ""
    echo "ClickHouse is reachable but the analytics schema is incomplete."
    echo "Run 'make reset-analytics-clickhouse' to recreate the ClickHouse analytics volume."
    cleanup 1
fi

echo "Running Postgres migrations..."
just migrate-pg

echo "Starting Decision Engine server..."
cargo run --no-default-features --features postgres &
SERVER_PID=$!

if ! wait_for_backend; then
    cleanup 1
fi

echo "Installing dashboard dependencies..."
cd "$SCRIPT_DIR/website"
npm install --silent

echo "Starting docs preview..."
cd "$SCRIPT_DIR/docs"
rm -f "$DOCS_LOG_PATH"
if [ "${DOCS_PORT}" != "3000" ]; then
    echo "Mint preview uses port 3000 in this environment; overriding DOCS_PORT=${DOCS_PORT} to 3000."
    DOCS_PORT="3000"
    DOCS_URL="http://localhost:${DOCS_PORT}"
    DOCS_HOME_URL="${DOCS_URL}/introduction"
    API_REF_URL="${DOCS_URL}/api-reference"
    API_EXAMPLES_URL="${DOCS_URL}/api-refs/api-ref"
fi
PORT="$DOCS_PORT" mint dev --no-open >"$DOCS_LOG_PATH" 2>&1 &
DOCS_PID=$!

if ! wait_for_docs; then
    cleanup 1
fi

cd "$SCRIPT_DIR/website"
echo "Starting dashboard..."
npm run dev &
DASHBOARD_PID=$!

cd "$SCRIPT_DIR"

echo ""
echo "=========================================="
echo "  Decision Engine is starting up!"
echo "=========================================="
echo ""
echo "  Server:       http://localhost:8080"
echo "  Dashboard:    http://localhost:5173/"
echo "  Docs:         $DOCS_HOME_URL"
echo "  API Ref:      $API_REF_URL"
echo "  API Examples: $API_EXAMPLES_URL"
echo "  OpenAPI:      $OPENAPI_PATH"
echo ""
echo "=========================================="
echo ""

wait $SERVER_PID $DASHBOARD_PID
