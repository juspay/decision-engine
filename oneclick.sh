#!/bin/bash

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

PORTS=(8080 5173)

kill_port_process() {
    local port=$1
    local pid
    pid=$(lsof -t -i :$port 2>/dev/null || true)
    if [ -n "$pid" ]; then
        local cmd
        cmd=$(ps -p $pid -o command= 2>/dev/null || echo "unknown process")
        echo "  Port $port: PID $pid ($cmd)"
        echo $pid
    fi
}

check_and_kill_ports() {
    local pids_to_kill=()
    local ports_in_use=()

    echo "Checking for processes on ports ${PORTS[*]}..."
    echo ""

    for port in "${PORTS[@]}"; do
        local pid
        pid=$(lsof -t -i :$port 2>/dev/null || true)
        if [ -n "$pid" ]; then
            local cmd
            cmd=$(ps -p $pid -o command= 2>/dev/null || echo "unknown process")
            ports_in_use+=("$port")
            pids_to_kill+=("$pid")
            echo "  [!] Port $port is in use by PID $pid"
            echo "      Command: $cmd"
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
            pid=$(lsof -t -i :$port 2>/dev/null || true)
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
    echo ""
    echo "Stopping services..."
    if [ -n "$SERVER_PID" ]; then
        kill $SERVER_PID 2>/dev/null || true
    fi
    if [ -n "$DASHBOARD_PID" ]; then
        kill $DASHBOARD_PID 2>/dev/null || true
    fi
    exit 0
}

trap cleanup SIGINT SIGTERM

check_and_kill_ports

echo "Starting Decision Engine server..."
cargo run --no-default-features --features postgres &
SERVER_PID=$!

echo "Installing dashboard dependencies..."
cd "$SCRIPT_DIR/website"
npm install --silent

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
echo ""
echo "=========================================="
echo ""

wait $SERVER_PID $DASHBOARD_PID
