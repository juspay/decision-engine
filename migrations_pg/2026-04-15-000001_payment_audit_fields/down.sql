DROP INDEX IF EXISTS idx_analytics_event_stage;
DROP INDEX IF EXISTS idx_analytics_event_request_id;
DROP INDEX IF EXISTS idx_analytics_event_payment_id;

ALTER TABLE analytics_event
    DROP COLUMN IF EXISTS event_stage,
    DROP COLUMN IF EXISTS request_id,
    DROP COLUMN IF EXISTS payment_id;
