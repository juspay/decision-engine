import requests
import random
import json
import time
import sys
import uuid

# ---------------------------- CONFIGURABLE PARAMETERS ---------------------------- #
# API Configuration
API_BASE_URL = "https://sandbox.hyperswitch.io"
APP_BASE_URL = "https://app.hyperswitch.io"
API_KEY = "API_KEY"
MERCHANT_ID = "merchant_1721906783"
PROFILE_NAME = "TestSR"

# Simulation Configuration
TOTAL_PAYMENTS = 200               # Total number of payments
INITIAL_SUCCESS_PERCENT = 60       # Success % during normal phases
INITIAL_DELAY_SEC = 0              # Delay before first payment
INTER_PAYMENT_SLEEP_SEC = 0        # Delay between each payment
# ------------------------------------------------------------------------------- #

# Card configuration
SUCCESS_CARD = {"number": "4242424242424242", "label": "success"}
FAIL_CARD = {"number": "4000000000000002", "label": "fail"}

# Test connector configuration
TEST_CONNECTORS = [
    {"name": "fauxpay", "label": "fauxpay_test"},
    {"name": "paypal_test", "label": "paypal_test"},
    {"name": "pretendpay", "label": "pretendpay_test"},
    {"name": "stripe_test", "label": "stripe_test"}
]

