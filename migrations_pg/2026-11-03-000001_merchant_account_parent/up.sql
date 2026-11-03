-- A profile is a merchant_account row whose parent_merchant_id names the merchant it belongs to. A
-- merchant has none. Existing rows stay NULL, so every one of them remains a merchant.
ALTER TABLE merchant_account ADD COLUMN IF NOT EXISTS parent_merchant_id VARCHAR(255);
CREATE INDEX IF NOT EXISTS idx_merchant_account_parent ON merchant_account (parent_merchant_id);
