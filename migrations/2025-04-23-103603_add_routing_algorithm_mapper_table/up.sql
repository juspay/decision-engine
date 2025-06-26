-- Your SQL goes here
CREATE TABLE routing_algorithm_mapper (
    id SERIAL PRIMARY KEY,
    created_by VARCHAR(255) NOT NULL,
    routing_algorithm_id VARCHAR(255) NOT NULL,
    algorithm_for VARCHAR(64) NOT NULL,
    UNIQUE (created_by, algorithm_for)
);
