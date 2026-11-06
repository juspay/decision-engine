#!/usr/bin/env python3
"""Exercise the real grant migrations in disposable PostgreSQL 16 / MySQL 8 containers."""

from contextlib import contextmanager
from pathlib import Path
import subprocess
import sys
import time
import uuid


ROOT = Path(__file__).resolve().parents[1]
MIGRATION = "2026-11-03-000001_api_key_merchant_grants"
PASSWORD = "key-grants-test-only"


def command(args, sql=None, timeout=30, error_code=None):
    result = subprocess.run(args, input=sql, capture_output=True, text=True, timeout=timeout)
    if error_code:
        if result.returncode == 0 or f"ERROR {error_code}" not in result.stderr:
            raise RuntimeError(f"Expected SQL error {error_code}: {result.stdout}{result.stderr}")
    elif result.returncode:
        raise RuntimeError(result.stdout + result.stderr)
    return result.stdout.strip()


@contextmanager
def database(backend):
    name = f"de-key-grants-{backend}-{uuid.uuid4().hex[:12]}"
    postgres = backend == "postgres"
    image = "postgres:16-alpine" if postgres else "mysql:8.0"
    env = f"POSTGRES_PASSWORD={PASSWORD}" if postgres else f"MYSQL_ROOT_PASSWORD={PASSWORD}"
    # TCP readiness excludes the temporary, socket-only server used during image initialization.
    client = (
        ["-e", f"PGPASSWORD={PASSWORD}", name, "psql", "-X", "-v", "ON_ERROR_STOP=1",
         "-h", "127.0.0.1", "-U", "postgres", "-d", "postgres", "-At"]
        if postgres else
        [name, "mysql", "--protocol=tcp", "-h127.0.0.1", "-uroot", f"-p{PASSWORD}",
         "--batch", "--skip-column-names"]
    )
    sql_command = ["docker", "exec", "-i", *client]
    try:
        # No host ports, mounts, existing containers, or external database credentials are used.
        command(["docker", "run", "--detach", "--name", name, "-e", env, image], timeout=180)
        deadline = time.monotonic() + 120
        while True:
            try:
                command(sql_command, "SELECT 1;", timeout=10)
                break
            except (RuntimeError, subprocess.TimeoutExpired):
                if time.monotonic() >= deadline:
                    logs = command(["docker", "logs", name])
                    raise RuntimeError(f"{backend} readiness timed out\n{logs}")
                time.sleep(1)
        yield sql_command
    finally:
        cleanup = subprocess.run(
            ["docker", "rm", "--force", "--volumes", name],
            capture_output=True, text=True, timeout=30,
        )
        if cleanup.returncode:
            print(f"Cleanup failed for {name}: {cleanup.stderr}", file=sys.stderr)


def sql_file(backend, migration, direction="up"):
    folder = "migrations_pg" if backend == "postgres" else "migrations"
    return (ROOT / folder / migration / f"{direction}.sql").read_text()


def check(actual, expected):
    if actual != expected:
        raise RuntimeError(f"Expected {expected!r}, got {actual!r}")


