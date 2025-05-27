import requests
import random
import json
import time
import textwrap
import uuid
import pprint
import os
from dotenv import load_dotenv

# ---------------------------- CONFIGS ---------------------------- #

# Load environment variables from .env file if it exists
load_dotenv()

TOTAL_PAYMENTS = 30
INITIAL_SUCCESS_PERCENT = 60
INITIAL_DELAY_SEC = 0
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

CONNECTOR_MAP = {
    "pretendpay": "mca_d8F6FwL4z6HEOljk0xRR",
    "fauxpay": "mca_R5ufT3y8ppixWuL1NQVM"
}

STATUS_MAP = {
    "charged": "CHARGED",
    "succeeded": "CHARGED",
    "authorized": "AUTHORIZED",
    "failed": "FAILURE",
    "declined": "DECLINED"
}

SUCCESS_CARD = {"number": "4242424242424242", "label": "success"}
FAIL_CARD = {"number": "4000000000000002", "label": "fail"}

# ---------------------------------------------------------------- #

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
            "Authorization": f"Bearer {BEARER_TOKEN}"
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

def build_card_pool(success_percent):
    return [SUCCESS_CARD] * success_percent + [FAIL_CARD] * (100 - success_percent)

def decide_gateway(payment_id):
    url = 'https://sandbox.juspay.in/decide-gateway'
    headers = {
        'Content-Type': 'application/json',
        'x-merchantid': 'hyperswitchTest',
    }
    payload = {
        "merchantId": "hyperswitchTest",
        "eligibleGatewayList": list(CONNECTOR_MAP.keys()),
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
        response = requests.post(url, headers=headers, data=json.dumps(payload))
        data = response.json()

        priority_map = data.get("gateway_priority_map", {})
        print(f"priority_map:{priority_map}")
        if priority_map:
            print("📊 Connector Success Percentages:")
            for connector, score in priority_map.items():
                print(f"   - {connector}: {int(score * 100)}%")

        return data.get("decided_gateway")

    except Exception as e:
        print(f"❌ Failed to call decide-gateway: {e}")
        return None

def update_gateway_score(gateway, status, payment_id):
    url = 'https://sandbox.juspay.in/update-gateway-score'
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
        response = requests.post(url, headers=headers, data=json.dumps(payload))
        return {
            "status_code": response.status_code,
            "text": response.text.strip()
        }
    except Exception as e:
        return {"error": str(e)}

def generate_payload(card_number, connector_name, mca_id):
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

def send_logs_to_gemini(payment_results):
    prompt_text = textwrap.dedent(f"""
    You are an AI analyst. The following is a list of payment simulation logs.
    Each entry contains the payment ID, card type, selected gateway, payment status, and error message.
    Please generate a summary report that includes:
    - Total payments
    - Number of successes and failures
    - Success percentage per connector
    - Most common failure reasons
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
        response = requests.post(GEMINI_API_URL, headers=headers, json=payload)
        if response.status_code == 200:
            report = response.json()["candidates"][0]["content"]["parts"][0]["text"]
            print("\n📋 Gemini AI Report:\n")
            print(report)
        else:
            print(f"❌ Gemini API error: {response.status_code}")
            print(response.text)
    except Exception as e:
        print(f"❌ Gemini API request failed: {str(e)}")

def simulate_payments():
    payment_results = []

    force_fail_start = TOTAL_PAYMENTS // 2
    force_fail_end = force_fail_start + (TOTAL_PAYMENTS // 4)

    print(f"🔁 Starting payment simulation: {TOTAL_PAYMENTS} payments\n")

    for i in range(1, TOTAL_PAYMENTS + 1):
        card_pool = (
            build_card_pool(INITIAL_SUCCESS_PERCENT) if i <= force_fail_start or i > force_fail_end
            else build_card_pool(0)
        )

        card = random.choice(card_pool)
        payment_id = f"PAY_SIM_{i:05d}"

        print(f"\n🔸 Payment {i}: ID = {payment_id}")
        decided_gateway = decide_gateway(payment_id)
        if not decided_gateway:
            print(f"❌ Gateway not decided for {payment_id}")
            continue

        mca_id = CONNECTOR_MAP.get(decided_gateway)
        if not mca_id:
            print(f"❌ MCA ID not found for connector: {decided_gateway}")
            continue

        payload = generate_payload(card["number"], decided_gateway, mca_id)

        try:
            response = requests.post(PAYMENT_URL, headers=HEADERS, data=json.dumps(payload))
            resp_json = response.json()
        except Exception as e:
            print(f"❌ Payment request failed: {e}")
            continue

        raw_status = resp_json.get("status", "").lower()
        status = STATUS_MAP.get(raw_status, "FAILURE")
        error_message = resp_json.get("error_message", "None")

        print(f"✅ Card: {card['label'].upper()} | Gateway: {decided_gateway} | Status: {status} | Error: {error_message}")

        payment_results.append({
            "payment_id": payment_id,
            "card_type": card["label"].upper(),
            "gateway": decided_gateway,
            "status": status,
            "error": error_message
        })

        update_gateway_score(decided_gateway, status, payment_id)

        time.sleep(INTER_PAYMENT_SLEEP_SEC)

    send_logs_to_gemini(payment_results)

class DecisionEngineAPI:
    """A class to interact with the Decision Engine API endpoints"""
    
    def __init__(self, base_url=DECISION_ENGINE_API):
        self.base_url = base_url
        self.headers = {
            "Content-Type": "application/json"
        }
    
    def create_rule_success_rate(self, merchant_id="test_merchant_123"):
        """Create a success rate rule"""
        url = f"{self.base_url}/rule/create"
        payload = {
            "merchant_id": merchant_id,
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
        
        try:
            response = requests.post(url, headers=self.headers, json=payload)
            response.raise_for_status()
            print(f"✅ Created success rate rule for merchant: {merchant_id}")
            return response.json()
        except requests.exceptions.RequestException as e:
            print(f"❌ Failed to create success rate rule: {str(e)}")
            if hasattr(e, 'response') and e.response is not None:
                print(f"Response: {e.response.text}")
            return None
    
    def create_rule_elimination(self, merchant_id="test_merchant_123"):
        """Create an elimination rule"""
        url = f"{self.base_url}/rule/create"
        payload = {
            "merchant_id": merchant_id,
            "config": {
                "type": "elimination",
                "data": {
                    "threshold": 0.35
                }
            }
        }
        
        try:
            response = requests.post(url, headers=self.headers, json=payload)
            response.raise_for_status()
            print(f"✅ Created elimination rule for merchant: {merchant_id}")
            return response.json()
        except requests.exceptions.RequestException as e:
            print(f"❌ Failed to create elimination rule: {str(e)}")
            if hasattr(e, 'response') and e.response is not None:
                print(f"Response: {e.response.text}")
            return None
    
    def create_rule_debit_routing(self, merchant_id="test_merchant_123"):
        """Create a debit routing rule"""
        url = f"{self.base_url}/rule/create"
        payload = {
            "merchant_id": merchant_id,
            "config": {
                "type": "debitRouting",
                "data": {
                    "merchantCategoryCode": "mcc0001",
                    "acquirerCountry": "ecuador"
                }
            }
        }
        
        try:
            response = requests.post(url, headers=self.headers, json=payload)
            response.raise_for_status()
            print(f"✅ Created debit routing rule for merchant: {merchant_id}")
            return response.json()
        except requests.exceptions.RequestException as e:
            print(f"❌ Failed to create debit routing rule: {str(e)}")
            if hasattr(e, 'response') and e.response is not None:
                print(f"Response: {e.response.text}")
            return None
    
    def get_rule(self, merchant_id="test_merchant_123", algorithm="successRate"):
        """Fetch a rule by merchant ID and algorithm"""
        url = f"{self.base_url}/rule/get"
        payload = {
            "merchant_id": merchant_id,
            "algorithm": algorithm
        }
        
        try:
            response = requests.post(url, headers=self.headers, json=payload)
            response.raise_for_status()
            print(f"✅ Fetched {algorithm} rule for merchant: {merchant_id}")
            return response.json()
        except requests.exceptions.RequestException as e:
            print(f"❌ Failed to fetch rule: {str(e)}")
            if hasattr(e, 'response') and e.response is not None:
                print(f"Response: {e.response.text}")
            return None
    
    def update_rule_debit_routing(self, merchant_id="test_merchant_123"):
        """Update a debit routing rule"""
        url = f"{self.base_url}/rule/update"
        payload = {
            "merchant_id": merchant_id,
            "config": {
                "type": "debitRouting",
                "data": {
                    "merchantCategoryCode": "mcc0001",
                    "acquirerCountry": "ecuador"
                }
            }
        }
        
        try:
            response = requests.post(url, headers=self.headers, json=payload)
            response.raise_for_status()
            print(f"✅ Updated debit routing rule for merchant: {merchant_id}")
            return response.json()
        except requests.exceptions.RequestException as e:
            print(f"❌ Failed to update rule: {str(e)}")
            if hasattr(e, 'response') and e.response is not None:
                print(f"Response: {e.response.text}")
            return None
    
    def delete_rule(self, merchant_id="test_merchant_123", algorithm="successRate"):
        """Delete a rule by merchant ID and algorithm"""
        url = f"{self.base_url}/rule/delete"
        payload = {
            "merchant_id": merchant_id,
            "algorithm": algorithm
        }
        
        try:
            response = requests.post(url, headers=self.headers, json=payload)
            response.raise_for_status()
            print(f"✅ Deleted {algorithm} rule for merchant: {merchant_id}")
            return response.json()
        except requests.exceptions.RequestException as e:
            print(f"❌ Failed to delete rule: {str(e)}")
            if hasattr(e, 'response') and e.response is not None:
                print(f"Response: {e.response.text}")
            return None
    
    def create_merchant_account(self, merchant_id="test_merchant_123"):
        """Create a merchant account"""
        url = f"{self.base_url}/merchant-account/create"
        payload = {
            "merchant_id": merchant_id
        }
        
        try:
            response = requests.post(url, headers=self.headers, json=payload)
            response.raise_for_status()
            print(f"✅ Created merchant account: {merchant_id}")
            return response.json()
        except requests.exceptions.RequestException as e:
            print(f"❌ Failed to create merchant account: {str(e)}")
            if hasattr(e, 'response') and e.response is not None:
                print(f"Response: {e.response.text}")
            return None
    
    def get_merchant_account(self, merchant_id="test_merchant_123"):
        """Fetch a merchant account by ID"""
        url = f"{self.base_url}/merchant-account/{merchant_id}"
        
        try:
            response = requests.get(url, headers=self.headers)
            response.raise_for_status()
            print(f"✅ Fetched merchant account: {merchant_id}")
            return response.json()
        except requests.exceptions.RequestException as e:
            print(f"❌ Failed to fetch merchant account: {str(e)}")
            if hasattr(e, 'response') and e.response is not None:
                print(f"Response: {e.response.text}")
            return None
    
    def delete_merchant_account(self, merchant_id="test_merchant_123"):
        """Delete a merchant account by ID"""
        url = f"{self.base_url}/merchant-account/{merchant_id}"
        
        try:
            response = requests.delete(url, headers=self.headers)
            response.raise_for_status()
            print(f"✅ Deleted merchant account: {merchant_id}")
            return response.json()
        except requests.exceptions.RequestException as e:
            print(f"❌ Failed to delete merchant account: {str(e)}")
            if hasattr(e, 'response') and e.response is not None:
                print(f"Response: {e.response.text}")
            return None

def test_decision_engine_endpoints():
    """Test all the Decision Engine API endpoints"""
    print("\n🔍 Testing Decision Engine API Endpoints")
    print("---------------------------------------")
    
    # Initialize API client
    de_api = DecisionEngineAPI()
    merchant_id = "test_merchant_123"
    
    # Test merchant account operations
    print("\n📋 Testing Merchant Account Operations...")
    
    # Create merchant account
    print("\n▶️ Creating merchant account...")
    create_result = de_api.create_merchant_account(merchant_id)
    if create_result:
        print(f"Create merchant result: {json.dumps(create_result, indent=2)}")
    
    # Get merchant account
    print("\n▶️ Fetching merchant account...")
    account_result = de_api.get_merchant_account(merchant_id)
    if account_result:
        print(f"Merchant account: {json.dumps(account_result, indent=2)}")
    
    # Test rule operations
    print("\n📋 Testing Rule Operations...")
    
    # Create success rate rule
    print("\n▶️ Creating success rate rule...")
    sr_result = de_api.create_rule_success_rate(merchant_id)
    if sr_result:
        print(f"Success rate rule created: {json.dumps(sr_result, indent=2)}")
    
    # Create elimination rule
    print("\n▶️ Creating elimination rule...")
    elim_result = de_api.create_rule_elimination(merchant_id)
    if elim_result:
        print(f"Elimination rule created: {json.dumps(elim_result, indent=2)}")
    
    # Create debit routing rule
    print("\n▶️ Creating debit routing rule...")
    dr_result = de_api.create_rule_debit_routing(merchant_id)
    if dr_result:
        print(f"Debit routing rule created: {json.dumps(dr_result, indent=2)}")
    
    # Get rules
    print("\n▶️ Fetching success rate rule...")
    get_sr_result = de_api.get_rule(merchant_id, "successRate")
    if get_sr_result:
        print(f"Success rate rule: {json.dumps(get_sr_result, indent=2)}")
    
    # Update debit routing rule
    print("\n▶️ Updating debit routing rule...")
    update_dr_result = de_api.update_rule_debit_routing(merchant_id)
    if update_dr_result:
        print(f"Updated debit routing rule: {json.dumps(update_dr_result, indent=2)}")
    
    # Delete success rate rule
    print("\n▶️ Deleting success rate rule...")
    delete_sr_result = de_api.delete_rule(merchant_id, "successRate")
    if delete_sr_result:
        print(f"Success rate rule deletion result: {json.dumps(delete_sr_result, indent=2)}")
    
    # Delete merchant account
    print("\n▶️ Deleting merchant account...")
    delete_result = de_api.delete_merchant_account(merchant_id)
    if delete_result:
        print(f"Delete merchant result: {json.dumps(delete_result, indent=2)}")
    
    print("\n✅ Decision Engine API Testing completed!")

def setup_and_run():
    """Main function to set up the environment and run the simulation"""
    print("🚀 Setting up Success Rate Routing Demo")
    print("---------------------------------------")
    
    # Initialize API client
    api_client = HyperswitchAPI(API_KEY, MERCHANT_ID)
    
    # Step 1: Create business profile or use existing one
    profile_id = PROFILE_ID
    print(f"Using profile ID: {profile_id}")
    
    # Dictionary to store merchant_connector_ids
    merchant_connector_ids = {}
    
    # Step 2: Create test connectors
    print("\n📋 Creating test connectors...")
    for connector in CREATE_CONNECTORS:
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
    simulate_payments()
    
    print("\n✅ Success Rate Routing Demo completed!")


if __name__ == "__main__":
    # Test Decision Engine endpoints
    test_decision_engine_endpoints()
    
    # Run the main simulation
    setup_and_run()
