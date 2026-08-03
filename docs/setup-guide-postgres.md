---
title: "PostgreSQL Setup"
description: "Set up Decision Engine with a PostgreSQL database."
---

# PostgreSQL Setup Guide

This page provides PostgreSQL-focused commands. For the full end-to-end setup — CLI, Docker, Compose, Helm — see the [Local Setup Guide](https://github.com/juspay/decision-engine/blob/main/docs/local-setup.md).

## Docker Compose (GHCR track)

```bash
export DECISION_ENGINE_TAG=v1.4
COMPOSE_PROFILES= docker compose --profile postgres-ghcr up -d
```

With dashboard + docs:

```bash
COMPOSE_PROFILES= docker compose --profile dashboard-postgres-ghcr up -d
```

## Docker Compose (Local build track)

```bash
COMPOSE_PROFILES= docker compose --profile postgres-local up -d --build
```

With dashboard + docs:

```bash
COMPOSE_PROFILES= docker compose --profile dashboard-postgres-local up -d --build
```

## Make targets

```bash
make init-pg-ghcr
make init-pg-local
```

## Verify

```bash
curl http://localhost:8080/health
```

Expected response:

```json
{"message":"Health is good"}
```

## Related Docs

- [Installation](https://github.com/juspay/decision-engine/blob/main/docs/installation.md)
- [Local Setup Guide](https://github.com/juspay/decision-engine/blob/main/docs/local-setup.md)
- [MySQL Setup](https://github.com/juspay/decision-engine/blob/main/docs/setup-guide-mysql.md)
- [Configuration](https://github.com/juspay/decision-engine/blob/main/docs/configuration.md)
