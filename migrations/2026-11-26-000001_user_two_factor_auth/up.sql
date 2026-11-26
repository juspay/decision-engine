-- Two-factor authentication. `totp_secret` is the TOTP seed sealed with the configured key and
-- `totp_recovery_codes` a JSON array of keyed digests of the unused recovery codes. Both are NULL
-- until the user enrolls.
ALTER TABLE users ADD COLUMN totp_secret TEXT NULL DEFAULT NULL;
ALTER TABLE users ADD COLUMN totp_recovery_codes TEXT NULL DEFAULT NULL;
