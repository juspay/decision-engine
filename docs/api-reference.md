# API Overview

The canonical OpenAPI contract for the docs site is `docs/openapi.json`. Use this page for schema-oriented navigation. Use [API Examples](./api-refs/api-ref.mdx) for curl-first flows, valid payloads, and route variants.

## Docs surfaces

| Surface | What it is for | Path |
| --- | --- | --- |
| API Examples | Public integration guide with cURL flows, variants, and working payloads. | [`docs/api-refs/`](./api-refs/api-ref.mdx) |
| OpenAPI Endpoint Reference | Schema-backed endpoint pages generated around `docs/openapi.json`. | [`docs/api-reference/endpoint/`](./api-reference/endpoint/createRoutingRule.mdx) |
| OpenAPI contract | Machine-readable contract consumed by Mintlify and tooling. | [`docs/openapi.json`](./openapi.json) |

If you need rule examples such as AND, OR, nested AND+OR, `volume_split_priority`, enum arrays, or number-array matching, start with [Advanced Routing Example](./api-refs/routing-advanced-example.mdx). If you need the exact `POST /routing/create` schema and playground metadata, use [Create Routing Rule](./api-reference/endpoint/createRoutingRule.mdx).

## Access classes

| Class | Routes | Authentication |
| --- | --- | --- |
| Public | `GET /health`, `GET /health/ready`, `GET /health/diagnostics`, `POST /auth/signup`, `POST /auth/login` | None |
| Admin bootstrap | `POST /merchant-account/create` | Admin secret |
| Protected | All routing, decision, score update, rule config, API key, merchant read/delete, analytics, audit, config, and authenticated auth routes | `Authorization: Bearer <jwt_token>` or `x-api-key: <api_key>` |
| Sandbox | Any Decision Engine route served through `https://sandbox.hyperswitch.io` | Same auth rules plus `x-feature: decision-engine` |

## Endpoint Families

### Health

- [Health Check](./api-reference/endpoint/healthCheck.mdx)
- [Health Ready](./api-reference/endpoint/healthReady.mdx)
- [Health Diagnostics](./api-reference/endpoint/healthDiagnostics.mdx)

### Auth And Onboarding

- [Signup](./api-reference/endpoint/signup.mdx)
- [Login](./api-reference/endpoint/login.mdx)
- [Logout](./api-reference/endpoint/logout.mdx)
- [Current User](./api-reference/endpoint/me.mdx)
- [List User Merchants](./api-reference/endpoint/listUserMerchants.mdx)
- [Switch Merchant](./api-reference/endpoint/switchMerchant.mdx)
- [Onboard Merchant](./api-reference/endpoint/onboardMerchant.mdx)

### API Keys

- [Create API Key](./api-reference/endpoint/createApiKey.mdx)
- [List API Keys](./api-reference/endpoint/listApiKeys.mdx)
- [Revoke API Key](./api-reference/endpoint/revokeApiKey.mdx)

### Merchant Account

- [Create Merchant](./api-reference/endpoint/createMerchant.mdx)
- [Get Merchant](./api-reference/endpoint/getMerchant.mdx)
- [Delete Merchant](./api-reference/endpoint/deleteMerchant.mdx)
- [Get Merchant Debit Routing](./api-reference/endpoint/getMerchantDebitRouting.mdx)
- [Update Merchant Debit Routing](./api-reference/endpoint/updateMerchantDebitRouting.mdx)

### Gateway Decision

- [Decide Gateway](./api-reference/endpoint/decideGateway.mdx)
- [Legacy Decision Gateway](./api-reference/endpoint/legacyDecisionGateway.mdx)
- [Update Gateway Score](./api-reference/endpoint/updateGatewayScore.mdx)
- [Legacy Update Score](./api-reference/endpoint/legacyUpdateScore.mdx)

### Routing Rules

- [Create Routing Rule](./api-reference/endpoint/createRoutingRule.mdx)
- [Activate Routing Rule](./api-reference/endpoint/activateRoutingRule.mdx)
- [List Routing Rules](./api-reference/endpoint/listRoutingRules.mdx)
- [Get Active Routing Rule](./api-reference/endpoint/getActiveRoutingRule.mdx)
- [Evaluate Routing Rule](./api-reference/endpoint/evaluateRoutingRule.mdx)
- [Hybrid Routing](./api-reference/endpoint/hybridRouting.mdx)

### Rule Configuration

- [Create Rule Config](./api-reference/endpoint/createRuleConfig.mdx)
- [Get Rule Config](./api-reference/endpoint/getRuleConfig.mdx)
- [Update Rule Config](./api-reference/endpoint/updateRuleConfig.mdx)
- [Delete Rule Config](./api-reference/endpoint/deleteRuleConfig.mdx)

### Config

- [Get Routing Config](./api-reference/endpoint/getRoutingConfig.mdx)
- [Configure SR Dimensions](./api-reference/endpoint/configSrDimension.mdx)

### Analytics

- [Overview](./api-reference/endpoint/analyticsOverview.mdx)
- [Gateway Scores](./api-reference/endpoint/analyticsGatewayScores.mdx)
- [Decisions](./api-reference/endpoint/analyticsDecisions.mdx)
- [Routing Stats](./api-reference/endpoint/analyticsRoutingStats.mdx)
- [Log Summaries](./api-reference/endpoint/analyticsLogSummaries.mdx)
- [Payment Audit](./api-reference/endpoint/analyticsPaymentAudit.mdx)
- [Preview Trace](./api-reference/endpoint/analyticsPreviewTrace.mdx)

## Curl Examples

For local and sandbox smoke-test examples, use [API Examples](./api-refs/api-ref.mdx).
