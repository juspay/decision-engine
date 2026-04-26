# Configuration Guide

This document explains how to configure Decision Engine for local and on-prem deployments.

## Primary Config Files

- `config/development.toml`: host/source runs
- `config/docker-configuration.toml`: Docker/Compose runs
- `helm-charts/config/development.toml`: Kubernetes chart template config

## Core Sections

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

Use either MySQL or PostgreSQL as required by your deployment mode.

For Docker Compose profiles, connection details are pre-wired via service names and mounted config.
For source runs, ensure your database URL in config matches your local DB.

### Redis

```toml
[redis]
host = "127.0.0.1"
port = 6379
```

### Secrets Manager

`secrets_manager` controls encryption/key-management behavior. In local environments this is commonly set to `no_encryption`.

## Environment Overrides

Use environment variables to override selected runtime values when needed (for example in Helm via `extraEnvVars`).

For deployment-specific examples, see:

- [Local Setup Guide](local-setup.md)
- [Helm Chart README](https://github.com/juspay/decision-engine/blob/main/helm-charts/README.md)

## Related Docs

- [Local Setup Guide](local-setup.md)
- [API Reference](api-reference.md)
- [API Examples](api-refs/api-ref.mdx)
