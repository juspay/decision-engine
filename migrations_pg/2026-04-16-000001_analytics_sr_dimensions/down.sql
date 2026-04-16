ALTER TABLE analytics_event
    DROP COLUMN IF EXISTS auth_type,
    DROP COLUMN IF EXISTS country,
    DROP COLUMN IF EXISTS currency,
    DROP COLUMN IF EXISTS card_is_in,
    DROP COLUMN IF EXISTS card_network;
