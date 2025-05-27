import requests
import random
import json
import time

# ---------------------------- CONFIGURABLE PARAMETERS ---------------------------- #
TOTAL_PAYMENTS = 200               # Total number of payments
INITIAL_SUCCESS_PERCENT = 60      # Success % during normal phases
INITIAL_DELAY_SEC = 0             # Delay before first payment
INTER_PAYMENT_SLEEP_SEC = 0       # Delay between each payment
# ------------------------------------------------------------------------------- #

API_URL = "https://sandbox.hyperswitch.io/payments"
API_KEY = "API_KEY"
PROFILE_ID = "PROFILE_ID"
HEADERS = {
    "Content-Type": "application/json",
    "Accept": "application/json",
    "api-key": API_KEY
}

SUCCESS_CARD = {"number": "4242424242424242", "label": "success"}
FAIL_CARD = {"number": "4000000000000002", "label": "fail"}

def build_card_pool(success_percent):
    fail_percent = 100 - success_percent
    return [SUCCESS_CARD] * success_percent + [FAIL_CARD] * fail_percent

def generate_payload(card_number):
    return {
        "amount": 640,
        "currency": "USD",
        "confirm": True,
        "capture_method": "automatic",
        "profile_id": PROFILE_ID,
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

def simulate_payments():
    force_fail_start = TOTAL_PAYMENTS // 2
    force_fail_end = force_fail_start + (TOTAL_PAYMENTS // 4)

    print(f"🔁 Simulating {TOTAL_PAYMENTS} payments...")
    print(f"🟢 First {force_fail_start} payments = {INITIAL_SUCCESS_PERCENT}% success")
    print(f"🔴 Next {force_fail_end - force_fail_start} payments = 100% failures to trigger SR")
    print(f"🟢 Last {TOTAL_PAYMENTS - force_fail_end} payments = {INITIAL_SUCCESS_PERCENT}% success again\n")

    time.sleep(INITIAL_DELAY_SEC)

    for i in range(1, TOTAL_PAYMENTS + 1):
        if i <= force_fail_start:
            card_pool = build_card_pool(INITIAL_SUCCESS_PERCENT)
        elif i <= force_fail_end:
            card_pool = build_card_pool(0)  # 100% fail
        else:
            card_pool = build_card_pool(INITIAL_SUCCESS_PERCENT)

        card = random.choice(card_pool)
        payload = generate_payload(card["number"])

        try:
            response = requests.post(API_URL, headers=HEADERS, data=json.dumps(payload))
            resp_json = response.json()

            payment_id = resp_json.get("payment_id", "")
            connector = resp_json.get("connector", "")
            status = resp_json.get("status", "")
            error_code = resp_json.get("error_code", "")
            error_message = resp_json.get("error_message", "")

            print(f"[{i}] Card: {card['label'].upper()} | Status: {status} | Connector: {connector} | Error: {error_message or 'None'}")

        except Exception as e:
            print(f"[{i}] ❌ Request failed: {str(e)}")

        time.sleep(INTER_PAYMENT_SLEEP_SEC)

if __name__ == "__main__":
    simulate_payments()

