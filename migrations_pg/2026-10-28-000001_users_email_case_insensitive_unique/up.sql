-- One account per email regardless of case. The application keys accounts by lowercased email and
-- looks them up by lower(email); this index is what makes a case variant of an existing address
-- impossible to register, even under concurrent signups.
--
-- Fails if the table already holds addresses that differ only in case. Find them with
--   SELECT lower(email), array_agg(email) FROM users GROUP BY lower(email) HAVING count(*) > 1;
-- and resolve them by hand: they are distinct accounts, so none is dropped automatically.
CREATE UNIQUE INDEX IF NOT EXISTS uq_users_email_lower ON users (lower(email));
