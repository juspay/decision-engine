#!/usr/bin/env bash
# Latency and throughput benchmark of the routing endpoints against a running server.
#
# Two phases, each a series of k6 runs of load_test.js in its open (fixed arrival rate) model:
#
#   1. Latency: every endpoint at LAT_RATE req/s for LAT_DURATION, ROUNDS times. Endpoints
#      alternate within each round and the order rotates between rounds, so drift over the run
#      spreads across all endpoints instead of landing on whichever ran last.
#   2. Throughput: every endpoint at each rate in TPS_RATES for TPS_DURATION. A step is sustained
#      when k6 dropped no iterations, every check passed and no routing or feedback call errored;
#      the highest sustained step is the endpoint's throughput.
#
# Each endpoint gets a short warm-up run first so that the measured runs start with warm caches.
# summarize_ci_benchmark.py turns OUT into a Markdown report.
#
# Usage (server already listening on BASE_URL):
#   bash scripts/load-test/ci_benchmark.sh
#   ROUNDS=5 TPS_RATES=500,1000,2000 OUT=/tmp/bench bash scripts/load-test/ci_benchmark.sh
#   K6_PREFIX="taskset -c 2,3" bash scripts/load-test/ci_benchmark.sh   # pin k6 off the server's cores

set -euo pipefail

BASE_URL="${BASE_URL:-http://127.0.0.1:8080}"
OUT="${OUT:-scripts/load-test/profile_results/ci_benchmark_$(date +%Y%m%d_%H%M%S)}"
ENDPOINTS="${ENDPOINTS:-DECIDE,HYBRID_ROUTING,SR_BASED_ROUTING,RULE_BASED_ROUTING}"
LAT_RATE="${LAT_RATE:-200}"
LAT_DURATION="${LAT_DURATION:-60s}"
ROUNDS="${ROUNDS:-3}"
TPS_RATES="${TPS_RATES:-200,400,800,1200,1600,2000}"
TPS_DURATION="${TPS_DURATION:-30s}"
WARMUP_DURATION="${WARMUP_DURATION:-15s}"
K6_PREFIX="${K6_PREFIX:-}"

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
IFS=',' read -r -a endpoints <<< "$ENDPOINTS"
IFS=',' read -r -a tps_rates <<< "$TPS_RATES"

mkdir -p "$OUT"
cat > "$OUT/params.json" <<EOF
{"base_url": "$BASE_URL", "endpoints": "$ENDPOINTS", "lat_rate": $LAT_RATE,
 "lat_duration": "$LAT_DURATION", "rounds": $ROUNDS, "tps_rates": "$TPS_RATES",
 "tps_duration": "$TPS_DURATION"}
EOF

# run_k6 <algorithm> <rate> <duration> <results dir>
# k6 exits non-zero when a threshold fails, which is an expected outcome on the higher throughput
# steps; the summary judges each run from its results file instead.
run_k6() {
  local algo="$1" rate="$2" duration="$3" dir="$4"
  mkdir -p "$dir"
  echo "── $algo @ ${rate} req/s for $duration → $dir"
  $K6_PREFIX k6 run --quiet \
    -e BASE_URL="$BASE_URL" \
    -e ALGORITHM="$algo" \
    -e RATE="$rate" \
    -e DURATION="$duration" \
    -e RESULTS_DIR="$dir/" \
    "$HERE/load_test.js" > "$dir/${algo}.log" 2>&1 || echo "   k6 exited $? (see $dir/${algo}.log)"
}

for algo in "${endpoints[@]}"; do
  run_k6 "$algo" "$LAT_RATE" "$WARMUP_DURATION" "$OUT/warmup"
done

n=${#endpoints[@]}
for ((round = 1; round <= ROUNDS; round++)); do
  for ((i = 0; i < n; i++)); do
    algo="${endpoints[$(((i + round - 1) % n))]}"
    run_k6 "$algo" "$LAT_RATE" "$LAT_DURATION" "$OUT/latency/round_$round"
  done
done

for algo in "${endpoints[@]}"; do
  for rate in "${tps_rates[@]}"; do
    run_k6 "$algo" "$rate" "$TPS_DURATION" "$OUT/throughput/rate_$rate"
  done
done

echo "Results in $OUT"
