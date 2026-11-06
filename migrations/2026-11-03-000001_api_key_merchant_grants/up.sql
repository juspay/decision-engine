-- Additive foundation only: authentication continues to use the credential's merchant_id.
-- Compose replays migrations, so never overwrite a grant on a subsequent run.
SET @has_key_authz_version := (
    SELECT COUNT(*) FROM information_schema.columns
    WHERE table_schema = DATABASE() AND table_name = 'merchant_api_keys'
      AND column_name = 'authz_version'
);
SET @add_key_authz_version := IF(
    @has_key_authz_version = 0,
    'ALTER TABLE merchant_api_keys ADD COLUMN authz_version BIGINT NOT NULL DEFAULT 1 CHECK (authz_version > 0)',
    'DO 0'
);
PREPARE add_key_authz_version FROM @add_key_authz_version;
EXECUTE add_key_authz_version;
DEALLOCATE PREPARE add_key_authz_version;

-- Match the parent's character set and collation even if the database default has changed.
SELECT character_set_name, collation_name INTO @key_charset, @key_collation
FROM information_schema.columns
WHERE table_schema = DATABASE() AND table_name = 'merchant_api_keys' AND column_name = 'key_id';
SET @create_key_merchants := CONCAT(
    'CREATE TABLE IF NOT EXISTS api_key_merchants (
        key_id VARCHAR(64) CHARACTER SET ', @key_charset, ' COLLATE ', @key_collation, ' NOT NULL,
        merchant_id VARCHAR(255) NOT NULL,
        status VARCHAR(16) NOT NULL,
        permissions TEXT NULL,
        authz_version BIGINT NOT NULL DEFAULT 1,
        created_at DATETIME(6) NOT NULL,
        PRIMARY KEY (key_id, merchant_id),
        CONSTRAINT fk_api_key_merchants_key FOREIGN KEY (key_id) REFERENCES merchant_api_keys(key_id) ON DELETE CASCADE,
        CONSTRAINT ck_api_key_merchants_status CHECK (status IN (''active'', ''revoked'')),
        CONSTRAINT ck_api_key_merchants_version CHECK (authz_version > 0)
    ) DEFAULT CHARACTER SET ', @key_charset, ' COLLATE ', @key_collation
);
PREPARE create_key_merchants FROM @create_key_merchants;
EXECUTE create_key_merchants;
DEALLOCATE PREPARE create_key_merchants;

SET @has_grant_merchant_index := (
    SELECT COUNT(*) FROM information_schema.statistics
    WHERE table_schema = DATABASE() AND table_name = 'api_key_merchants'
      AND index_name = 'idx_api_key_merchants_merchant_id'
);
SET @add_grant_merchant_index := IF(
    @has_grant_merchant_index = 0,
    'CREATE INDEX idx_api_key_merchants_merchant_id ON api_key_merchants (merchant_id)',
    'DO 0'
);
PREPARE add_grant_merchant_index FROM @add_grant_merchant_index;
EXECUTE add_grant_merchant_index;
DEALLOCATE PREPARE add_grant_merchant_index;

-- NULL permissions preserve unrestricted single-merchant service credentials only.
-- Orphan credentials stay untouched, but their grants cannot become active on a later deploy.
INSERT INTO api_key_merchants (key_id, merchant_id, status, permissions, authz_version, created_at)
SELECT k.key_id, k.merchant_id,
    CASE WHEN k.is_active <> 0 AND EXISTS (
        SELECT 1 FROM merchant_account m WHERE m.merchant_id = k.merchant_id
    ) THEN 'active' ELSE 'revoked' END,
    NULL, 1, k.created_at
FROM merchant_api_keys k
WHERE NOT EXISTS (SELECT 1 FROM api_key_merchants g WHERE g.key_id = k.key_id AND g.merchant_id = k.merchant_id);
