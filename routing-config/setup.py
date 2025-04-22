import yaml
import json
from datetime import datetime
import mysql.connector
import sys
import os

# === CONFIG ===
YAML_FILE = "config.yaml"
SQL_FILE = "output.sql"
EXECUTE_ON_DB = True  # ✅ Execution happens at the very bottom!
DB_CONFIG = {
    "host": "mysql",
    "port": 3306,
    "user": "root",
    "password": "root",
    "database": "jdb"
}

# === LOAD YAML ===
with open(YAML_FILE, "r") as f:
    config = yaml.safe_load(f)

# === Validate Required Fields ===
if "merchant_id" not in config:
    print("❌ 'merchant_id' is required in the YAML file.")
    sys.exit(1)

merchant_id = config["merchant_id"]
priority_logic = config.get("priority_logic", {})
sr_config = config.get("sr_routing_config", None)
elimination_config = config.get("elimination_config", {})

priority_script_path = priority_logic.get("script", None)
priority_tag = priority_logic.get("tag", None)

priority_script_content = None

if priority_script_path:
    if not priority_tag:
        print("❌ If 'priority_logic.script' is provided, 'priority_logic.tag' must also be present.")
        sys.exit(1)
    if not os.path.isfile(priority_script_path):
        print(f"❌ Priority logic script file not found: {priority_script_path}")
        sys.exit(1)
    with open(priority_script_path, 'r') as f:
        priority_script_content = f.read().replace("'", "''")  # Escape single quotes for SQL

elimination_threshold = elimination_config.get("threshold", 0.35)

now = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
bit_true = "b'1'"
null = "NULL"

sql_statements = []

# === Delete Previous Data ===
sql_statements.append(f"DELETE FROM merchant_account WHERE merchant_id = '{merchant_id}';")
sql_statements.append(f"DELETE FROM merchant_iframe_preferences WHERE merchant_id = '{merchant_id}';")
sql_statements.append(f"DELETE FROM service_configuration WHERE name = 'SR_V3_INPUT_CONFIG_{merchant_id}';")

# === Insert MerchantAccount ===
merchant_account_sql = f"""
INSERT INTO merchant_account (
    merchant_id, date_created, gateway_decided_by_health_enabled,
    gateway_priority, gateway_priority_logic, internal_hash_key,
    locker_id, token_locker_id, user_id, settlement_account_id,
    secondary_merchant_account_id, use_code_for_gateway_priority,
    enable_gateway_reference_id_based_routing, gateway_success_rate_based_decider_input,
    internal_metadata, enabled, country, installment_enabled,
    tenant_account_id, priority_logic_config, merchant_category_code
) VALUES (
    '{merchant_id}', '{now}', {null},
    {null}, {f"'{priority_script_content}'" if priority_script_content else null}, {null},
    {null}, {null}, {null}, {null},
    {null}, {bit_true},
    {null}, '{json.dumps({"defaultEliminationThreshold": elimination_threshold, "defaultEliminationLevel": "PAYMENT_METHOD"})}',
    {f"'{json.dumps({'active_priority_logic_name': priority_tag})}'" if priority_tag else null}, {bit_true}, {null}, {null},
    {null}, {null}, {null}
);
""".strip()
sql_statements.append(merchant_account_sql)

# === Insert merchant_iframe_preferences ===
iframe_sql = f"""
INSERT INTO merchant_iframe_preferences (
    merchant_id, dynamic_switching_enabled,
    isin_routing_enabled, issuer_routing_enabled,
    txn_failure_gateway_penality, card_brand_routing_enabled
) VALUES (
    '{merchant_id}', {null}, {null}, {null}, {null}, {null}
);
""".strip()
sql_statements.append(iframe_sql)

# === Insert SR Config if present ===
if sr_config:
    sr_config_sql = f"""
    INSERT INTO service_configuration (
        name, value, new_value, previous_value, new_value_status
    ) VALUES (
        'SR_V3_INPUT_CONFIG_{merchant_id}', '{json.dumps(sr_config)}', {null}, {null}, {null}
    );
    """.strip()
    sql_statements.append(sr_config_sql)

# === Static Global Configs (insert only if not already present) ===
static_configs = [
    ("merchants_enabled_for_score_keys_unification", {
        "enableAll": True, "enableAllRollout": 100
    }),
    ("ENABLE_MERCHANT_ON_VOLUME_DISTRIBUTION_FEATURE_SR_V3", {
        "enableAll": True, "enableAllRollout": 100
    }),
    ("ENABLE_RESET_ON_SR_V3", {
        "enableAll": True, "enableAllRollout": 100
    }),
    ("SR_V3_INPUT_CONFIG_DEFAULT", {
        "defaultLatencyThreshold": 90,
        "defaultBucketSize": 125,
        "defaultHedgingPercent": 5
    })
]

for name, value in static_configs:
    insert_if_not_exists_sql = f"""
    INSERT INTO service_configuration (
        name, value, new_value, previous_value, new_value_status
    )
    SELECT * FROM (
        SELECT
            '{name}' AS name,
            '{json.dumps(value)}' AS value,
            NULL AS new_value,
            NULL AS previous_value,
            NULL AS new_value_status
    ) AS tmp
    WHERE NOT EXISTS (
        SELECT 1 FROM service_configuration WHERE name = '{name}'
    ) LIMIT 1;
    """.strip()
    sql_statements.append(insert_if_not_exists_sql)

# === Write SQL File ===
with open(SQL_FILE, "w") as f:
    f.write("\n\n".join(sql_statements))

print(f"✅ SQL written to {SQL_FILE}")

# === Optional Execution ===
if EXECUTE_ON_DB:
    try:
        conn = mysql.connector.connect(**DB_CONFIG)
        cursor = conn.cursor()
        for stmt in sql_statements:
            cursor.execute(stmt)
        conn.commit()
        print("✅ SQL executed successfully in MySQL DB.")
    except mysql.connector.Error as err:
        print("❌ MySQL execution error:", err)
        conn.rollback()
    finally:
        cursor.close()
        conn.close()
