ALTER TABLE analytics_event
    ADD COLUMN card_network VARCHAR(255),
    ADD COLUMN card_is_in VARCHAR(255),
    ADD COLUMN currency VARCHAR(64),
    ADD COLUMN country VARCHAR(64),
    ADD COLUMN auth_type VARCHAR(64);
