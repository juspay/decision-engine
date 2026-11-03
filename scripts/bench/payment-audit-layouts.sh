#!/usr/bin/env bash
# Compares table layouts for the payment field filter's subquery, on copies of one merchant's
# events (seed them with seed-payment-audit.sh):
#
#   all       analytics_domain_events as it is, parsing every event's request (the filter before
#             it was limited to request-carrying events)
#   current   analytics_domain_events as it is: monthly partitions, sorted by
#             (merchant_id, lookup_key, created_at_ms, …)
#   minmax    the same, plus a minmax skip index on created_at_ms
#   daily     partitioned by day instead of month
#
# Each runs the subquery the server sends for `payment_fields={"card_bin":…}` RUNS times under the
# sandbox profile (50 MB, 2 threads) with caches dropped, and reports median time, rows and bytes
# read. The copies are dropped at the end.
#
#   MERCHANT_ID=bench_audit END_MS=1793491199999 scripts/bench/payment-audit-layouts.sh
set -euo pipefail

MERCHANT_ID=${MERCHANT_ID:-bench_audit}
END_MS=${END_MS:?set END_MS, the window end in epoch ms}
WINDOWS=${WINDOWS:-"1h 1d 7d"}
RUNS=${RUNS:-5}
BIN=${BIN:-400000}
CLICKHOUSE=${CLICKHOUSE:-"docker exec -i decision-engine-clickhouse-1 clickhouse-client"}
SANDBOX="--max_memory_usage=50000000 --max_threads=2 --use_query_condition_cache=0"

ch() { $CLICKHOUSE "$@"; }

columns=$(ch -q "SELECT arrayStringConcat(groupArray(concat(name, ' ', type)), ', ') FROM system.columns \
  WHERE database = currentDatabase() AND table = 'analytics_domain_events' AND default_kind != 'MATERIALIZED'")
sort_key="merchant_id, lookup_key, created_at_ms, api_flow, flow_type, event_id"
created_at="created_at DateTime64(3, 'UTC') MATERIALIZED fromUnixTimestamp64Milli(created_at_ms)"

echo "Copying ${MERCHANT_ID}'s events into the layouts..."
ch --multiquery <<SQL
DROP TABLE IF EXISTS bench_layout_current;
DROP TABLE IF EXISTS bench_layout_minmax;
DROP TABLE IF EXISTS bench_layout_daily;
CREATE TABLE bench_layout_current (${columns}, ${created_at}) ENGINE = ReplacingMergeTree
  PARTITION BY toYYYYMM(created_at) ORDER BY (${sort_key}) SETTINGS allow_nullable_key = 1;
CREATE TABLE bench_layout_minmax (${columns}, ${created_at},
  INDEX idx_created_at_ms created_at_ms TYPE minmax GRANULARITY 1) ENGINE = ReplacingMergeTree
  PARTITION BY toYYYYMM(created_at) ORDER BY (${sort_key}) SETTINGS allow_nullable_key = 1;
CREATE TABLE bench_layout_daily (${columns}, ${created_at}) ENGINE = ReplacingMergeTree
  PARTITION BY toYYYYMMDD(created_at) ORDER BY (${sort_key}) SETTINGS allow_nullable_key = 1;
INSERT INTO bench_layout_current SELECT * EXCEPT created_at FROM analytics_domain_events WHERE merchant_id = '${MERCHANT_ID}';
INSERT INTO bench_layout_minmax SELECT * EXCEPT created_at FROM analytics_domain_events WHERE merchant_id = '${MERCHANT_ID}';
INSERT INTO bench_layout_daily SELECT * EXCEPT created_at FROM analytics_domain_events WHERE merchant_id = '${MERCHANT_ID}';
OPTIMIZE TABLE bench_layout_current FINAL;
OPTIMIZE TABLE bench_layout_minmax FINAL;
OPTIMIZE TABLE bench_layout_daily FINAL;
SQL

# The predicate `filters::payment_field_match` builds for a card_bin field.
param() {
  local path="assumeNotNull(details), $1, 'card_bin', 'value'"
  echo "if(JSONType(${path}) = 'Object', JSONExtractString(${path}, 'value'), trim(BOTH '\"' FROM JSONExtractRaw(${path})))"
}
match="hasAny(arrayMap(value -> lowerUTF8(value), [
  $(param "'request', 'static_routing_request', 'parameters'"),
  $(param "'request', 'parameters'"),
  JSONExtractString(assumeNotNull(details), 'request', 'paymentInfo', 'cardIsin'),
  JSONExtractString(assumeNotNull(details), 'request', 'dynamic_routing_request', 'paymentInfo', 'cardIsin')
]), [lowerUTF8('${BIN}')])"
flows="flow_type IN ('decide_gateway_decision', 'routing_hybrid_decision', 'routing_hybrid_error',
  'routing_evaluate_single', 'routing_evaluate_priority', 'routing_evaluate_volume_split',
  'routing_evaluate_advanced', 'routing_evaluate_preview', 'routing_evaluate_error')"

window_ms() {
  case "${1: -1}" in m) echo $((${1%?} * 60000)) ;; h) echo $((${1%?} * 3600000)) ;; d) echo $((${1%?} * 86400000)) ;; esac
}
printf '%-7s %-8s %8s %9s %11s %10s\n' window layout p50_ms rows_read read matches
for window in $WINDOWS; do
  start_ms=$((END_MS - $(window_ms "$window")))
  for layout in all current minmax daily; do
    table=$([ "$layout" = all ] && echo current || echo "$layout")
    events=$([ "$layout" = all ] && echo 1 || echo "$flows")
    sql="SELECT count() FROM (SELECT assumeNotNull(lookup_key) FROM bench_layout_${table}
      WHERE created_at_ms >= ${start_ms} AND created_at_ms <= ${END_MS}
        AND created_at >= fromUnixTimestamp64Milli(toInt64(${start_ms}))
        AND created_at <= fromUnixTimestamp64Milli(toInt64(${END_MS}))
        AND merchant_id = '${MERCHANT_ID}' AND notEmpty(ifNull(lookup_key, ''))
        AND ${events} AND ${match})"
    tag="layout-$(date +%s%N)"
    for _ in $(seq "$RUNS"); do
      ch -q "SYSTEM DROP MARK CACHE"; ch -q "SYSTEM DROP UNCOMPRESSED CACHE"
      matches=$(ch $SANDBOX --log_comment="$tag" -q "$sql")
    done
    ch -q "SYSTEM FLUSH LOGS"
    ch -q "SELECT '${window}', '${layout}', quantileExact(0.5)(query_duration_ms), quantileExact(0.5)(read_rows),
        formatReadableSize(quantileExact(0.5)(read_bytes)), '${matches}'
      FROM system.query_log WHERE log_comment = '${tag}' AND type = 'QueryFinish' FORMAT TSV" |
      awk -F'\t' '{ printf "%-7s %-8s %8s %9s %11s %10s\n", $1, $2, $3, $4, $5, $6 }'
  done
done

ch --multiquery -q "DROP TABLE bench_layout_current; DROP TABLE bench_layout_minmax; DROP TABLE bench_layout_daily;"
