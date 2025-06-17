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

It enables merchants and platforms to define their own routing algorithms—such as **priority-based**, **volume-split**, or **hybrid logic**—and evaluate transaction parameters against them **in real time**.

## Create Routing Algorithm:
### Request:
```
curl --location 'http://127.0.0.1:8082/routing/create' \
--header 'Content-Type: application/json' \
--data '
{
    "name": "Priority rule",
    "created_by": "merchant_1234",
    "description": "this is my priority rule",
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

### Response:
```
{
   "rule_id": "routing_e641380c-6f24-4405-8454-5ae6cbceb7a0",
   "name": "Priority rule",
   "created_at": "2025-04-22 11:45:03.411134513",
   "modified_at": "2025-04-22 11:45:03.411134513"
}
```

### What Happens on Evaluation(Rule explaination)?

```
curl --location '{base_url}/routing/evaluate' \
--header 'Content-Type: application/json' \
--data '{
    "created_by": "merchant_1234",
    "parameters": {
        "payment_method": {
            "type": "enum_variant",
            "value": "card"
        }
        "amount": {
            "type": "number",
            "value": 1
        },
        "billing_country": {
            "type": "enum_variant",
            "value": "Netherlands"
        }
    }
}'
```
If the input has:

- **Either Of**:
  - `amount > 100` **OR**
  - `payment_method == card`

💡 So this makes the rule as **OR** rule.
🔄 **Then** the rule `"Card rule"` matches → returns `Paytm`.
📆 **Otherwise** → returns fallback `defaultSelection` → `[Stripe, Adyen, checkout]`.


## Activate Routing rule for a creator_id.
### Request
```
curl --location 'http://localhost:8080/routing/activate' \
--header 'Content-Type: application/json' \
--data '{
    "created_by": "merchant_1234",
    "routing_algorithm_id": "routing_8711ce52-33e2-473f-9c8f-91a406acb850"
}'
```

### Response
```
status_code: 200
```

## Evaluate Payment parameters using Routing Algorithm (Euclid):
### Request:
```
curl --location 'http://localhost:8080/routing/evaluate' \
--header 'Content-Type: application/json' \
--data '{
 "created_by": "merchant_1234",
 "parameters": {
   "payment_method": {
     "type": "enum_variant",
     "value": "upi"
   },
   "amount": {
     "type": "number",
     "value": 10
   }
 }
}
'
```


### Response:
This will go to default_selection as the amount is less than 1000 and the payment_method is upi.
```
{
   "status": "default_selection",
   "output": {
       "type": "priority",
       "connectors": [
           "stripe",
           "adyen",
           "checkout"
       ]
   },
   "evaluated_output": [
       "stripe"
   ],
   "eligible_connectors": []
}
```

## List all Routing rules for a creator_id.
### Request
```
curl --location --request POST 'http://localhost:8080/routing/list/merchant_1234' \
--header 'Content-Type: application/json'
```

### Response
```
[
    {
        "id": "routing_bff2f300-6acb-4dd1-80e4-c99233f45d0b",
        "created_by": "merchant_1234",
        "name": "Priority rule",
        "description": "this is my priority rule",
        "algorithm_data": {
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
                        "routing_type": "priority",
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
                                ],
                                "nested": null
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
                                ],
                                "nested": null
                            }
                        ]
                    }
                ],
                "metadata": null
            }
        },
        "created_at": "2025-06-17 13:00:41.506841",
        "modified_at": "2025-06-17 13:00:41.506841"
    },
    {
        "id": "routing_8ba5c1a6-d01b-4a5e-b894-b55afa224896",
        "created_by": "merchant_1234",
        "name": "Volume split based config",
        "description": "test volume based rule",
        "algorithm_data": {
            "type": "advanced",
            "data": {
                "globals": {},
                "default_selection": {
                    "priority": [
                        {
                            "gateway_name": "Bambora",
                            "gateway_id": "mca_111"
                        },
                        {
                            "gateway_name": "Paytm",
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
                        "name": "HDFC Rule",
                        "routing_type": "volume_split",
                        "output": {
                            "volume_split": [
                                {
                                    "split": 60,
                                    "output": {
                                        "gateway_name": "hdfc",
                                        "gateway_id": "mca_114"
                                    }
                                },
                                {
                                    "split": 40,
                                    "output": {
                                        "gateway_name": "instamojo",
                                        "gateway_id": "mca_115"
                                    }
                                }
                            ]
                        },
                        "statements": [
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
                                    },
                                    {
                                        "lhs": "billing_country",
                                        "comparison": "equal",
                                        "value": {
                                            "type": "enum_variant",
                                            "value": "Netherlands"
                                        },
                                        "metadata": {}
                                    }
                                ],
                                "nested": null
                            }
                        ]
                    }
                ],
                "metadata": {
                    "transaction": "data"
                }
            }
        },
        "created_at": "2025-06-17 13:01:25.223118",
        "modified_at": "2025-06-17 13:01:25.223118"
    },
    {
        "id": "routing_8711ce52-33e2-473f-9c8f-91a406acb850",
        "created_by": "merchant_1234",
        "name": "AND OR rule example",
        "description": "priority rule which demonstrates AND and OR rule",
        "algorithm_data": {
            "type": "advanced",
            "data": {
                "globals": {},
                "default_selection": {
                    "priority": [
                        {
                            "gateway_name": "Bambora",
                            "gateway_id": "mca_111"
                        },
                        {
                            "gateway_name": "Paytm",
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
                        "routing_type": "priority",
                        "output": {
                            "priority": [
                                {
                                    "gateway_name": "rbl",
                                    "gateway_id": "mca_114"
                                },
                                {
                                    "gateway_name": "instamojo",
                                    "gateway_id": "mca_115"
                                }
                            ]
                        },
                        "statements": [
                            {
                                "condition": [
                                    {
                                        "lhs": "amount",
                                        "comparison": "greater_than",
                                        "value": {
                                            "type": "number",
                                            "value": 10
                                        },
                                        "metadata": {}
                                    }
                                ],
                                "nested": [
                                    {
                                        "condition": [
                                            {
                                                "lhs": "card_network",
                                                "comparison": "equal",
                                                "value": {
                                                    "type": "enum_variant",
                                                    "value": "Visa"
                                                },
                                                "metadata": {}
                                            }
                                        ],
                                        "nested": null
                                    },
                                    {
                                        "condition": [
                                            {
                                                "lhs": "billing_country",
                                                "comparison": "equal",
                                                "value": {
                                                    "type": "enum_variant",
                                                    "value": "Netherlands"
                                                },
                                                "metadata": {}
                                            }
                                        ],
                                        "nested": null
                                    }
                                ]
                            }
                        ]
                    }
                ],
                "metadata": {
                    "transaction": "data"
                }
            }
        },
        "created_at": "2025-06-17 13:02:07.795246",
        "modified_at": "2025-06-17 13:02:07.795246"
    }
]
```

## List active Routing rule for a creator_id.
### Request
```
curl --location --request POST 'http://localhost:8080/routing/list/active/merchant_1234' \
--header 'Content-Type: application/json'
```

### Response
```
{
    "id": "routing_8711ce52-33e2-473f-9c8f-91a406acb850",
    "created_by": "merchant_1234",
    "name": "AND OR rule example",
    "description": "priority rule which demonstrates AND and OR rule",
    "algorithm_data": {
        "type": "advanced",
        "data": {
            "globals": {},
            "default_selection": {
                "priority": [
                    {
                        "gateway_name": "Bambora",
                        "gateway_id": "mca_111"
                    },
                    {
                        "gateway_name": "Paytm",
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
                    "routing_type": "priority",
                    "output": {
                        "priority": [
                            {
                                "gateway_name": "rbl",
                                "gateway_id": "mca_114"
                            },
                            {
                                "gateway_name": "instamojo",
                                "gateway_id": "mca_115"
                            }
                        ]
                    },
                    "statements": [
                        {
                            "condition": [
                                {
                                    "lhs": "amount",
                                    "comparison": "greater_than",
                                    "value": {
                                        "type": "number",
                                        "value": 10
                                    },
                                    "metadata": {}
                                }
                            ],
                            "nested": [
                                {
                                    "condition": [
                                        {
                                            "lhs": "card_network",
                                            "comparison": "equal",
                                            "value": {
                                                "type": "enum_variant",
                                                "value": "Visa"
                                            },
                                            "metadata": {}
                                        }
                                    ],
                                    "nested": null
                                },
                                {
                                    "condition": [
                                        {
                                            "lhs": "billing_country",
                                            "comparison": "equal",
                                            "value": {
                                                "type": "enum_variant",
                                                "value": "Netherlands"
                                            },
                                            "metadata": {}
                                        }
                                    ],
                                    "nested": null
                                }
                            ]
                        }
                    ]
                }
            ],
            "metadata": {
                "transaction": "data"
            }
        }
    },
    "created_at": "2025-06-17 13:02:07.795246",
    "modified_at": "2025-06-17 13:02:07.795246"
}
```

### 1. Volume split rule with fallback
```
curl --location 'http://127.0.0.1:8082/routing/create' \
--header 'Content-Type: application/json' \
--data '
{
    "name": "Volume split based config",
    "created_by": "merchant_1234",
    "description": "test volume based rule",
    "algorithm": {
        "type": "advanced",
        "data": {
            "globals": {},
            "default_selection": {
                "priority": [
                    {
                        "gateway_name": "Bambora",
                        "gateway_id": "mca_111"
                    },
                    {
                        "gateway_name": "Paytm",
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
                    "name": "HDFC Rule",
                    "routing_type": "volume_split",
                    "output": {
                        "volume_split": [
                            {
                                "split": 60,
                                "output": {
                                    "gateway_name": "hdfc",
                                    "gateway_id": "mca_114"
                                }
                            },
                            {
                                "split": 40,
                                "output": {
                                    "gateway_name": "instamojo",
                                    "gateway_id": "mca_115"
                                }
                            }
                        ]
                    },
                    "statements": [
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
                                },
                                {
                                    "lhs": "billing_country",
                                    "comparison": "equal",
                                    "value": {
                                        "type": "enum_variant",
                                        "value": "Netherlands"
                                    },
                                    "metadata": {}
                                }
                            ]
                        }
                    ]
                }
            ],
            "metadata": {
                "transaction": "data"
            }
        },
        "metadata": {
            "transaction_type": "payment"
        }
    }
}
'
```

### What Happens on Evaluation?

```
curl --location '{base_url}/routing/evaluate' \
--header 'Content-Type: application/json' \
--data '{
    "created_by": "merchant_1234",
    "parameters": {
        "payment_method": {
            "type": "enum_variant",
            "value": "card"
        },
        "amount": {
            "type": "number",
            "value": 10000
        },
        "billing_country": {
            "type": "enum_variant",
            "value": "Netherlands"
        }
    }
}'
```
If the input has:

- **Both Of**:
  - `amount > 100` **AND**
  - `billing_country == Netherlands`

💡 So this makes the rule as **AND** rule.
🔄 **Then** the rule `"Hdfc rule"` matches → returns volume split between `hdfc` and `instamojo`.
📆 **Otherwise** → returns fallback `defaultSelection` → `[bambora, Paytm, checkout]`.


### 2. Nested Rule with Fallback
```
curl --location 'http://127.0.0.1:8082/routing/create' \
--header 'Content-Type: application/json' \
--data '
{
    "name": "AND OR rule example",
    "created_by": "merchant_1234",
    "description": "priority rule which demonstrates AND and OR rule",
    "algorithm": {
        "type": "advanced",
        "data": {
            "globals": {},
            "default_selection": {
                "priority": [
                    {
                        "gateway_name": "Bambora",
                        "gateway_id": "mca_111"
                    },
                    {
                        "gateway_name": "Paytm",
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
                    "routing_type": "priority",
                    "output": {
                        "priority": [
                            {
                                "gateway_name": "rbl",
                                "gateway_id": "mca_114"
                            },
                            {
                                "gateway_name": "instamojo",
                                "gateway_id": "mca_115"
                            }
                        ]
                    },
                    "statements": [
                        {
                            "condition": [
                                {
                                    "lhs": "amount",
                                    "comparison": "greater_than",
                                    "value": {
                                        "type": "number",
                                        "value": 10
                                    },
                                    "metadata": {}
                                }
                            ],
                            "nested": [
                                {
                                    "condition": [
                                        {
                                            "lhs": "card_network",
                                            "comparison": "equal",
                                            "value": {
                                                "type": "enum_variant",
                                                "value": "Visa"
                                            },
                                            "metadata": {}
                                        }
                                    ]
                                },
                                {
                                    "condition": [
                                        {
                                            "lhs": "billing_country",
                                            "comparison": "equal",
                                            "value": {
                                                "type": "enum_variant",
                                                "value": "Netherlands"
                                            },
                                            "metadata": {}
                                        }
                                    ]
                                }
                            ]
                        }
                    ]
                }
            ],
            "metadata": {
                "transaction": "data"
            }
        },
        "metadata": {
            "transaction_type": "payout"
        }
    }
}'
```

### What Happens on Evaluation?
```
curl --location '{base_url}/routing/evaluate' \
--header 'Content-Type: application/json' \
--data '{
    "created_by": "merchant_1234",
    "parameters": {
        "card_network": {
            "type": "enum_variant",
            "value": "Visa"
        },
        "amount": {
            "type": "number",
            "value": 10000
        }
    }
}'
```

If the input has:

- `amount > 100` **AND**
- **EITHER**:
  - `card_network == Visa` **OR**
  - `billing_country == Netherlands`

🔄 **Then** the rule `"RBL rule"` matches → returns `rbl`.

📆 **Otherwise** → returns fallback `defaultSelection` → `[bambora, Paytm, checkout]`.
