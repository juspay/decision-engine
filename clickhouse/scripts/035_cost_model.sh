#!/bin/sh
set -eu

# In-house cost estimation — settlement ingestion + fitted cost models.
#
# Unlike the analytics scripts (015/025) this pipeline does NOT use Kafka: settlement
# reports are a once-daily-per-merchant batch, so the ingest worker bulk-INSERTs cleaned
# rows directly (see scratch/inhouse-cost-architecture.md §7). Hence plain MergeTree
# tables, no *_queue / *_mv. Everything is connector-generic: a connector is a value in
# the `connector` column, never a separate table.

CLICKHOUSE_DATABASE="${CLICKHOUSE_DATABASE:-default}"
CLICKHOUSE_USER="${CLICKHOUSE_USER:-default}"
CLICKHOUSE_PASSWORD="${CLICKHOUSE_PASSWORD:-}"

auth_args="--database=${CLICKHOUSE_DATABASE} --user=${CLICKHOUSE_USER}"
if [ -n "${CLICKHOUSE_PASSWORD}" ]; then
  auth_args="${auth_args} --password=${CLICKHOUSE_PASSWORD}"
fi

clickhouse-client ${auth_args} --multiquery <<SQL
-- ─────────────────────────────────────────────────────────────────────────────
-- Staging: one canonical normalized settled-transaction row per fee-bearing txn.
-- Every connector native report is mapped onto THIS schema (the SettledFeeRow of §7.1).
-- Once here, the fit and serving are 100% connector-agnostic. Short-lived: retained
-- only long enough to fit and reconcile, then expired by TTL.
-- ─────────────────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS settlement_txn_fees (
    connector        LowCardinality(String),   -- 'adyen', 'stripe', … (never a code branch)
    account          String,                    -- connector-side account (e.g. Adyen merchantAccountCode)
    merchant_id      String,                    -- our merchant that owns the account
    txn_date         Date,                      -- the TRANSACTION (booking) date — the fit windows on this
    ingestion_id     Int64 DEFAULT 0,           -- the cost_ingestion row that staged this (for delete-by-ingestion)
    txn_ref          String,                    -- connector's unique txn id (e.g. Psp Reference)
    card_network     LowCardinality(String),    -- 'visa', 'mc', …
    variant          String,                    -- 'visastandarddebit', … (carries tier + funding)
    funding          LowCardinality(String),    -- 'debit' | 'credit' | '' (derived from variant)
    issuer_country   LowCardinality(String),    -- 'FR', 'IT', …
    currency         LowCardinality(String),    -- settlement currency: 'EUR', 'AUD', …
    ic_category      String,                     -- interchange category ('' = flat-fee methods)
    channel          LowCardinality(String) DEFAULT '',  -- 'pos' | 'ecom' — predictor feature (§9)
    gross            Float64,                    -- payable + total_fee  (regression x)
    total_fee        Float64,                    -- sum of fee components (regression y)
    interchange      Float64 DEFAULT 0,          -- fee components kept split so we can later
    scheme_fee       Float64 DEFAULT 0,          -- separate the shared interchange model from
    markup           Float64 DEFAULT 0,          -- the per-connector markup overlay (§3.3)
    commission       Float64 DEFAULT 0,
    ingested_at      DateTime DEFAULT now()
)
-- Identity is the TRANSACTION, not the file: sorting/deduping by (connector, account, txn_ref)
-- means the same txn delivered twice (overlapping monthly+daily reports, a re-upload, webhook +
-- manual) collapses to one row — the latest `ingested_at` wins. So ingestion is cadence- and
-- source-agnostic and overlap-safe. The fit windows on `txn_date` (see §10), independent of when
-- or how the rows arrived.
ENGINE = ReplacingMergeTree(ingested_at)
PARTITION BY toYYYYMM(txn_date)
ORDER BY (connector, account, txn_ref)
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

-- ─────────────────────────────────────────────────────────────────────────────
-- Per-connector scheme + markup delta layered on top of the (shared) interchange
-- model, so EV has a rankable cost for connectors whose own report we don't yet fit.
-- ─────────────────────────────────────────────────────────────────────────────
CREATE TABLE IF NOT EXISTS connector_markup_overlay (
    report_date      Date,
    connector        LowCardinality(String),
    account          String,
    merchant_id      String,
    card_network     LowCardinality(String),
    funding          LowCardinality(String),
    currency         LowCardinality(String),
    scheme_bps       Float64,
    markup_bps       Float64,
    fixed            Float64,
    fitted_at        DateTime DEFAULT now()
) ENGINE = ReplacingMergeTree(fitted_at)
PARTITION BY toYYYYMM(report_date)
ORDER BY (connector, account, merchant_id, report_date, card_network, funding, currency);
SQL
