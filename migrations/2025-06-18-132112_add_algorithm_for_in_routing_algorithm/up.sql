-- Your SQL goes here
ALTER TABLE routing_algorithm ADD COLUMN algorithm_for VARCHAR(64);
ALTER TABLE routing_algorithm_mapper ADD COLUMN algorithm_for VARCHAR(64);

