#!/bin/bash

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

OPENAPI_PATH="$SCRIPT_DIR/docs/openapi.json"
DOCS_PORT="${DOCS_PORT:-3000}"
DOCS_URL="http://localhost:${DOCS_PORT}"
DOCS_HOME_URL="${DOCS_URL}/introduction"
API_REF_URL="${DOCS_URL}/api-reference"
API_EXAMPLES_URL="${DOCS_URL}/api-reference1"
DOCS_LOG_PATH="${SCRIPT_DIR}/.mintlify-dev.log"

PORTS=(8080 5173 "$DOCS_PORT")

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
            kill $pid 2>/dev/null || true
            echo "  Killed PID $pid"
        done

        sleep 1

        for port in "${PORTS[@]}"; do
            local pid
            pid=$(lsof -t -iTCP:$port -sTCP:LISTEN 2>/dev/null || true)
            if [ -n "$pid" ]; then
                kill -9 $pid 2>/dev/null || true
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
        kill $SERVER_PID 2>/dev/null || true
    fi
    if [ -n "$DASHBOARD_PID" ]; then
        kill $DASHBOARD_PID 2>/dev/null || true
    fi
    if [ -n "$DOCS_PID" ]; then
        kill $DOCS_PID 2>/dev/null || true
    fi
    exit "$exit_code"
}

trap cleanup SIGINT SIGTERM

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
    API_EXAMPLES_URL="${DOCS_URL}/api-reference1"
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
echo "  Server:      http://localhost:8080"
echo "  Dashboard:   http://localhost:5173/dashboard/"
echo "  Docs:        $DOCS_HOME_URL"
echo "  API Ref:     $API_REF_URL"
echo "  API Examples:$API_EXAMPLES_URL"
echo "  OpenAPI:     $OPENAPI_PATH"
echo ""
echo "=========================================="
echo ""

wait $SERVER_PID $DASHBOARD_PID
