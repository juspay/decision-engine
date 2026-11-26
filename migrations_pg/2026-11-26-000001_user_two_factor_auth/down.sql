ALTER TABLE users DROP COLUMN IF EXISTS totp_recovery_codes;
ALTER TABLE users DROP COLUMN IF EXISTS totp_secret;
