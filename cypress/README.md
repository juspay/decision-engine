# Decision Engine Cypress E2E

This directory owns the end-to-end contract for Decision Engine on the Cypress branch.

The default contract is:
- PostgreSQL only
- analytics-enabled stack
- API + dashboard UI + docs smoke
- two runtime modes:
  - source-run via `oneclick.sh`
  - Docker Compose via `dashboard-postgres-local`

## Main Commands

Install dependencies once:

```bash
npm install
```

Run both required runtime modes:

```bash
npm run test:e2e
```

Run only the source-run stack:

```bash
npm run test:e2e:source
```

Run only the Docker Compose stack:

```bash
npm run test:e2e:docker
```

Run only the Cypress specs against an already-running stack:

```bash
npm run test:all
```

## What The Runner Does

`cypress/scripts/run-e2e.js` owns:
- stack bring-up
- readiness checks
- Cypress invocation
- cleanup
- exit code propagation

### Source mode

- starts `./oneclick.sh` with non-interactive confirmation enabled
- waits for API, UI, docs, Kafka, and ClickHouse readiness
- verifies ClickHouse analytics tables exist
- runs the full Cypress suite

### Docker mode

- builds dashboard assets with `npm --prefix website run build`
- starts `dashboard-postgres-local` with Docker Compose
- waits for API, UI, docs, Kafka, and ClickHouse readiness
- verifies ClickHouse analytics tables exist
- runs the full Cypress suite

## Coverage Baseline

Mandatory API coverage:
- merchant CRUD
- dynamic routing
- rule configuration CRUD
- single / priority / advanced routing
- volume split
- analytics API
- routing mutation regression

Mandatory UI coverage:
- auth/login smoke
- dashboard overview
- analytics page
- payment audit
- decision explorer
- Euclid rules
- volume split page

Mandatory docs/infra smoke:
- docs home page
- API reference page
- local setup/operator page
- ClickHouse analytics schema availability

## Runtime Configuration

The runner passes these environment variables into Cypress:
- `CYPRESS_RUNTIME_MODE`
- `CYPRESS_API_BASE_URL`
- `CYPRESS_UI_BASE_URL`
- `CYPRESS_DOCS_BASE_URL`
- `CYPRESS_CLICKHOUSE_HTTP_URL`
- `CYPRESS_CLICKHOUSE_DATABASE`
- `CYPRESS_CLICKHOUSE_USER`
- `CYPRESS_CLICKHOUSE_PASSWORD`

Specs should stay startup-agnostic and consume runtime information through Cypress env or shared commands.

## Failure Ownership

- stack startup/readiness failure = infra/orchestration regression
- Cypress API/UI failure = product regression
- docs smoke failure = docs-serving regression
