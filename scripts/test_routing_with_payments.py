import uuid
import requests
import os
from dotenv import load_dotenv
# Use relative imports to fix Pylance warnings
from decision_engine_api import DecisionEngineAPI, test_decision_engine_endpoints
from hyperswitch_api import HyperswitchAPI, setup_and_run_demo
from payment_operations import (
    simulate_payments, STATUS_MAP, SUCCESS_CARD, FAIL_CARD
)

# Load environment variables from .env file if it exists
load_dotenv()

# Payment simulation settings
INITIAL_SUCCESS_PERCENT = 60
INTER_PAYMENT_SLEEP_SEC = 0

DECISION_ENGINE_API = "https://integ.juspay.io"
API_BASE_URL = "https://integ.hyperswitch.io"
APP_BASE_URL = "https://integ.hyperswitch.io"
PAYMENT_URL = f"{API_BASE_URL}/api/payments"

# Load API credentials from environment or use defaults
PROFILE_ID = os.getenv("PROFILE_ID", "pro_UJ68AkfFHb9gIGqQ8TMR")
BEARER_TOKEN = os.getenv("BEARER_TOKEN", "BEARER TOKEN")
MERCHANT_ID = os.getenv("MERCHANT_ID", "merchant_1721906783")
GIMINI_API_KEY = os.getenv("GIMINI_API_KEY", "")
API_KEY = os.getenv("API_KEY", "API KEY")
BUCKET_SIZE = int(os.getenv("BUCKET_SIZE", 10))
HEDGING_PERCENT = int(os.getenv("HEDGING_PERCENT", 10))
TOTAL_PAYMENTS = int(os.getenv("TOTAL_PAYMENTS", 10))

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

def toggle_decision_engine_in_hs():
    """
    Complete Hyperswitch decision engine setup - all 4 API calls in sequence:
    1. Toggle dynamic connector selection
    2. Set volume split
    3. Update decision engine config (using routing_id from step 1)
    4. Activate routing (using routing_id from step 3)
    """

    # Configuration
    base_url = "https://integ-api.hyperswitch.io"
    merchant_id = MERCHANT_ID
    business_profile_id = PROFILE_ID
    api_key = API_KEY

    # Common headers
    headers = {
        'Content-Type': 'application/json',
        'Accept': 'application/json',
        'api-key': api_key
    }

    try:
        # Step 1: Toggle dynamic connector selection
        print("Step 1: Toggle dynamic connector selection...")
        endpoint1_url = f"{base_url}/account/{merchant_id}/business_profile/{business_profile_id}/dynamic_routing/success_based/toggle"
        params1 = {'enable': 'dynamic_connector_selection'}

        response1 = requests.post(endpoint1_url, headers=headers, params=params1)
        print(f"✅ Toggle Response Status: {response1.status_code}")
        print(f"Toggle Response: {response1.text}")
        print("-" * 60)

        if response1.status_code != 200:
            print("❌ Failed at step 1 - Toggle")
            return response1, None, None, None

        # Extract routing_id from toggle response
        toggle_data = response1.json()
        routing_id = toggle_data.get('id')

        if not routing_id:
            print("❌ No routing_id found in toggle response")
            return response1, None, None, None

        print(f"🔗 Retrieved routing_id: {routing_id}")

        # Step 2: Set volume split
        print("\nStep 2: Set volume split...")
        endpoint2_url = f"{base_url}/account/{merchant_id}/business_profile/{business_profile_id}/dynamic_routing/set_volume_split"
        params2 = {'split': '100'}

        response2 = requests.post(endpoint2_url, headers=headers, params=params2)
        print(f"✅ Volume Split Response Status: {response2.status_code}")
        print(f"Volume Split Response: {response2.text}")
        print("-" * 60)

        if response2.status_code != 200:
            print("❌ Failed at step 2 - Volume Split")
            return response1, response2, None, None

        # Step 3: Update decision engine config using routing_id
        print(f"\nStep 3: Update decision engine config (routing_id: {routing_id})...")
        endpoint3_url = f"{base_url}/account/{merchant_id}/business_profile/{business_profile_id}/dynamic_routing/success_based/config/{routing_id}"

        config_payload = {
            "decision_engine_configs": {
                "defaultLatencyThreshold": 90,
                "defaultSuccessRate": 0.5,
                "defaultBucketSize": 10,
                "defaultHedgingPercent": 10,
                "subLevelInputConfig": [{
                    "paymentMethodType": "UNKNOWN",
                    "paymentMethod": "custom_pm",
                    "bucketSize": 10,
                    "hedgingPercent": 10
                }]
            }
        }

        response3 = requests.patch(endpoint3_url, headers=headers, json=config_payload)
        print(f"✅ Config Update Response Status: {response3.status_code}")
        print(f"Config Update Response: {response3.text}")
        print("-" * 60)

        if response3.status_code not in [200, 201]:
            print("❌ Failed at step 3 - Config Update")
            return response1, response2, response3, None

        # Get final routing_id (might be updated in config response)
        config_data = response3.json()
        final_routing_id = config_data.get('id', routing_id)  # Use original if not found

        print(f"🔗 Final routing_id for activation: {final_routing_id}")

        # Step 4: Activate routing
        print(f"\nStep 4: Activate routing (routing_id: {final_routing_id})...")
        endpoint4_url = f"{base_url}/routing/{final_routing_id}/activate"

        activate_payload = {}  # Empty payload as per API spec

        response4 = requests.post(endpoint4_url, headers=headers, json=activate_payload)
        print(f"✅ Activation Response Status: {response4.status_code}")
        print(f"Activation Response: {response4.text}")
        print("-" * 60)

        # Final status check
        all_successful = (
            response1.status_code == 200 and
            response2.status_code == 200 and
            response3.status_code in [200, 201] and
            response4.status_code in [200, 201]
        )

        if all_successful:
            print("🎉 ALL STEPS COMPLETED SUCCESSFULLY!")
            print("✅ Decision engine fully configured and activated!")
        else:
            print("⚠️  Some steps failed. Check individual responses above.")

        print(f"\n📊 Summary:")
        print(f"  Step 1 (Toggle): {response1.status_code}")
        print(f"  Step 2 (Volume Split): {response2.status_code}")
        print(f"  Step 3 (Config Update): {response3.status_code}")
        print(f"  Step 4 (Activate): {response4.status_code}")
        print(f"  Routing ID Used: {routing_id} → {final_routing_id}")

        return response1, response2, response3, response4

    except requests.exceptions.RequestException as e:
        print(f"❌ Network error: {e}")
        return None, None, None, None
    except json.JSONDecodeError as e:
        print(f"❌ JSON parsing error: {e}")
        return None, None, None, None
    except Exception as e:
        print(f"❌ Unexpected error: {e}")
        return None, None, None, None

