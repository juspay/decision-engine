---
title: "Configuration"
description: "Configure Decision Engine via config files and environment variable overrides."
---

# Configuration Guide

This document explains how to configure Decision Engine for local and on-prem deployments.

## Primary Config Files

- `config/development.toml`: used for host/source runs
- `config/docker-configuration.toml`: used for Docker and Compose runs
- `helm-charts/config/development.toml`: Kubernetes chart template config

These files already exist with all required sections. Edit the one that matches your runtime rather than copying from `config.example.toml`, which is incomplete.

## Config Sections

### Server

```toml
[server]
host = "0.0.0.0"
port = 8080
```

`host` is the bind address. Use `0.0.0.0` for Docker/deployed, `127.0.0.1` for local-only.

### Logging

```toml
[log.console]
enabled = true
level = "DEBUG"
log_format = "default"
```

`log_format` accepts `"default"` (human-readable) or `"json"` (structured, recommended for prod).

### Metrics (OpenTelemetry push)

```toml
[log.telemetry]
metrics_enabled = true
ignore_errors = true
otel_exporter_otlp_endpoint = "http://localhost:4317"
otel_exporter_otlp_timeout = 10000 # milliseconds per export request
```

There is no metrics port to scrape. When `metrics_enabled` is on, metrics are pushed over OTLP/gRPC to an OpenTelemetry collector every 3 seconds, the same cadence as Hyperswitch. In the Hyperswitch clusters the collector re-exposes them to vmagent, so they land in VictoriaMetrics and the Grafana `VictoriaMetrics` datasource. The collector adds `source_namespace`, `source_pod` and `source_app` labels; filter by `source_namespace="decision-engine"` in Grafana. With `metrics_enabled = false` the instruments are no-ops.

`metrics_enabled` defaults to `false` and the endpoint defaults to the local collector (`http://localhost:4317` for a native binary, `http://otel-collector:4317` inside Compose), so enabling is a single flag. `oneclick.sh` starts the collector and Prometheus and runs the API with metrics enabled; the `monitoring` Compose profile adds Grafana. Metrics are then at `http://localhost:9898/metrics` on the collector and in Prometheus at `http://localhost:9090`.

Environment variables follow the usual prefix, e.g. `DECISION_ENGINE__LOG__TELEMETRY__METRICS_ENABLED=true` and `DECISION_ENGINE__LOG__TELEMETRY__OTEL_EXPORTER_OTLP_ENDPOINT=...`.

### Rate Limiting

```toml
[limit]
request_count = 1
duration = 60
```

Controls rate limiting on the delete APIs. `request_count` requests allowed per `duration` seconds.

### Redis cache config

```toml
[cache_config]
service_config_redis_prefix = "DE_service_config_"
service_config_ttl = 300    # Redis TTL for service config entries, in seconds
```

### Database

MySQL and PostgreSQL use separate config sections.

**MySQL:**

```toml
[database]
username = "db_user"
password = "db_pass"
host = "localhost"
port = 3306
dbname = "decision_engine_db"
```

**PostgreSQL:**

```toml
[pg_database]
pg_username = "db_user"
pg_password = "db_pass"
pg_host = "localhost"
pg_port = 5432
pg_dbname = "decision_engine_db"
```

For Docker Compose runs both are pre-wired via service names in `config/docker-configuration.toml`.

### Multi-Tenant Schema

```toml
[tenant_secrets]
public = { schema = "public" }
```

Maps tenant identifiers to database schemas. The shipped config files (`config/development.toml`, `config/docker-configuration.toml`) define only the `public` tenant — add entries for any additional tenant you want to support.

Some routes resolve the tenant from an `x-tenant-id` request header rather than the authenticated merchant, and reject the request outright if it's missing (`TE_03`). Send `x-tenant-id: public` on `GET /health/diagnostics`, every `GET /analytics/*` route, and `POST /gateway-score/reset` — see [API Guide](https://github.com/juspay/decision-engine/blob/main/docs/api-refs/api-ref.mdx#environment-setup).

