# Local Setup Guide

This is the canonical local startup guide for Decision Engine.

## Prerequisites

- Docker 20+
- Docker Compose v2+
- Git 2+

Optional for source runs:

- Rust 1.85+
- `just`
- PostgreSQL or MySQL
- Redis

## Runtime Tracks

Decision Engine supports two local tracks:

1. published-image track: pull existing images
2. local-build track: build images or binaries from the current source tree

Default tags used in this repo:

- `DECISION_ENGINE_TAG=v1.4`
- `GROOVY_RUNNER_TAG=v1.4`

## Docker Compose Profiles

You must pass at least one profile.

### Core runtime profiles

| Profile | DB | Includes |
|---|---|---|
| `postgres-ghcr` | PostgreSQL | API + PostgreSQL + Redis + PG migrations |
| `postgres-local` | PostgreSQL | API + PostgreSQL + Redis + PG migrations |
| `mysql-ghcr` | MySQL | API + MySQL + Redis + MySQL migrations + routing-config |
| `mysql-local` | MySQL | API + MySQL + Redis + MySQL migrations + routing-config |

### Dashboard profiles

| Profile | DB | Includes |
|---|---|---|
| `dashboard-postgres-ghcr` | PostgreSQL | core PG stack + dashboard + Mintlify docs |
| `dashboard-postgres-local` | PostgreSQL | core PG stack + dashboard + Mintlify docs |
| `dashboard-mysql-ghcr` | MySQL | core MySQL stack + dashboard + Mintlify docs |
| `dashboard-mysql-local` | MySQL | core MySQL stack + dashboard + Mintlify docs |

### Optional profiles

| Profile | Adds |
|---|---|
| `monitoring` | Prometheus + Grafana |
| `groovy-ghcr` | Groovy runner image |
| `groovy-local` | Groovy runner built from local source |

## Fastest Bring-Up

### API Only

```bash
docker compose --profile postgres-ghcr up -d
```

### API + Dashboard + Docs

```bash
docker compose --profile dashboard-postgres-ghcr up -d
```

### With Monitoring

```bash
docker compose --profile postgres-ghcr --profile monitoring up -d
```

## Make Targets

Common wrappers:

```bash
make init-pg-ghcr
make init-pg-local
make init-mysql-ghcr
make init-mysql-local
make run-pg-ghcr
make run-mysql-local
make stop
```

## Source Build And Run

### PostgreSQL

```bash
cargo build --release --no-default-features --features middleware,kms-aws,postgres
just migrate-pg
RUSTFLAGS="-Awarnings" cargo run --no-default-features --features postgres
```

### MySQL

```bash
cargo build --release --features release
RUSTFLAGS="-Awarnings" cargo run --features release
```

## Docker Builds Without Compose

```bash
docker build --platform=linux/amd64 -t decision-engine-mysql:local -f Dockerfile .
docker build --platform=linux/amd64 -t decision-engine-pg:local -f Dockerfile.postgres .
```

Example container run:

```bash
docker run --platform=linux/amd64 \
  -v $(pwd)/config/docker-configuration.toml:/local/config/development.toml \
  -p 8080:8080 \
  decision-engine-pg:local
```

## Helm

Chart location: `helm-charts/`

```bash
cd helm-charts
helm dependency build
helm install my-release .
```

For image overrides, use `image.repository`, `image.version`, and `image.pullPolicy`. Verify the rendered templates directly when troubleshooting chart behavior.

## Verification

```bash
curl http://localhost:8080/health
```

Expected response:

```json
{"message":"Health is good"}
```

Dashboard profiles also expose:

- Dashboard: `http://localhost:8081/dashboard/`
- Docs: `http://localhost:8081/introduction`

Monitoring profile also exposes:

- Prometheus: `http://localhost:9090`
- Grafana: `http://localhost:3000`

## Troubleshooting

### Recreate a profile with clean volumes

```bash
docker compose --profile postgres-ghcr down -v
docker compose --profile postgres-ghcr up -d
```

### Inspect migration jobs

```bash
docker compose logs db-migrator-postgres
docker compose logs db-migrator
```

### Common next files to inspect

- `docker-compose.yaml`
- `config/docker-configuration.toml`
- `src/config.rs`
- `src/app.rs`
