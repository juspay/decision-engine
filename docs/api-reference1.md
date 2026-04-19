# API Examples

Use these examples for local smoke tests. For request and response schemas, use the OpenAPI-backed endpoint pages and `docs/openapi.json`.

Base URL:

```bash
export BASE_URL=http://localhost:8080
```

## Health

```bash
curl "$BASE_URL/health"
```

## Create Merchant

```bash
curl -X POST "$BASE_URL/merchant-account/create" \
  -H "Content-Type: application/json" \
  -d '{
    "merchant_id": "demo_merchant",
    "gateway_success_rate_based_decider_input": null
  }'
```

## Decide Gateway

```bash
curl -X POST "$BASE_URL/decide-gateway" \
  -H "Content-Type: application/json" \
  -d '{
    "merchantId": "demo_merchant",
    "paymentInfo": {
      "paymentId": "pay_001",
      "amount": 1000.0,
      "currency": "USD",
      "paymentType": "ORDER_PAYMENT",
      "paymentMethodType": "CARD",
      "paymentMethod": "CREDIT"
    },
    "eligibleGatewayList": ["stripe", "paypal", "adyen"],
    "rankingAlgorithm": "SrBasedRouting",
    "eliminationEnabled": false
  }'
```

## Update Gateway Score

```bash
curl -X POST "$BASE_URL/update-gateway-score" \
  -H "Content-Type: application/json" \
  -d '{
    "merchantId": "demo_merchant",
    "gateway": "stripe",
    "paymentId": "pay_001",
    "status": "CHARGED",
    "gatewayReferenceId": null,
    "enforceDynamicRoutingFailure": null
  }'
```

## Create Routing Rule

```bash
curl -X POST "$BASE_URL/routing/create" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "default-priority",
    "description": "route to stripe first",
    "created_by": "demo_merchant",
    "algorithm_for": "payment",
    "algorithm": {
      "type": "priority",
      "data": [
        { "gateway_name": "stripe", "gateway_id": null },
        { "gateway_name": "paypal", "gateway_id": null }
      ]
    }
  }'
```

## List Routing Rules

```bash
curl -X POST "$BASE_URL/routing/list/demo_merchant"
```

## Activate Routing Rule

```bash
curl -X POST "$BASE_URL/routing/activate" \
  -H "Content-Type: application/json" \
  -d '{
    "created_by": "demo_merchant",
    "routing_algorithm_id": "rule_id_here"
  }'
```

## Get Active Routing Rule

```bash
curl -X POST "$BASE_URL/routing/list/active/demo_merchant"
```

## Create Rule Config

```bash
curl -X POST "$BASE_URL/rule/create" \
  -H "Content-Type: application/json" \
  -d '{
    "merchant_id": "demo_merchant",
    "config": {
      "type": "successRate",
      "data": {
        "defaultLatencyThreshold": 90,
        "defaultSuccessRate": 0.5,
        "defaultBucketSize": 200,
        "defaultHedgingPercent": 5,
        "subLevelInputConfig": []
      }
    }
  }'
```

## Get Rule Config

```bash
curl -X POST "$BASE_URL/rule/get" \
  -H "Content-Type: application/json" \
  -d '{
    "merchant_id": "demo_merchant",
    "algorithm": "successRate"
  }'
```

## Update Rule Config

```bash
curl -X POST "$BASE_URL/rule/update" \
  -H "Content-Type: application/json" \
  -d '{
    "merchant_id": "demo_merchant",
    "config": {
      "type": "successRate",
      "data": {
        "defaultLatencyThreshold": 95,
        "defaultSuccessRate": 0.5,
        "defaultBucketSize": 250,
        "defaultHedgingPercent": 5,
        "subLevelInputConfig": []
      }
    }
  }'
```

## Delete Rule Config

```bash
curl -X POST "$BASE_URL/rule/delete" \
  -H "Content-Type: application/json" \
  -d '{
    "merchant_id": "demo_merchant",
    "algorithm": "successRate"
  }'
```
