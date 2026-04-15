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

### Priority rule

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

### Single connector rule

```bash
curl -X POST "$BASE_URL/routing/create" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "always-stripe",
    "description": "Pin all traffic to stripe",
    "created_by": "demo_merchant",
    "algorithm_for": "payment",
    "algorithm": {
      "type": "single",
      "data": {
        "gateway_name": "stripe",
        "gateway_id": null
      }
    }
  }'
```

### Volume split rule

```bash
curl -X POST "$BASE_URL/routing/create" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "card-ab-test",
    "description": "Split card traffic between stripe and checkout",
    "created_by": "demo_merchant",
    "algorithm_for": "payment",
    "algorithm": {
      "type": "volume_split",
      "data": [
        {
          "split": 70,
          "output": { "gateway_name": "stripe", "gateway_id": null }
        },
        {
          "split": 30,
          "output": { "gateway_name": "checkout", "gateway_id": null }
        }
      ]
    }
  }'
```

### Advanced rule with OR branches

`statements` are OR branches. Each `condition` array is an AND block.

```bash
curl -X POST "$BASE_URL/routing/create" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "wallet-or-credit-card",
    "description": "Route wallet traffic to checkout and credit cards to stripe first",
    "created_by": "demo_merchant",
    "algorithm_for": "payment",
    "algorithm": {
      "type": "advanced",
      "data": {
        "globals": {},
        "default_selection": {
          "priority": [
            { "gateway_name": "adyen", "gateway_id": null }
          ]
        },
        "rules": [
          {
            "name": "wallet_or_credit_card",
            "routing_type": "priority",
            "output": {
              "priority": [
                { "gateway_name": "checkout", "gateway_id": null },
                { "gateway_name": "stripe", "gateway_id": null }
              ]
            },
            "statements": [
              {
                "condition": [
                  {
                    "lhs": "payment_method",
                    "comparison": "equal",
                    "value": { "type": "enum_variant", "value": "wallet" },
                    "metadata": {}
                  }
                ],
                "nested": null
              },
              {
                "condition": [
                  {
                    "lhs": "payment_method",
                    "comparison": "equal",
                    "value": { "type": "enum_variant", "value": "card" },
                    "metadata": {}
                  },
                  {
                    "lhs": "card_type",
                    "comparison": "equal",
                    "value": { "type": "enum_variant", "value": "credit" },
                    "metadata": {}
                  }
                ],
                "nested": null
              }
            ]
          }
        ],
        "metadata": {}
      }
    }
  }'
```

### Advanced rule with nested conditions

Use `nested` when the second block should only run after the parent condition matched.

```bash
curl -X POST "$BASE_URL/routing/create" \
  -H "Content-Type: application/json" \
  -d '{
    "name": "visa-usd-nested-routing",
    "description": "Use nested card checks, then split matched traffic by percentage",
    "created_by": "demo_merchant",
    "algorithm_for": "payment",
    "algorithm": {
      "type": "advanced",
      "data": {
        "globals": {},
        "default_selection": {
          "volume_split": [
            {
              "split": 60,
              "output": { "gateway_name": "stripe", "gateway_id": null }
            },
            {
              "split": 40,
              "output": { "gateway_name": "checkout", "gateway_id": null }
            }
          ]
        },
        "rules": [
          {
            "name": "card_credit_visa",
            "routing_type": "volume_split",
            "output": {
              "volume_split": [
                {
                  "split": 80,
                  "output": { "gateway_name": "stripe", "gateway_id": null }
                },
                {
                  "split": 20,
                  "output": { "gateway_name": "adyen", "gateway_id": null }
                }
              ]
            },
            "statements": [
              {
                "condition": [
                  {
                    "lhs": "payment_method",
                    "comparison": "equal",
                    "value": { "type": "enum_variant", "value": "card" },
                    "metadata": {}
                  }
                ],
                "nested": [
                  {
                    "condition": [
                      {
                        "lhs": "card_type",
                        "comparison": "equal",
                        "value": { "type": "enum_variant", "value": "credit" },
                        "metadata": {}
                      }
                    ],
                    "nested": [
                      {
                        "condition": [
                          {
                            "lhs": "card_network",
                            "comparison": "equal",
                            "value": { "type": "enum_variant", "value": "visa" },
                            "metadata": {}
                          },
                          {
                            "lhs": "currency",
                            "comparison": "equal",
                            "value": { "type": "enum_variant", "value": "USD" },
                            "metadata": {}
                          }
                        ],
                        "nested": null
                      }
                    ]
                  }
                ]
              }
            ]
          }
        ],
        "metadata": {}
      }
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

## Create Euclid Rule Config

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

## Get Euclid Rule Config

```bash
curl -X POST "$BASE_URL/rule/get" \
  -H "Content-Type: application/json" \
  -d '{
    "merchant_id": "demo_merchant",
    "algorithm": "successRate"
  }'
```

## Update Euclid Rule Config

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

## Delete Euclid Rule Config

```bash
curl -X POST "$BASE_URL/rule/delete" \
  -H "Content-Type: application/json" \
  -d '{
    "merchant_id": "demo_merchant",
    "algorithm": "successRate"
  }'
```
