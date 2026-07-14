# Running Decision Engine locally

Only do this if the user does **not** already have a Decision Engine instance to
point the orchestrator at. If they do (self-hosted, staging, or Hyperswitch
sandbox), skip straight to bootstrapping the merchant + API key against that host.

## 1. Get the service running

The fastest path is Docker Compose with a Postgres profile.

```bash
git clone https://github.com/juspay/decision-engine.git
cd decision-engine

# API only, prebuilt images, Postgres:
docker compose --profile postgres-ghcr up -d

# API + dashboard + docs (nice for exploring):
docker compose --profile dashboard-postgres-ghcr up -d
```

The API comes up at `http://localhost:8080`. With the dashboard profile you also
get the dashboard at `http://localhost:8081/dashboard/` and API docs at
`http://localhost:8081/api-overview`.

Profile cheat sheet (`--profile <name>`):

| Profile | What you get |
| --- | --- |
| `postgres-ghcr` / `mysql-ghcr` | API only, prebuilt image, Postgres/MySQL. |
| `postgres-local` / `mysql-local` | API only, built from local source. |
| `dashboard-postgres-ghcr` (etc.) | API + dashboard + docs + nginx. |
| `monitoring` | Prometheus/Grafana. |

There is also a guided `./oneclick.sh` in the repo root that provisions
dependencies (Postgres, Redis, ClickHouse, Kafka) and opens the docs — handy for
a from-source run. From-source needs Rust 1.85+, a DB, and Redis; see the repo
README.

Verify:

```bash
curl -sS http://localhost:8080/health          # 200
curl -sS http://localhost:8080/health/ready     # 200 once DB + Redis are ready
```

## 2. Bootstrap a merchant (gets you an API key)

Merchant creation is the admin-bootstrap route. The local default admin secret in
`config/development.toml` is `test_admin` (override for any real deployment).

```bash
export BASE_URL=http://localhost:8080

curl -sS --location "$BASE_URL/merchant-account/create" \
  --header "x-admin-secret: test_admin" \
  --header "Content-Type: application/json" \
  --data '{
    "merchant_id": "merchant_demo",
    "gateway_success_rate_based_decider_input": null
  }'
```

The response includes an `api_key` (prefixed `DE_`). Save it — that's the
`x-api-key` your orchestrator will send:

```json
{ "message": "Merchant account created successfully",
  "merchant_id": "merchant_demo", "api_key": "DE_..." }
```

Need another key later:

```bash
curl -sS "$BASE_URL/api-key/create" \
  --header "x-api-key: DE_..." \
  --header "Content-Type: application/json" \
  --data '{ "merchant_id": "merchant_demo", "description": "orchestrator key" }'
```

## 3. Give the merchant something to route with

`SR_BASED_ROUTING` needs a success-rate config, and you generally want an
eligible gateway set. Create a success-rate config:

```bash
export AUTH="x-api-key: DE_..."

curl -sS --location "$BASE_URL/rule/create" \
  --header "$AUTH" \
  --header "Content-Type: application/json" \
  --data '{
    "merchant_id": "merchant_demo",
    "config": {
      "type": "successRate",
      "data": {
        "defaultBucketSize": 20,
        "defaultLatencyThreshold": null,
        "defaultHedgingPercent": null,
        "subLevelInputConfig": {
          "paymentMethodType": { "CARD": { "bucketSize": 30, "hedgingPercent": 0.05 } }
        }
      }
    }
  }'
```

Optionally create + activate a priority routing rule (for `PL_BASED_ROUTING`) via
`POST /routing/create` then `POST /routing/activate` — see
`docs/api-refs/routing-algorithm-*.mdx` in the Decision Engine repo.

## 4. Smoke test the round trip

Use [../scripts/smoke_test.sh](../scripts/smoke_test.sh):

```bash
DECISION_ENGINE_URL=http://localhost:8080 \
ADMIN_SECRET=test_admin \
MERCHANT_ID=merchant_demo \
bash scripts/smoke_test.sh
```

It bootstraps (if needed), creates an SR config, calls `decide-gateway`, then
`update-gateway-score`, and prints a PASS/FAIL summary. When it passes, hand the
`DECISION_ENGINE_URL`, `x-api-key`, and `merchant_id` to Phase 4 of the main
workflow.

## Notes

- Fresh merchants have no SR history, so early `decide-gateway` calls fall back
  to configured priority/first-eligible until `update-gateway-score` feedback
  accumulates. This is expected — the model warms up.
- For anything beyond local testing, change the admin secret, use real secrets
  management, and put the service behind TLS. The defaults here are for local dev.
