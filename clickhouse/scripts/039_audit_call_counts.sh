#!/bin/sh
set -eu

# Migration: add `call_count_state` (distinct evaluation calls) to the payment-audit lookup
# summaries in an existing database — e.g. a deployment provisioned before the Decision Audit
# started separating calls from per-entry events.
#
# 025 is an init script: it builds the FINAL schema on a fresh volume (and now includes this
# column), but ClickHouse only runs /docker-entrypoint-initdb.d on first boot, so an existing
# database never gains the column from it. This script is the additive migration for that case.
#
# Why NOT "just re-run 025": 025 opens with DROP TABLE on the summary tables, so re-running it
# against a live database would discard every accumulated summary — the Matches list would go
# empty and only refill as new traffic arrives. Nothing here drops a table that holds data.
#
# What it does, and why each step is safe:
#   1. ALTER TABLE ... ADD COLUMN IF NOT EXISTS — metadata-only, no rewrite. The column is NOT in
#      the sorting key (unlike 038's card_product), so no MODIFY ORDER BY and no re-sort. Rows
#      written before the migration hold an empty uniq state, which finalizes to 0; the reader
#      treats 0 as "unknown" and reports the event count for those rows instead. Summaries
#      accumulate per lookup_key, so a payment seen after the migration reports real call counts.
#   2. DROP + CREATE the materialized view — a MV's SELECT cannot be ALTERed. Dropping a MV
#      removes only the view; its TO-table (the summaries above) keeps every row. The gap between
#      drop and create is one statement in a single --multiquery batch; events produced inside it
#      stay in the raw events table (`analytics_domain_events`, the audit's source of truth for
#      traces), they are just not folded into the summary aggregate.
#
# Idempotent: ADD COLUMN is guarded by IF NOT EXISTS and the MV is recreated unconditionally, so
# re-running is a no-op on an already-migrated database and harmless on a fresh one.

CLICKHOUSE_DATABASE="${CLICKHOUSE_DATABASE:-default}"
CLICKHOUSE_USER="${CLICKHOUSE_USER:-default}"
CLICKHOUSE_PASSWORD="${CLICKHOUSE_PASSWORD:-}"

auth_args="--database=${CLICKHOUSE_DATABASE} --user=${CLICKHOUSE_USER}"
if [ -n "${CLICKHOUSE_PASSWORD}" ]; then
  auth_args="${auth_args} --password=${CLICKHOUSE_PASSWORD}"
fi

# Skip when there is nothing to do: either the table does not exist yet (a fresh volume runs
# initdb.d in lexical order, so 025 creates it — with call_count_state already in it — after
# this script would run), or the column is already there (a previous run, or that same fresh
# install on a later re-run). Checking the COLUMN, not just the table, is what makes the
# already-migrated case a no-op instead of a needless view rebuild.
already_migrated=$(clickhouse-client ${auth_args} -q \
  "SELECT count() FROM system.columns \
   WHERE database = currentDatabase() \
     AND table = 'analytics_payment_audit_lookup_summaries' \
     AND name = 'call_count_state'")

table_exists=$(clickhouse-client ${auth_args} -q \
  "SELECT count() FROM system.tables \
   WHERE database = currentDatabase() AND name = 'analytics_payment_audit_lookup_summaries'")

if [ "${table_exists}" = "0" ]; then
  echo "  analytics_payment_audit_lookup_summaries absent — 025 creates it with call_count_state; nothing to migrate."
  exit 0
fi

if [ "${already_migrated}" != "0" ]; then
  echo "  analytics_payment_audit_lookup_summaries.call_count_state already present; nothing to migrate."
  exit 0
fi

clickhouse-client ${auth_args} --multiquery <<'SQL'
ALTER TABLE analytics_payment_audit_lookup_summaries
    ADD COLUMN IF NOT EXISTS call_count_state AggregateFunction(uniq, Nullable(String))
    AFTER event_count_state;

DROP TABLE IF EXISTS analytics_payment_audit_lookup_summaries_mv;

CREATE MATERIALIZED VIEW analytics_payment_audit_lookup_summaries_mv
TO analytics_payment_audit_lookup_summaries AS
SELECT
    merchant_id,
    effective_lookup_key AS lookup_key,
    summary_kind,
    minState(created_at_ms) AS first_seen_ms_state,
    maxState(created_at_ms) AS last_seen_ms_state,
    sumState(toUInt64(1)) AS event_count_state,
    uniqState(request_id) AS call_count_state,
    argMaxState(payment_id, created_at_ms) AS payment_id_state,
    argMaxState(request_id, created_at_ms) AS request_id_state,
    argMaxState(merchant_id, created_at_ms) AS merchant_id_state,
    argMaxState(status, created_at_ms) AS latest_status_state,
    argMaxState(gateway, created_at_ms) AS latest_gateway_state,
    argMaxState(event_stage, created_at_ms) AS latest_stage_state,
    groupUniqArrayState(ifNull(gateway, '')) AS gateways_state,
    groupUniqArrayState(ifNull(route, '')) AS routes_state,
    groupUniqArrayState(ifNull(status, '')) AS statuses_state,
    groupUniqArrayState(flow_type) AS flow_types_state,
    groupUniqArrayState(ifNull(error_code, '')) AS error_codes_state
FROM (
    SELECT
        merchant_id,
        lookup_key AS effective_lookup_key,
        summary_kind,
        created_at_ms,
        payment_id,
        request_id,
        status,
        gateway,
        event_stage,
        route,
        flow_type,
        error_code
    FROM analytics_payment_audit_lookup_summaries_queue
    WHERE merchant_id IS NOT NULL
      AND merchant_id != ''
      AND lookup_key IS NOT NULL
      AND lookup_key != ''
) AS source
WHERE summary_kind != ''
GROUP BY merchant_id, effective_lookup_key, summary_kind;
SQL

# The DROP and CREATE above are not atomic. If the CREATE failed, `set -eu` aborts — but the
# view is already gone and the summaries table would silently stop receiving rows, which shows
# up only as a Decision Audit list that quietly stops updating. Assert it is back so a failed
# migration fails the deploy instead.
mv_exists=$(clickhouse-client ${auth_args} -q \
  "SELECT count() FROM system.tables \
   WHERE database = currentDatabase() AND name = 'analytics_payment_audit_lookup_summaries_mv'")

if [ "${mv_exists}" = "0" ]; then
  echo "  ERROR: analytics_payment_audit_lookup_summaries_mv is missing after the rebuild." >&2
  echo "  The summaries pipeline is stopped. Re-run this script; the Kafka consumer group has" >&2
  echo "  not advanced, so no events are lost once the view is back." >&2
  exit 1
fi

echo "  analytics_payment_audit_lookup_summaries: call_count_state added; summaries MV rebuilt (rows preserved)."
