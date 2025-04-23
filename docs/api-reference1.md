API REFERENCE

Sample curl for decide-gateway

Request:

curl --location 'http://localhost:8080/decide-gateway' \
--header 'Content-Type: application/json' \
--data '{ 
        "merchantId": "test_merchant1",<br>
        "eligibleGatewayList": ["GatewayA", "GatewayB", "GatewayC"],<br>
        "rankingAlgorithm": "SR_BASED_ROUTING",<br>
        "eliminationEnabled": true,<br>
        "paymentInfo": {<br>
            "paymentId": "PAY12359",<br>
            "amount": 100.50,<br>
            "currency": "USD",<br>
            "customerId": "CUST12345",<br>
            "udfs": null,<br>
            "preferredGateway": null,<br>
            "paymentType": "ORDER_PAYMENT",<br>
            "metadata": null,<br>
            "internalMetadata": null,<br>
            "isEmi": false,<br>
            "emiBank": null,<br>
            "emiTenure": null,<br>
            "paymentMethodType": "UPI",<br>
            "paymentMethod": "UPI_PAY",<br>
            "paymentSource": null,<br>
            "authType": null,<br>
            "cardIssuerBankName": null,<br>
            "cardIsin": null,<br>
            "cardType": null,<br>
            "cardSwitchProvider": null<br>
        }<br>
}'<br>

Response:

{<br>
    "decided_gateway": "GatewayA",<br>
    "gateway_priority_map": {<br>
        "GatewayA": 1.0,<br>
        "GatewayB": 1.0,<br>
        "GatewayC": 1.0<br>
    },<br>
    "filter_wise_gateways": null,<br>
    "priority_logic_tag": null,<br>
    "routing_approach": "SR_SELECTION_V3_ROUTING",<br>
    "gateway_before_evaluation": "GatewayA",<br>
    "priority_logic_output": {<br>
        "isEnforcement": false,<br>
        "gws": [<br>
            "GatewayA",<br>
            "GatewayB",<br>
            "GatewayC"<br>
        ],<br>
        "priorityLogicTag": null,<br>
        "gatewayReferenceIds": {},<br>
        "primaryLogic": null,<br>
        "fallbackLogic": null<br>
    },<br>
    "reset_approach": "NO_RESET",<br>
    "routing_dimension": "ORDER_PAYMENT, UPI, UPI_PAY",<br>
    "routing_dimension_level": "PM_LEVEL",<br>
    "is_scheduled_outage": false,<br>
    "is_dynamic_mga_enabled": false,<br>
    "gateway_mga_id_map": null<br>
}<br>


Sample curl for update-gateway-score<br>

curl --location 'http://localhost:8080/update-gateway-score' \
--header 'Content-Type: application/json' \
--data '{
  "merchantId" : "test_merchant1",
  "gateway": "RAZORPAY",<br>
  "gatewayReferenceId": null,<br>
  "status": "FAILURE",<br>
  "paymentId": "PAY12359",<br>
  "enforceDynamicRoutingFailure" : null<br>
}'
<br>

Response:

Success
