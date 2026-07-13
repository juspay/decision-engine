#!/bin/sh
set -eu

# In-house cost estimation — settlement ingestion + fitted cost models.
#
# Unlike the analytics scripts (015/025) this pipeline does NOT use Kafka: settlement
# reports are a once-daily-per-merchant batch, so the ingest worker aggregates each report
# in-flight and bulk-inserts per-day sufficient statistics directly (see
# scratch/inhouse-cost-architecture.md §7). Hence plain MergeTree tables, no *_queue / *_mv.
# Everything is connector-generic: a connector is a value in the `connector` column, never a
# separate table. Individual transactions are never stored — only per-day cluster summaries.

CLICKHOUSE_DATABASE="${CLICKHOUSE_DATABASE:-default}"
CLICKHOUSE_USER="${CLICKHOUSE_USER:-default}"
CLICKHOUSE_PASSWORD="${CLICKHOUSE_PASSWORD:-}"

auth_args="--database=${CLICKHOUSE_DATABASE} --user=${CLICKHOUSE_USER}"
if [ -n "${CLICKHOUSE_PASSWORD}" ]; then
  auth_args="${auth_args} --password=${CLICKHOUSE_PASSWORD}"
fi

clickhouse-client ${auth_args} --multiquery <<SQL
-- ─────────────────────────────────────────────────────────────────────────────
-- Daily per-cluster sufficient statistics — the ONLY persistent settlement store.
-- We do NOT keep individual transactions. As each report streams in it is aggregated
-- (in the ingest worker) into one row per (cluster × transaction-day × amount-band ×
-- channel), holding the additive sums an OLS fit needs (n, Σx, Σy, Σx², Σxy, Σy² and the
-- reciprocal terms for the bps-RMSE / NON_LINEAR check). Because these sums are additive
-- across days, the fit reconstructs the exact same line it would get from raw rows, for any
-- window — and can re-slice at a price-change date. See
-- scratch/settlement-table-removal-worked-example.md for the correctness walkthrough.
--
-- band + channel are carried so the §9 interchange-category predictor (which keys on the
-- amount band and pos/ecom channel) can be served from the same rollup; the fit sums them
-- away. The €5 micro-amount floor (WHERE gross >= 5) is applied at aggregation time, so it
-- cannot be recovered later — no consumer wants sub-floor txns.
-- ─────────────────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS cost_daily_stats (
    connector        LowCardinality(String),   -- 'adyen', 'stripe', … (never a code branch)
    account          String,                    -- connector-side account (e.g. Adyen merchantAccountCode)
    merchant_id      String,                    -- our merchant that owns the account
    txn_date         Date,                      -- the TRANSACTION (booking) day this bucket aggregates
    ingestion_id     String DEFAULT '',         -- the cost_ingestion row (UUIDv7) that last wrote this bucket (delete-by-ingestion)
    card_network     LowCardinality(String),    -- 'visa', 'mc', …          ┐
    variant          String,                    -- 'visastandarddebit', …    │ cluster key
    funding          LowCardinality(String),    -- 'debit' | 'credit' | ''   │ (fit groups on this)
    issuer_country   LowCardinality(String),    -- 'FR', 'IT', …             │
    currency         LowCardinality(String),    -- 'EUR', 'AUD', …           │
    ic_category      String,                     -- interchange category (''=flat-fee) ┘
    channel          LowCardinality(String) DEFAULT '',  -- 'pos' | 'ecom' — predictor feature (§9), summed away by the fit
    band             LowCardinality(String) DEFAULT '',  -- amount band ('lo'..'hi') — predictor feature, summed away by the fit
    -- Sufficient statistics over the txns in this bucket (gross >= 5 only). All additive.
    n                UInt64,                     -- count
    sx               Float64,                    -- Σ gross            (regression x)
    sy               Float64,                    -- Σ total_fee        (regression y)
    sxx              Float64,                    -- Σ gross²
    sxy              Float64,                    -- Σ gross·total_fee
    syy              Float64,                    -- Σ total_fee²
    su               Float64,                    -- Σ 1/gross          ┐ reciprocal terms:
    suu              Float64,                    -- Σ 1/gross²         │ the bps-RMSE sum-of-squares
    suy              Float64,                    -- Σ total_fee/gross  │ and NON_LINEAR check are
    suuy             Float64,                    -- Σ total_fee/gross² │ built from these
    syyuu            Float64,                    -- Σ total_fee²/gross²┘
    ingested_at      DateTime DEFAULT now()
)
-- Identity is the (cluster, DAY, band, channel) bucket, NOT the transaction. A day re-delivered by
-- a later, authoritative report (overlapping monthly+daily, a re-upload, webhook+manual) collapses
-- to one bucket — the latest `ingested_at` wins. This is the "latest report wins per day" de-dup:
-- correct as long as a report is complete for each day it covers (settlement reports are day- or
-- month-complete batches). The fit windows on `txn_date`, independent of when the rows arrived.
ENGINE = ReplacingMergeTree(ingested_at)
PARTITION BY toYYYYMM(txn_date)
ORDER BY (connector, account, merchant_id, txn_date,
          card_network, variant, funding, issuer_country, currency, ic_category, channel, band)
-- Generous retention: the fit windows on the *latest* transaction date in the data (not the wall
-- clock), so this only needs to outlast a backfill of older reports, not track "now".
TTL txn_date + INTERVAL 400 DAY;

-- ─────────────────────────────────────────────────────────────────────────────
-- Fitted cost models: one row per cluster per snapshot (the OLS output of §3).
-- Keyed by (connector, merchant_id) so multi-tenant, multi-connector ingestion each
-- get their own snapshot. The hot path reads only the latest report_date + GOOD rows.
-- ─────────────────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS cost_fee_model (
    report_date      Date,
    connector        LowCardinality(String),
    account          String,                    -- connector-side account this snapshot was fit from
    merchant_id      String,
    card_network     LowCardinality(String),
    variant          String,
    funding          LowCardinality(String),
    issuer_country   LowCardinality(String),
    currency         LowCardinality(String),
    ic_category      String,                     -- '' = first-class key (iDEAL / Klarna / CB)
    pct_bps          Float64,                     -- OLS slope × 10 000
    fixed            Float64,                     -- OLS intercept (settlement-currency units)
    n                UInt64,                      -- cluster sample size
    gross_sum        Float64 DEFAULT 0,           -- settled volume in this cluster (money-weighted coverage)
    bps_rmse         Float64,                     -- typical per-txn cost error (the gate metric)
    r2               Float64,                     -- reference only — NOT used to gate
    verdict          Enum8('GOOD' = 1, 'NON_LINEAR' = 2, 'THIN' = 3),
    fitted_at        DateTime DEFAULT now()
) ENGINE = ReplacingMergeTree(fitted_at)
PARTITION BY toYYYYMM(report_date)
ORDER BY (connector, account, merchant_id, report_date,
          card_network, variant, issuer_country, currency, ic_category);
SQL
