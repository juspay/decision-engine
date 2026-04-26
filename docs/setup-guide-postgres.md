# PostgreSQL Setup Guide

This page provides PostgreSQL-focused commands. The full end-to-end setup (CLI, Docker, Compose, Helm) is in [local-setup.md](local-setup.md).

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
