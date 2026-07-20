---
title: "API Reference"
description: "Schema-backed reference for every Decision Engine endpoint, with request/response models and an interactive playground."
---

# API Reference

This is the schema-backed reference for every Decision Engine endpoint. Each page below shows the full request and response model and includes an interactive playground, generated from the OpenAPI contract.

Looking for copy-paste examples and end-to-end flows instead? Start with the [API Guide](/api-refs/api-ref).

## Two ways to read the API

| Surface | Best for |
| --- | --- |
| [API Guide](/api-refs/api-ref) | Task-oriented `curl` examples, complete flows, and request variants. |
| API Reference (this section) | Exact request/response schemas and an interactive playground, one page per endpoint. |

For advanced rule examples — AND, OR, nested AND+OR, `volume_split_priority`, enum arrays, and number-array matching — see the [Advanced Routing Example](/api-refs/routing-advanced-example). For the exact `POST /routing/create` schema, use [Create Routing Rule](/api-reference/endpoint/createRoutingRule).

## Access classes

| Class | Routes | Authentication |
| --- | --- | --- |
| Public | `GET /health`, `GET /health/ready`, `GET /health/diagnostics`, `POST /auth/signup`, `POST /auth/login` | None |
| Admin bootstrap | `POST /merchant-account/create` | Admin secret |
| Protected | All routing, decision, score update, rule config, API key, merchant read/delete, analytics, audit, config, and authenticated auth routes | `Authorization: Bearer <jwt_token>` or `x-api-key: <api_key>` |
| Sandbox | Any Decision Engine route served through `https://sandbox.hyperswitch.io` | Same auth rules plus `x-feature: decision-engine` |

## Endpoint Families

### Health

- [Health Check](/api-reference/endpoint/healthCheck)
- [Health Ready](/api-reference/endpoint/healthReady)
- [Health Diagnostics](/api-reference/endpoint/healthDiagnostics)

### Auth And Onboarding

- [Signup](/api-reference/endpoint/signup)
- [Login](/api-reference/endpoint/login)
- [Logout](/api-reference/endpoint/logout)
- [Current User](/api-reference/endpoint/me)
- [List User Merchants](/api-reference/endpoint/listUserMerchants)
- [Switch Merchant](/api-reference/endpoint/switchMerchant)
- [Onboard Merchant](/api-reference/endpoint/onboardMerchant)

### API Keys

- [Create API Key](/api-reference/endpoint/createApiKey)
- [List API Keys](/api-reference/endpoint/listApiKeys)
- [Revoke API Key](/api-reference/endpoint/revokeApiKey)

### Merchant Account

- [Create Merchant](/api-reference/endpoint/createMerchant)
- [Get Merchant](/api-reference/endpoint/getMerchant)
- [Delete Merchant](/api-reference/endpoint/deleteMerchant)
- [Get Merchant Debit Routing](/api-reference/endpoint/getMerchantDebitRouting)
- [Update Merchant Debit Routing](/api-reference/endpoint/updateMerchantDebitRouting)

### Gateway Decision

- [Decide Gateway](/api-reference/endpoint/decideGateway)
- [Legacy Decision Gateway](/api-reference/endpoint/legacyDecisionGateway)
- [Update Gateway Score](/api-reference/endpoint/updateGatewayScore)
- [Legacy Update Score](/api-reference/endpoint/legacyUpdateScore)

### Routing Rules

- [Create Routing Rule](/api-reference/endpoint/createRoutingRule)
- [Activate Routing Rule](/api-reference/endpoint/activateRoutingRule)
- [Deactivate Routing Rule](/api-reference/endpoint/deactivateRoutingRule)
- [List Routing Rules](/api-reference/endpoint/listRoutingRules)
- [Get Active Routing Rule](/api-reference/endpoint/getActiveRoutingRule)
- [Evaluate Routing Rule](/api-reference/endpoint/evaluateRoutingRule)
- [Hybrid Routing](/api-reference/endpoint/hybridRouting)

### Rule Configuration

- [Create Rule Config](/api-reference/endpoint/createRuleConfig)
- [Get Rule Config](/api-reference/endpoint/getRuleConfig)
- [Update Rule Config](/api-reference/endpoint/updateRuleConfig)
- [Delete Rule Config](/api-reference/endpoint/deleteRuleConfig)

### Config

- [Get Routing Config](/api-reference/endpoint/getRoutingConfig)
- [Configure SR Dimensions](/api-reference/endpoint/configSrDimension)

### Analytics

- [Overview](/api-reference/endpoint/analyticsOverview)
- [Gateway Scores](/api-reference/endpoint/analyticsGatewayScores)
- [Decisions](/api-reference/endpoint/analyticsDecisions)
- [Routing Stats](/api-reference/endpoint/analyticsRoutingStats)
- [Log Summaries](/api-reference/endpoint/analyticsLogSummaries)
- [Payment Audit](/api-reference/endpoint/analyticsPaymentAudit)
- [Preview Trace](/api-reference/endpoint/analyticsPreviewTrace)
- [Cost Savings](/api-reference/endpoint/analyticsCostSavings)
- [Routing Events](/api-reference/endpoint/analyticsRoutingEvents)
- [A/B Test Experiment Results](/api-reference/endpoint/analyticsExperimentResults)
- [A/B Test Experiment Transactions](/api-reference/endpoint/analyticsExperimentTransactions)

## Curl Examples

For local and sandbox smoke-test examples, use [API Examples](/api-refs/api-ref).
