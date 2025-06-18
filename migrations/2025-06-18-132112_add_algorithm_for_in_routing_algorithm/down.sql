-- This file should undo anything in `up.sql`
ALTER TABLE routing_algorithm
DROP COLUMN algorithm_for;


ALTER TABLE routing_algorithm_mapper
DROP COLUMN algorithm_for;
