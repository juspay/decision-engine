# API REFERENCE

## Decision Gateway API

### Sample curl for decide-gateway

#### Request: SR BASED ROUTING
```bash
curl --location 'http://localhost:8080/decide-gateway' \
--header 'Content-Type: application/json' \
--data '{           
        "merchantId": "test_merchant1",
        "eligibleGatewayList": ["GatewayA", "GatewayB", "GatewayC"],
        "rankingAlgorithm": "SR_BASED_ROUTING",
        "eliminationEnabled": true,
        "paymentInfo": {
            "paymentId": "PAY12359",
            "amount": 100.50,
            "currency": "USD",
            "customerId": "CUST12345",
            "udfs": null,
            "preferredGateway": null,
            "paymentType": "ORDER_PAYMENT",
            "metadata": null,
            "internalMetadata": null,
            "isEmi": false,
            "emiBank": null,
            "emiTenure": null,
            "paymentMethodType": "UPI",
            "paymentMethod": "UPI_PAY",
            "paymentSource": null,
            "authType": null,
            "cardIssuerBankName": null,
            "cardIsin": null,
            "cardType": null,
            "cardSwitchProvider": null
        }
}'
```

#### Response:
```json
{
    "decided_gateway": "GatewayA",
    "gateway_priority_map": {
        "GatewayA": 1.0,
        "GatewayB": 1.0,
        "GatewayC": 1.0
    },
    "filter_wise_gateways": null,
    "priority_logic_tag": null,
    "routing_approach": "SR_SELECTION_V3_ROUTING",
    "gateway_before_evaluation": "GatewayA",
    "priority_logic_output": {
        "isEnforcement": false,
        "gws": [
            "GatewayA",
            "GatewayB",
            "GatewayC"
        ],
        "priorityLogicTag": null,
        "gatewayReferenceIds": {},
        "primaryLogic": null,
        "fallbackLogic": null
    },
    "reset_approach": "NO_RESET",
    "routing_dimension": "ORDER_PAYMENT, UPI, UPI_PAY",
    "routing_dimension_level": "PM_LEVEL",
    "is_scheduled_outage": false,
    "is_dynamic_mga_enabled": false,
    "gateway_mga_id_map": null
}
```
##### Routing Approach
This field available in the response for the decide-gateway api call provides visibility into the routing logic applied by the decision engine for a transaction. The possible values are as follows:

- **SR_SELECTION_V3_ROUTING** : Routing is based on the gateway with the highest Success Rate (SR) score for the merchant, evaluated at the dimension on which routing is happening

- **SR_V3_DOWNTIME_ROUTING** : Routing uses SR-based selection, but one or more (not all) eligible gateways have been deprioritized due to downtime (i.e., having a score below the elimination threshold). The system selects the best available gateway (based on SR) amongst the gateways which are not facing downtime.

- **SR_V3_ALL_DOWNTIME_ROUTING** : All eligible gateways are facing downtime and have been deprioritized via elimination. Routing still uses SR scores at the configured dimension level to select the best among the degraded options.

- **SR_V3_HEDGING** : Routing is done across all eligible gateways irrespective of their SR performance. This mode is used for exploration and evaluation of gateway SR performance and is controlled via configuration in the SR routing setup.

- **SR_V3_DOWNTIME_HEDGING** : Routing follows the hedging strategy, where SR performance is not a strict criterion. However, one or more (but not all) eligible gateways are facing downtime. The system prefers gateways that are currently healthy while maintaining the exploration objective.

- **SR_V3_ALL_DOWNTIME_HEDGING** :  Routing follows the configured hedging strategy, but all eligible gateways are experiencing downtime. In this scenario, routing proceeds without reprioritization, in accordance with the defined hedging configuration.

#### Request: DEBIT ROUTING
```bash
curl --location 'http://localhost:8080/decide-gateway' \
--header 'Content-Type: application/json' \
--data '{
  "merchantId": "pro_OiJkBiFuCYbYAkCG9X02",
  "eligibleGatewayList": ["PAYU", "RAZORPAY", "PAYTM_V2"],
  "rankingAlgorithm": "NTW_BASED_ROUTING",
  "eliminationEnabled": true,
  "paymentInfo": {
    "paymentId": "PAY12345",
    "amount": 100.50,
    "currency": "USD",
    "customerId": "CUST12345",
    "udfs": null,
    "preferredGateway": null,
    "paymentType": "ORDER_PAYMENT",
    "metadata": "{\"merchant_category_code\":\"merchant_category_code_0001\",\"acquirer_country\":\"US\"}",
    "internalMetadata": null,
    "isEmi": false,
    "emiBank": null,
    "emiTenure": null,
    "paymentMethodType": "UPI",
    "paymentMethod": "UPI_PAY",
    "paymentSource": null,
    "authType": null,
    "cardIssuerBankName": null,
    "cardIsin": "440000",
    "cardType": null,
    "cardSwitchProvider": null
  }
}'
```

