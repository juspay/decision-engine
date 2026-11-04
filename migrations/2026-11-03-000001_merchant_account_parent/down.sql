ALTER TABLE merchant_account
    DROP INDEX idx_merchant_account_parent,
    DROP COLUMN parent_merchant_id;
