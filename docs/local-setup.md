# Local Setup Guide

This is the canonical setup guide for running Decision Engine locally and for on-prem style validation.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Image Strategy](#image-strategy)
- [Quick Start (Compose)](#quick-start-compose)
- [Docker Compose Profiles](#docker-compose-profiles)
- [Build and Run from CLI (Cargo)](#build-and-run-from-cli-cargo)
- [Build and Run with Docker (Without Compose)](#build-and-run-with-docker-without-compose)
- [Helm Chart Deployment](#helm-chart-deployment)
- [Verification](#verification)
- [Common Commands](#common-commands)
- [Troubleshooting](#troubleshooting)

## Prerequisites

- Docker 20+
- Docker Compose v2+
- Git 2+

Optional for source builds:
- Rust 1.85+
- `just` (recommended)
- PostgreSQL/MySQL + Redis if running without Docker Compose

## Image Strategy

Decision Engine supports two deployment tracks:

1. `ghcr` track (recommended for on-prem): pulls pinned images from GHCR.
2. `local` track: builds images from your current local source.

Default pinned tags:

- `DECISION_ENGINE_TAG=v1.3.4`
- `GROOVY_RUNNER_TAG=v1.3.4`

Override example:

```bash
export DECISION_ENGINE_TAG=v1.3.5
export GROOVY_RUNNER_TAG=v1.3.5
```

## Quick Start (Compose)

```bash
git clone https://github.com/juspay/decision-engine.git
cd decision-engine

# On-prem style run: GHCR pinned image + PostgreSQL
docker compose --profile postgres-ghcr up -d
```

For dashboard + docs:

```bash
docker compose --profile dashboard-postgres-ghcr up -d
```

## Docker Compose Profiles

You must pass at least one profile.

### Core runtime profiles

| Profile | Image Source | DB | Includes |
|---|---|---|---|
| `postgres-ghcr` | GHCR | PostgreSQL | API + PostgreSQL + Redis + PG migrations |
| `postgres-local` | Local build | PostgreSQL | API + PostgreSQL + Redis + PG migrations |
| `mysql-ghcr` | GHCR | MySQL | API + MySQL + Redis + MySQL migrations + routing-config |
| `mysql-local` | Local build | MySQL | API + MySQL + Redis + MySQL migrations + routing-config |

### Dashboard profiles

| Profile | Image Source | DB | Includes |
|---|---|---|---|
| `dashboard-postgres-ghcr` | GHCR | PostgreSQL | Core PG stack + Nginx dashboard + Mintlify docs |
| `dashboard-postgres-local` | Local build | PostgreSQL | Core PG stack + Nginx dashboard + Mintlify docs |
| `dashboard-mysql-ghcr` | GHCR | MySQL | Core MySQL stack + Nginx dashboard + Mintlify docs |
| `dashboard-mysql-local` | Local build | MySQL | Core MySQL stack + Nginx dashboard + Mintlify docs |

### Optional profiles

| Profile | Description |
|---|---|
| `monitoring` | Prometheus + Grafana |
| `groovy-ghcr` | Groovy runner from GHCR (`GROOVY_RUNNER_TAG`) |
| `groovy-local` | Groovy runner built from local `groovy.Dockerfile` |

### Common combinations

```bash
# PostgreSQL (GHCR) + monitoring
docker compose --profile postgres-ghcr --profile monitoring up -d

# PostgreSQL (local build) + dashboard + docs
docker compose --profile dashboard-postgres-local up -d --build

# MySQL (GHCR) + dashboard + docs
docker compose --profile dashboard-mysql-ghcr up -d
```

## Build and Run from CLI (Cargo)

### PostgreSQL build

```bash
cargo build --release --no-default-features --features middleware,kms-aws,postgres
```

Run migrations:

```bash
just migrate-pg
```

Run service:

```bash
RUSTFLAGS="-Awarnings" cargo run --no-default-features --features postgres
```

### MySQL build

```bash
cargo build --release --features release
```

Run service:

```bash
RUSTFLAGS="-Awarnings" cargo run --features release
```

## Build and Run with Docker (Without Compose)

### Build images locally

```bash
# MySQL-target binary image
docker build --platform=linux/amd64 -t decision-engine-mysql:local -f Dockerfile .

# PostgreSQL-target binary image
docker build --platform=linux/amd64 -t decision-engine-pg:local -f Dockerfile.postgres .
```

### Run image

```bash
docker run --platform=linux/amd64 \
  -v $(pwd)/config/docker-configuration.toml:/local/config/development.toml \
  -p 8080:8080 \
  decision-engine-pg:local
```

## Helm Chart Deployment

Chart location: `helm-charts/`

### Install with defaults

```bash
cd helm-charts
helm dependency build
helm install my-release .
```

### Pin GHCR tag explicitly

```bash
helm install my-release . \
  --set image.repository=ghcr.io/juspay/decision-engine/postgres \
  --set image.version=v1.3.4 \
  --set image.pullPolicy=Always
```

### Use local/private registry image

```bash
helm install my-release . \
  --set image.repository=<your-registry>/decision-engine/postgres \
  --set image.version=<your-tag> \
  --set image.pullPolicy=IfNotPresent
```

## Verification

```bash
curl http://localhost:8080/health
```

Expected response:

```json
{"message":"Health is good"}
```

Dashboard/docs (if dashboard profile is used):

- Dashboard: `http://localhost:8081/dashboard/`
- Docs: `http://localhost:8081/introduction`

## Common Commands

Make targets are aligned to ghcr/local tracks:

```bash
# GHCR tracks
make init-pg-ghcr
make init-mysql-ghcr

# Local build tracks
make init-pg-local
make init-mysql-local

# Run one API service (when infra is ready)
make run-pg-ghcr
make run-mysql-local

# Stop everything
make stop
```

## Troubleshooting

### Port conflicts

```bash
lsof -ti:8080 | xargs kill -9
lsof -ti:8081 | xargs kill -9
```

### Recreate stack with clean volumes

```bash
docker compose --profile postgres-ghcr down -v
docker compose --profile postgres-ghcr up -d
```

### Verify migration jobs

```bash
docker compose logs db-migrator-postgres
docker compose logs db-migrator
```
