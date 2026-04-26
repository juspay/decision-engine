# API Documentation Map

Decision Engine keeps API documentation in two complementary forms: example-first guides for integration work, and OpenAPI-backed endpoint pages for schema lookup.

## Start here

| Page | Use it for |
| --- | --- |
| [API Examples](api-refs/api-ref.mdx) | Copy-paste cURL flows, request variants, rule examples, merchant/API-key CRUD, decision calls, debit routing, analytics, and audit. |
| [OpenAPI Overview](api-reference.md) | Schema-oriented endpoint navigation backed by `docs/openapi.json`. |
| [OpenAPI contract](openapi.json) | Machine-readable contract used by Mintlify and API tooling. |

## Local docs URLs

When running the dashboard/docs Compose profile, docs are available through the local Nginx route:

```text
http://localhost:8081/introduction
http://localhost:8081/api-overview
http://localhost:8081/api-refs/api-ref
http://localhost:8081/api-reference
```

## Deployed docs URLs

For a deployed docs/dashboard host, use the same paths under the deployed base URL:

```text
https://<docs-host>/introduction
https://<docs-host>/api-overview
https://<docs-host>/api-refs/api-ref
https://<docs-host>/api-reference
```

If the dashboard is hosted under a prefix such as `/decision-engine/`, the docs proxy should preserve the same page paths under that prefix.

## Route access classes

| Access class | Routes | Authentication |
| --- | --- | --- |
| Public | `GET /health`, `GET /health/ready`, `GET /health/diagnostics`, `POST /auth/signup`, `POST /auth/login` | None |
| Admin bootstrap | `POST /merchant-account/create` | Admin secret configured for the deployment |
| Protected | Routing, decision, score update, rule config, API key, merchant read/delete, analytics, audit, config, and authenticated auth routes | `Authorization: Bearer <jwt_token>` or `x-api-key: <api_key>` |
| Sandbox | Same routes through `https://sandbox.hyperswitch.io` | Same auth rules plus `x-feature: decision-engine` |

## Common flows

| Flow | Primary docs |
| --- | --- |
| Merchant CRUD | [API Examples: Merchant CRUD](api-refs/api-ref.mdx#merchant-crud) |
| API key CRUD | [API Examples: API Key CRUD](api-refs/api-ref.mdx#api-key-crud) |
| Rule-based routing lifecycle | [Create routing algorithm](api-refs/routing-algorithm-create.mdx) |
| Advanced AND/OR/nested rules | [Advanced rule tree](api-refs/routing-advanced-example.mdx) |
| Decide-gateway transactions | [SR-based routing](api-refs/decide-gateway-sr-based.mdx) |
| Debit routing | [Debit-routing merchant flag](api-refs/merchant-debit-routing.mdx) |
| Analytics and audit | [Analytics endpoints](api-refs/analytics-endpoints.mdx) |
