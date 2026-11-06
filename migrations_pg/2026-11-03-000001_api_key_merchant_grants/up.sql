-- Additive foundation only: authentication continues to use the credential's merchant_id.
ALTER TABLE merchant_api_keys ADD COLUMN IF NOT EXISTS authz_version BIGINT NOT NULL DEFAULT 1 CHECK (authz_version > 0);

CREATE TABLE IF NOT EXISTS api_key_merchants (
    key_id VARCHAR(64) NOT NULL REFERENCES merchant_api_keys(key_id) ON DELETE CASCADE,
    merchant_id VARCHAR(255) NOT NULL,
    status VARCHAR(16) NOT NULL CHECK (status IN ('active', 'revoked')),
    permissions TEXT NULL,
    authz_version BIGINT NOT NULL DEFAULT 1 CHECK (authz_version > 0),
    created_at TIMESTAMP NOT NULL,
    PRIMARY KEY (key_id, merchant_id)
);

CREATE INDEX IF NOT EXISTS idx_api_key_merchants_merchant_id ON api_key_merchants (merchant_id);

-- NULL permissions preserve unrestricted single-merchant service credentials only.
-- Keep credentials unchanged and backfill orphan owners as revoked grants, never active ones.
INSERT INTO api_key_merchants (key_id, merchant_id, status, permissions, authz_version, created_at)
SELECT k.key_id, k.merchant_id,
    CASE WHEN k.is_active AND EXISTS (
        SELECT 1 FROM merchant_account m WHERE m.merchant_id = k.merchant_id
    ) THEN 'active' ELSE 'revoked' END,
    NULL, 1, k.created_at
FROM merchant_api_keys k
ON CONFLICT (key_id, merchant_id) DO NOTHING;
