#!/bin/sh
set -eu

# Add the settlement report's internal account dimension (Adyen CSV `Merchant Account`) to the
# cost model identity. This must rebuild the ReplacingMergeTree tables because ClickHouse cannot
# ALTER an existing sorting key; adding the column alone would still let FINAL collapse distinct
# report-account buckets.

CLICKHOUSE_DATABASE="${CLICKHOUSE_DATABASE:-default}"
CLICKHOUSE_USER="${CLICKHOUSE_USER:-default}"
CLICKHOUSE_PASSWORD="${CLICKHOUSE_PASSWORD:-}"

auth_args="--database=${CLICKHOUSE_DATABASE} --user=${CLICKHOUSE_USER}"
if [ -n "${CLICKHOUSE_PASSWORD}" ]; then
  auth_args="${auth_args} --password=${CLICKHOUSE_PASSWORD}"
fi

clickhouse-client ${auth_args} --multiquery <<SQL
ALTER TABLE cost_daily_stats ADD COLUMN IF NOT EXISTS report_account String DEFAULT '' AFTER account;
ALTER TABLE cost_fee_model ADD COLUMN IF NOT EXISTS report_account String DEFAULT '' AFTER account;

DROP TABLE IF EXISTS cost_daily_stats__report_account_new;
DROP TABLE IF EXISTS cost_daily_stats__report_account_backup;

CREATE TABLE cost_daily_stats__report_account_new (
    connector        LowCardinality(String),
    account          String,
    report_account   String DEFAULT '',
    merchant_id      String,
    txn_date         Date,
    ingestion_id     String DEFAULT '',
    card_network     LowCardinality(String),
    variant          String,
    funding          LowCardinality(String),
    issuer_country   LowCardinality(String),
    currency         LowCardinality(String),
    ic_category      String,
    interchange_bps  String DEFAULT '',
    channel          LowCardinality(String) DEFAULT '',
    band             LowCardinality(String) DEFAULT '',
    fit_bucket       Int32 DEFAULT 0,
    n                UInt64,
    sx               Float64,
    sy               Float64,
    sxx              Float64,
    sxy              Float64,
    syy              Float64,
    su               Float64,
    suu              Float64,
    suy              Float64,
    suuy             Float64,
    syyuu            Float64,
    sample_x         Array(Float64) DEFAULT [],
    sample_y         Array(Float64) DEFAULT [],
    ingested_at      DateTime DEFAULT now()
) ENGINE = ReplacingMergeTree(ingested_at)
PARTITION BY toYYYYMM(txn_date)
ORDER BY (connector, account, merchant_id, txn_date,
          report_account, card_network, variant, funding, issuer_country, currency, ic_category,
          interchange_bps, channel, band, fit_bucket)
TTL txn_date + INTERVAL 400 DAY;

INSERT INTO cost_daily_stats__report_account_new
SELECT
    connector, account, report_account, merchant_id, txn_date, ingestion_id,
    card_network, variant, funding, issuer_country, currency, ic_category, interchange_bps,
    channel, band, fit_bucket, n, sx, sy, sxx, sxy, syy, su, suu, suy, suuy, syyuu,
    sample_x, sample_y, ingested_at
FROM cost_daily_stats;

RENAME TABLE
    cost_daily_stats TO cost_daily_stats__report_account_backup,
    cost_daily_stats__report_account_new TO cost_daily_stats;

DROP TABLE cost_daily_stats__report_account_backup;

DROP TABLE IF EXISTS cost_fee_model__report_account_new;
DROP TABLE IF EXISTS cost_fee_model__report_account_backup;

CREATE TABLE cost_fee_model__report_account_new (
    report_date      Date,
    connector        LowCardinality(String),
    account          String,
    report_account   String DEFAULT '',
    merchant_id      String,
    card_network     LowCardinality(String),
    variant          String,
    funding          LowCardinality(String),
    issuer_country   LowCardinality(String),
    currency         LowCardinality(String),
    ic_category      String,
    interchange_bps  String DEFAULT '',
    segment_idx      UInt16 DEFAULT 0,
    amount_lo        Float64 DEFAULT 0,
    amount_hi        Float64 DEFAULT 0,
    pct_bps          Float64,
    fixed            Float64,
    n                UInt64,
    gross_sum        Float64 DEFAULT 0,
    bps_rmse         Float64,
    grade_bps        Float64 DEFAULT 0,
    pct_ci95_bps     Float64 DEFAULT 0,
    crossover_amount Float64 DEFAULT 0,
    prop_bps         Float64 DEFAULT 0,
    fix_abs          Float64 DEFAULT 0,
    fix_bps          Float64 DEFAULT 0,
    below_gross_frac Float64 DEFAULT 0,
    fan_frac         Float64 DEFAULT 0,
    fan_money_bps    Float64 DEFAULT 0,
    r2               Float64,
    verdict          Enum8('GOOD' = 1, 'NON_LINEAR' = 2, 'THIN' = 3, 'FAN' = 4),
    fitted_at        DateTime DEFAULT now()
) ENGINE = ReplacingMergeTree(fitted_at)
PARTITION BY toYYYYMM(report_date)
ORDER BY (connector, account, merchant_id, report_date,
          report_account, card_network, variant, funding, issuer_country, currency, ic_category,
          interchange_bps, segment_idx);

INSERT INTO cost_fee_model__report_account_new
SELECT
    report_date, connector, account, report_account, merchant_id,
    card_network, variant, funding, issuer_country, currency, ic_category, interchange_bps,
    segment_idx, amount_lo, amount_hi, pct_bps, fixed, n, gross_sum, bps_rmse,
    grade_bps, pct_ci95_bps, crossover_amount, prop_bps, fix_abs, fix_bps, below_gross_frac,
    fan_frac, fan_money_bps, r2, verdict, fitted_at
FROM cost_fee_model;

RENAME TABLE
    cost_fee_model TO cost_fee_model__report_account_backup,
    cost_fee_model__report_account_new TO cost_fee_model;

DROP TABLE cost_fee_model__report_account_backup;
SQL
