import requests
import random
import json
import time
import textwrap

# Constants and types for payment operations
STATUS_MAP = {
    "charged": "CHARGED",
    "succeeded": "CHARGED",
    "authorized": "AUTHORIZED",
    "failed": "FAILURE",
    "declined": "DECLINED"
}

SUCCESS_CARD = {"number": "4242424242424242", "label": "success"}
FAIL_CARD = {"number": "4000000000000002", "label": "fail"}

def build_connector_card_pool(success_rates):
    card_pools = {}
    for connector, success_percent in success_rates.items():
        pool = [SUCCESS_CARD] * success_percent + [FAIL_CARD] * (100 - success_percent)
        card_pools[connector] = pool
    return card_pools

def get_user_defined_success_rates(connector_map):
    print("\n🎯 Define success rates for each connector (0-100):")
    success_rates = {}
    for connector in connector_map:
        while True:
            try:
                rate = int(input(f"  ➤ {connector}: "))
                if 0 <= rate <= 100:
                    success_rates[connector] = rate
                    break
                else:
                    print("Please enter a value between 0 and 100.")
            except ValueError:
                print("Invalid input. Please enter an integer.")
    return success_rates

def build_card_pool(success_percent):
    """Build a pool of cards with the given success percentage"""
    return [SUCCESS_CARD] * success_percent + [FAIL_CARD] * (100 - success_percent)

def decide_gateway(payment_id, connector_map, juspay_url='https://sandbox.juspay.in/decide-gateway'):
    """Call the decide-gateway endpoint to get the gateway decision"""
    headers = {
        'Content-Type': 'application/json',
        'x-merchantid': 'hyperswitchTest',
    }
    payload = {
        "merchantId": "hyperswitchTest",
        "eligibleGatewayList": list(connector_map.keys()),
        "rankingAlgorithm": "SR_BASED_ROUTING",
        "eliminationEnabled": True,
        "paymentInfo": {
            "paymentId": payment_id,
            "amount": 100.50,
            "currency": "USD",
            "customerId": "CUST12345",
            "paymentType": "ORDER_PAYMENT",
            "paymentMethodType": "CARD",
            "paymentMethod": "VISA"
        }
    }
    try:
        response = requests.post(juspay_url, headers=headers, data=json.dumps(payload))
        data = response.json()

        priority_map = data.get("gateway_priority_map", {})
        print(f"priority_map:{priority_map}")
        if priority_map:
            print("📊 Connector Success Percentages:")
            for connector, score in priority_map.items():
                print(f"   - {connector}: {int(score * 100)}%")

        return data.get("decided_gateway"), data.get("routing_approach", "UNKNOWN")

    except Exception as e:
        print(f"❌ Failed to call decide-gateway: {e}")
        return None, None

def update_gateway_score(gateway, status, payment_id, juspay_url='https://sandbox.juspay.in/update-gateway-score'):
    """Update the gateway score based on payment result"""
    headers = {
        'Content-Type': 'application/json',
    }
    payload = {
        "merchantId": "hyperswitchTest",
        "gateway": gateway,
        "gatewayReferenceId": None,
        "status": status,
        "paymentId": payment_id,
        "enforceDynamicRoutingFailure": None
    }

    try:
        response = requests.post(juspay_url, headers=headers, data=json.dumps(payload))
        return {
            "status_code": response.status_code,
            "text": response.text.strip()
        }
    except Exception as e:
        return {"error": str(e)}

def generate_payload(card_number, connector_name, mca_id, profile_id):
    """Generate the payment payload for processing a payment"""
    return {
        "amount": 640,
        "currency": "USD",
        "routing": {
            "type": "single",
            "data": {
                "connector": connector_name,
                "merchant_connector_id": mca_id
            }
        },
        "confirm": True,
        "capture_method": "automatic",
        "profile_id": profile_id,
        "capture_on": "2022-09-10T10:11:12Z",
        "amount_to_capture": 640,
        "customer_id": "cus_jkjdkjakd",
        "email": "guest@example.com",
        "name": "pklllll",
        "phone": "999999999",
        "phone_country_code": "+1",
        "description": "Demo SR payment",
        "authentication_type": "no_three_ds",
        "return_url": "https://duck.com",
        "payment_method": "card",
        "payment_method_type": "credit",
        "payment_method_data": {
            "card": {
                "card_number": card_number,
                "card_exp_month": "10",
                "card_exp_year": "25",
                "card_holder_name": "joseph Doe",
                "card_cvc": "123"
            }
        },
        "billing": {
            "address": {
                "line1": "1467",
                "line2": "Harrison Street",
                "line3": "Harrison Street",
                "city": "San Fransico",
                "state": "California",
                "zip": "94122",
                "country": "IN",
                "first_name": "joseph",
                "last_name": "Doe"
            },
            "phone": {
                "number": "8056594427",
                "country_code": "+91"
            },
            "email": "example@example.com"
        },
        "browser_info": {
            "user_agent": "Mozilla/5.0",
            "accept_header": "text/html",
            "language": "en-US",
            "color_depth": 24,
            "screen_height": 1080,
            "screen_width": 1920,
            "time_zone": 0,
            "java_enabled": True,
            "java_script_enabled": True,
            "ip_address": "127.0.0.1"
        },
        "statement_descriptor_name": "joseph",
        "statement_descriptor_suffix": "JS",
        "metadata": {
            "udf1": "value1",
            "new_customer": "true",
            "login_date": "2019-09-10T10:11:12Z"
        }
    }

