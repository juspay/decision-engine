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
    "merchant_id": "test_merchant_123",
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
           "merchant_id": "test_merchant_123",
           "algorithm": "successRate"
         }'
```

#### Response:
```json
{
   "merchant_id": "test_merchant_123",
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
    "merchant_id": "test_merchant_123",
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
           "merchant_id": "test_merchant_123",
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
    "merchant_id": "test_merchant_123",
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
           "merchant_id": "test_merchant_123",
           "algorithm": "elimination"
         }'
```

#### Response:
```json
{
    "merchant_id": "test_merchant_123",
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
    "merchant_id": "test_merchant_123",
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
           "merchant_id": "test_merchant_123",
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
  "merchant_id": "test_merchant_123"  
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
curl -X GET http://localhost:8080/merchant-account/test_merchant_123            
```

#### Response:
```json
{
    "merchant_id": "test_merchant_123",
    "gateway_success_rate_based_decider_input": null
}
```

#### Request: Merchant account delete
```bash
curl -X DELETE http://localhost:8080/merchant-account/test_merchant_123  
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
curl --location 'http://localhost:8080/routing/create' \
--header 'Content-Type: application/json' \
--data '{
   "name": "Priority Based Config",
   "created_by": "merchant_1",
   "algorithm": {
       "globals": {},
       "defaultSelection": {
           "priority": ["stripe", "adyen", "checkout"]
       },
       "rules": [
           {
               "name": "Card Rule",
               "routingType": "priority",
               "output": {
                   "priority": ["stripe", "adyen"]
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
                           },
                           {
                               "lhs": "amount",
                               "comparison": "greater_than",
                               "value": {
                                   "type": "number",
                                   "value": 1000
                               },
                               "metadata": {}
                           }
                       ]
                   }
               ]
           }
       ],
       "metadata": {}
   }
}'
```

### Response:
```
{
   "rule_id": "routing_e641380c-6f24-4405-8454-5ae6cbceb7a0",
   "name": "Priority Based Config",
   "created_at": "2025-04-22 11:45:03.411134513",
   "modified_at": "2025-04-22 11:45:03.411134513"
}
```

## Activate Routing rule for a creator_id.
### Request
```
curl --location 'http://localhost:8080/routing/activate' \
--header 'Content-Type: application/json' \
--data '{
    "created_by": "merchant_1",
    "routing_algorithm_id": "routing_49ffa266-28ca-40be-9b90-f978190aa571"
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
 "created_by": "merchant_1",
 "parameters": {
   "payment_method": {
     "type": "enum_variant",
     "value": "card"
   },
   "amount": {
     "type": "number",
     "value": 100
   }
 }
}
'
```


### Response:
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
        "id": "routing_7db79cc9-c49a-4f94-8e1c-5ed19c2388df",
        "created_by": "merchant_1234",
        "name": "My Algo",
        "description": "Test algo",
        "algorithm_data": {
            "rules": [
                {
                    "name": "Card Rule",
                    "output": {
                        "priority": [
                            "stripe",
                            "adyen"
                        ]
                    },
                    "statements": [
                        {
                            "nested": null,
                            "condition": [
                                {
                                    "lhs": "payment_method",
                                    "value": {
                                        "type": "enum_variant",
                                        "value": "card"
                                    },
                                    "metadata": {},
                                    "comparison": "equal"
                                },
                                {
                                    "lhs": "amount",
                                    "value": {
                                        "type": "number",
                                        "value": 1000
                                    },
                                    "metadata": {},
                                    "comparison": "greater_than"
                                }
                            ]
                        }
                    ],
                    "routingType": "priority"
                }
            ],
            "globals": {},
            "metadata": {},
            "defaultSelection": {
                "priority": [
                    "stripe",
                    "adyen",
                    "checkout"
                ]
            }
        },
        "created_at": "2025-04-24 09:30:44.0",
        "modified_at": "2025-04-24 09:30:44.0"
    },
    {
        "id": "routing_b15157b3-5c3d-4ea7-b2af-6dc773ff1bf6",
        "created_by": "merchant_1234",
        "name": "My Algo",
        "description": "Test algo",
        "algorithm_data": {
            "rules": [
                {
                    "name": "Card Rule",
                    "output": {
                        "priority": [
                            "stripe",
                            "adyen"
                        ]
                    },
                    "statements": [
                        {
                            "nested": null,
                            "condition": [
                                {
                                    "lhs": "payment_method",
                                    "value": {
                                        "type": "enum_variant",
                                        "value": "card"
                                    },
                                    "metadata": {},
                                    "comparison": "equal"
                                },
                                {
                                    "lhs": "amount",
                                    "value": {
                                        "type": "number",
                                        "value": 1000
                                    },
                                    "metadata": {},
                                    "comparison": "greater_than"
                                }
                            ]
                        }
                    ],
                    "routingType": "priority"
                }
            ],
            "globals": {},
            "metadata": {},
            "defaultSelection": {
                "priority": [
                    "stripe",
                    "adyen",
                    "checkout"
                ]
            }
        },
        "created_at": "2025-04-24 08:50:26.0",
        "modified_at": "2025-04-24 08:50:26.0"
    }
]
```

