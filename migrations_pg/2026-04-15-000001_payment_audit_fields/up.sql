ALTER TABLE analytics_event
    ADD COLUMN payment_id VARCHAR(255),
    ADD COLUMN request_id VARCHAR(255),
    ADD COLUMN event_stage VARCHAR(128);

CREATE INDEX idx_analytics_event_payment_id ON analytics_event (payment_id);
CREATE INDEX idx_analytics_event_request_id ON analytics_event (request_id);
CREATE INDEX idx_analytics_event_stage ON analytics_event (event_stage);
