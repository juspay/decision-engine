#!/bin/sh
set -eu

# Global BIN → card-product observations for in-house cost estimation.
#
# Cost fit derives `funding` from the payment-method variant, but that is blank for co-badged
# schemes, leaving those cards unable to separate consumer-debit / consumer-credit / commercial.
# This table records, per issuer BIN, the resolved funding so rollup can fill the blank funding /
# stamp card_product from the highest-support observation.
#
# A BIN's product is a stable property of the card, identical across merchants and connectors, so
# the table is deliberately NOT keyed by merchant/connector: one global map maximises coverage.
# It stores no amounts or fees, only the product signal and how many txns back it, so it stays a
# per-BIN summary rather than a per-transaction store.

CLICKHOUSE_DATABASE="${CLICKHOUSE_DATABASE:-default}"
CLICKHOUSE_USER="${CLICKHOUSE_USER:-default}"
CLICKHOUSE_PASSWORD="${CLICKHOUSE_PASSWORD:-}"

auth_args="--database=${CLICKHOUSE_DATABASE} --user=${CLICKHOUSE_USER}"
if [ -n "${CLICKHOUSE_PASSWORD}" ]; then
  auth_args="${auth_args} --password=${CLICKHOUSE_PASSWORD}"
fi

clickhouse-client ${auth_args} --multiquery <<SQL
CREATE TABLE IF NOT EXISTS cost_bin_product (
    bin              String,                    -- issuer BIN (leading PAN digits; '' if PAN absent)
    card_network     LowCardinality(String),    -- 'visa', 'mc', … (as reported)
    issuer_country   LowCardinality(String),    -- 'FR', 'IT', …
    funding          LowCardinality(String),    -- resolved product: 'debit'/'credit'/'commercial', '' if unresolved
    -- SummingMergeTree accumulates support across every report & merchant that saw this BIN, so a
    -- BIN's product mapping is the union of all traffic (global coverage). Step B picks, per BIN,
    -- the funding with the greatest support_n as the resolved product signal.
    support_n        UInt64                      -- cumulative settled txns backing this observation
)
ENGINE = SummingMergeTree(support_n)
ORDER BY (bin, card_network, issuer_country, funding);
SQL
