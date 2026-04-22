#!/bin/bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

COMPOSE_PROFILE="${ONECLICK_COMPOSE_PROFILE:-postgres-local}"
KEEP_INFRA="${ONECLICK_KEEP_INFRA:-0}"
API_URL="http://localhost:8080/health"
DASHBOARD_URL="http://localhost:5173/dashboard/"
PORTS=(8080 5173)
CORE_INFRA_SERVICES=(postgresql redis kafka clickhouse)
INIT_INFRA_SERVICES=(kafka-init clickhouse-init)

SERVER_PID=""
DASHBOARD_PID=""
STARTED_SERVICES=()
INITIAL_RUNNING_SERVICES=""

require_command() {
    local command_name="$1"
    if ! command -v "$command_name" >/dev/null 2>&1; then
        echo "Missing required command: $command_name" >&2
        exit 1
    fi
}

compose() {
    docker compose --profile "$COMPOSE_PROFILE" "$@"
}

service_in_list() {
    local target="$1"
    local services="$2"

    printf '%s\n' "$services" | grep -Fxq "$target"
}

compose_service_exists() {
    local service="$1"

    compose config --services | grep -Fxq "$service"
}

check_and_kill_ports() {
    local pids_to_kill=()
    local ports_in_use=()

    echo "Checking for processes on ports ${PORTS[*]}..."
    echo ""

    for port in "${PORTS[@]}"; do
        local pid
        pid="$(lsof -t -i :"$port" 2>/dev/null || true)"
        if [ -n "$pid" ]; then
            local cmd
            cmd="$(ps -p "$pid" -o command= 2>/dev/null || echo "unknown process")"
            ports_in_use+=("$port")
            pids_to_kill+=("$pid")
            echo "  [!] Port $port is in use by PID $pid"
            echo "      Command: $cmd"
        fi
    done

    if [ ${#pids_to_kill[@]} -eq 0 ]; then
        echo "No processes found on ports ${PORTS[*]}."
        echo ""
        return
    fi

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
        pid="$(lsof -t -i :"$port" 2>/dev/null || true)"
        if [ -n "$pid" ]; then
            kill -9 "$pid" 2>/dev/null || true
            echo "  Force killed PID $pid on port $port"
        fi
    done

    echo "Done. App ports cleared."
    echo ""
}

wait_for_service_health() {
    local service="$1"
    local timeout="${2:-120}"
    local elapsed=0

    echo "Waiting for $service to become healthy..."
    while [ "$elapsed" -lt "$timeout" ]; do
        local container_id
        container_id="$(compose ps -q "$service" 2>/dev/null || true)"
        if [ -n "$container_id" ]; then
            local status
            status="$(docker inspect --format '{{if .State.Health}}{{.State.Health.Status}}{{else}}{{.State.Status}}{{end}}' "$container_id" 2>/dev/null || true)"
            if [ "$status" = "healthy" ] || [ "$status" = "running" ]; then
                echo "  $service is $status"
                return
            fi
        fi

        sleep 2
        elapsed=$((elapsed + 2))
    done

    echo "Timed out waiting for $service to become healthy." >&2
    compose logs "$service" || true
    exit 1
}

wait_for_service_completion() {
    local service="$1"
    local timeout="${2:-120}"
    local elapsed=0

    echo "Waiting for $service to complete..."
    while [ "$elapsed" -lt "$timeout" ]; do
        local container_id
        container_id="$(compose ps -a -q "$service" 2>/dev/null || true)"
        if [ -n "$container_id" ]; then
            local status
            local exit_code
            status="$(docker inspect --format '{{.State.Status}}' "$container_id" 2>/dev/null || true)"
            exit_code="$(docker inspect --format '{{.State.ExitCode}}' "$container_id" 2>/dev/null || true)"

            if [ "$status" = "exited" ] && [ "$exit_code" = "0" ]; then
                echo "  $service completed successfully"
                return
            fi

            if [ "$status" = "exited" ] && [ "$exit_code" != "0" ]; then
                echo "$service failed with exit code $exit_code." >&2
                compose logs "$service" || true
                exit 1
            fi
        fi

        sleep 2
        elapsed=$((elapsed + 2))
    done

    echo "Timed out waiting for $service to complete." >&2
    compose logs "$service" || true
    exit 1
}

wait_for_http() {
    local url="$1"
    local name="$2"
    local timeout="${3:-120}"
    local elapsed=0

    echo "Waiting for $name at $url ..."
    while [ "$elapsed" -lt "$timeout" ]; do
        if curl -fsS "$url" >/dev/null 2>&1; then
            echo "  $name is ready"
            return
        fi

        sleep 2
        elapsed=$((elapsed + 2))
    done

    echo "Timed out waiting for $name at $url." >&2
    exit 1
}

start_compose_services() {
    local services_to_start=()
    local service

    for service in "$@"; do
        if ! compose_service_exists "$service"; then
            continue
        fi

        services_to_start+=("$service")
        if ! service_in_list "$service" "$INITIAL_RUNNING_SERVICES"; then
            STARTED_SERVICES+=("$service")
        fi
    done

    if [ ${#services_to_start[@]} -gt 0 ]; then
        compose up -d "${services_to_start[@]}"
    fi
}

run_postgres_migrations() {
    echo "Running PostgreSQL migrations..."
    if command -v just >/dev/null 2>&1 && command -v diesel >/dev/null 2>&1; then
        just migrate-pg
    else
        compose run --rm db-migrator-postgres
    fi
}

cleanup() {
    local exit_code=$?

    trap - EXIT SIGINT SIGTERM

    echo ""
    echo "Stopping local processes..."

    if [ -n "$SERVER_PID" ]; then
        kill "$SERVER_PID" 2>/dev/null || true
        wait "$SERVER_PID" 2>/dev/null || true
    fi

    if [ -n "$DASHBOARD_PID" ]; then
        kill "$DASHBOARD_PID" 2>/dev/null || true
        wait "$DASHBOARD_PID" 2>/dev/null || true
    fi

    if [ "$KEEP_INFRA" != "1" ] && [ ${#STARTED_SERVICES[@]} -gt 0 ]; then
        echo "Stopping compose services started by oneclick..."
        compose stop "${STARTED_SERVICES[@]}" >/dev/null 2>&1 || true
    fi

    exit "$exit_code"
}

trap cleanup EXIT SIGINT SIGTERM

require_command docker
require_command cargo
require_command npm
require_command lsof
require_command curl

docker info >/dev/null
compose config -q

check_and_kill_ports

INITIAL_RUNNING_SERVICES="$(compose ps --status running --services 2>/dev/null || true)"

echo "Starting infrastructure with docker compose profile '$COMPOSE_PROFILE'..."
start_compose_services "${CORE_INFRA_SERVICES[@]}"

wait_for_service_health postgresql
wait_for_service_health redis
wait_for_service_health kafka
wait_for_service_health clickhouse

for service in "${INIT_INFRA_SERVICES[@]}"; do
    if compose_service_exists "$service"; then
        compose rm -f -s "$service" >/dev/null 2>&1 || true
        start_compose_services "$service"
        wait_for_service_completion "$service"
    fi
done

run_postgres_migrations

echo "Starting Decision Engine server..."
cargo run --no-default-features --features postgres &
SERVER_PID=$!

wait_for_http "$API_URL" "Decision Engine API"

echo "Installing dashboard dependencies..."
cd "$SCRIPT_DIR/website"
npm install --silent

echo "Starting dashboard..."
npm run dev &
DASHBOARD_PID=$!

cd "$SCRIPT_DIR"

wait_for_http "$DASHBOARD_URL" "Dashboard"

echo ""
echo "=========================================="
echo "  Decision Engine full stack is ready"
echo "=========================================="
echo ""
echo "  API:         http://localhost:8080"
echo "  Health:      $API_URL"
echo "  Dashboard:   $DASHBOARD_URL"
echo "  PostgreSQL:  localhost:5432"
echo "  Redis:       localhost:6379"
echo "  Kafka:       localhost:9092"
echo "  ClickHouse:  localhost:8123"
if [ "$KEEP_INFRA" = "1" ]; then
    echo "  Infra stop:  skipped on exit because ONECLICK_KEEP_INFRA=1"
else
    echo "  Infra stop:  started services will be stopped on exit"
fi
echo ""
echo "Press Ctrl+C to stop the local API/dashboard."
echo ""

wait "$SERVER_PID" "$DASHBOARD_PID"
