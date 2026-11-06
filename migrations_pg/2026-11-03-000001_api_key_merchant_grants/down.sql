DROP TABLE IF EXISTS api_key_merchants;
ALTER TABLE merchant_api_keys DROP COLUMN IF EXISTS authz_version;
