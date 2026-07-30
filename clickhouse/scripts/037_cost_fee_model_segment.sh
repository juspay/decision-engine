#!/bin/sh
set -eu

# In-house cost estimation — piecewise (L1) segments for capped / tiered clusters.
#
# Some clusters can't be priced by one line: where interchange has an absolute cap (e.g. UAE
# `1.00% cap AED 50`) or a tiered schedule, `fee ~ gross` is piecewise-linear and the whole-cluster
# OLS reads NON_LINEAR. `cost_ingestion::segment` recovers such a cluster into up to 5 straight
# pieces (each with its own {pct_bps, fixed} over an amount range), computed in Rust from the same
# `cost_daily_stats` band rollup the OLS uses. This table stores those pieces, one row per segment.
#
# Additive & reversible: a NEW table only — nothing here alters `cost_fee_model` or `cost_daily_stats`.
# Phase 1 is display-only (the merchant-facing API reads these to show per-segment rates); the decide
# path is NOT changed by this migration. Rolling back is `DROP TABLE cost_fee_model_segment`.

CLICKHOUSE_DATABASE="${CLICKHOUSE_DATABASE:-default}"
CLICKHOUSE_USER="${CLICKHOUSE_USER:-default}"
CLICKHOUSE_PASSWORD="${CLICKHOUSE_PASSWORD:-}"

auth_args="--database=${CLICKHOUSE_DATABASE} --user=${CLICKHOUSE_USER}"
if [ -n "${CLICKHOUSE_PASSWORD}" ]; then
  auth_args="${auth_args} --password=${CLICKHOUSE_PASSWORD}"
fi

clickhouse-client ${auth_args} --multiquery <<SQL
-- One row per recovered segment of a NON_LINEAR/tiered cluster, per snapshot. A GOOD (single-line)
-- cluster produces NO rows here — its whole-cluster `cost_fee_model` row already prices it. Keyed by
-- (connector, account, merchant_id, report_date, cluster…, seg_idx) so a refit REPLACES cleanly.
CREATE TABLE IF NOT EXISTS cost_fee_model_segment (
    report_date      Date,
    connector        LowCardinality(String),
    account          String,
    merchant_id      String,
    card_network     LowCardinality(String),
    variant          String,
    funding          LowCardinality(String),
    issuer_country   LowCardinality(String),
    currency         LowCardinality(String),
    ic_category      String,
    card_product     LowCardinality(String) DEFAULT '',  -- resolved card-product tier — fan-separating dimension (matches cost_fee_model)
    seg_idx          UInt8,                       -- 0-based, ascending by amount
    lo               Float64,                     -- inclusive amount lower bound of the segment
    hi               Float64,                     -- exclusive amount upper bound of the segment
    pct_bps          Nullable(Float64),           -- segment OLS slope × 10 000 (null = unfittable piece)
    fixed            Nullable(Float64),           -- segment OLS intercept (settlement-currency units)
    bps_rmse         Nullable(Float64),           -- segment per-txn fit error
    n                UInt64,                      -- segment sample size
    gross_sum        Float64,                     -- segment settled volume
    verdict          LowCardinality(String),      -- 'GOOD' | 'THIN' | 'NON_LINEAR'
    fitted_at        DateTime DEFAULT now()
) ENGINE = ReplacingMergeTree(fitted_at)
PARTITION BY toYYYYMM(report_date)
-- card_product sits LAST (after seg_idx) so an already-created segment table can gain it as a pure
-- sort-key APPEND — the only in-place MODIFY ORDER BY ClickHouse allows (see 038_cost_card_product.sh).
-- Position doesn't affect the ReplacingMergeTree dedup identity (the full column set).
ORDER BY (connector, account, merchant_id, report_date,
          card_network, variant, issuer_country, currency, ic_category, seg_idx, card_product);
SQL
