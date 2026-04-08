# Local Development Setup Guide

Complete guide for setting up Decision Engine locally with Docker, Kubernetes (Helm), or from source.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Quick Start](#quick-start)
- [Setup Modes](#setup-modes)
  - [1. Docker (Recommended)](#1-docker-recommended)
  - [2. Kubernetes with Helm](#2-kubernetes-with-helm)
  - [3. From Source](#3-from-source)
- [Configuration](#configuration)
- [Verifying Installation](#verifying-installation)
- [Troubleshooting](#troubleshooting)

---

## Prerequisites

### Common Requirements

| Tool | Version | Purpose |
|------|---------|---------|
| Git | 2.0+ | Clone repository |
| Docker | 20.0+ | Container runtime |
| Docker Compose | 2.0+ | Multi-container orchestration |

### Platform-Specific Dependencies

#### Ubuntu/Debian

```bash
sudo apt-get update
sudo apt-get install -y pkg-config libssl-dev protobuf-compiler libpq-dev curl git
```

#### macOS

```bash
brew install pkg-config openssl protobuf postgresql curl git
```

### Additional Requirements (From Source)

| Tool | Version | Install Command |
|------|---------|-----------------|
| Rust | 1.85+ | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| Just | Latest | `cargo install just` |
| Diesel CLI | Latest | `cargo install diesel_cli --no-default-features --features postgres` |

### Helm Requirements (Kubernetes)

| Tool | Version | Install Command |
|------|---------|-----------------|
| kubectl | 1.19+ | See [kubernetes.io](https://kubernetes.io/docs/tasks/tools/) |
| Helm | 3.2+ | See [helm.sh](https://helm.sh/docs/intro/install/) |
| Minikube/Kind | Latest | For local K8s cluster |

---

## Quick Start

Fastest way to get Decision Engine running:

```bash
git clone https://github.com/juspay/decision-engine.git
cd decision-engine
make init-pg
```

Verify: `curl http://localhost:8080/health`

---

## Setup Modes

### 1. Docker (Recommended)

#### Option A: Pre-built Images (Fastest)

```bash
make init-pg
```

This pulls pre-built images and starts:
- PostgreSQL database
- Redis cache
- Groovy Runner
- Decision Engine server

#### Option B: Local Build with PostgreSQL

```bash
make init-local-pg
```

Builds from local source and runs with PostgreSQL.

#### Option C: With Monitoring Stack

```bash
make init-pg-monitor
```

Includes Prometheus and Grafana.

| Service | Port |
|---------|------|
| Decision Engine API | 8080 |
| Prometheus | 9090 |
| Grafana | 3000 |
| PostgreSQL | 5432 |
| Redis | 6379 |
| Groovy Runner | 8085 |

#### Option D: MySQL Backend

```bash
make init
```

Uses MySQL instead of PostgreSQL.

#### Docker Compose Profiles

For advanced usage, use profiles directly:

```bash
docker compose up open-router-pg                    # PostgreSQL setup
docker compose --profile local up open-router-pg    # With nginx & docs
docker compose --profile monitoring up              # With Prometheus & Grafana
```

#### Stopping Services

```bash
make stop
# or
docker compose down
```

---

### 2. Kubernetes with Helm

#### Prerequisites

1. Running Kubernetes cluster (minikube, kind, or remote)
2. Helm 3.2+ installed
3. kubectl configured

#### Option A: Using Install Script

```bash
cd helm-charts
./install.sh
```

Custom options:

```bash
./install.sh --name my-decision-engine
./install.sh --values values-postgresql.yaml
```

#### Option B: Manual Installation

```bash
cd helm-charts

helm repo add bitnami https://charts.bitnami.com/bitnami
helm repo update
helm dependency build

helm install my-release .
```

#### Using with External Database

Create `my-values.yaml`:

```yaml
postgresql:
  enabled: false
  hostname: "external-postgres-host"
  auth:
    username: "external_user"
    password: "external_password"
    database: "external_db"

redis:
  enabled: false
  hostname: "external-redis-host"
```

Install:

```bash
helm install my-release . -f my-values.yaml
```

#### Uninstalling

```bash
helm delete my-release
```

---

### 3. From Source

#### Step 1: Clone Repository

```bash
git clone https://github.com/juspay/decision-engine.git
cd decision-engine
```

#### Step 2: Install Rust

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

#### Step 3: Install Dependencies

```bash
cargo install just
cargo install diesel_cli --no-default-features --features postgres
```

#### Step 4: Start Database

Option A - Local PostgreSQL:

```bash
brew services start postgresql    # macOS
sudo systemctl start postgresql   # Linux
```

Option B - Docker:

```bash
docker run -d --name postgres \
  -e POSTGRES_USER=db_user \
  -e POSTGRES_PASSWORD=db_pass \
  -e POSTGRES_DB=decision_engine_db \
  -p 5432:5432 \
  postgres:16
```

Option C - From docker-compose:

```bash
docker compose up -d postgresql redis groovy-runner
```

#### Step 5: Configure Database

Set environment variables:

```bash
export DB_USER="db_user"
export DB_PASSWORD="db_pass"
export DB_HOST="localhost"
export DB_PORT="5432"
export DB_NAME="decision_engine_db"
```

Create database and run migrations:

```bash
just resurrect    # Drop and recreate database
just migrate-pg   # Run PostgreSQL migrations
```

#### Step 6: Configure Application

```bash
cp config/example.toml config/development.toml
```

Edit `config/development.toml` with your settings.

#### Step 7: Run Application

```bash
cargo run --no-default-features --features postgres
```

Or with release optimizations:

```bash
cargo run --release --no-default-features --features postgres
```

---

## Configuration

### Database URLs

The application uses `DATABASE_URL` environment variable or configuration file:

```bash
export DATABASE_URL="postgresql://db_user:db_pass@localhost:5432/decision_engine_db"
```

### Configuration Files

| File | Purpose |
|------|---------|
| `config/development.toml` | Local development |
| `config/docker-configuration.toml` | Docker deployments |
| `helm-charts/values.yaml` | Kubernetes/Helm deployments |

### Key Configuration Options

```toml
[database]
url = "postgresql://db_user:db_pass@localhost:5432/decision_engine_db"

[server]
host = "0.0.0.0"
port = 8080

[redis]
url = "redis://localhost:6379"

[groovy_runner]
host = "localhost:8085"
```

---

## Verifying Installation

### Health Check

```bash
curl http://localhost:8080/health
```

Expected response:
```json
{"message":"Health is good"}
```

### API Test

```bash
curl --location 'http://localhost:8080/decide-gateway' \
--header 'Content-Type: application/json' \
--data '{
    "merchantId": "test_merchant1",
    "eligibleGatewayList": ["PAYU", "RAZORPAY", "PAYTM_V2"],
    "rankingAlgorithm": "SR_BASED_ROUTING",
    "eliminationEnabled": true,
    "paymentInfo": {
        "paymentId": "PAY12345",
        "amount": 100.50,
        "currency": "USD",
        "customerId": "CUST12345",
        "paymentType": "ORDER_PAYMENT",
        "paymentMethodType": "UPI",
        "paymentMethod": "UPI_PAY"
    }
}'
```

### Metrics Check

```bash
curl http://localhost:9094/metrics
```

---

## Troubleshooting

### Common Issues

#### Port Already in Use

**Error:** `Port 8080 is already in use`

**Solution:**
```bash
lsof -ti:8080 | xargs kill -9
```

Or change port in configuration.

#### Database Connection Failed

**Error:** `Connection refused` or `database "decision_engine_db" does not exist`

**Solutions:**

1. Verify PostgreSQL is running:
   ```bash
   pg_isready -h localhost -p 5432
   ```

2. Create database manually:
   ```bash
   createdb -U db_user decision_engine_db
   ```

3. Check credentials in `DATABASE_URL`

#### Redis Connection Failed

**Error:** `Could not connect to Redis`

**Solutions:**

1. Start Redis:
   ```bash
   redis-server
   ```

2. Or via Docker:
   ```bash
   docker run -d -p 6379:6379 redis:7
   ```

#### Diesel CLI Not Found

**Error:** `diesel: command not found`

**Solution:**
```bash
cargo install diesel_cli --no-default-features --features postgres
```

If you see `libpq` errors on macOS:
```bash
brew install libpq
export PKG_CONFIG_PATH="/opt/homebrew/opt/libpq/lib/pkgconfig"
cargo install diesel_cli --no-default-features --features postgres
```

#### Protobuf Compiler Missing

**Error:** `protoc: not found`

**Solution:**
```bash
brew install protobuf          # macOS
sudo apt-get install protobuf-compiler  # Ubuntu/Debian
```

#### OpenSSL Errors

**Error:** `Could not find directory of OpenSSL installation`

**Solution:**
```bash
brew install openssl           # macOS
sudo apt-get install libssl-dev  # Ubuntu/Debian
```

On macOS with Apple Silicon:
```bash
export PKG_CONFIG_PATH="/opt/homebrew/opt/openssl@3/lib/pkgconfig"
```

#### Docker Platform Errors

**Error:** `no matching manifest for linux/arm64`

**Solution:** Use `--platform linux/amd64`:
```bash
docker compose --platform linux/amd64 up
```

Or enable Rosetta in Docker Desktop (macOS).

#### Groovy Runner Health Check Failing

**Error:** Groovy runner container keeps restarting

**Solutions:**

1. Wait for warm-up (10+ seconds)
2. Check logs:
   ```bash
   docker logs open-router-groovy
   ```
3. Verify network connectivity between containers

#### Helm Chart Dependencies

**Error:** `Error: found in requirements.yaml, but missing in charts/`

**Solution:**
```bash
cd helm-charts
helm dependency build
```

#### Migration Failures

**Error:** `Migration failed: database is locked` or dirty migration

**Solution:**
```bash
just resurrect  # Drops and recreates database
just migrate-pg  # Re-run migrations
```

### Getting Help

| Resource | Link |
|----------|------|
| GitHub Issues | https://github.com/juspay/decision-engine/issues |
| Slack | [Join Chat](https://join.slack.com/t/hyperswitch-io/shared_invite/zt-2jqxmpsbm-WXUENx022HjNEy~Ark7Orw) |
| Discussions | https://github.com/juspay/decision-engine/discussions |

---

## Next Steps

- [API Reference](api-reference.md)
- [Configuration Guide](configuration.md)
- [MySQL Setup Guide](setup-guide-mysql.md)
- [PostgreSQL Setup Guide](setup-guide-postgres.md)
