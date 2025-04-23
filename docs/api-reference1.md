# API REFERENCE

## Decision Gateway API

### Sample curl for decide-gateway

#### Request:
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

# 🚦 Euclid Routing Engine

**Euclid** is a pluggable, dynamic routing rule evaluation engine designed to power **payment connector selection** based on customizable business rules.

It enables merchants and platforms to define their own routing algorithms—such as **priority-based**, **volume-split**, or **hybrid logic**—and evaluate transaction parameters against them **in real time**.

---

## ✅ Features

- 🔧 **Flexible DSL (Domain-Specific Language)** for defining complex routing logic  
- 📡 **APIs to create, update, and evaluate** routing algorithms dynamically  
- 🧠 **Condition-based evaluation** using payment metadata (e.g. method type, amount, etc.)

---

## 💡 Use Cases

- 🎯 **Prioritizing gateways** based on card type, transaction amount, or other dynamic criteria  
- 🔁 **Implementing fallback strategies** for gateway outages or errors  
- ⚙️ **Adapting routing behavior** without code changes or redeployments

---

## Create Routing Algorithm (Euclid):
### Request:
```
curl --location 'http://localhost:8080/routing/create' \
--header 'Content-Type: application/json' \
--data '{
   "name": "Priority Based Config",
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

## Evaluate Payment paramenters using Routing Algorithm (Euclid):
### Request:
```
curl --location 'http://localhost:8080/routing/evaluate' \
--header 'Content-Type: application/json' \
--data '{
 "routing_id": "routing_3cfeb35f-dcd8-40f9-9ad9-542874a662d8",
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
