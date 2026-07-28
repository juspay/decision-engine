#!/bin/sh
set -eu

# Migration: add the fan-separating `card_product` dimension to cost tables that were already created
# (by 035/036/037) in an existing database — e.g. a deployment provisioned before card_product existed.
#
# 035/036/037 are `CREATE TABLE IF NOT EXISTS` init scripts: they build the FINAL schema on a fresh
# volume, but are a no-op against a table that already exists, so a pre-existing database never gains
# the new column from them. This script is the additive migration for that case.
#
# Why a pure ALTER (no rebuild, no data loss): because `funding` was KEPT on cost_bin_product,
# card_product is a sort-key APPEND on every table here, and an append is the one MODIFY ORDER BY
# ClickHouse applies as metadata only. Existing rows read card_product = '' (the default) and price as
# blended until re-ingested/refit — correct, since those rows carry no BIN/rate to recover.
#
# Two ClickHouse constraints shape the mechanics:
#   1. A column can only be added to the sorting key in the SAME `ALTER` that adds the column — a
#      pre-existing column can't be pulled into the key. So add + `MODIFY ORDER BY` are ONE statement.
#   2. That makes a naive re-run fail (the column is now "existing"). So this script is guarded: it
#      checks whether card_product is already in each table's sorting key and skips it if so, making
#      the migration idempotent (safe to re-run, and a no-op after a fresh install). If the column
#      exists but ISN'T in the key (a half-applied prior run), it is dropped first — safe, because
#      such a column only ever holds the default '' (real values require it to be in the key).
#
# ASSUMES the pre-card_product (main) schema; a scratch DB that ran the abandoned `ic_bps` branch has
# `ic_bps` mid-key instead (not an append) — reset that volume rather than migrating it.

CLICKHOUSE_DATABASE="${CLICKHOUSE_DATABASE:-default}"
CLICKHOUSE_USER="${CLICKHOUSE_USER:-default}"
CLICKHOUSE_PASSWORD="${CLICKHOUSE_PASSWORD:-}"

auth_args="--database=${CLICKHOUSE_DATABASE} --user=${CLICKHOUSE_USER}"
if [ -n "${CLICKHOUSE_PASSWORD}" ]; then
  auth_args="${auth_args} --password=${CLICKHOUSE_PASSWORD}"
fi

# Append card_product to one table's sorting key, idempotently.
#   $1 = table, $2 = column to add the new column AFTER, $3 = the full new ORDER BY tuple.
migrate_table() {
  table="$1"
  after="$2"
  order_by="$3"

  in_key=$(clickhouse-client ${auth_args} -q \
    "SELECT sorting_key LIKE '%card_product%' FROM system.tables \
     WHERE database = currentDatabase() AND name = '${table}'")

  if [ "${in_key}" = "1" ]; then
    echo "  ${table}: card_product already in sorting key — skipping."
    return 0
  fi

  # Clear any half-applied bare column (holds only the default ''), then add it AND extend the sorting
  # key in a single ALTER so ClickHouse accepts the newly-added column in the key. The added column
  # carries NO DEFAULT: ClickHouse forbids a defaulted column in the sorting key on ALTER (the fresh
  # CREATE keeps DEFAULT '', which is allowed at create time). Old rows read the type default '' —
  # identical behaviour — and every insert supplies card_product explicitly.
  clickhouse-client ${auth_args} --multiquery <<SQL
ALTER TABLE ${table} DROP COLUMN IF EXISTS card_product;
ALTER TABLE ${table}
    ADD COLUMN card_product LowCardinality(String) AFTER ${after},
    MODIFY ORDER BY (${order_by});
SQL
  echo "  ${table}: card_product added and appended to sorting key."
}

migrate_table cost_daily_stats ic_category \
  "connector, account, merchant_id, txn_date, card_network, variant, funding, issuer_country, currency, ic_category, channel, band, card_product"

migrate_table cost_fee_model ic_category \
  "connector, account, merchant_id, report_date, card_network, variant, issuer_country, currency, ic_category, card_product"

migrate_table cost_fee_model_segment ic_category \
  "connector, account, merchant_id, report_date, card_network, variant, issuer_country, currency, ic_category, seg_idx, card_product"

migrate_table cost_bin_product funding \
  "bin, card_network, issuer_country, funding, card_product"