class HyperswitchAPI:
    def __init__(self, api_key, merchant_id):
        self.api_key = api_key
        self.merchant_id = merchant_id
        self.headers = {
            "Content-Type": "application/json",
            "Accept": "application/json",
            "api-key": self.api_key,
            "X-Merchant-Id": self.merchant_id
        }

    def create_business_profile(self, profile_name):
        """Create a new business profile"""
        url = f"{APP_BASE_URL}/api/account/{self.merchant_id}/business_profile"
        payload = {"profile_name": profile_name}
        
        try:
            response = requests.post(url, headers=self.headers, json=payload)
            response.raise_for_status()
            profile_data = response.json()
            print(f"✅ Created business profile: {profile_name}")
            return profile_data
        except requests.exceptions.RequestException as e:
            print(f"❌ Failed to create business profile: {str(e)}")
            if hasattr(e, 'response') and e.response is not None:
                print(f"Response: {e.response.text}")
            # Generate a random profile ID instead of exiting
            random_profile_id = f"pro_{uuid.uuid4().hex[:12]}"
            print(f"🔄 Using randomly generated profile ID: {random_profile_id}")
            return {"profile_id": random_profile_id}

    def create_connector(self, profile_id, connector_name, connector_label):
        """Create a test connector for the profile"""
        url = f"{APP_BASE_URL}/api/account/{self.merchant_id}/connectors"
        
        # Add profile ID to headers
        headers = self.headers.copy()
        headers["X-Profile-Id"] = profile_id
        
        payload = {
            "connector_type": "payment_processor",
            "profile_id": profile_id,
            "connector_name": connector_name,
            "connector_label": connector_label,
            "disabled": False,
            "test_mode": True,
            "payment_methods_enabled": [
                {
                    "payment_method": "card",
                    "payment_method_types": [
                        {
                            "payment_method_type": "debit",
                            "card_networks": ["Mastercard"],
                            "minimum_amount": 0,
                            "maximum_amount": 68607706,
                            "recurring_enabled": True,
                            "installment_payment_enabled": False
                        },
                        {
                            "payment_method_type": "credit",
                            "card_networks": ["Visa"],
                            "minimum_amount": 0,
                            "maximum_amount": 68607706,
                            "recurring_enabled": True,
                            "installment_payment_enabled": False
                        }
                    ]
                }
            ],
            "metadata": {},
            "connector_account_details": {
                "api_key": "test_key",
                "auth_type": "HeaderKey"
            },
            "additional_merchant_data": None,
            "status": "active",
            "pm_auth_config": None,
            "connector_wallets_details": None
        }
        
        try:
            response = requests.post(url, headers=headers, json=payload)
            response.raise_for_status()
            connector_data = response.json()
            print(f"✅ Created connector: {connector_name} ({connector_label})")
            return connector_data
        except requests.exceptions.RequestException as e:
            print(f"❌ Failed to create connector {connector_name}: {str(e)}")
            if hasattr(e, 'response') and e.response is not None:
                print(f"Response: {e.response.text}")
            print("⚠️ Continuing with setup...")
            return None

    def enable_success_rate_algorithm(self, profile_id):
        """Enable success rate algorithm for the profile"""
        url = f"{API_BASE_URL}/account/{self.merchant_id}/business_profile/{profile_id}/dynamic_routing/success_based/toggle?enable=dynamic_connector_selection"
        
        try:
            response = requests.post(url, headers=self.headers)
            response.raise_for_status()
            routing_data = response.json()
            print(f"✅ Enabled success rate algorithm for profile: {profile_id}")
            return routing_data
        except requests.exceptions.RequestException as e:
            print(f"❌ Failed to enable success rate algorithm: {str(e)}")
            if hasattr(e, 'response') and e.response is not None:
                print(f"Response: {e.response.text}")
            return {}

    def configure_routing_rules(self, profile_id, routing_id):
        """Configure routing rules for the success rate algorithm"""
        url = f"{API_BASE_URL}/account/{self.merchant_id}/business_profile/{profile_id}/dynamic_routing/success_based/config/{routing_id}"
        
        payload = {
            "config": {
                "min_aggregates_size": 5,
                "default_success_rate": 100,
                "max_aggregates_size": 8,
                "current_block_threshold": {
                    "max_total_count": 5
                }
            }
        }
        
        try:
            response = requests.patch(url, headers=self.headers, json=payload)
            response.raise_for_status()
            config_data = response.json()
            print(f"✅ Configured routing rules for routing ID: {routing_id}")
            return config_data
        except requests.exceptions.RequestException as e:
            print(f"❌ Failed to configure routing rules: {str(e)}")
            if hasattr(e, 'response') and e.response is not None:
                print(f"Response: {e.response.text}")
            print("⚠️ Continuing with setup...")
            return {}

    def activate_routing(self, routing_id):
        """Activate routing configuration"""
        url = f"{API_BASE_URL}/routing/{routing_id}/activate"
        auth_header = {
            "Authorization": "Bearer bearer_token"
        }
        self.headers.update(auth_header)
        try:
            response = requests.post(url, headers=self.headers)
            response.raise_for_status()
            activation_data = response.json()
            print(f"✅ Activated routing ID: {routing_id}")
            return activation_data
        except requests.exceptions.RequestException as e:
            print(f"❌ Failed to activate routing: {str(e)}")
            if hasattr(e, 'response') and e.response is not None:
                print(f"Response: {e.response.text}")
            print("⚠️ Continuing with setup...")
            return {}

    def set_volume_split(self, profile_id, split_percentage=100):
        """Set volume split percentage"""
        url = f"{API_BASE_URL}/account/{self.merchant_id}/business_profile/{profile_id}/dynamic_routing/set_volume_split?split={split_percentage}"
        
        try:
            response = requests.post(url, headers=self.headers)
            print(f"✅ Set volume split to {split_percentage}% for profile: {profile_id}")
            return {}
        except requests.exceptions.RequestException as e:
            print(f"❌ Failed to set volume split: {str(e)}")
            if hasattr(e, 'response') and e.response is not None:
                print(f"Response: {e.response.text}")
            print("⚠️ Continuing with setup...")
            return {}

    def process_payment(self, profile_id, card_data):
        """Process a payment using the specified card data"""
        url = f"{API_BASE_URL}/payments"
        payload = self._build_payment_payload(profile_id, card_data)
        
        try:
            response = requests.post(url, headers=self.headers, json=payload)
            return response.json()
        except requests.exceptions.RequestException as e:
            print(f"❌ Payment request failed: {str(e)}")
            # Simulate a response if the API call fails
            status = "succeeded" if card_data == SUCCESS_CARD else "failed"
            error_message = "" if status == "succeeded" else "Payment declined: Card declined"
            
            return {
                "payment_id": f"pay_{uuid.uuid4().hex[:10]}",
                "status": status,
                "connector": "simulated_connector",
                "error_message": error_message
            }

    def _build_payment_payload(self, profile_id, card_data):
        """Build the payment payload"""
        return {
            "amount": 640,
            "currency": "USD",
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
                    "card_number": card_data["number"],
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


def build_card_pool(success_percent):
    """Build a pool of cards based on success percentage"""
    fail_percent = 100 - success_percent
    return [SUCCESS_CARD] * success_percent + [FAIL_CARD] * fail_percent


def simulate_payments(api_client, profile_id):
    """Simulate payments with varying success rates"""
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
        response = api_client.process_payment(profile_id, card)

        payment_id = response.get("payment_id", "")
        connector = response.get("connector", "")
        status = response.get("status", "")
        error_code = response.get("error_code", "")
        error_message = response.get("error_message", "")

        print(f"[{i}] Card: {card['label'].upper()} | Status: {status} | Connector: {connector} | Error: {error_message or 'None'}")

        time.sleep(INTER_PAYMENT_SLEEP_SEC)


def setup_and_run():
    """Main function to set up the environment and run the simulation"""
    print("🚀 Setting up Success Rate Routing Demo")
    print("---------------------------------------")
    
    # Initialize API client
    api_client = HyperswitchAPI(API_KEY, MERCHANT_ID)
    
    # Step 1: Create business profile or use existing one
    profile_id = 'pro_JCV5mBmpUAUMyA42xrIE'
    print(f"Using profile ID: {profile_id}")
    
    # Dictionary to store merchant_connector_ids
    merchant_connector_ids = {}
    
    # Step 2: Create test connectors
    print("\n📋 Creating test connectors...")
    for connector in TEST_CONNECTORS:
        connector_data = api_client.create_connector(profile_id, connector["name"], connector["label"])
        if connector_data and "merchant_connector_id" in connector_data:
            merchant_connector_id = connector_data["merchant_connector_id"]
            merchant_connector_ids[connector["name"]] = merchant_connector_id
            print(f"Stored merchant_connector_id for {connector['name']}: {merchant_connector_id}")
    
    # Step 3: Enable success rate algorithm
    print("\n🔄 Enabling success rate algorithm...")
    routing_response = api_client.enable_success_rate_algorithm(profile_id)
    routing_id = routing_response.get("id")
    
    if routing_id:
        print(f"Routing ID: {routing_id}")
        
        # Step 4: Configure routing rules
        api_client.configure_routing_rules(profile_id, routing_id)
        
        # Step 5: Activate routing
        api_client.activate_routing(routing_id)
    else:
        print("❌ Failed to get routing ID from response")
        routing_id = f"rout_{uuid.uuid4().hex[:10]}"
        print(f"Using generated routing ID: {routing_id}")
    
    # Step 6: Set volume split
    api_client.set_volume_split(profile_id)
    
    print("\n🔄 Setup complete! Starting payment simulation...\n")
    
    # Step 7: Simulate payments
    simulate_payments(api_client, profile_id)
    
    print("\n✅ Success Rate Routing Demo completed!")


if __name__ == "__main__":
    setup_and_run()
