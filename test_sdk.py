from decision_engine_sdk.client import DecisionEngineClient
import requests

client = DecisionEngineClient(base_url="http://localhost:8080")

# Test decide_gateway
decide_payload = {
    "merchantId": "test_merchant_1",
    "eligibleGatewayList": ["GatewayA", "GatewayB", "GatewayC"],
    "rankingAlgorithm": "SR_BASED_ROUTING",
    "eliminationEnabled": True,
    "paymentInfo": {
        "paymentId": "PAY12359",
        "amount": 100.50,
        "currency": "USD",
        "customerId": "CUST12345",
        "paymentType": "ORDER_PAYMENT",
        "paymentMethodType": "UPI",
        "paymentMethod": "UPI_PAY",
        "isEmi": False
    }
}

try:
    decide_response = client.decide_gateway(decide_payload)
    print("Decide Gateway Response:")
    print(decide_response)
except requests.HTTPError as e:
    print("Error in decide_gateway:", e)
    if e.response is not None:
        print("Server response:", e.response.text)
except Exception as e:
    print("Unexpected error in decide_gateway:", e)

# Test update_gateway_score
score_payload = {
    "merchantId": "test_merchant_1",
    "gateway": "RAZORPAY",
    "status": "FAILURE",
    "paymentId": "PAY12359"
}

try:
    score_response = client.update_gateway_score(score_payload)
    print("Update Gateway Score Response:")
    print(score_response)
except requests.HTTPError as e:
    print("Error in update_gateway_score:", e)
    if e.response is not None:
        print("Server response:", e.response.text)
except Exception as e:
    print("Unexpected error in update_gateway_score:", e) 