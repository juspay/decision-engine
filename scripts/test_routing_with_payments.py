import uuid
import os
from dotenv import load_dotenv
# Use relative imports to fix Pylance warnings
from .decision_engine_api import DecisionEngineAPI, test_decision_engine_endpoints
from .hyperswitch_api import HyperswitchAPI, setup_and_run_demo
from .payment_operations import (
    simulate_payments, STATUS_MAP, SUCCESS_CARD, FAIL_CARD
)

# Load environment variables from .env file if it exists
load_dotenv()

# Payment simulation settings
TOTAL_PAYMENTS = 30
INITIAL_SUCCESS_PERCENT = 60
INTER_PAYMENT_SLEEP_SEC = 0

DECISION_ENGINE_API = "https://sandbox.juspay.io"
API_BASE_URL = "https://sandbox.hyperswitch.io"
APP_BASE_URL = "https://app.hyperswitch.io"
PAYMENT_URL = f"{API_BASE_URL}/payments"

# Load API credentials from environment or use defaults
PROFILE_ID = os.getenv("PROFILE_ID", "pro_UJ68AkfFHb9gIGqQ8TMR")
BEARER_TOKEN = os.getenv("BEARER_TOKEN", "BEARER TOKEN")
MERCHANT_ID = os.getenv("MERCHANT_ID", "merchant_1721906783")
GIMINI_API_KEY = os.getenv("GIMINI_API_KEY", "")
API_KEY = os.getenv("API_KEY", "API KEY")

# Print configuration information
print(f"🔑 Using API Key: {API_KEY[:5]}... (truncated for security)")
print(f"🆔 Using Merchant ID: {MERCHANT_ID}")
print(f"📋 Using Profile ID: {PROFILE_ID}")

HEADERS = {
    "Content-Type": "application/json",
    "Accept": "application/json",
    "api-key": API_KEY
}

CREATE_CONNECTORS = [
    {"name": "fauxpay", "label": "fauxpay_test"},
    {"name": "paypal_test", "label": "paypal_test"},
    {"name": "pretendpay", "label": "pretendpay_test"},
    {"name": "stripe_test", "label": "stripe_test"}
]
GEMINI_API_URL = f"https://generativelanguage.googleapis.com/v1beta/models/gemini-2.0-flash:generateContent?key={GIMINI_API_KEY}"


# ---------------------------------------------------------------- #


# Initialize the HyperswitchAPI and fetch connector map
api_client = HyperswitchAPI(API_KEY, MERCHANT_ID, BEARER_TOKEN)
CONNECTOR_MAP = api_client.fetch_connector_map(PROFILE_ID)
print("✅ CONNECTOR_MAP loaded:", CONNECTOR_MAP)



def setup_and_run():
    """Main function to set up the environment and run the simulation"""
    setup_and_run_demo(API_KEY, MERCHANT_ID, PROFILE_ID, BEARER_TOKEN, CREATE_CONNECTORS, simulate_payments)


def run_payment_simulation():
    """Run the payment simulation using the modularized components"""
    return simulate_payments(
        total_payments=TOTAL_PAYMENTS,
        initial_success_percent=INITIAL_SUCCESS_PERCENT,
        payment_url=PAYMENT_URL,
        profile_id=PROFILE_ID,
        connector_map=CONNECTOR_MAP,
        headers=HEADERS,
        gemini_api_url=GEMINI_API_URL,
        sleep_sec=INTER_PAYMENT_SLEEP_SEC
    )

if __name__ == "__main__":
    run_payment_simulation()
