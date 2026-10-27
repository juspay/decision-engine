-- One row per configuration name. Duplicates come from concurrent first writes; keep the lowest id,
-- which is the row reads return today (updates already write every row of a name alike).
DELETE a FROM service_configuration a
    JOIN service_configuration b ON a.name = b.name AND a.id > b.id;

-- MySQL indexes TEXT only by prefix, which would let names sharing their first characters collide.
-- VARCHAR(768) is the longest utf8mb4 column a unique index covers in full (3072 bytes).
ALTER TABLE service_configuration MODIFY name VARCHAR(768) NOT NULL;

-- The compose migrator replays every up.sql on each start, and MySQL has no ADD KEY IF NOT EXISTS,
-- so the key is added only when it is not there yet.
SET @has_unique_name := (
    SELECT COUNT(*) FROM information_schema.statistics
    WHERE table_schema = DATABASE()
      AND table_name = 'service_configuration'
      AND index_name = 'uq_service_configuration_name'
);
SET @add_unique_name := IF(
    @has_unique_name = 0,
    'ALTER TABLE service_configuration ADD UNIQUE KEY uq_service_configuration_name (name)',
    'DO 0'
);
PREPARE add_unique_name FROM @add_unique_name;
EXECUTE add_unique_name;
DEALLOCATE PREPARE add_unique_name;
