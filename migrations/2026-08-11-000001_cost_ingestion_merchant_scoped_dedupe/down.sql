ALTER TABLE cost_ingestion
    DROP INDEX uq_cost_ingestion_notif,
    ADD UNIQUE KEY uq_cost_ingestion_notif (connector, notification_id);
