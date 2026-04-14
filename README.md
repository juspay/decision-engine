# Decision Engine

Decision Engine is a Rust service that selects a gateway from an eligible list and updates routing scores from transaction outcomes.

## What Exists In This Repository

- HTTP APIs for:
  - gateway selection: `/decide-gateway`
  - score feedback: `/update-gateway-score`
  - merchant account management: `/merchant-account/*`
  - routing rule management: `/routing/*`
  - rule configuration: `/rule/*`
- PostgreSQL and MySQL runtime tracks
- Docker Compose profiles for API-only, dashboard + docs, and monitoring setups
- Helm chart assets under `helm-charts/`
- A React dashboard served at `/dashboard/` in the `dashboard-*` compose profiles
- Mintlify docs served alongside the dashboard in the `dashboard-*` compose profiles

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

## Local Development Paths

Use `docs/local-setup.md` as the canonical startup guide.

Common options:

- Published PostgreSQL image track:
  `docker compose --profile postgres-ghcr up -d`
- Local PostgreSQL build track:
  `docker compose --profile postgres-local up -d --build`
- Makefile wrappers:
  `make init-pg-ghcr`
  `make init-pg-local`
- PostgreSQL source build:
  `cargo build --release --no-default-features --features middleware,kms-aws,postgres`
- PostgreSQL migration:
  `just migrate-pg`

## Documentation Map

| File | Purpose |
|------|---------|
| `docs/local-setup.md` | Canonical local, Compose, source-run, and Helm guide |
| `docs/configuration.md` | Config files, env overrides, and runtime config model |
| `docs/dashboard.mdx` | Dashboard availability, routes, and local usage |
| `docs/api-reference.md` | API overview and endpoint families |
| `docs/api-reference1.md` | Curl examples and local smoke-test flows |
| `docs/openapi.json` | OpenAPI source used by Mintlify |
| `docs/setup-guide-postgres.md` | PostgreSQL-focused setup commands |
| `docs/setup-guide-mysql.md` | MySQL-focused setup commands |

## Runtime Shape

At a high level:

```text
client or orchestrator
        |
        v
POST /decide-gateway
        |
        v
Decision Engine evaluates the request against merchant config,
routing rules, score data, and eligible gateways
        |
        v
response with the selected gateway and routing metadata
```

Related flows:

- `POST /update-gateway-score` feeds transaction outcomes back into scoring
- `POST /routing/*` manages routing algorithms
- `POST /rule/*` manages service-level routing configuration

## Development Commands

```bash
# lint and compile coverage
just clippy
just check

# tests
cargo test

# postgres migrations
just migrate-pg
```

CI-relevant compile matrix lives in `scripts/ci-checks.sh`.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for contribution workflow and expectations.

## License

Licensed under [GNU AGPL v3.0](LICENSE).
