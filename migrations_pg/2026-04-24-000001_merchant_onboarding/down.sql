ALTER TABLE users ALTER COLUMN merchant_id SET NOT NULL;
DROP TABLE IF EXISTS user_merchants;
ALTER TABLE merchant_account DROP COLUMN IF EXISTS merchant_name;
