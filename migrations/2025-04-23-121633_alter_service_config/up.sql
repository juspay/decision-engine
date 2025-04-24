-- Your SQL goes here
-- ALTER TABLE service_configuration DROP COLUMN version;
ALTER TABLE service_configuration ADD COLUMN version BIGINT DEFAULT 0;
