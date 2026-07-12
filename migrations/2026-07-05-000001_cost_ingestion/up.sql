-- Unified settlement-report ingestion table: work queue + live progress + history (MySQL parity
-- of the Postgres migration). See scratch/inhouse-cost-architecture.md §7.
CREATE TABLE cost_ingestion (
    id               BIGINT       NOT NULL AUTO_INCREMENT PRIMARY KEY,
    merchant_id      VARCHAR(255) NOT NULL,
    connector        VARCHAR(64)  NOT NULL,
    account          VARCHAR(255) NOT NULL,
    source           VARCHAR(16)  NOT NULL,
    notification_id  VARCHAR(255),
    report_ref       TEXT         NOT NULL,
    status           VARCHAR(32)  NOT NULL DEFAULT 'pending',
    attempts         INT          NOT NULL DEFAULT 0,
    last_error       TEXT,

    staged_rows      BIGINT       NOT NULL DEFAULT 0,

    report_date      DATE,
    period_start     DATE,
    period_end       DATE,
    currency_count   INT          NOT NULL DEFAULT 0,
    currencies       TEXT,
    country_count    INT          NOT NULL DEFAULT 0,
    countries        TEXT,
    total_gross      DOUBLE       NOT NULL DEFAULT 0,
    total_clusters   BIGINT       NOT NULL DEFAULT 0,
    good_clusters    BIGINT       NOT NULL DEFAULT 0,

    created_at       TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at       TIMESTAMP    NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE KEY uq_cost_ingestion_notif (connector, notification_id),
    KEY idx_cost_ingestion_claim (status, created_at),
    KEY idx_cost_ingestion_history (merchant_id, created_at)
);
