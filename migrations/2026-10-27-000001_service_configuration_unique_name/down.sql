ALTER TABLE service_configuration
    DROP INDEX uq_service_configuration_name;

ALTER TABLE service_configuration MODIFY name TEXT NOT NULL;
