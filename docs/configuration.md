# Configuration Reference

Decision Engine is configured from a single TOML file loaded at process start. This page documents every section that the service reads. For deployment-specific setup (Docker, Helm, cargo), see the [Local Setup Guide](local-setup.md).

## Config file locations

| Deployment target | File |
|---|---|
| Host / source run (`cargo run`) | `config/development.toml` |
| Docker & Docker Compose | `config/docker-configuration.toml` (mounted at `/local/config/development.toml`) |
| Helm chart | `helm-charts/config/development.toml` (rendered into the config map) |

A full, commented reference lives at [`config.example.toml`](../config.example.toml) at the repo root — copy and edit it for custom deployments.

## Precedence

1. Defaults compiled into the binary.
2. Values from the TOML file.
3. Environment variables (when explicitly supported — see [Environment overrides](#environment-overrides)).
4. Helm `extraEnvVars` (Kubernetes only).

---

## Sections

### `[server]`

HTTP listener for the public API.

```toml
[server]
host = "0.0.0.0"
port = 8080
```

### `[metrics]`

Prometheus scrape endpoint. Exposed on a separate port so it can be firewalled off from public traffic.

```toml
[metrics]
host = "0.0.0.0"
port = 9094
```

### `[log.console]`

Structured logging configuration.

```toml
[log.console]
enabled    = true
level      = "DEBUG"      # TRACE | DEBUG | INFO | WARN | ERROR
log_format = "default"    # default | json
```

In production, prefer `level = "INFO"` and `log_format = "json"` so logs ship cleanly into Loki / ELK / Datadog.

### `[limit]`

Coarse global rate limiter.

```toml
[limit]
request_count = 1     # requests...
duration      = 60    # ...per this many seconds per key
```

### `[database]` — MySQL

Used when the binary is built with the default (`release`) features.

```toml
[database]
username = "root"
password = "root"
host     = "127.0.0.1"
port     = 3306
dbname   = "jdb"
```

### `[pg_database]` — PostgreSQL

Used when the binary is built with `--features postgres`.

```toml
[pg_database]
pg_username = "db_user"
pg_password = "db_pass"
pg_host     = "localhost"
pg_port     = 5432
pg_dbname   = "decision_engine_db"
```

Exactly one of `[database]` or `[pg_database]` is consumed, depending on the build. The other section is ignored.

### `[redis]`

Redis holds ephemeral SR state, caches, and rate-limit counters. All fields below are honoured by the [`fred`](https://crates.io/crates/fred) client.

```toml
[redis]
host                      = "127.0.0.1"
port                      = 6379
pool_size                 = 5
reconnect_max_attempts    = 5
reconnect_delay           = 5       # seconds
use_legacy_version        = false
stream_read_count         = 1
auto_pipeline             = true
disable_auto_backpressure = false
max_in_flight_commands    = 5000
default_command_timeout   = 30      # seconds
unresponsive_timeout      = 10      # seconds
max_feed_count            = 200
```

### `[cache]` and `[cache_config]`

In-process cache (Moka) in front of Redis, plus TTLs for config lookups.

```toml
[cache]
tti          = 7200   # time-to-idle, seconds
max_capacity = 5000

[cache_config]
service_config_redis_prefix = "DE_service_config_"
service_config_ttl          = 300   # seconds
```

### `[compression_filepath]`

Path on disk where Redis ZSTD compression dictionaries are loaded from.

```toml
[compression_filepath]
zstd_compression_filepath = "/tmp/extra-paths/redis-zstd-dictionaries/sbx"
```

### `[tenant_secrets]`

Per-tenant isolation. Each entry maps a tenant id to its DB schema (and, in commercial builds, its secret manager binding).

```toml
[tenant_secrets]
public = { schema = "public" }
```

### `[routing_config.keys]`

The **schema of fields the rule engine can match on**. Every `lhs` you can use inside a Priority Logic V2 rule must be declared here, together with its type:

| Declared type | Matches against |
|---|---|
| `integer` | numeric conditions with optional `min` |
| `enum` | `enum_variant` / `enum_variant_array` with a fixed value set |
| `str_value` | free-form strings with optional `min_length`, `exact_length`, `regex` |
| `udf` | opaque user-defined field (e.g., `metadata`) |

```toml
[routing_config.keys]
amount       = { type = "integer", min = 0 }
payment_method = { type = "enum", values = "card, upi, wallet, bank_transfer, …" }
card_bin     = { type = "str_value", exact_length = 6, regex = "^[0-9]{6}$" }
billing_country = { type = "enum", values = "India, UnitedStatesOfAmerica, …" }
```

> See the top of [`config/development.toml`](../config/development.toml) for the full list of fields that ship with the default build, including `payment_method`, `payment_method_type`, `card_network`, `currency`, `issuer_country`, `authentication_type`, `capture_method`, and more.

### `[debit_routing_config]` and subsections

Fees used by the US debit-routing algorithm (`NTW_BASED_ROUTING`).

```toml
[debit_routing_config]
fraud_check_fee = 1.0

[debit_routing_config.network_fee]
visa       = { percentage = 0.1375, fixed_amount = 2.0 }
mastercard = { percentage = 0.15,   fixed_amount = 4.0 }
accel      = { percentage = 0.0,    fixed_amount = 4.0 }
nyce       = { percentage = 0.10,   fixed_amount = 1.5 }
pulse      = { percentage = 0.10,   fixed_amount = 3.0 }

[debit_routing_config.interchange_fee]
# Per-network interchange — see config.example.toml for the full matrix.
```

The engine uses these numbers to pick the cheapest eligible network on a co-badged debit card.

### `[pm_filters.*]`

Eligibility filters per connector. Each connector section declares which payment-method / country / currency combinations it supports, and the engine uses that to filter the `eligibleGatewayList` before ranking.

```toml
[pm_filters.stripe]
# payment_method.payment_method_type = "country, currency, …"
card.credit = "UnitedStatesOfAmerica, France, … : USD, EUR, …"
# …
```

Add a new connector by adding a new `[pm_filters.<connector>]` section. The complete list of shipped connectors is in `config/development.toml`.

### `[secrets_manager]`

Controls how secrets (DB credentials, gateway keys) are resolved at boot.

```toml
[secrets_manager]
secrets_management_config = "no_encryption"   # no_encryption | aws_kms | hashicorp_vault
```

In local development, `no_encryption` is the norm. For production, see the KMS-backed and Vault-backed variants in `config.example.toml`.

---

## Environment overrides

Many scalar values can be overridden via environment variables at runtime — useful in Kubernetes, where secrets should never land in a committed config file. Common ones:

| Variable | Overrides |
|---|---|
| `ROUTER__REDIS__HOST` | `[redis].host` |
| `ROUTER__REDIS__PORT` | `[redis].port` |
| `ROUTER__PG_DATABASE__PG_PASSWORD` | `[pg_database].pg_password` |
| `ROUTER__DATABASE__PASSWORD` | `[database].password` |
| `ROUTER__SERVER__PORT` | `[server].port` |
| `ROUTER__LOG__CONSOLE__LEVEL` | `[log.console].level` |

The pattern is `ROUTER__<SECTION>__<KEY>` (double underscores between levels). Nested tables like `[log.console]` become `ROUTER__LOG__CONSOLE__…`.

For Helm, set these via `extraEnvVars` — see `helm-charts/values.yaml`.

---

## Related documentation

- [Local Setup Guide](local-setup.md) — how to run the engine with a given config
- [API Reference](api-reference1.md) — how the routing configuration is used at request time
- [Helm Chart](../helm-charts/) — Kubernetes packaging
