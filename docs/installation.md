---
title: "Installation"
description: "Everything needed to get Decision Engine running locally, end to end."
---

# Installation Guide

This section covers everything needed to get Decision Engine running locally — from a single `docker compose up` to the full CLI, Docker, Compose, and Helm matrix.

## Quick Start

The fastest path to a running instance. Every service in `docker-compose.yaml` is gated behind a profile, so a profile is required — there is no default/unprofiled bring-up:

```bash
docker compose --profile postgres-ghcr up -d
curl http://localhost:8080/health
```

Expected response:

```json
{ "message": "Health is good" }
```

For the API, dashboard, and docs together, use `--profile dashboard-postgres-ghcr` instead — see [Dashboard](/dashboard-guide).

## In This Section

| Page | Use it for |
| --- | --- |
| [Local Setup](/local-setup) | The canonical guide — Compose profiles, source builds, Docker images without Compose, Helm, and troubleshooting. |
| [PostgreSQL Setup](/setup-guide-postgres) | Postgres-specific Compose profiles, `make` targets, and verification. |
| [MySQL Setup](/setup-guide-mysql) | The same, for MySQL. |
| [Configuration](/configuration) | Config file reference and environment variable overrides once the service is up. |
| [Dashboard](/dashboard-guide) | Bring up the React operator dashboard alongside the API. |

## Choosing A Database

Decision Engine supports PostgreSQL and MySQL as interchangeable backends. Pick one and follow its dedicated guide, or go straight to [Local Setup](/local-setup) if you want the full profile matrix (dashboard, monitoring, source builds) rather than a database-first walkthrough.

## Next Steps

- [API Guide](/api-refs/api-ref) — copy-paste `curl` examples once the service is running.
