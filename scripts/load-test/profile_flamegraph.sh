#!/usr/bin/env bash
# Profile the decision-engine binary under load and produce a samply/Firefox Profiler flamegraph.
#
# What it does:
#   1. Stops the Docker container (frees port 8080)
#   2. Starts the native profiling binary on port 8080 under samply
#   3. Runs the standard 3-stage k6 load test (5→20→50 VUs)
#   4. Saves profile.json.gz; restarts Docker container
#
# Requirements: samply, k6, decision-engine-local:infra Docker image running
# Build the profiling binary first:
#   cargo build --profile release-profiling
#
# Usage: ./scripts/load-test/profile_flamegraph.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
BINARY="$ROOT_DIR/target/release-profiling/open_router"
PROFILE_OUT="$SCRIPT_DIR/profile.json.gz"
SAMPLY_PID=""

if [[ ! -f "$BINARY" ]]; then
    echo "ERROR: profiling binary not found at $BINARY"
    echo "Build it first: cargo build --profile release-profiling"
    exit 1
fi

cleanup() {
    echo ""
    if [[ -n "$SAMPLY_PID" ]]; then
        echo "[cleanup] Stopping profiling binary (PID $SAMPLY_PID)..."
        kill -INT "$SAMPLY_PID" 2>/dev/null || true
        wait "$SAMPLY_PID" 2>/dev/null || true
    fi
    echo "[cleanup] Restarting Docker container..."
    docker compose start open-router-pg-ghcr >/dev/null 2>&1 || true
    echo "[cleanup] Done."
}
trap cleanup EXIT

echo "=== Decision-Engine Flamegraph Profiler ==="
echo "Binary : $BINARY ($(du -sh "$BINARY" | cut -f1))"
echo "Output : $PROFILE_OUT"
echo ""

# Stop the Docker container to free port 8080
echo "[1/5] Stopping Docker container (frees port 8080)..."
docker compose stop open-router-pg-ghcr

# Start the native binary under samply
echo "[2/5] Starting native binary under samply (rate=2000 Hz)..."
samply record --save-only -o "$PROFILE_OUT" -r 2000 -- "$BINARY" &
SAMPLY_PID=$!

# Wait for server to accept connections
echo "[3/5] Waiting for server to be ready..."
for i in $(seq 1 40); do
    if curl -s "http://localhost:8080/health" >/dev/null 2>&1; then
        echo "      Ready after ${i}s"
        break
    fi
    if ! kill -0 "$SAMPLY_PID" 2>/dev/null; then
        echo "ERROR: samply/binary exited early"
        exit 1
    fi
    sleep 1
done

if ! curl -s "http://localhost:8080/health" >/dev/null 2>&1; then
    echo "ERROR: server not responding after 40s"
    exit 1
fi

# Run the standard load test (same format as benchmark runs)
echo "[4/5] Running load test (5→20→50 VUs, 3×30s)..."
k6 run \
    --env ENV=local \
    --stage "0s:5,30s:5" \
    --stage "0s:20,30s:20" \
    --stage "0s:50,90s:50" \
    "$SCRIPT_DIR/load_test.js" \
  || true  # k6 non-zero exit (thresholds) should not abort profiling

echo "[5/5] Stopping binary and saving profile..."
kill -INT "$SAMPLY_PID" 2>/dev/null || true
wait "$SAMPLY_PID" 2>/dev/null || true
SAMPLY_PID=""  # prevent double-kill in cleanup

if [[ -f "$PROFILE_OUT" ]]; then
    SIZE=$(du -sh "$PROFILE_OUT" | cut -f1)
    echo ""
    echo "======================================"
    echo "Profile saved: $PROFILE_OUT ($SIZE)"
    echo "======================================"
    echo ""
    echo "View it with:"
    echo "  samply load $PROFILE_OUT"
    echo ""
    echo "Or open https://profiler.firefox.com and drag-and-drop the .gz file."
else
    echo "ERROR: profile not saved. Check samply output above."
    exit 1
fi
