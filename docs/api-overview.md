# API Documentation Map

Decision Engine keeps API documentation in two complementary forms: example-first guides for integration work, and OpenAPI-backed endpoint pages for schema lookup.

## Start here

| Page | Use it for |
| --- | --- |
| [API Examples](api-refs/api-ref.mdx) | Copy-paste cURL flows, request variants, rule examples, merchant/API-key CRUD, decision calls, debit routing, analytics, and audit. |
| [OpenAPI Overview](api-reference.md) | Schema-oriented endpoint navigation backed by `docs/openapi.json`. |
| [OpenAPI contract](openapi.json) | Machine-readable contract used by Mintlify and API tooling. |

## API examples by route family

| Route family | Example docs |
| --- | --- |
| Health | [Health, readiness, and diagnostics](api-refs/health-check.mdx) |
| Auth and onboarding | [Auth and onboarding](api-refs/auth-and-onboarding.mdx) |
| Merchant CRUD | [Create merchant](api-refs/merchant-account-create.mdx), [get merchant](api-refs/merchant-account-get.mdx), [delete merchant](api-refs/merchant-account-delete.mdx) |
| API key CRUD | [Create, list, and revoke API keys](api-refs/api-keys.mdx) |
| Rule-based routing lifecycle | [Create](api-refs/routing-algorithm-create.mdx), [activate](api-refs/routing-algorithm-activate.mdx), [list](api-refs/routing-algorithm-list.mdx), [list active](api-refs/routing-algorithm-list-active.mdx), [evaluate](api-refs/routing-algorithm-evaluate.mdx), [hybrid](api-refs/routing-hybrid.mdx) |
| Rule algorithm variants | [Single connector](api-refs/routing-single-connector-example.mdx), [priority](api-refs/routing-priority-example.mdx), [volume split](api-refs/routing-volume-split-example.mdx), [advanced AND/OR/nested](api-refs/routing-advanced-example.mdx) |
| Rule config CRUD | [Success-rate create](api-refs/success-rate-config-create.mdx), [get](api-refs/success-rate-config-get.mdx), [update](api-refs/success-rate-config-update.mdx), [delete](api-refs/success-rate-config-delete.mdx), [elimination create](api-refs/elimination-config-create.mdx), [get](api-refs/elimination-config-get.mdx), [update](api-refs/elimination-config-update.mdx), [delete](api-refs/elimination-config-delete.mdx) |
| Decide gateway transactions | [SR-based](api-refs/decide-gateway-sr-based.mdx), [priority-list](api-refs/decide-gateway-pl-based.mdx), [debit/network](api-refs/decide-gateway-debit-routing.mdx), [network + SR hybrid](api-refs/decide-gateway-hybrid-routing.mdx) |
| Score update | [Update gateway score](api-refs/update-gateway-score.mdx), [legacy update score](api-refs/update-score-legacy.mdx) |
| Debit routing | [Merchant debit-routing flag](api-refs/merchant-debit-routing.mdx), [debit transaction request](api-refs/decide-gateway-debit-routing.mdx) |
| Config | [Routing config endpoints](api-refs/config-endpoints.mdx) |
| Analytics and audit | [Analytics endpoints](api-refs/analytics-endpoints.mdx) |
| Compatibility | [Legacy decision endpoint](api-refs/decision-gateway-legacy.mdx), [legacy update score](api-refs/update-score-legacy.mdx) |

## OpenAPI endpoint pages

Use these when you need schema-backed endpoint pages or Mintlify playground metadata:

| Family | OpenAPI docs |
| --- | --- |
| Health | [Health](api-reference/endpoint/healthCheck.mdx), [ready](api-reference/endpoint/healthReady.mdx), [diagnostics](api-reference/endpoint/healthDiagnostics.mdx) |
| Auth and onboarding | [Signup](api-reference/endpoint/signup.mdx), [login](api-reference/endpoint/login.mdx), [logout](api-reference/endpoint/logout.mdx), [me](api-reference/endpoint/me.mdx), [merchants](api-reference/endpoint/listUserMerchants.mdx), [switch merchant](api-reference/endpoint/switchMerchant.mdx), [onboard merchant](api-reference/endpoint/onboardMerchant.mdx) |
| API keys | [Create](api-reference/endpoint/createApiKey.mdx), [list](api-reference/endpoint/listApiKeys.mdx), [revoke](api-reference/endpoint/revokeApiKey.mdx) |
| Merchant account | [Create](api-reference/endpoint/createMerchant.mdx), [get](api-reference/endpoint/getMerchant.mdx), [delete](api-reference/endpoint/deleteMerchant.mdx), [get debit routing](api-reference/endpoint/getMerchantDebitRouting.mdx), [update debit routing](api-reference/endpoint/updateMerchantDebitRouting.mdx) |
| Routing | [Create](api-reference/endpoint/createRoutingRule.mdx), [activate](api-reference/endpoint/activateRoutingRule.mdx), [list](api-reference/endpoint/listRoutingRules.mdx), [active](api-reference/endpoint/getActiveRoutingRule.mdx), [evaluate](api-reference/endpoint/evaluateRoutingRule.mdx), [hybrid](api-reference/endpoint/hybridRouting.mdx) |
| Rule config | [Create](api-reference/endpoint/createRuleConfig.mdx), [get](api-reference/endpoint/getRuleConfig.mdx), [update](api-reference/endpoint/updateRuleConfig.mdx), [delete](api-reference/endpoint/deleteRuleConfig.mdx) |
| Decision and score | [Decide gateway](api-reference/endpoint/decideGateway.mdx), [legacy decision gateway](api-reference/endpoint/legacyDecisionGateway.mdx), [update gateway score](api-reference/endpoint/updateGatewayScore.mdx), [legacy update score](api-reference/endpoint/legacyUpdateScore.mdx) |
| Config | [Routing keys](api-reference/endpoint/getRoutingConfig.mdx), [SR dimensions](api-reference/endpoint/configSrDimension.mdx) |
| Analytics | [Overview](api-reference/endpoint/analyticsOverview.mdx), [gateway scores](api-reference/endpoint/analyticsGatewayScores.mdx), [decisions](api-reference/endpoint/analyticsDecisions.mdx), [routing stats](api-reference/endpoint/analyticsRoutingStats.mdx), [log summaries](api-reference/endpoint/analyticsLogSummaries.mdx), [payment audit](api-reference/endpoint/analyticsPaymentAudit.mdx), [preview trace](api-reference/endpoint/analyticsPreviewTrace.mdx) |

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
