#!/usr/bin/env bash
# Benchmarks the Decision Audit list's filters against a running server and its ClickHouse.
#
# Each case calls GET /analytics/payment-audit over custom windows of a merchant's events, then
# reads what the server's ClickHouse queries cost from system.query_log: wall time, the summed
# query time, the peak memory of any one query, and rows / bytes read.
#
#   API_KEY=DE_... MERCHANT_ID=merchant_x START_MS=1789983504540 \
#     scripts/bench/payment-audit-filters.sh
#
# Optional: BASE_URL (http://localhost:8080), WINDOWS_MIN ("15 60 360"), RUNS (1),
# CLICKHOUSE (the clickhouse-client command; default: docker exec decision-engine-clickhouse-1
# clickhouse-client).
set -euo pipefail

BASE_URL=${BASE_URL:-http://localhost:8080}
WINDOWS_MIN=${WINDOWS_MIN:-"15 60 360"}
RUNS=${RUNS:-1}
CLICKHOUSE=${CLICKHOUSE:-"docker exec decision-engine-clickhouse-1 clickhouse-client"}
: "${API_KEY:?set API_KEY to an API key of MERCHANT_ID}"
: "${MERCHANT_ID:?set MERCHANT_ID}"
: "${START_MS:?set START_MS, the window start in epoch ms}"

ch() { $CLICKHOUSE -q "$1"; }

# name|query string (without the window)
CASES=(
  "no filter|"
  "currency=USD (existing attribute filter)|currency=USD"
  "payment_fields card_bin (no match: full scan)|payment_fields=%7B%22card_bin%22%3A%22424242%22%7D"
  "payment_fields currency=USD (~1/3 match)|payment_fields=%7B%22currency%22%3A%22USD%22%7D"
  "payment_fields metadata.segment|payment_fields=%7B%22metadata.segment%22%3A%22premium%22%7D"
  "payment_fields card_bin + currency|payment_fields=%7B%22card_bin%22%3A%22424242%22%2C%22currency%22%3A%22USD%22%7D"
)

printf '%-48s %7s %9s %9s %9s %10s %11s %10s\n' case window events wall_ms ch_ms peak_mem rows_read read
for minutes in $WINDOWS_MIN; do
  end_ms=$((START_MS + minutes * 60 * 1000))
  events=$(ch "SELECT count() FROM analytics_domain_events WHERE merchant_id = '$MERCHANT_ID' AND created_at_ms BETWEEN $START_MS AND $end_ms")
  for entry in "${CASES[@]}"; do
    name=${entry%%|*}
    filter=${entry#*|}
    url="$BASE_URL/analytics/payment-audit?range=1h&start_ms=$START_MS&end_ms=$end_ms&page=1&page_size=20${filter:+&$filter}"
    best=""
    for _ in $(seq "$RUNS"); do
      ch "SYSTEM FLUSH LOGS"
      since=$(ch "SELECT now64(3)")
      wall=$(curl -s -o /dev/null -w '%{http_code} %{time_total}' "$url" \
        -H "x-api-key: $API_KEY" -H 'x-tenant-id: public')
      status=${wall%% *}
      wall_ms=$(awk -v s="${wall#* }" 'BEGIN { printf "%d", s * 1000 }')
      ch "SYSTEM FLUSH LOGS"
      cost=$(ch "SELECT sum(query_duration_ms), formatReadableSize(max(memory_usage)), sum(read_rows), formatReadableSize(sum(read_bytes)) FROM system.query_log WHERE event_time_microseconds >= '$since' AND type IN ('QueryFinish', 'ExceptionWhileProcessing') AND query LIKE '%analytics_%' AND query NOT LIKE '%system.query_log%' FORMAT TSV")
      # Keep the first run. ClickHouse's query condition cache remembers which granules a
      # predicate ruled out, so an identical repeat of a filter can skip the scan entirely.
      [[ -n $best ]] || best="$status $wall_ms $cost"
    done
    read -r status wall_ms ch_ms mem mem_unit rows read read_unit <<<"$best"
    [[ $status == 200 ]] || name="$name (HTTP $status)"
    printf '%-48s %6sm %9s %9s %9s %10s %11s %10s\n' "$name" "$minutes" "$events" "$wall_ms" "$ch_ms" "$mem $mem_unit" "$rows" "$read $read_unit"
  done
done
