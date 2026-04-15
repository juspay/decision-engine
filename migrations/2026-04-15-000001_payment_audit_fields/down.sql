DROP INDEX idx_analytics_event_stage ON analytics_event;
DROP INDEX idx_analytics_event_request_id ON analytics_event;
DROP INDEX idx_analytics_event_payment_id ON analytics_event;

ALTER TABLE analytics_event
    DROP COLUMN event_stage,
    DROP COLUMN request_id,
    DROP COLUMN payment_id;