### Redis

```toml
[redis]
host = "127.0.0.1"
port = 6379
```

Redis is required — it's used for caching routing config and service config. For Docker, use the service name as host.

### Auth

```toml
[user_auth]
jwt_secret = "change_me_in_production_use_32chars!!"
jwt_expiry_seconds = 86400
email_verification_enabled = false
jwt_revocation_cache_ttl_ms = 0
```

Use a strong, random `jwt_secret` — 32+ characters recommended. Set `email_verification_enabled = true` if you've wired an email provider.

`jwt_revocation_cache_ttl_ms` caches "this token is not revoked" in memory, so authenticated
requests skip the JWT denylist read in Redis for that many milliseconds. The cost is that
**a logged-out session keeps working until the window expires**. Only the confirmed
"not revoked" answer is cached — a denylisted token is refused, and a failed Redis read isn't
stored — so an outage can't extend a revoked session. Token expiry (`exp`) is unaffected.
`0` (the default) reads Redis on every request; `1000` bounds revocation to a second.

### Admin Secret

```toml
[admin_secret]
secret = "test_admin"
```

Used to authenticate the `POST /merchant-account/create` endpoint (admin bootstrap). Change this in production.

### API Key Auth

```toml
api_key_auth_enabled = true
```

Top-level flag. When `true`, protected routes accept `x-api-key` in addition to JWT bearer tokens. When `false`, the auth middleware allows **all requests through without authentication** — this effectively disables auth for every protected route. Do not set this to `false` in any environment that should enforce auth.

### Analytics

Both Kafka and ClickHouse require `enabled = true` — without it, analytics is disabled even if the connection details are configured.

```toml
[analytics.kafka]
enabled = true
brokers = "localhost:9092"
api_topic = "api"
domain_topic = "domain"

[analytics.clickhouse]
enabled = true
url = "http://localhost:8123"
user = "decision_engine"
password = "decision_engine"
```

Decision outcomes are published to Kafka and consumed into ClickHouse. Both are required for analytics and audit dashboard views. For Docker runs, these are pre-configured and enabled via the Compose profiles.

### SR v3 cold-start prior

A new SR v3 window starts with `bucketSize` synthetic successes, so a new or sparse segment (for
example one split by country or card network) ranks every gateway at ~100% until real outcomes
replace the seed. With the cold-start prior, a gateway's score in such a segment is shrunk toward
the gateway's merchant-wide success rate `p`:

```
score = (m·p + real successes) / (m + real outcomes)
```

Synthetic seed and reset entries are written as `1.0` / `0.0` (read exactly like `1` / `0` by the
existing score path) and are not counted as real outcomes. Windows written before this change
count as real, which reproduces the existing score for them.

Two per-merchant feature flags (FeatureConf service configs, like the other SR v3 flags), both off
by default:

| Flag | Effect |
|---|---|
| `ENABLE_SR_V3_COLD_START_PRIOR` | Scores gateways with a usable prior by the posterior above (Beta sampling and the sigma bonus use the posterior too). The merchant must be listed explicitly in the flag's `merchants`, with its exact merchant id, so the background job can find it. A `rollout` below 100 applies the prior to that share of decisions, as for other flags. |
| `ENABLE_SR_V3_LEARNED_PRIOR_STRENGTH` | Learns `m` per gateway by empirical Bayes from ClickHouse outcomes. No effect on routing unless the cold-start flag is also on. |

With both off, scoring is unchanged. With only the cold-start flag, `m` is `defaultPriorStrength`
from the merchant's success-rate config (`/rule/create`, `/rule/update`), else the default config's,
else 20. With both on, a learned `m` is used where the fit succeeded and the configured `m`
elsewhere.

