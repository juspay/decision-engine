CREATE TABLE analytics_event (
    id SERIAL PRIMARY KEY,
    event_type VARCHAR(64) NOT NULL,
    merchant_id VARCHAR(255),
    payment_method_type VARCHAR(255),
    payment_method VARCHAR(255),
    gateway VARCHAR(255),
    routing_approach VARCHAR(128),
    rule_name VARCHAR(255),
    status VARCHAR(64),
    error_code VARCHAR(64),
    error_message TEXT,
    score_value DOUBLE PRECISION,
    sigma_factor DOUBLE PRECISION,
    average_latency DOUBLE PRECISION,
    tp99_latency DOUBLE PRECISION,
    transaction_count BIGINT,
    route VARCHAR(128),
    details TEXT,
    created_at_ms BIGINT NOT NULL
);

CREATE INDEX idx_analytics_event_created_at_ms ON analytics_event (created_at_ms);
CREATE INDEX idx_analytics_event_merchant_id ON analytics_event (merchant_id);
CREATE INDEX idx_analytics_event_type ON analytics_event (event_type);
