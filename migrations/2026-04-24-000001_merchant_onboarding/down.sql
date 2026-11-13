ALTER TABLE users MODIFY COLUMN merchant_id VARCHAR(255) NOT NULL DEFAULT '';
DROP TABLE IF EXISTS user_merchants;
ALTER TABLE merchant_account DROP COLUMN merchant_name;
