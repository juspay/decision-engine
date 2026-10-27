-- One row per configuration name. Duplicates come from concurrent first writes; keep the lowest id,
-- which is the row reads return today (updates already write every row of a name alike).
DELETE a FROM service_configuration a
    JOIN service_configuration b ON a.name = b.name AND a.id > b.id;

-- `name` is TEXT, which MySQL indexes by prefix: the key covers each name's first 255 characters.
ALTER TABLE service_configuration
    ADD UNIQUE KEY uq_service_configuration_name (name(255));