def test_postgres(client):
    fixture = """
CREATE TABLE merchant_account (merchant_id VARCHAR(255));
INSERT INTO merchant_account VALUES ('merchant_a'), ('merchant_a'), ('merchant_b');
INSERT INTO merchant_api_keys (key_id,merchant_id,key_hash,key_prefix,is_active) VALUES
('key_a','merchant_a',repeat('a',64),'DE_a',TRUE),
('key_b','merchant_b',repeat('b',64),'DE_b',FALSE),
('key_orphan','missing_merchant',repeat('c',64),'DE_c',TRUE);
CREATE TEMP TABLE original_keys AS SELECT * FROM merchant_api_keys;
"""
    first_check = """
DO $$ BEGIN
 IF (SELECT count(*) FROM api_key_merchants) <> 3 THEN RAISE EXCEPTION 'backfill count'; END IF;
 IF (SELECT count(*) FROM merchant_api_keys k JOIN api_key_merchants g ON g.key_id=k.key_id AND g.merchant_id=k.merchant_id AND g.created_at=k.created_at) <> 3 THEN RAISE EXCEPTION 'owner grant'; END IF;
 IF (SELECT status FROM api_key_merchants WHERE key_id='key_a') <> 'active' THEN RAISE EXCEPTION 'active'; END IF;
 IF EXISTS (SELECT 1 FROM api_key_merchants WHERE key_id IN ('key_b','key_orphan') AND status <> 'revoked') THEN RAISE EXCEPTION 'revoked/orphan'; END IF;
 IF EXISTS (SELECT 1 FROM api_key_merchants WHERE permissions IS NOT NULL OR authz_version<>1) THEN RAISE EXCEPTION 'defaults'; END IF;
 IF EXISTS (SELECT 1 FROM merchant_api_keys WHERE authz_version<>1) THEN RAISE EXCEPTION 'key version default'; END IF;
 IF EXISTS (SELECT 1 FROM merchant_api_keys k JOIN original_keys o USING(key_id) WHERE (to_jsonb(k)-'authz_version') <> to_jsonb(o)) THEN RAISE EXCEPTION 'credential changed'; END IF;
END $$;
UPDATE api_key_merchants SET status='revoked',authz_version=3,permissions='["feedback:write"]' WHERE key_id='key_a';
INSERT INTO merchant_account VALUES ('missing_merchant');
"""
    replay_check = """
DO $$ BEGIN
 IF (SELECT authz_version FROM api_key_merchants WHERE key_id='key_a')<>3 OR (SELECT status FROM api_key_merchants WHERE key_id='key_a')<>'revoked' OR (SELECT permissions FROM api_key_merchants WHERE key_id='key_a')<>'["feedback:write"]' THEN RAISE EXCEPTION 'replay reset'; END IF;
 IF (SELECT status FROM api_key_merchants WHERE key_id='key_orphan')<>'revoked' THEN RAISE EXCEPTION 'orphan reactivated'; END IF;
 BEGIN INSERT INTO api_key_merchants SELECT * FROM api_key_merchants LIMIT 1; RAISE EXCEPTION 'duplicate accepted'; EXCEPTION WHEN unique_violation THEN NULL; END;
 BEGIN UPDATE api_key_merchants SET status='invalid'; RAISE EXCEPTION 'bad status'; EXCEPTION WHEN check_violation THEN NULL; END;
 BEGIN UPDATE api_key_merchants SET authz_version=0; RAISE EXCEPTION 'bad grant version'; EXCEPTION WHEN check_violation THEN NULL; END;
 BEGIN UPDATE merchant_api_keys SET authz_version=0; RAISE EXCEPTION 'bad key version'; EXCEPTION WHEN check_violation THEN NULL; END;
 BEGIN INSERT INTO api_key_merchants VALUES ('no_key','merchant_a','active',NULL,1,now()); RAISE EXCEPTION 'missing FK'; EXCEPTION WHEN foreign_key_violation THEN NULL; END;
 IF NOT EXISTS (SELECT 1 FROM pg_indexes WHERE schemaname=current_schema() AND indexname='idx_api_key_merchants_merchant_id') THEN RAISE EXCEPTION 'reverse index'; END IF;
END $$;
INSERT INTO merchant_api_keys (key_id,merchant_id,key_hash,key_prefix) VALUES ('cascade','merchant_a',repeat('d',64),'DE_d');
INSERT INTO api_key_merchants VALUES ('cascade','merchant_a','active',NULL,1,now());
DELETE FROM merchant_api_keys WHERE key_id='cascade';
DO $$ BEGIN IF EXISTS (SELECT 1 FROM api_key_merchants WHERE key_id='cascade') THEN RAISE EXCEPTION 'cascade'; END IF; END $$;
"""
    rollback_check = """
DO $$ BEGIN
 IF EXISTS (SELECT 1 FROM merchant_api_keys k JOIN original_keys o USING(key_id) WHERE to_jsonb(k) <> to_jsonb(o)) THEN RAISE EXCEPTION 'rollback changed credentials'; END IF;
 IF to_regclass('api_key_merchants') IS NOT NULL THEN RAISE EXCEPTION 'rollback table'; END IF;
END $$;
"""
    command(client, "BEGIN;\n" + sql_file("postgres", "2026-04-21-000000_add_merchant_api_keys")
            + fixture + sql_file("postgres", MIGRATION) + first_check
            + sql_file("postgres", MIGRATION) + replay_check
            + sql_file("postgres", MIGRATION, "down") + rollback_check
            + sql_file("postgres", MIGRATION) + "ROLLBACK;")