def send_logs_to_gemini(payment_results, gemini_api_url):
    """Send payment logs to Gemini for analysis"""
    prompt_text = textwrap.dedent(f"""
    You are an AI analyst. The following is a list of payment simulation logs.
    Each entry contains the payment ID, card type, selected gateway, payment status, and error message.
    Please generate a summary report that includes:
    - Total payments
    - Number of successes and failures
    - Success percentage per connector
    - Most common failure reasons
    - Ratio of exploitation vs exploration
    - Suggestions to improve routing or connector reliability
    Respond in markdown format.

    Logs:
    {json.dumps(payment_results, indent=2)}
    """)

    payload = {
        "contents": [
            {
                "parts": [{"text": prompt_text}]
            }
        ]
    }

    headers = {
        "Content-Type": "application/json"
    }

    try:
        response = requests.post(gemini_api_url, headers=headers, json=payload)
        if response.status_code == 200:
            report = response.json()["candidates"][0]["content"]["parts"][0]["text"]
            print("\n📋 Gemini AI Report:\n")
            print(report)
        else:
            print(f"❌ Gemini API error: {response.status_code}")
            print(response.text)
    except Exception as e:
        print(f"❌ Gemini API request failed: {str(e)}")

def simulate_payments(total_payments=30, initial_success_percent=60, 
                     payment_url=None, profile_id=None, connector_map=None,
                     headers=None, gemini_api_url=None, sleep_sec=0):
    """Simulate a series of payments using routing decisions"""
    payment_results = []

    force_fail_start = total_payments // 2
    force_fail_end = force_fail_start + (total_payments // 4)

    success_rates = get_user_defined_success_rates(connector_map)
    card_pools = build_connector_card_pool(success_rates)

    print(f"🔁 Starting payment simulation: {total_payments} payments\n")

    for i in range(1, total_payments + 1):
        payment_id = f"PAY_SIM_{i:05d}"

        print(f"\n🔸 Payment {i}: ID = {payment_id}")
        decided_gateway, routing_approach = decide_gateway(payment_id, connector_map)
        card_pool = card_pools.get(decided_gateway, [SUCCESS_CARD] * 50 + [FAIL_CARD] * 50)
        card = random.choice(card_pool)
        if not decided_gateway:
            print(f"❌ Gateway not decided for {payment_id}")
            continue

        mca_id = connector_map.get(decided_gateway)
        if not mca_id:
            print(f"❌ MCA ID not found for connector: {decided_gateway}")
            continue

        payload = generate_payload(card["number"], decided_gateway, mca_id, profile_id)

        try:
            response = requests.post(payment_url, headers=headers, data=json.dumps(payload))
            resp_json = response.json()
        except Exception as e:
            print(f"❌ Payment request failed: {e}")
            continue

        raw_status = resp_json.get("status", "").lower()
        status = STATUS_MAP.get(raw_status, "FAILURE")
        error_message = resp_json.get("error_message", "None")

        mode = "Exploitation" if routing_approach == "SR_SELECTION_V3_ROUTING" else "Exploration"
        print(f"✅ Card: {card['label'].upper()} | Gateway: {decided_gateway} | routing_approach: {routing_approach} | Mode: {mode} | Status: {status} | Error: {error_message}")

        payment_results.append({
            "payment_id": payment_id,
            "card_type": card["label"].upper(),
            "gateway": decided_gateway,
            "status": status,
            "error": error_message,
            "routing_approach": routing_approach,
            "mode": "Exploitation" if routing_approach == "SR_SELECTION_V3_ROUTING" else "Exploration"
        })

        update_gateway_score(decided_gateway, status, payment_id)

        time.sleep(sleep_sec)

    if gemini_api_url:
        send_logs_to_gemini(payment_results, gemini_api_url)
    
    return payment_results