#### Response:
```json
{
  "decided_gateway": "PAYU",
  "gateway_priority_map": null,
  "filter_wise_gateways": null,
  "priority_logic_tag": null,
  "routing_approach": "NONE",
  "gateway_before_evaluation": null,
  "priority_logic_output": null,
  "debit_routing_output": {
    "co_badged_card_networks": [
      "STAR",
      "VISA"
    ],
    "issuer_country": "US",
    "is_regulated": false,
    "regulated_name": "GOVERNMENT EXEMPT INTERCHANGE FEE",
    "card_type": "debit"
  },
  "reset_approach": "NO_RESET",
  "routing_dimension": null,
  "routing_dimension_level": null,
  "is_scheduled_outage": false,
  "is_dynamic_mga_enabled": false,
  "gateway_mga_id_map": null
}
```

## Update Gateway Score API

### Sample curl for update-gateway-score

#### Request:
```bash
curl --location 'http://localhost:8080/update-gateway-score' \
--header 'Content-Type: application/json' \
--data '{
  "merchantId" : "test_merchant1",
  "gateway": "RAZORPAY",
  "gatewayReferenceId": null,
  "status": "FAILURE",
  "paymentId": "PAY12359",
  "enforceDynamicRoutingFailure" : null
}'
```

#### Response:
```
Success
```

## Config APIs

#### Request: Success Rate Config Create
```bash
curl -X POST http://localhost:8080/rule/create \
  -H "Content-Type: application/json" \
  -d '{
    "merchant_id": "test_merchant_123423",
    "config": {
      "type": "successRate",
      "data": {
        "defaultLatencyThreshold": 90,
        "defaultSuccessRate": 0.5,
        "defaultBucketSize": 200,
        "defaultHedgingPercent": 5,
        "subLevelInputConfig": [
          {
            "paymentMethodType": "upi",
            "paymentMethod": "upi_collect",
            "bucketSize": 250,
            "hedgingPercent": 1
          }
        ]
      }
    }
  }'
```

#### Response:
```json
{
  "Success Rate Configuration created successfully"
}
```

#### Request: Success Rate Config retrieve
```bash
curl -X POST http://localhost:8080/rule/get \
     -H "Content-Type: application/json" \
     -d '{
           "merchant_id": "test_merchant_123423",
           "algorithm": "successRate"
         }'
```

#### Response:
```json
{
   "merchant_id": "test_merchant_123423",
    "config": {
      "type": "successRate",
      "data": {
        "defaultLatencyThreshold": 90,
        "defaultSuccessRate": 0.5,
        "defaultBucketSize": 200,
        "defaultHedgingPercent": 5,
        "subLevelInputConfig": [
          {
            "paymentMethodType": "upi",
            "paymentMethod": "upi_collect",
            "bucketSize": 250,
            "hedgingPercent": 1
          }
        ]
      }
    }
}
```

#### Request: Success Rate Config update
```bash
curl -X POST http://localhost:8080/rule/update \
  -H "Content-Type: application/json" \
  -d '{
    "merchant_id": "test_merchant_123423",
    "config": {
      "type": "successRate",
      "data": {
        "defaultLatencyThreshold": 90,
        "defaultSuccessRate": 0.5,
        "defaultBucketSize": 200,
        "defaultHedgingPercent": 5,
        "subLevelInputConfig": [
          {
            "paymentMethodType": "upi",
            "paymentMethod": "upi_collect",
            "bucketSize": 250,
            "hedgingPercent": 1
          }
        ]
      }
    }
  }'
```

#### Response:
```json
{
  "Success Rate Configuration updated successfully"
}
```

#### Request: Success Rate Config delete
```bash
curl -X POST http://localhost:8080/rule/delete \
     -H "Content-Type: application/json" \
     -d '{
           "merchant_id": "test_merchant_123423",
           "algorithm": "successRate"
         }'
```

#### Response:
```json
{
  "Success Rate Configuration deleted successfully"
}
```

