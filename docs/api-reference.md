# API Reference

Decision Engine exposes a small, JSON-over-HTTP API. The authoritative reference — with request / response examples for every endpoint — lives in **[api-reference1.md](api-reference1.md)**.

## Endpoint map

| Area | Method + path | Purpose |
|---|---|---|
| **Routing decision** | `POST /decide-gateway` | Pick the best gateway for a transaction |
| **Feedback loop** | `POST /update-gateway-score` | Report a transaction outcome so SR stats update |
| **Health** | `GET  /health` | Liveness probe |
| **Rule CRUD** | `POST /rule/create` | Create a routing / elimination config |
| | `POST /rule/get` | Retrieve a config |
| | `POST /rule/update` | Update a config |
| | `POST /rule/delete` | Remove a config |
| **Merchant CRUD** | `POST   /merchant-account/create` | Register a merchant |
| | `GET    /merchant-account/{id}` | Retrieve a merchant |
| | `DELETE /merchant-account/{id}` | Delete a merchant |
| **Priority Logic V2** | `POST /routing/create` | Create a routing algorithm |
| | `POST /routing/evaluate` | Dry-run an algorithm against a payload |
| | `POST /routing/activate` | Make an algorithm the active one |
| | `POST /routing/list/{merchant_id}` | List all algorithms for a merchant |
| | `POST /routing/list/active/{merchant_id}` | List the currently active algorithms |

## Other formats

- **OpenAPI spec** — [`openapi.json`](openapi.json) (machine-readable, importable into Postman / Insomnia / Swagger UI).
- **Postman collection** — [`decision-engine.postman_collection.json`](../decision-engine.postman_collection.json) at the repo root.
- **Hosted docs** — start any `dashboard-*` Docker Compose profile and browse `http://localhost:8081/introduction`.

## Full reference

👉 **[api-reference1.md](api-reference1.md)** — complete endpoint documentation with curl examples and response schemas.
