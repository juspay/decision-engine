# API Reference

The canonical API contract for the docs site is `docs/openapi.json`.

Use this page to find the right endpoint family. Use the OpenAPI-backed endpoint pages for request and response schema details.

## Endpoint Families

### Health

- [Health Check](api-reference/endpoint/healthCheck)

### Merchant API

- [Create Merchant](api-reference/endpoint/createMerchant)
- [Get Merchant](api-reference/endpoint/getMerchant)
- [Delete Merchant](api-reference/endpoint/deleteMerchant)

### Rule Based Routing

These are the `/routing/*` APIs for creating, activating, listing, and evaluating routing rules. The create endpoint supports `priority`, `single`, `volume_split`, and `advanced` Euclid programs.

- [Create Routing Rule](api-reference/endpoint/createRoutingRule)
- [Activate Routing Rule](api-reference/endpoint/activateRoutingRule)
- [List Routing Rules](api-reference/endpoint/listRoutingRules)
- [Get Active Routing Rule](api-reference/endpoint/getActiveRoutingRule)
- [Evaluate Routing Rule](api-reference/endpoint/evaluateRoutingRule)

### Dynamic Routing APIs

These are the dynamic routing APIs for gateway decisioning, gateway score updates, and `/rule/*` configuration.

- [POST /decide-gateway](api-reference/endpoint/decideGateway)
- [POST /update-gateway-score](api-reference/endpoint/updateGatewayScore)
- [POST /rule/create](api-reference/endpoint/createRuleConfig)
- [POST /rule/get](api-reference/endpoint/getRuleConfig)
- [POST /rule/update](api-reference/endpoint/updateRuleConfig)
- [POST /rule/delete](api-reference/endpoint/deleteRuleConfig)

## Curl Examples

For local smoke-test examples, use [API Examples](/api-reference1).
