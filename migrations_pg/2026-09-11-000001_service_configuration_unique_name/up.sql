-- `service_configuration` is a key/value table that every caller reads through
-- `find_config_by_name`, i.e. as if `name` identified at most one row — but nothing enforced that.
-- Two concurrent first-writes of the same key each saw "no row" and inserted, leaving duplicates
-- that `find_config_by_name` then picks between arbitrarily, so a feature could read as on or off
-- depending on which row came back.
--
-- Collapse any duplicates that predate the constraint, keeping the highest id. `update_config`
-- filters on `name` alone and so has always written every duplicate at once, which means the rows
-- hold the same value and the choice only matters for rows written before they diverged.
DELETE FROM service_configuration a
    USING service_configuration b
    WHERE a.name = b.name AND a.id < b.id;

ALTER TABLE service_configuration
    ADD CONSTRAINT service_configuration_name_key UNIQUE (name);
