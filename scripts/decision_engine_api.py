import requests
import json

# Default API endpoint
DECISION_ENGINE_API = "https://sandbox.juspay.io"

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


if __name__ == "__main__":
    test_decision_engine_endpoints()