#### Request: Elimination Config Create
```bash
curl -X POST http://localhost:8080/rule/create \
  -H "Content-Type: application/json" \
  -d '{
    "merchant_id": "test_merchant_123423",
    "config": {
      "type": "elimination",
      "data": {
        "threshold": 0.35
      }
    }
  }'
```

#### Response:
```json
{
  "Elimination Configuration created successfully"
}
```

#### Request: Elimination Config retrieve
```bash
curl -X POST http://localhost:8080/rule/get \
     -H "Content-Type: application/json" \
     -d '{
           "merchant_id": "test_merchant_123423",
           "algorithm": "elimination"
         }'
```

#### Response:
```json
{
    "merchant_id": "test_merchant_123423",
    "config": {
      "type": "elimination",
      "data": {
        "threshold": 0.35
      }
    }
}
```

#### Request: Elimination Config update
```bash
curl -X POST http://localhost:8080/rule/update \
  -H "Content-Type: application/json" \
  -d '{
    "merchant_id": "test_merchant_123423",
    "config": {
      "type": "elimination",
      "data": {
        "threshold": 0.35
      }
    }
  }'
```

#### Response:
```json
{
  "Elimination Configuration updated successfully"
}
```

#### Request: Elimination Config delete
```bash
curl -X POST http://localhost:8080/rule/delete \
     -H "Content-Type: application/json" \
     -d '{
           "merchant_id": "test_merchant_123423",
           "algorithm": "elimination"
         }'
```

#### Response:
```json
{
  "Elimination Configuration deleted successfully"
}
```

#### Request: Merchant account create
```bash
curl --location --request POST 'http://localhost:8080/merchant-account/create' \
--header 'Content-Type: application/json' \
--data-raw '{
  "merchant_id": "test_merchant_123423"  
}'
```

#### Response:
```json
{
  "Merchant account created successfully"
}
```

#### Request: Merchant account retrieve
```bash
curl -X GET http://localhost:8080/merchant-account/test_merchant_123423            
```

#### Response:
```json
{
    "merchant_id": "test_merchant_123423",
    "gateway_success_rate_based_decider_input": null
}
```

#### Request: Merchant account delete
```bash
curl -X DELETE http://localhost:8080/merchant-account/test_merchant_123423  
```     

#### Response:
```json
{
    "Merchant account deleted successfully"
}
```

# Priority Logic V2
---

