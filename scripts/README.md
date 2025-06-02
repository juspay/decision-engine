# Decision Engine Testing Scripts

This directory contains modular Python scripts for testing payment routing and decision engine functionality.

## Module Structure

The project has been organized into these modules:

### 1. decision_engine_api.py
Contains the API client for interacting with the Decision Engine service.

- `DecisionEngineAPI` class: Provides methods for interacting with rules, merchant accounts, and other Decision Engine endpoints
- `test_decision_engine_endpoints()`: Helper function to test all available endpoints

### 2. hyperswitch_api.py
Contains the API client for interacting with the Hyperswitch payment gateway service.

- `HyperswitchAPI` class: Handles connector creation, routing algorithm configuration, and other Hyperswitch-specific operations
- `setup_and_run_demo()`: Orchestrates the setup of test connectors and routing rules before running simulations

### 3. payment_operations.py
Contains functions for payment simulation and processing.

- Card pool management for test scenarios
- Gateway decision-making through Juspay's API
- Payment payload generation
- Score updating for gateways
- AI-assisted log analysis via Gemini API

### 4. test_routing_with_payments.py
The main script that orchestrates the testing process.

- Environment configuration and credential management
- Initialization of API clients
- Running payment simulations with configurable parameters

## Usage

### Basic Usage

To run a payment simulation:

```bash
python test_routing_with_payments.py
```

### Customization

You can modify the following in the main script:

- `TOTAL_PAYMENTS`: Number of payments to simulate
- `INITIAL_SUCCESS_PERCENT`: Percentage of successful payments in the simulation
- `INTER_PAYMENT_SLEEP_SEC`: Delay between payments

## Environment Variables

The scripts look for the following environment variables (can be set in .env file):

- `PROFILE_ID`: Hyperswitch profile ID
- `BEARER_TOKEN`: Authentication bearer token
- `MERCHANT_ID`: Merchant ID for API calls
- `GIMINI_API_KEY`: API key for Gemini AI (for log analysis)
- `API_KEY`: Main API key for Hyperswitch
