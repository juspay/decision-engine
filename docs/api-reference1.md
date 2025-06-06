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

## Advanced Rule Creation Examples
### 1. Volume Split Rule (with fallback)
```
curl --location 'http://127.0.0.1:8080/routing' \
--header 'Content-Type: application/json' \
--header 'api-key: *****' \
--data '
{
   "name": "advanced config",
   "description": "It is my ADVANCED config",
   "profile_id": "pro_rfW0Fv5J0Cct1Bnw2EuS",
   "algorithm": {
       "type": "advanced",
       "data": {
           "defaultSelection": {
               "type": "priority",
               "data": [
                   {
                       "connector": "stripe",
                       "merchant_connector_id": "mca_aHTJXYcakT5Nlx48kuSh"
                   }
               ]
           },
           "rules": [
               {
                   "name": "cybersource first",
                   "connectorSelection": {
                       "type": "volume_split",
                       "data": [
                           {
                               "split": 60,
                               "connector": "cybersource",
                               "merchant_connector_id": "mca_rJu5LzTmK2SjYgoRMWZ4"
                           },
                           {
                               "split": 40,
                               "connector": "stripe",
                               "merchant_connector_id": "mca_aHTJXYcakT5Nlx48kuSh"
                           }
                       ]
                   },
                   "statements": [
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

### 2. Nested Rule with Fallback
```
curl --location 'http://127.0.0.1:8080/routing' \
--header 'Content-Type: application/json' \
--header 'api-key: *****' \
--data '
{
   "name": "advanced config",
   "description": "It is my ADVANCED config",
   "profile_id": "pro_rfW0Fv5J0Cct1Bnw2EuS",
   "algorithm": {
       "type": "advanced",
       "data": {
           "defaultSelection": {
               "type": "priority",
               "data": [
                   {
                       "connector": "stripe",
                       "merchant_connector_id": "mca_aHTJXYcakT5Nlx48kuSh"
                   }
               ]
           },
           "rules": [
               {
                   "name": "cybersource first",
                   "connectorSelection": {
                       "type": "priority",
                       "data": [
                           {
                               "connector": "cybersource",
                               "merchant_connector_id": "mca_rJu5LzTmK2SjYgoRMWZ4"
                           }
                       ]
                   },
                   "statements": [
                       {
                           "condition": [
                               {
                                   "lhs": "upi",
                                   "comparison": "equal",
                                   "value": {
                                       "type": "enum_variant",
                                       "value": "upi_collect"
                                   },
                                   "metadata": {}
                               }
                           ],
                           "nested": [
                               {
                                   "condition": [
                                       {
                                           "lhs": "amount",
                                           "comparison": "greater_than",
                                           "value": {
                                               "type": "number",
                                               "value": 5000
                                           },
                                           "metadata": {}
                                       },
                                       {
                                           "lhs": "currency",
                                           "comparison": "equal",
                                           "value": {
                                               "type": "enum_variant",
                                               "value": "USD"
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
                                               "value": 10000
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
           "metadata": {}
       }
   }
}'
```

### What Happens on Evaluation?

If the input has:

- `upi = upi_collect` **AND**
- **EITHER**:
  - `amount > 5000` **AND** `currency == USD`
  - **OR** `amount > 10000`

🔄 **Then** the rule `"cybersource first"` matches → returns `cybersource`.

📆 **Otherwise** → returns fallback `defaultSelection` → `stripe`.
