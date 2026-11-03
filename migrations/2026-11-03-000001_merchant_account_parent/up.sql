-- A profile is a merchant_account row whose parent_merchant_id names the merchant it belongs to. A
-- merchant has none. Existing rows stay NULL, so every one of them remains a merchant.
--
-- The compose migrator replays every up.sql on each start, and MySQL has no ADD COLUMN IF NOT
-- EXISTS, so the column and its index are added only when they are not there yet.
SET @has_parent := (
    SELECT COUNT(*) FROM information_schema.columns
    WHERE table_schema = DATABASE()
      AND table_name = 'merchant_account'
      AND column_name = 'parent_merchant_id'
);
SET @add_parent := IF(
    @has_parent = 0,
    'ALTER TABLE merchant_account ADD COLUMN parent_merchant_id VARCHAR(255) NULL, ADD INDEX idx_merchant_account_parent (parent_merchant_id)',
    'DO 0'
);
PREPARE add_parent FROM @add_parent;
EXECUTE add_parent;
DEALLOCATE PREPARE add_parent;
