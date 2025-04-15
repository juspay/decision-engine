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
