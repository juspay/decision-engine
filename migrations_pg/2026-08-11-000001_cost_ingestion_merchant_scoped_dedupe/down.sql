ALTER TABLE cost_ingestion
    DROP CONSTRAINT cost_ingestion_merchant_connector_notification_id_key,
    ADD CONSTRAINT cost_ingestion_connector_notification_id_key
        UNIQUE (connector, notification_id);