**A rule engine to enable merchants to create complex logical expressions based on various payment related [parameters](https://github.com/juspay/decision-engine/blob/main/config/development.toml). These rules are executed on the payment payload to evaluate the gateway to be used.**

## Table of Contents
1. [API Components](#components)
2. [API Reference](#PL-api-reference)  
   &nbsp;&nbsp;2.1 [Create](#create)  
   &nbsp;&nbsp;2.2 [Evaluate](#evaluate)  
   &nbsp;&nbsp;2.3 [Operations](#operations)  
   &nbsp;&nbsp;&nbsp;2.3.1 [List](#list)  
   &nbsp;&nbsp;&nbsp;2.3.2 [Activate](#activate)  
   &nbsp;&nbsp;&nbsp;2.3.e [List-Activated](#list_activated)  
3. [Algorithm Types](#algorithm-types)  
   &nbsp;&nbsp;3.1 Advanced Logic  
   &nbsp;&nbsp;&nbsp;&nbsp;3.1.1 [AND](#and-rule)  
   &nbsp;&nbsp;&nbsp;&nbsp;3.1.2 [OR](#or-rule)  
   &nbsp;&nbsp;&nbsp;&nbsp;3.1.3 [AND-OR (Nested)](#and-or-rule)  
   &nbsp;&nbsp;&nbsp;&nbsp;3.1.4 [Enum variant](#enum-variant)  
   &nbsp;&nbsp;&nbsp;&nbsp;3.1.5 [Number array](#number-array)  
   &nbsp;&nbsp;&nbsp;&nbsp;3.1.6 [Number comparison array](#number-comparison-array)  
   &nbsp;&nbsp;3.2 [Priority](#priority)  
   &nbsp;&nbsp;3.3 [Single Connector](#single)  
   &nbsp;&nbsp;3.4 [Volume Split](#volume-split)

---

## <a id="components"></a>1 · API Components

| Components                   | Description & Accepted Values                                                                                  |
|------------------------------|----------------------------------------------------------------------------------------------------------------|
| `name`                       | Name of the rule (**string**, required)                                                                        |
| `created_by`                 | Merchant or platform ID (**string**, required)                                                                 |
| `description`                | Rule description (**string**)                                                                         |
| `algorithm_for`              | Routing scope (**enum**): `payment` (default), `payout`, `three_ds_authentication`                             |
| `algorithm.type`             | Routing algorithm type (**enum**): [`advanced`](#advanced), [`single`](#single), [`priority`](#priority), [`volume_split`](#volume-split)                            |
| `algorithm.data.globals`     | Optional constants (**object**): reusable values for expressions                                               |
| `default_selection`          | Fallback connectors if no rule matches (**object**) with `priority: [{ gateway_name, gateway_id }]`            |
| `rules[].name`               | Name of an individual rule (**string**)                                                                        |
| `rules[].routing_type`       | Rule behavior (**enum**): `priority`, `volume_split`, Required only in advanced rule.                                                         |
| `output.priority[]`          | Priority connector list (**array**): `[ { gateway_name, gateway_id } ]`                                        |
| `output.volume_split[]`      | Volume split rule (**array**): `[ { split: number, output: { gateway_name, gateway_id } } ]`, either priority or volume can be present at once in output.                  |
| `statements[].condition[]`   | AND logic conditions (**array**): each with `lhs`, `comparison`, `value`, and optional `metadata`              |
| `condition.lhs`              | Field to evaluate (**string**): e.g., `amount`, `payment_method`, `card_network`. Can be checked from development.toml.                             |
| `condition.comparison`       | Comparator (**enum**): `equal`, `greater_than`, `less_than`, `greater_than_equals`, etc.                                        |
| `condition.value.type`       | Value type (**enum**): `number`, [`number_array`](#number-array), [`enum_variant`](#enum-variant), `str_value`, [`number_comparison_array`](#number-comparison-array), etc.  |
| `condition.value.value`      | Value being compared (**any**): e.g., `100`, `"card"`, `[1000, 2000]`                                          |
| `condition.metadata`         | Optional metadata for tracing/debug (**object**)                                                               |
| `statements[].nested`        | Nested OR conditions (**array**) of `condition[]` blocks                                                       |
| `algorithm.metadata`         | Algorithm-level metadata (**object**, optional)                                                                |
| `rules[].metadata`           | Rule-level metadata (**object**, optional)                                                                     |


**Tip**: Use multiple `statements[]` blocks for OR logic. Use `nested` inside a `statement` for AND+OR nesting.

---


## <a id="PL-api-reference"></a>2 · API Reference 

### <a id="create"></a>2.1 Create Routing Algorithm

```bash
curl --location 'http://127.0.0.1:8082/routing/create' \
--header 'Content-Type: application/json' \
--data '
{
    "name": "Priority rule",
    "created_by": "merchant_1234",
    "description": "this is my priority rule",
    "algorithm_for": "payment",
    "algorithm": {
        "type": "advanced",
        "data": {
            "globals": {},
            "default_selection": {
                "priority": [
                    {
                        "gateway_name": "stripe",
                        "gateway_id": "mca_111"
                    },
                    {
                        "gateway_name": "adyen",
                        "gateway_id": "mca_112"
                    },
                    {
                        "gateway_name": "checkout",
                        "gateway_id": "mca_113"
                    }
                ]
            },
            "rules": [
                {
                    "name": "Card Rule",
                    "routingType": "priority",
                    "output": {
                        "priority": [
                            {
                                "gateway_name": "Paytm",
                                "gateway_id": "mca_114"
                            },
                            {
                                "gateway_name": "adyen",
                                "gateway_id": "mca_112"
                            }
                        ]
                    },
                    "statements": [
                        {
                            "condition": [
                                {
                                    "lhs": "payment_method",
                                    "comparison": "equal",
                                    "value": {
                                        "type": "enum_variant",
                                        "value": "card"
                                    },
                                    "metadata": {}
                                }
                            ]
                        },
                        {
                            "condition": [
                                {
                                    "lhs": "amount",
                                    "comparison": "greater_than",
                                    "value": {
                                        "type": "number",
                                        "value": 100
                                    },
                                    "metadata": {}
                                }
                            ]
                        }
                    ]
                }
            ]
        }
    },
    "metadata": {}
}
'
```

**Success Response**
```json
{
  "rule_id": "routing_e641380c-6f24-4405-8454-5ae6cbceb7a0",
  "name": "Priority rule",
  "created_at": "2025-04-22 11:45:03.411134513",
  "modified_at": "2025-04-22 11:45:03.411134513"
}
```

---

### <a id="evaluate"></a>2.2 Evaluate Routing Algorithm

```bash
curl --location 'http://127.0.0.1:8082/routing/evaluate' \
--header 'Content-Type: application/json' \
--data '{
  "created_by": "merchant_1234",
  "parameters": {
    "payment_method": { "type": "enum_variant", "value": "upi" },
    "amount":         { "type": "number",       "value": 10   }
  }
}'
```

**Example Response**
```json
{
  "status": "default_selection",
  "output": {
    "type": "priority",
    "connectors": [
      { "gateway_name": "stripe",   "gateway_id": "mca_111" },
      { "gateway_name": "adyen",    "gateway_id": "mca_112" },
      { "gateway_name": "checkout", "gateway_id": "mca_113" }
    ]
  },
  "evaluated_output": [
    { "gateway_name": "stripe", "gateway_id": "mca_111" }
  ],
  "eligible_connectors": []
}
```

---

## <a id="operations"></a>2.3 Operations

### <a id="list"></a>2.3.1 List Algorithms

```bash
curl --request POST 'http://127.0.0.1:8082/routing/list/merchant_1234'
```

Returns an array of algorithms for `merchant_1234`.

### <a id="activate"></a>2.3.2 Activate Algorithm

```bash
curl --location 'http://127.0.0.1:8082/routing/activate' \
--header 'Content-Type: application/json' \
--data '{
  "created_by": "merchant_1234",
  "routing_algorithm_id": "routing_8711ce52-33e2-473f-9c8f-91a406acb850"
}'
```
At a given time one algorithm for each transaction_type (`payment`, `payout`, `three_ds_authentication`) can be active for one created_by id.
HTTP 200 ⇒ algorithm is now active.

### <a id="list"></a>2.3.3 List Activated algorithm

```bash
curl --location --request POST 'http://127.0.0.1:8082/routing/list/active/merchant_31' \
--header 'Content-Type: application/json'
```

Returns algorithms currently active for the merchant.

---

## <a id="algorithm-types"></a>3 · Algorithm Types

### <a id="advanced"></a>3.1 Advanced Logic (AND / OR / AND-OR)

| Use-case            | Description                          |
|---------------------|--------------------------------------|
| **AND**             | All conditions must be true          |
| **OR**              | Any one condition may be true        |
| **AND-OR (nested)** | Parent condition + any nested match  |

---
Note: for advanced algorithm kinds we always require statements to be evaluated upoun, unlike the below priority, single and volume_split, which donot requires any statements and directly provide output.
<details>
<summary id="and-rule">AND Rule</summary>

```json
{
  "name": "HDFC Rule",
  "routing_type": "volume_split",
  "output": {
    "volume_split": [
      {
        "split": 60,
        "output": { "gateway_name": "hdfc", "gateway_id": "mca_114" }
      },
      {
        "split": 40,
        "output": { "gateway_name": "instamojo", "gateway_id": "mca_115" }
      }
    ]
  },
  "statements": [
    {
      "condition": [
        {
          "lhs": "amount",
          "comparison": "greater_than",
          "value": { "type": "number", "value": 100 }
        },
        {
          "lhs": "billing_country",
          "comparison": "equal",
          "value": { "type": "enum_variant", "value": "Netherlands" }
        }
      ]
    }
  ]
}
```

All conditions must match → volume split applies
</details>

---

<details>
<summary id="or-rule">OR Rule</summary>

```json
{
  "name": "Card Rule",
  "routing_type": "priority",
  "output": {
    "priority": [
      { "gateway_name": "Paytm", "gateway_id": "mca_114" },
      { "gateway_name": "adyen", "gateway_id": "mca_112" }
    ]
  },
  "statements": [
    {
      "condition": [
        {
          "lhs": "payment_method",
          "comparison": "equal",
          "value": { "type": "enum_variant", "value": "card" }
        }
      ]
    },
    {
      "condition": [
        {
          "lhs": "amount",
          "comparison": "greater_than",
          "value": { "type": "number", "value": 100 }
        }
      ]
    }
  ]
}
```

Any one condition match triggers the rule
</details>

---

<details>
<summary id="and-or-rule" >AND + OR (Nested)</summary>

```json
{
  "name": "RBL Rule",
  "routing_type": "priority",
  "output": {
    "priority": [
      { "gateway_name": "rbl", "gateway_id": "mca_114" },
      { "gateway_name": "instamojo", "gateway_id": "mca_115" }
    ]
  },
  "statements": [
    {
      "condition": [
        {
          "lhs": "amount",
          "comparison": "greater_than",
          "value": { "type": "number", "value": 10 }
        }
      ],
      "nested": [
        {
          "condition": [
            {
              "lhs": "card_network",
              "comparison": "equal",
              "value": { "type": "enum_variant", "value": "Visa" }
            }
          ]
        },
        {
          "condition": [
            {
              "lhs": "billing_country",
              "comparison": "equal",
              "value": { "type": "enum_variant", "value": "India" }
            }
          ]
        }
      ]
    }
  ]
}
```

Main condition must match + any one nested condition
</details>

---

<details>
<summary id="enum-variant" >Enum variant</summary>

```json
{
    "name": "Card Rule",
    "routing_type": "priority",
    "output": {
        "priority": [
            {
                "gateway_name": "rbl",
                "gateway_id": "mca_114"
            }
        ]
    },
    "statements": [
        {
            "condition": [
                {
                    "lhs": "card_network",
                    "comparison": "equal",
                    "value": {
                        "type": "enum_variant_array",
                        "value": [
                            "Visa",
                            "Mastercard"
                        ]
                    },
                    "metadata": {}
                }
            ]
        }
    ]
}
```

The input for evaluation parameter must be one of the mentioned types in array.
</details>

---

<details>
<summary id="number-array" >Number array</summary>

```json
{
    "name": "Card Rule",
    "routing_type": "priority",
    "output": {
        "priority": [
            {
                "gateway_name": "rbl",
                "gateway_id": "mca_114"
            }
        ]
    },
    "statements": [
        {
            "condition": [
                {
                    "lhs": "amount",
                    "comparison": "equal",
                    "value": {
                        "type": "number_array",
                        "value": [
                            1000,
                            2000,
                            5000
                        ]
                    },
                    "metadata": {}
                }
            ]
        }
    ]
}
```

The input for evaluation parameter must be one of the mentioned values in array.
</details>

---
<details>
<summary id="number-comparison-array" >Number comparison array</summary>

```json
{
    "name": "Card Rule",
    "routing_type": "priority",
    "output": {
        "priority": [
            {
                "gateway_name": "rbl",
                "gateway_id": "mca_114"
            }
        ]
    },
    "statements": [
        {
            "condition": [
                {
                    "lhs": "amount",
                    "comparison": "equal",
                    "value": {
                        "type": "number_comparison_array",
                        "value": [
                            {
                                "comparison_type": "greater_than",
                                "number": 1000
                            },
                            {
                                "comparison_type": "less_than_equal",
                                "number": 5000
                            }
                        ]
                    },
                    "metadata": {}
                }
            ]
        }
    ]
}
```

The input for evaluation parameter must be in the specifed thresholds.
</details>

---


### <a id="priority"></a>3.2 Priority Routing

```bash
curl --location 'http://127.0.0.1:8082/routing/create' \
--header 'Content-Type: application/json' \
--data '{
  "name": "priority rule test",
  "created_by": "merchant_123",
  "algorithm": {
    "type": "priority",
    "data": [
      { "gateway_name": "stripe",   "gateway_id": "mca_001" },
      { "gateway_name": "razorpay", "gateway_id": "mca_002" }
    ]
  }
}'
```

Always returns the connectors **in the given order**.

---

### <a id="single"></a>3.3 Single Connector  (straight-through)

```bash
curl --location 'http://127.0.0.1:8082/routing/create' \
--header 'Content-Type: application/json' \
--data '{
  "name": "single connector rule",
  "created_by": "merchant_123",
  "algorithm": {
    "type": "single",
    "data": { "gateway_name": "stripe", "gateway_id": "mca_00123" }
  }
}'
```

Regardless of parameters, Routing decision will always be **Stripe (mca_00123)**.

---

### <a id="volume-split"></a>3.4 Volume Split

```bash
curl --location 'http://127.0.0.1:8082/routing/create' \
--header 'Content-Type: application/json' \
--data '{
  "name": "volume split test rule",
  "created_by": "merchant_31",
  "algorithm_for": "payout",
  "algorithm": {
    "type": "volume_split",
    "data": [
      {
        "split": 70,
        "output": { "gateway_name": "stripe",   "gateway_id": "mca_001" }
      },
      {
        "split": 30,
        "output": { "gateway_name": "paytm", "gateway_id": "mca_002" }
      }
    ]
  }
}'
```

Provides **70 %** of decisions as Stripe and **30 %** as Paytm.

---

**Note:** Full routing rule example is provided in the [initial request section](#create). Use that as template to compose complex rules (AND / OR / AND-OR).

---
