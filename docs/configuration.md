# Configuration Guide

This page describes the runtime configuration model used by Decision Engine.

## How Configuration Is Loaded

The application loads config in this order:

1. file selected by `APP_ENV`
2. environment overrides with the `DECISION_ENGINE__` prefix

From `src/config.rs`:

- `APP_ENV=dev` or unset -> `config/development.toml`
- `APP_ENV=sandbox` -> `config/sandbox.toml`
- `APP_ENV=production` -> `config/production.toml`

Environment overrides use `__` as the separator.

Example:

```bash
DECISION_ENGINE__SERVER__PORT=8080
DECISION_ENGINE__METRICS__PORT=9090
```

## Primary Files

- `config.example.toml`: sample config
- `config/development.toml`: source-run default
- `config/docker-configuration.toml`: Compose-mounted config
- `src/config.rs`: actual config structs and load rules

For deployment behavior, also inspect:

- `docker-compose.yaml`
- `helm-charts/templates/*`

## Main Config Sections

The runtime config model in `src/config.rs` includes:

- `log`
- `server`
- `metrics`
- `database` or `pg_database`
- `redis`
- `cache_config`
- `tenant_secrets`
- `tls`
- `api_client`
- `routing_config`
- `pm_filters`
- `debit_routing_config`
- `compression_filepath`

## Common Areas

### Server

```toml
[server]
host = "0.0.0.0"
port = 8080
```

### Metrics

```toml
[metrics]
host = "0.0.0.0"
port = 9090
```

### Logging

```toml
[log.console]
enabled = true
level = "DEBUG"
log_format = "default"
```

### Database

Use one backend path:

- MySQL via `[database]`
- PostgreSQL via `[pg_database]`

### Redis

```toml
[redis]
host = "127.0.0.1"
port = 6379
```

### TLS

TLS is optional and configured through the `tls` section.

### Tenant Config

Tenant-aware behavior is driven by `tenant_secrets` and tenant-specific app-state wiring in `src/tenant.rs`.

## Deployment Notes

- Source runs default to `config/development.toml`
- Compose mounts `config/docker-configuration.toml` at `/local/config/development.toml`
- Helm behavior should be verified against the chart templates directly

## Related Docs

- [Local Setup Guide](local-setup.md)
- [PostgreSQL Setup Guide](setup-guide-postgres.md)
- [MySQL Setup Guide](setup-guide-mysql.md)
- [API Overview](api-reference.md)
