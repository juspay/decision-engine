-- Unified settlement-report ingestion table: work queue + live progress + history, in one place.
--
-- Every ingestion — webhook-delivered or manually uploaded — is one row here. The webhook route
-- inserts a `pending` job the ingest worker drains; a manual upload inserts a `processing` row and
-- runs its own background task. Both stream rows into ClickHouse, tick `staged_rows` for progress,
-- and on completion record the report's shape (period, currencies, countries, volume, fit outcome)
-- so the dashboard can show ingestion history. Connector-generic: `connector` is a value, never a
-- table. See scratch/inhouse-cost-architecture.md §7.
CREATE TABLE cost_ingestion (
    id               BIGSERIAL PRIMARY KEY,
    merchant_id      VARCHAR(255) NOT NULL,          -- our merchant that owns the account
    connector        VARCHAR(64)  NOT NULL,          -- 'adyen', 'stripe', …
    account          VARCHAR(255) NOT NULL,          -- connector-side account (Adyen merchantAccountCode)
    source           VARCHAR(16)  NOT NULL,          -- 'manual' | 'webhook'
    -- Connector's unique notification/event id (webhook only; NULL for manual uploads). The
    -- UNIQUE constraint below makes a re-delivered webhook a no-op (replay-idempotency). NULLs are
    -- distinct in both Postgres and MySQL, so manual rows never collide.
    notification_id  VARCHAR(255),
    report_ref       TEXT         NOT NULL,          -- download handle/URL (webhook) or temp file path (manual)
    status           VARCHAR(32)  NOT NULL DEFAULT 'pending',  -- pending|processing|completed|failed
    attempts         INTEGER      NOT NULL DEFAULT 0,
    last_error       TEXT,

    -- Live progress: rows staged into ClickHouse so far (polled by the dashboard).
    staged_rows      BIGINT       NOT NULL DEFAULT 0,

    -- Outcome / history: the shape of the ingested report, filled on completion.
    report_date      DATE,                           -- the fit snapshot date
    period_start     DATE,                           -- earliest transaction (Booking) date in the report
    period_end       DATE,                           -- latest transaction date in the report
    currency_count   INTEGER      NOT NULL DEFAULT 0,
    currencies       TEXT,                           -- comma-joined distinct settlement currencies
    country_count    INTEGER      NOT NULL DEFAULT 0,
    countries        TEXT,                           -- comma-joined distinct issuer countries
    total_gross      DOUBLE PRECISION NOT NULL DEFAULT 0,  -- settled volume ingested
    total_clusters   BIGINT       NOT NULL DEFAULT 0,
    good_clusters    BIGINT       NOT NULL DEFAULT 0,

    created_at       TIMESTAMP    NOT NULL DEFAULT NOW(),
    updated_at       TIMESTAMP    NOT NULL DEFAULT NOW(),
    UNIQUE (connector, notification_id)
);

-- Worker claims the oldest unfinished jobs; partial index keeps that scan to just the backlog.
CREATE INDEX idx_cost_ingestion_claim
    ON cost_ingestion (status, created_at)
    WHERE status IN ('pending', 'processing');

-- History listing for the dashboard: a merchant's ingestions, newest first.
CREATE INDEX idx_cost_ingestion_history
    ON cost_ingestion (merchant_id, created_at DESC);
