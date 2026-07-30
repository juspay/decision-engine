---
title: "API Reference"
description: "Schema-backed reference for every Decision Engine endpoint, with request/response models and an interactive playground."
---

# API Reference

This is the schema-backed reference for every Decision Engine endpoint. Each page below shows the full request and response model and includes an interactive playground, generated from the OpenAPI contract.

Looking for copy-paste examples and end-to-end flows instead? Start with the [API Guide](https://github.com/juspay/decision-engine/blob/main/docs/api-refs/api-ref.mdx).

## Two ways to read the API

| Surface | Best for |
| --- | --- |
| [API Guide](https://github.com/juspay/decision-engine/blob/main/docs/api-refs/api-ref.mdx) | Task-oriented `curl` examples, complete flows, and request variants. |
| API Reference (this section) | Exact request/response schemas and an interactive playground, one page per endpoint. |

For advanced rule examples — AND, OR, nested AND+OR, `volume_split_priority`, enum arrays, and number-array matching — see the [Advanced Routing Example](https://github.com/juspay/decision-engine/blob/main/docs/api-refs/routing-advanced-example.mdx). For the exact `POST /routing/create` schema, use [Create Routing Rule](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/createRoutingRule.mdx).

## Access classes

| Class | Routes | Authentication |
| --- | --- | --- |
| Public | `GET /health`, `GET /health/ready`, `GET /health/diagnostics`, `POST /auth/login` | None |
| Admin bootstrap | `POST /merchant-account/create`, `POST /auth/signup` | Admin secret |
| Protected | All routing, decision, score update, rule config, API key, merchant read/delete, analytics, audit, config, and authenticated auth routes | `Authorization: Bearer <jwt_token>` or `x-api-key: <api_key>` |
| Sandbox | Any Decision Engine route served through `https://sandbox.hyperswitch.io` | Same auth rules plus `x-feature: decision-engine` |

## Endpoint Families

### Health

- [Health Check](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/healthCheck.mdx)
- [Health Ready](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/healthReady.mdx)
- [Health Diagnostics](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/healthDiagnostics.mdx)

### Auth And Onboarding

- [Signup](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/signup.mdx)
- [Login](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/login.mdx)
- [Logout](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/logout.mdx)
- [Current User](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/me.mdx)
- [List User Merchants](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/listUserMerchants.mdx)
- [Switch Merchant](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/switchMerchant.mdx)
- [Onboard Merchant](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/onboardMerchant.mdx)

### API Keys

- [Create API Key](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/createApiKey.mdx)
- [List API Keys](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/listApiKeys.mdx)
- [Revoke API Key](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/revokeApiKey.mdx)

### Merchant Account

- [Create Merchant](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/createMerchant.mdx)
- [Get Merchant](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/getMerchant.mdx)
- [Delete Merchant](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/deleteMerchant.mdx)
- [Get Merchant Debit Routing](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/getMerchantDebitRouting.mdx)
- [Update Merchant Debit Routing](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/updateMerchantDebitRouting.mdx)

### Gateway Decision

- [Decide Gateway](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/decideGateway.mdx)
- [Legacy Decision Gateway](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/legacyDecisionGateway.mdx)
- [Update Gateway Score](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/updateGatewayScore.mdx)
- [Legacy Update Score](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/legacyUpdateScore.mdx)

### Routing Rules

- [Create Routing Rule](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/createRoutingRule.mdx)
- [Activate Routing Rule](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/activateRoutingRule.mdx)
- [Deactivate Routing Rule](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/deactivateRoutingRule.mdx)
- [List Routing Rules](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/listRoutingRules.mdx)
- [Get Active Routing Rule](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/getActiveRoutingRule.mdx)
- [Evaluate Routing Rule](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/evaluateRoutingRule.mdx)
- [Hybrid Routing](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/hybridRouting.mdx)

### Rule Configuration

- [Create Rule Config](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/createRuleConfig.mdx)
- [Get Rule Config](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/getRuleConfig.mdx)
- [Update Rule Config](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/updateRuleConfig.mdx)
- [Delete Rule Config](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/deleteRuleConfig.mdx)

### Config

- [Get Routing Config](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/getRoutingConfig.mdx)
- [Configure SR Dimensions](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/configSrDimension.mdx)

### Analytics

- [Overview](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/analyticsOverview.mdx)
- [Gateway Scores](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/analyticsGatewayScores.mdx)
- [Decisions](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/analyticsDecisions.mdx)
- [Routing Stats](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/analyticsRoutingStats.mdx)
- [Log Summaries](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/analyticsLogSummaries.mdx)
- [Payment Audit](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/analyticsPaymentAudit.mdx)
- [Preview Trace](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/analyticsPreviewTrace.mdx)
- [Cost Savings](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/analyticsCostSavings.mdx)
- [Routing Events](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/analyticsRoutingEvents.mdx)
- [A/B Test Experiment Results](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/analyticsExperimentResults.mdx)
- [A/B Test Experiment Transactions](https://github.com/juspay/decision-engine/blob/main/docs/api-reference/endpoint/analyticsExperimentTransactions.mdx)

## Curl Examples

For local and sandbox smoke-test examples, use [API Examples](https://github.com/juspay/decision-engine/blob/main/docs/api-refs/api-ref.mdx).