def test_mysql(client):
    def sql(statement, **kwargs):
        return command(client, "USE grants_test;\n" + statement, **kwargs)

    command(client, "CREATE DATABASE grants_test CHARACTER SET utf8mb4 COLLATE utf8mb4_0900_ai_ci;")
    sql(sql_file("mysql", "2026-04-21-000000_add_merchant_api_keys") + """
CREATE TABLE merchant_account (merchant_id VARCHAR(255));
INSERT INTO merchant_account VALUES ('merchant_a'),('merchant_a'),('merchant_b');
INSERT INTO merchant_api_keys (key_id,merchant_id,key_hash,key_prefix,is_active) VALUES
('key_a','merchant_a',repeat('a',64),'DE_a',1),
('key_b','merchant_b',repeat('b',64),'DE_b',0),
('key_orphan','missing_merchant',repeat('c',64),'DE_c',1);
CREATE TABLE original_keys AS SELECT * FROM merchant_api_keys;
ALTER DATABASE grants_test COLLATE utf8mb4_bin;
""" + sql_file("mysql", MIGRATION))
    check(sql("SELECT count(*) FROM api_key_merchants;"), "3")
    check(sql("SELECT count(*) FROM merchant_api_keys k JOIN api_key_merchants g ON g.key_id=k.key_id AND g.merchant_id=k.merchant_id AND g.created_at=k.created_at;"), "3")
    check(sql("SELECT group_concat(concat(key_id,':',status) ORDER BY key_id) FROM api_key_merchants;"),
          "key_a:active,key_b:revoked,key_orphan:revoked")
    check(sql("SELECT count(*) FROM api_key_merchants WHERE permissions IS NOT NULL OR authz_version<>1;"), "0")
    check(sql("SELECT count(*) FROM merchant_api_keys WHERE authz_version<>1;"), "0")
    columns = "id key_id merchant_id key_hash key_prefix description is_active created_at".split()
    unchanged = "SELECT count(*) FROM merchant_api_keys k JOIN original_keys o USING(key_id) WHERE " + " OR ".join(
        f"NOT (k.{column} <=> o.{column})" for column in columns
    ) + ";"
    check(sql(unchanged), "0")
    sql("""UPDATE api_key_merchants SET status='revoked',authz_version=3,permissions='["feedback:write"]' WHERE key_id='key_a';
INSERT INTO merchant_account VALUES ('missing_merchant');
""" + sql_file("mysql", MIGRATION))
    check(sql("SELECT concat(status,':',authz_version,':',permissions) FROM api_key_merchants WHERE key_id='key_a';"),
          'revoked:3:["feedback:write"]')
    check(sql("SELECT status FROM api_key_merchants WHERE key_id='key_orphan';"), "revoked")
    for statement, code in [
        ("INSERT INTO api_key_merchants SELECT * FROM api_key_merchants LIMIT 1;", 1062),
        ("UPDATE api_key_merchants SET status='invalid';", 3819),
        ("UPDATE api_key_merchants SET authz_version=0;", 3819),
        ("UPDATE merchant_api_keys SET authz_version=0;", 3819),
        ("INSERT INTO api_key_merchants VALUES ('no_key','merchant_a','active',NULL,1,now());", 1452),
    ]:
        sql(statement, error_code=code)
    check(sql("SELECT count(*) FROM information_schema.statistics WHERE table_schema=DATABASE() AND table_name='api_key_merchants' AND index_name='idx_api_key_merchants_merchant_id';"), "1")
    sql("""INSERT INTO merchant_api_keys (key_id,merchant_id,key_hash,key_prefix) VALUES ('cascade','merchant_a',repeat('d',64),'DE_d');
INSERT INTO api_key_merchants VALUES ('cascade','merchant_a','active',NULL,1,now());
DELETE FROM merchant_api_keys WHERE key_id='cascade';""")
    check(sql("SELECT count(*) FROM api_key_merchants WHERE key_id='cascade';"), "0")
    sql(sql_file("mysql", MIGRATION, "down"))
    check(sql(unchanged), "0")
    check(sql("SELECT count(*) FROM information_schema.columns WHERE table_schema=DATABASE() AND table_name='merchant_api_keys' AND column_name='authz_version';"), "0")
    check(sql("SELECT count(*) FROM information_schema.tables WHERE table_schema=DATABASE() AND table_name='api_key_merchants';"), "0")
    sql(sql_file("mysql", MIGRATION))


def main():
    for backend, test in [("postgres", test_postgres), ("mysql", test_mysql)]:
        with database(backend) as client:
            test(client)
        print(f"PASS {backend}: backfill, credential preservation, replay, constraints, index, cascade, down/up")


if __name__ == "__main__":
    main()
