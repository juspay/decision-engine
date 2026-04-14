# PostgreSQL Setup Guide

Use this page when the task is explicitly PostgreSQL-specific. For the complete local matrix, use [Local Setup Guide](local-setup.md).

## Compose Commands

### Published-image track

```bash
export DECISION_ENGINE_TAG=v1.4
docker compose --profile postgres-ghcr up -d
```

### Published-image track with dashboard + docs

```bash
docker compose --profile dashboard-postgres-ghcr up -d
```

### Local-build track

```bash
docker compose --profile postgres-local up -d --build
```

### Local-build track with dashboard + docs

```bash
docker compose --profile dashboard-postgres-local up -d --build
```

## Make Targets

```bash
make init-pg-ghcr
make init-pg-local
```

## Source Run

```bash
cargo build --release --no-default-features --features middleware,kms-aws,postgres
just migrate-pg
RUSTFLAGS="-Awarnings" cargo run --no-default-features --features postgres
```

## Verify

```bash
curl http://localhost:8080/health
```

Dashboard profiles also expose:

- `http://localhost:8081/dashboard/`
- `http://localhost:8081/introduction`
