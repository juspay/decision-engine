# MySQL Setup Guide

Use this page when the task is explicitly MySQL-specific. For the full local matrix, use [Local Setup Guide](local-setup.md).

## Compose Commands

### Published-image track

```bash
export DECISION_ENGINE_TAG=v1.4
docker compose --profile mysql-ghcr up -d
```

### Published-image track with dashboard + docs

```bash
docker compose --profile dashboard-mysql-ghcr up -d
```

### Local-build track

```bash
docker compose --profile mysql-local up -d --build
```

### Local-build track with dashboard + docs

```bash
docker compose --profile dashboard-mysql-local up -d --build
```

## Make Targets

```bash
make init-mysql-ghcr
make init-mysql-local
```

## Source Run

```bash
cargo build --release --features release
RUSTFLAGS="-Awarnings" cargo run --features release
```

## Verify

```bash
curl http://localhost:8080/health
```

Dashboard profiles also expose:

- `http://localhost:8081/dashboard/`
- `http://localhost:8081/introduction`