def call_decision_engine_apis(merchant_id, base_url="https://integ-api.hyperswitch.io"):
    """
    Call localhost decision engine APIs:
    1. Create merchant account
    2. Create rule configuration

    Args:
        merchant_id (str): Merchant ID to use for both endpoints
        base_url (str): Base URL for the API (default: http://localhost:8080)
    """

    try:
        # Step 1: Create merchant account
        print("Step 1: Creating merchant account...")
        merchant_url = f"{base_url}/merchant-account/create"

        merchant_headers = {
            'x-feature': 'decision-engine',
            'Content-Type': 'application/json'
        }

        merchant_payload = {
            "merchant_id": merchant_id
        }

        merchant_response = requests.post(merchant_url, headers=merchant_headers, json=merchant_payload)

        print(f"✅ Merchant Account Response Status: {merchant_response.status_code}")
        print(f"Merchant Account Response: {merchant_response.text}")
        print("-" * 60)

        if merchant_response.status_code not in [200, 201]:
            print("❌ Failed to create merchant account")
            return merchant_response, None

        # Step 2: Create rule configuration
        print("Step 2: Creating rule configuration...")
        rule_url = f"{base_url}/rule/create"

        rule_headers = {
            'x-feature': 'decision-engine',
            'Content-Type': 'application/json'
        }

        rule_payload = {
            "merchant_id": merchant_id,
            "config": {
                "type": "successRate",
                "data": {
                    "defaultLatencyThreshold": 90,
                    "defaultSuccessRate": 0.5,
                    "defaultBucketSize": BUCKET_SIZE,
                    "defaultHedgingPercent": HEDGING_PERCENT
                }
            }
        }

        rule_response = requests.post(rule_url, headers=rule_headers, json=rule_payload)

        print(f"✅ Rule Creation Response Status: {rule_response.status_code}")
        print(f"Rule Creation Response: {rule_response.text}")
        print("-" * 60)

        # Final status check
        both_successful = (
            merchant_response.status_code in [200, 201] and
            rule_response.status_code in [200, 201]
        )

        if both_successful:
            print("🎉 BOTH ENDPOINTS CALLED SUCCESSFULLY!")
            print(f"✅ Merchant account created and rule configured for: {merchant_id}")
        else:
            print("⚠️  One or both API calls failed. Check responses above.")

        print(f"\n📊 Summary:")
        print(f"  Step 1 (Merchant Account): {merchant_response.status_code}")
        print(f"  Step 2 (Rule Creation): {rule_response.status_code}")
        print(f"  Merchant ID: {merchant_id}")

        return merchant_response, rule_response

    except requests.exceptions.ConnectionError as e:
        print(f"❌ Connection error - Is the server running on {base_url}? Error: {e}")
        return None, None
    except requests.exceptions.RequestException as e:
        print(f"❌ Network error: {e}")
        return None, None
    except json.JSONDecodeError as e:
        print(f"❌ JSON parsing error: {e}")
        return None, None
    except Exception as e:
        print(f"❌ Unexpected error: {e}")
        return None, None


if __name__ == "__main__":
    # toggle_decision_engine_in_hs()
    call_decision_engine_apis(PROFILE_ID)
    run_payment_simulation()
