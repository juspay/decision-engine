# Local Development Setup Guide

Complete guide for setting up Decision Engine locally with Docker, Kubernetes (Helm), or from source.

## Table of Contents

- [Prerequisites](#prerequisites)
- [Quick Start](#quick-start)
- [Docker Compose Profiles](#docker-compose-profiles)
- [Setup Modes](#setup-modes)
  - [1. PostgreSQL with Dashboard (Recommended)](#1-postgresql-with-dashboard-recommended)
  - [2. PostgreSQL Only](#2-postgresql-only)
  - [3. MySQL with Dashboard](#3-mysql-with-dashboard)
  - [4. MySQL Only](#4-mysql-only)
  - [5. With Monitoring](#5-with-monitoring)
- [Optional Components](#optional-components)
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

---

## Quick Start

```bash
git clone https://github.com/juspay/decision-engine.git
cd decision-engine
docker compose --profile dashboard-postgres up -d
```

Access:
- **API**: http://localhost:8080/health
- **Dashboard**: http://localhost:8081/dashboard/
- **Documentation**: http://localhost:8081/introduction

---

## Docker Compose Profiles

Profiles control which services are started. **You must specify at least one profile** - nothing runs by default.

| Profile | Database | Dashboard | Redis | Services Started |
|---------|----------|-----------|-------|------------------|
| `postgres` | PostgreSQL | ❌ | ✅ | 4 services |
| `dashboard-postgres` | PostgreSQL | ✅ | ✅ | 6 services |
| `mysql` | MySQL | ❌ | ✅ | 5 services |
| `dashboard-mysql` | MySQL | ✅ | ✅ | 7 services |
| `monitoring` | N/A | Grafana | N/A | Prometheus + Grafana |
| `groovy` | N/A | N/A | N/A | Groovy Runner (optional) |

### Combining Profiles

```bash
# PostgreSQL + Dashboard + Monitoring
docker compose --profile dashboard-postgres --profile monitoring up -d

# PostgreSQL + Groovy Runner
docker compose --profile postgres --profile groovy up -d

# MySQL + Dashboard + Monitoring + Groovy
docker compose --profile dashboard-mysql --profile monitoring --profile groovy up -d
```

---

## Setup Modes

### 1. PostgreSQL with Dashboard (Recommended)

Best for local development with web UI and documentation.

```bash
docker compose --profile dashboard-postgres up -d
```

**Services:**
| Service | Port | Description |
|---------|------|-------------|
| Decision Engine API | 8080 | Main REST API |
| Nginx Proxy | 8081 | Dashboard + Docs proxy |
| PostgreSQL | 5432 | Primary database |
| Redis | 6379 | Cache store |
| Mintlify Docs | 3000 (internal) | Documentation site |
| DB Migrator | N/A | Runs migrations |

**URLs:**
- API: http://localhost:8080/health
- Dashboard: http://localhost:8081/dashboard/
- Docs: http://localhost:8081/introduction

### 2. PostgreSQL Only

Lightweight setup for API-only testing.

```bash
docker compose --profile postgres up -d
```

**Services:**
| Service | Port | Description |
|---------|------|-------------|
| Decision Engine API | 8080 | Main REST API |
| PostgreSQL | 5432 | Primary database |
| Redis | 6379 | Cache store |
| DB Migrator | N/A | Runs migrations |

**URL:** http://localhost:8080/health

### 3. MySQL with Dashboard

Alternative database with full UI stack.

```bash
docker compose --profile dashboard-mysql up -d
```

**Services:**
| Service | Port | Description |
|---------|------|-------------|
| Decision Engine API | 8080 | Main REST API |
| Nginx Proxy | 8081 | Dashboard + Docs proxy |
| MySQL | 3306 | Primary database |
| Redis | 6379 | Cache store |
| Mintlify Docs | 3000 (internal) | Documentation site |
| DB Migrator | N/A | MySQL migrations |
| Routing Config | N/A | Initial config setup |

### 4. MySQL Only

MySQL backend without dashboard.

```bash
docker compose --profile mysql up -d
```

### 5. With Monitoring

Add Prometheus metrics and Grafana dashboards to any profile.

```bash
# With PostgreSQL dashboard
docker compose --profile dashboard-postgres --profile monitoring up -d

# With MySQL only
docker compose --profile mysql --profile monitoring up -d
```

**Additional Services:**
| Service | Port | Description |
|---------|------|-------------|
| Prometheus | 9090 | Metrics collection |
| Grafana | 3000 | Visualization dashboards |

---

## Optional Components

### Groovy Runner

Enables Groovy scripting support (needed for dynamic routing rules).

```bash
# Add to any profile
docker compose --profile postgres --profile groovy up -d
```

**Profile:** `groovy` (pre-built image) or `groovy-local` (build from source)

**Port:** 8085

---

## Configuration

### Environment Variables

Set in `docker-compose.yaml` or create `.env` file:

```bash
# Database URLs
export DATABASE_URL="postgresql://db_user:db_pass@localhost:5432/decision_engine_db"

# Groovy Runner (if using)
export GROOVY_RUNNER_HOST="host.docker.internal:8085"
```

### Configuration Files

| File | Purpose |
|------|---------|
| `config/development.toml` | Local development settings |
| `config/docker-configuration.toml` | Docker deployment settings |

---

## Verifying Installation

### Health Check

```bash
curl http://localhost:8080/health
```

Expected: `{"message":"Health is good"}`

### API Test

```bash
curl -X POST http://localhost:8080/decide-gateway \
  -H "Content-Type: application/json" \
  -d '{
    "merchantId": "test_merchant1",
    "eligibleGatewayList": ["stripe", "adyen"],
    "paymentInfo": {
      "paymentId": "test_123",
      "amount": 100.50,
      "currency": "USD"
    }
  }'
```

### Dashboard Access

Open browser to: http://localhost:8081/dashboard/

---

## Troubleshooting

### Port Already in Use

**Error:** `Port 8080 is already in use`

**Solution:**
```bash
lsof -ti:8080 | xargs kill -9
# or change ports in docker-compose.yaml
```

### Container Conflicts

**Error:** `container name "X" is already in use`

**Solution:**
```bash
docker compose --profile <profile> down
docker system prune -f
docker compose --profile <profile> up -d
```

### Database Connection Failed

**Error:** `Connection refused` or `database does not exist`

**Solutions:**

1. Check database is running:
   ```bash
   docker ps | grep postgres
   ```

2. Verify migrations ran:
   ```bash
   docker logs db-migrator-postgres
   ```

3. Restart with fresh state:
   ```bash
   docker compose --profile postgres down -v
   docker compose --profile postgres up -d
   ```

### Dashboard Shows Old Version

The `website/dist` folder contains old build artifacts.

**Solution:**
```bash
cd website
npm install
npm run build
cd ..
docker restart open-router-nginx
```

### Profile Not Found

**Error:** `service "X" has neither an image nor a build context`

**Cause:** Missing profile flag

**Solution:** Always specify a profile:
```bash
# Wrong
docker compose up -d

# Correct
docker compose --profile postgres up -d
```

---

## Stopping Services

```bash
# Stop specific profile
docker compose --profile dashboard-postgres down

# Stop all profiles and remove volumes
docker compose --profile dashboard-postgres --profile monitoring down -v

# Clean up everything
docker system prune -af --volumes
```

---

## Next Steps

- [API Reference](api-reference.md)
- [Configuration Guide](configuration.md)
- [MySQL Setup Guide](setup-guide-mysql.md)
- [PostgreSQL Setup Guide](setup-guide-postgres.md)
