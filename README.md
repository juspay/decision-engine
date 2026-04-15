# Decision Engine

<div align="center">

<img src="https://img.shields.io/badge/Rust-1.85%2B-orange?style=for-the-badge&logo=rust&logoColor=white" alt="Rust 1.85+"/>
<img src="https://img.shields.io/badge/License-AGPL%20v3-blue?style=for-the-badge" alt="AGPL v3"/>
<img src="https://img.shields.io/badge/PostgreSQL-%26%20MySQL-336791?style=for-the-badge" alt="PostgreSQL and MySQL"/>
<img src="https://img.shields.io/badge/Docker%20Compose-Local%20Profiles-2496ED?style=for-the-badge&logo=docker&logoColor=white" alt="Docker Compose profiles"/>
<img src="https://img.shields.io/badge/Dashboard-React-61DAFB?style=for-the-badge&logo=react&logoColor=111827" alt="React dashboard"/>

Rust routing service for selecting a gateway from an eligible list and recording outcome feedback used by routing decisions.

**[Quick Start](#quick-start)** •
**[Docs Map](#docs-map)** •
**[Dashboard](#dashboard--docs)** •
**[Development](#development-commands)** •
**[Contributing](#contributing)**

</div>

## What Ships In This Repository

- HTTP APIs for gateway selection, routing configuration, rule configuration, and merchant account configuration
- PostgreSQL and MySQL runtime tracks
- Docker Compose profiles for API-only, dashboard, docs, and monitoring flows
- Helm chart assets under [`helm-charts/`](helm-charts/)
- A React dashboard under [`website/`](website/) served at `/dashboard/` in the `dashboard-*` Compose profiles
- An Analytics surface under `/analytics` in the dashboard
- Mintlify docs under [`docs/`](docs/) served alongside the dashboard in the `dashboard-*` Compose profiles

## Start Here

| If you want to... | Open this first |
|---|---|
| Run the API locally | [`docs/local-setup.md`](docs/local-setup.md) |
| Understand configuration | [`docs/configuration.md`](docs/configuration.md) |
| Browse the API surface | [`docs/api-reference.md`](docs/api-reference.md) |
| Use ready-made curl flows | [`docs/api-reference1.md`](docs/api-reference1.md) |
| Inspect request and response schemas | [`docs/openapi.json`](docs/openapi.json) |
| Bring up the dashboard | [`docs/dashboard.mdx`](docs/dashboard.mdx) |
| Inspect analytics | [`docs/analytics.mdx`](docs/analytics.mdx) |
| Inspect Kubernetes/on-prem assets | [`helm-charts/`](helm-charts/) |

## Quick Start

### API Only

```bash
git clone https://github.com/juspay/decision-engine.git
cd decision-engine
docker compose --profile postgres-ghcr up -d
curl http://localhost:8080/health
```

Expected response:

```json
{"message":"Health is good"}
```

### Dashboard + Docs

```bash
docker compose --profile dashboard-postgres-ghcr up -d
```

Available URLs:

- API: `http://localhost:8080`
- Dashboard: `http://localhost:8081/dashboard/`
- Docs: `http://localhost:8081/introduction`

For source builds, Helm installs, and MySQL-specific flows, use [`docs/local-setup.md`](docs/local-setup.md).

## Docs Map

| Path | What it is for |
|---|---|
| [`docs/introduction.mdx`](docs/introduction.mdx) | Product-level docs landing page |
| [`docs/local-setup.md`](docs/local-setup.md) | Canonical local, Compose, source-run, and Helm setup guide |
| [`docs/configuration.md`](docs/configuration.md) | Config files, env overrides, and runtime config model |
| [`docs/dashboard.mdx`](docs/dashboard.mdx) | Dashboard routes, availability, and serving model |
| [`docs/analytics.mdx`](docs/analytics.mdx) | Analytics page, data sources, and operator scope |
| [`docs/payment-audit.mdx`](docs/payment-audit.mdx) | Per-payment audit search and timeline view |
| [`docs/api-reference.md`](docs/api-reference.md) | API overview grouped by endpoint family |
| [`docs/api-reference1.md`](docs/api-reference1.md) | Local curl examples and smoke-test flows |
| [`docs/openapi.json`](docs/openapi.json) | OpenAPI source consumed by the docs site |
| [`docs/setup-guide-postgres.md`](docs/setup-guide-postgres.md) | PostgreSQL-focused setup commands |
| [`docs/setup-guide-mysql.md`](docs/setup-guide-mysql.md) | MySQL-focused setup commands |

## Runtime Shape

```text
client or orchestrator
        |
        v
POST /decide-gateway
        |
        v
Decision Engine evaluates eligible gateways against merchant config,
routing rules, and stored score data
        |
        v
response with selected gateway and routing metadata
```

Related flows:

- `POST /update-gateway-score` records transaction outcomes used by routing
- `POST /routing/*` manages routing algorithms and routing metadata
- `POST /rule/*` manages service-level rule configuration
- `POST /merchant-account/*` manages merchant account configuration

## Dashboard & Docs

When the `dashboard-*` Compose profiles are running, Nginx serves:

- the React dashboard at `/dashboard/`
- Mintlify docs at `/introduction`
- built frontend assets from `website/dist`

Documented dashboard routes include:

- `/dashboard/`
- `/dashboard/routing`
- `/dashboard/routing/sr`
- `/dashboard/routing/rules`
- `/dashboard/routing/volume`
- `/dashboard/routing/debit`
- `/dashboard/decisions`
- `/dashboard/analytics`
- `/dashboard/audit`

See [`docs/dashboard.mdx`](docs/dashboard.mdx), [`website/src/App.tsx`](website/src/App.tsx), and [`nginx/nginx.conf`](nginx/nginx.conf).

## Development Commands

```bash
# lint
just clippy

# compile matrix checks
just check

# tests
cargo test

# postgres migrations
just migrate-pg
```

CI-sensitive compile and lint coverage is driven by [`scripts/ci-checks.sh`](scripts/ci-checks.sh) and [`.github/workflows/`](.github/workflows/).

## Repository Pointers

- Runtime entrypoint: [`src/bin/open_router.rs`](src/bin/open_router.rs)
- Router and middleware wiring: [`src/app.rs`](src/app.rs)
- API handlers: [`src/routes/`](src/routes/)
- Config loading: [`src/config.rs`](src/config.rs)
- Tenant state wiring: [`src/tenant.rs`](src/tenant.rs)
- Frontend dashboard: [`website/`](website/)
- Local deployment topology: [`docker-compose.yaml`](docker-compose.yaml)
- Kubernetes assets: [`helm-charts/`](helm-charts/)

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for contribution workflow and expectations.

## License

Licensed under [GNU AGPL v3.0](LICENSE).