Priors are computed off the request path by the `[sr_auto_calibration]` job (it runs for these
merchants whether or not `autopilot_enabled` is set) and stored in the job-owned service config
`SR_V3_PRIOR_<merchant_id>`; the merchant's own `SR_V3_INPUT_CONFIG_*` is never modified. Parent
rates come from a short recent window (gateway health changes quickly); the prior strength is
fitted over a long window (segment heterogeneity changes slowly). A gateway with fewer than
`prior_min_parent_observations` recent outcomes shrinks toward the payment method's pooled rate
across all of the merchant's gateways. All gateways keep the existing score when the snapshot is
missing, older than `prior_max_age_secs` (never more than 7 days, also enforced by the decider),
for another merchant, malformed, or analytics is disabled. The decider keeps the decoded snapshot
in process for 5 seconds. The prior strength is not
learned when the merchant's SR key splits by `card_is_in` or UDFs, which decision events do not
record, or when fewer than `prior_min_segments` segments have `prior_min_segment_observations`
outcomes; the reason is stored on the entry (`fitError`).

```toml
[sr_auto_calibration]
prior_parent_lookback_secs = 3600      # window for parent (and pooled) success rates
prior_fit_lookback_secs = 86400        # window for the learned prior strength
prior_max_age_secs = 21600             # snapshot validity; older snapshots are ignored
prior_min_parent_observations = 30     # recent outcomes before a gateway gets its own prior
prior_min_segments = 10                # segments needed to learn m
prior_min_segment_observations = 5     # outcomes before a segment enters the fit
```

Known limits (see the evaluation): right after a gateway-wide degradation the parent rate lags by
up to one refresh interval plus the parent window, so sparse segments can stay on the degraded
gateway longer than with the existing score (a check that weakens the prior when recent outcomes
contradict it was evaluated and did not close this gap); and when a snapshot expires (the job
stopped), the existing score's exploration of never-tried windows starts at that point, which
costs more than it would have at the start. Priors apply again as soon as the job writes a new
snapshot.

The values shown are the defaults. `examples/sr_v3_cold_start_eval.rs` is an offline evaluation
on synthetic data (`cargo run --release --example sr_v3_cold_start_eval`).

### TLS

```toml
[tls]
certificate = "cert.pem"
private_key = "key.pem"
```

Paths to PEM-format certificate and key files. Only needed if you're terminating TLS at the app layer rather than a reverse proxy.

### API Client

```toml
[api_client]
client_idle_timeout = 90
pool_max_idle_per_host = 10
identity = ""
```

Controls the outbound HTTP client used for upstream calls. Set `identity` to a PEM path if you need mTLS.

## Secrets Management

By default, secrets in config are stored in plaintext. For production, use one of the two supported backends.

### AWS KMS

Requires the `kms-aws` feature (included in the `release` feature set).

```toml
[secrets_management]
secrets_manager = "aws_kms"

[secrets_management.aws_kms]
key_id = "your-kms-key-id"
region = "us-east-1"
```

### HashiCorp Vault

Requires the `kms-hashicorp-vault` feature.

```toml
[secrets_management]
secrets_manager = "hashi_corp_vault"

[secrets_management.hashi_corp_vault]
url = "http://127.0.0.1:8200"
token = "hvs.your_token"
```

When a secrets manager is configured, sensitive fields like `database.password` and `user_auth.jwt_secret` are resolved from the vault rather than the config file.

## Environment Overrides

Selected values can be overridden at runtime via environment variables. This is useful in Helm deployments via `extraEnvVars`. Consult `src/config.rs` for the full mapping.

## Related Docs

- [Installation](https://github.com/juspay/decision-engine/blob/main/docs/installation.md)
- [Local Setup Guide](https://github.com/juspay/decision-engine/blob/main/docs/local-setup.md)
- [API Guide](https://github.com/juspay/decision-engine/blob/main/docs/api-refs/api-ref.mdx)
- [API Reference](https://github.com/juspay/decision-engine/blob/main/docs/api-reference.md)
