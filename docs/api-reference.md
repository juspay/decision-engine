# API Overview

The canonical OpenAPI contract for the docs site is `docs/openapi.json`. Use this page for schema-oriented navigation. Use [API Examples](/api-refs/api-ref) for curl-first flows, valid payloads, and route variants.

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

## Curl Examples

For local and sandbox smoke-test examples, use [API Examples](/api-refs/api-ref).