## List active Routing rule for a creator_id.
### Request
```
curl --location --request POST 'http://localhost:8080/routing/list/active/merchant_1' \
--header 'Content-Type: application/json'
```

### Response
```
{
    "id": "routing_49ffa266-28ca-40be-9b90-f978190aa571",
    "created_by": "merchant_1",
    "name": "My Algo",
    "description": "Test algo",
    "algorithm_data": {
        "rules": [
            {
                "name": "Card Rule",
                "output": {
                    "priority": [
                        "stripe",
                        "adyen"
                    ]
                },
                "statements": [
                    {
                        "nested": null,
                        "condition": [
                            {
                                "lhs": "payment_method",
                                "value": {
                                    "type": "enum_variant",
                                    "value": "card"
                                },
                                "metadata": {},
                                "comparison": "equal"
                            },
                            {
                                "lhs": "amount",
                                "value": {
                                    "type": "number",
                                    "value": 10000
                                },
                                "metadata": {},
                                "comparison": "greater_than"
                            }
                        ]
                    }
                ],
                "routingType": "priority"
            }
        ],
        "globals": {},
        "metadata": {},
        "defaultSelection": {
            "priority": [
                "stripe",
                "adyen",
                "checkout"
            ]
        }
    },
    "created_at": "2025-04-24 10:26:58.0",
    "modified_at": "2025-04-24 10:26:58.0"
}
```

### 1. Volume split rule with fallback
```
curl --location 'http://127.0.0.1:8082/routing/create' \
--header 'Content-Type: application/json' \
--data '
{
    "id": "12345",
    "name": "Volume split based config",
    "created_by": "merchant_1",
    "description": "test",
    "algorithm": {
        "globals": {},
        "defaultSelection": {
            "priority": [
                "bambora",
                "Paytm",
                "checkout"
            ]
        },
        "rules": [
            {
                "name": "HDFC Rule",
                "routingType": "volume_split",
                "output": {
                    "volumeSplit": [
                        {
                            "split": 60,
                            "output": "hdfc"
                        },
                        {
                            "split": 40,
                            "output": "instamojo"
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
                        "nested": [
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
                                        "lhs": "payment_method",
                                        "comparison": "equal",
                                        "value": {
                                            "type": "enum_variant",
                                            "value": "upi"
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
        "transaction_type": "payment"
    }
}'
```

### What Happens on Evaluation?

```
curl --location '{base_url}/routing/evaluate' \
--header 'Content-Type: application/json' \
--data '{
    "created_by": "merchant_1",
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
  - `billing_country == Netherland`
- **EITHER**:
  - `payment_method == card` **OR**
  - `payment_method == upi`

🔄 **Then** the rule `"Hdfc rule"` matches → returns volume split between `hdfc` and `instamojo`.
📆 **Otherwise** → returns fallback `defaultSelection` → `[bambora, Paytm, checkout]`.


### 2. Nested Rule with Fallback
```
curl --location 'http://127.0.0.1:8082/routing/create' \
--header 'Content-Type: application/json' \
--data '
{
    "id": "12345",
    "name": "AND OR rule example",
    "created_by": "merchant_1",
    "description": "priority rule which demonstrates AND and OR rule",
    "algorithm": {
        "globals": {},
        "defaultSelection": {
            "priority": [
                "bambora",
                "Paytm",
                "checkout"
            ]
        },
        "rules": [
            {
                "name": "Card Rule",
                "routingType": "priority",
                "output": {
                    "priority": [
                        "rbl",
                        "instamojo"
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
                                        "lhs": "payment_method",
                                        "comparison": "equal",
                                        "value": {
                                            "type": "enum_variant",
                                            "value": "upi"
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
        "metadata": {"transaction":"data"}
    },
    "metadata": {"transaction_type": "payout"}
}'
```

### What Happens on Evaluation?
```
curl --location '{base_url}/routing/evaluate' \
--header 'Content-Type: application/json' \
--data '{
    "created_by": "merchant_1",
    "parameters": {
        "payment_method": {
            "type": "enum_variant",
            "value": "card"
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
  - `payment_method == card` **OR**
  - `payment_method == upi`

🔄 **Then** the rule `"RBL rule"` matches → returns `rbl`.

📆 **Otherwise** → returns fallback `defaultSelection` → `[bambora, Paytm, checkout]`.
