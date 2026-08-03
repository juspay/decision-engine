#!/bin/sh
set -eu

# Global BIN → card-product observations for in-house cost estimation.
#
# Two same-`ic_category` cards can settle at different interchange rates (a "fan" — e.g. MC
# commercial "Intra EEA Enhanced Electronic" at 135 vs 180 bps). The rate is the only signal that
# separates them, but the rate is a SETTLEMENT-time fact — unknown at decide time. The BIN, however,
# is present at BOTH ingest (report PAN) and decide (card_isin), so this table learns, per issuer
# BIN, the DOMINANT interchange rate it settles at (`card_product`). Rollup stamps each row's
# `card_product` from this map at ingest, and serving resolves the same value from the card's BIN at
# decide — so the fan's two rates land in separate clusters that are reproducible on both sides.
#
# A BIN's product is a stable property of the card, identical across merchants and connectors, so
# the table is deliberately NOT keyed by merchant/connector: one global map maximises coverage.
# It also keeps the resolved `funding` (debit/credit/commercial) per BIN — the co-badge resolver
# (Open Risk #4) that fills funding for schemes whose variant leaves it blank. Both signals ride the
# same per-BIN row; each reader marginalises the one it needs.
#
# It stores no amounts or fees, only the (funding, rate) tiers and how many txns back them, so it
# stays a per-BIN summary rather than a per-transaction store. `bin` is canonicalised to its leading
# 6 digits (the classic issuer BIN) so the ingest PAN and the decide-time card_isin resolve to the
# same key.

CLICKHOUSE_DATABASE="${CLICKHOUSE_DATABASE:-default}"
CLICKHOUSE_USER="${CLICKHOUSE_USER:-default}"
CLICKHOUSE_PASSWORD="${CLICKHOUSE_PASSWORD:-}"

auth_args="--database=${CLICKHOUSE_DATABASE} --user=${CLICKHOUSE_USER}"
if [ -n "${CLICKHOUSE_PASSWORD}" ]; then
  auth_args="${auth_args} --password=${CLICKHOUSE_PASSWORD}"
fi

clickhouse-client ${auth_args} --multiquery <<SQL
CREATE TABLE IF NOT EXISTS cost_bin_product (
    bin              String,                    -- issuer BIN, canonicalised to leading 6 digits (matches the decide-time card_isin)
    card_network     LowCardinality(String),    -- 'visa', 'mc', … (as reported)
    issuer_country   LowCardinality(String),    -- 'FR', 'IT', …
    funding          LowCardinality(String),    -- resolved funding: 'debit'/'credit'/'commercial', '' if unresolved (co-badge resolver, Open Risk #4)
    card_product     LowCardinality(String),    -- interchange rate this BIN settled at, integer-bps string ('135','180', …); '' when the row carried no rate
    -- SummingMergeTree accumulates support across every report & merchant that saw this BIN, so a
    -- BIN's (funding, rate) distribution is the union of all traffic (global coverage). Two readers
    -- marginalise it independently: the fan resolver picks, per BIN, the DOMINANT non-empty
    -- card_product (the value rollup stamps at ingest and serving resolves at decide); the co-badge
    -- resolver picks the dominant funding. Keeping both columns is why card_product could be APPENDED
    -- to the sort key — the one schema change ClickHouse can ALTER on an existing table in place.
    support_n        UInt64                      -- cumulative settled txns backing this observation
)
ENGINE = SummingMergeTree(support_n)
ORDER BY (bin, card_network, issuer_country, funding, card_product);
SQL
