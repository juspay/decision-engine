-- See migrations_pg/2026-09-11-000001_service_configuration_unique_name/up.sql for why.
-- `name` is TEXT, so MySQL needs a prefix length on the index. Every key this table holds is a
-- short constant or a constant plus a merchant id, far inside 255 characters.
DELETE a FROM service_configuration a
    JOIN service_configuration b
      ON a.name = b.name AND a.id < b.id;

ALTER TABLE service_configuration
    ADD UNIQUE KEY uq_service_configuration_name (name(255));
