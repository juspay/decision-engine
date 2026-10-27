-- One row per configuration name. Duplicates come from concurrent first writes; keep the lowest id,
-- which is the row reads return today (updates already write every row of a name alike).
DELETE FROM service_configuration a
    USING service_configuration b
    WHERE a.name = b.name AND a.id > b.id;

ALTER TABLE service_configuration
    ADD CONSTRAINT service_configuration_name_key UNIQUE (name);
