-- This file should undo anything in `up.sql`
ALTER TABLE IF EXISTS service_configuration
    DROP COLUMN IF EXISTS version;