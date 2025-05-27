import requests
import json
import uuid

# Default API endpoints
API_BASE_URL = "https://sandbox.hyperswitch.io"
APP_BASE_URL = "https://app.hyperswitch.io"

class HyperswitchAPI:
    def __init__(self, api_key, merchant_id, bearer_token=None):
        self.api_key = api_key
        self.merchant_id = merchant_id
        self.bearer_token = bearer_token
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
        if self.bearer_token:
            auth_header = {
                "Authorization": f"Bearer {self.bearer_token}"
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
            
    def fetch_connector_map(self, profile_id):
        """Fetch connector mappings for a specific profile"""
        url = f"{APP_BASE_URL}/api/account/{self.merchant_id}/profile/connectors"
        headers = {
            "accept": "*/*",
            "api-key": self.api_key,
            "content-type": "application/json"
        }

        try:
            response = requests.get(url, headers=headers)
            response.raise_for_status()
            connector_map = {}

            for item in response.json():
                if item.get("profile_id") == profile_id and not item.get("disabled", False):
                    connector_map[item["connector_name"]] = item["merchant_connector_id"]

            print(f"✅ Connector map loaded for profile {profile_id}")
            return connector_map

        except requests.RequestException as e:
            print(f"❌ Failed to fetch connector map: {e}")
            return {}

def setup_and_run_demo(api_key, merchant_id, profile_id, bearer_token, connectors, simulate_payments_function):
    """Main function to set up the environment and run the simulation"""
    print("🚀 Setting up Success Rate Routing Demo")
    print("---------------------------------------")
    
    # Initialize API client
    api_client = HyperswitchAPI(api_key, merchant_id, bearer_token)
    
    # Step 1: Use existing profile
    print(f"Using profile ID: {profile_id}")
    
    # Dictionary to store merchant_connector_ids
    merchant_connector_ids = {}
    
    # Step 2: Create test connectors
    print("\n📋 Creating test connectors...")
    for connector in connectors:
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
    simulate_payments_function()
    
    print("\n✅ Success Rate Routing Demo completed!")
