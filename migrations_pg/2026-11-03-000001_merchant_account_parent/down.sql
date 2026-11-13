DROP INDEX IF EXISTS idx_merchant_account_parent;
ALTER TABLE merchant_account DROP COLUMN IF EXISTS parent_merchant_id;
