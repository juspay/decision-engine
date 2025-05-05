# Setup Instructions:

Follow the steps below to set up and run the project locally.

## 1. Clone the Repository

```bash
git clone {repo-url}
cd {repo-directory}/crates/open-router
```

---

## 2. Install Docker

Make sure Docker is installed on your system.
You can download and install Docker Desktop from the below links.

- Mac - https://docs.docker.com/desktop/setup/install/mac-install/
- Windows - https://docs.docker.com/desktop/setup/install/windows-install/
- Linux - https://docs.docker.com/desktop/setup/install/linux/

---

## 3. Run the Project

### a. First-Time Setup

If you're setting up the environment for the first time, run:

```bash
make init
```

This command performs the following under the hood:

```bash
docker-compose run --rm db-migrator && docker-compose up open-router
```
This will:
- Set up the environment
- Set up the database with the required schema
- Sets up redis and the server for running the application
- Push the configs defined in the config.yaml & the static rules defined for routing in priority_logic.txt to the DB

### b. Start the Server (without resetting DB)

If the DB schema is already set up and you don't want to reset the DB, use:

```bash
make run
```
**System Requirements:** Approximately 2GB of disk space

After successful setup, the server will start running.
### c. Update Configs / Static Rules

To update the configs (from the config.yaml file) or the static rules (from priority_logic.txt), run:

```bash
make update-config
```

### d. Stop Running Instances

To stop the running Docker instances:

```bash
make stop
```

---

## 4. Running Local Code Changes

If you've made changes to the code locally and want to test them:

### a. Initialize Local Environment

```bash
make init-local
```

This command performs the following under the hood:

```bash
docker-compose run --rm db-migrator && docker-compose up open-router-local
```

### b. Run Locally

```bash
make run-local
```

## Using the Decision Engine APIs

### 1. Get the Gateway Decision

Use the following cURL with payment info to get the gateway-decision:

```bash
curl --location 'http://localhost:8080/decide-gateway' \
--header 'Content-Type: application/json' \
--data '{
    "merchantId": "test_merchant1",
    "eligibleGatewayList": ["PAYU", "RAZORPAY", "PAYTM_V2"],
    "rankingAlgorithm": "SR_BASED_ROUTING",
    "eliminationEnabled": true,
    "paymentInfo": {
        "paymentId": "PAY12345",
        "amount": 100.50,
        "currency": "USD",
        "customerId": "CUST12345",
        "udfs": null,
        "preferredGateway": null,
        "paymentType": "ORDER_PAYMENT",
        "metadata": null,
        "internalMetadata": null,
        "isEmi": false,
        "emiBank": null,
        "emiTenure": null,
        "paymentMethodType": "UPI",
        "paymentMethod": "UPI_PAY",
        "paymentSource": null,
        "authType": null,
        "cardIssuerBankName": null,
        "cardIsin": null,
        "cardType": null,
        "cardSwitchProvider": null
    }
}'
```

#### Response Example

```json
{
    "decided_gateway": "PAYTM_V2",
    "gateway_priority_map": {
        "PAYU": 1.0,
        "RAZORPAY": 1.0,
        "PAYTM_V2": 1.0
    },
    "filter_wise_gateways": null,
    "priority_logic_tag": "PL_TEST",
    "routing_approach": "PRIORITY_LOGIC",
    "gateway_before_evaluation": "RAZORPAY",
    "priority_logic_output": {
        "isEnforcement": false,
        "gws": [],
        "priorityLogicTag": "PL_TEST",
        "gatewayReferenceIds": {},
        "primaryLogic": {
            "name": "PL_TEST",
            "status": "SUCCESS",
            "failure_reason": "NO_ERROR"
        },
        "fallbackLogic": null
    },
    "reset_approach": "NO_RESET",
    "routing_dimension": "ORDER_PAYMENT, UPI, UPI_PAY",
    "routing_dimension_level": "PM_LEVEL",
    "is_scheduled_outage": false,
    "gateway_mga_id_map": null
}
```

### 2. Update Gateway Score

This will update the decision-engine with the transaction status to optimize for future decisions:

```bash
curl --location 'http://localhost:8080/update-gateway-score' \
--header 'Content-Type: application/json' \
--data '{
    "merchantId": "test_merchant1",
    "gateway": "PAYU",
    "gatewayReferenceId": null,
    "status": "PENDING_VBV",
    "paymentId": "123"
}'
```

## Configuration Options

### 1. Priority Logic (PL) to be updated in this (file)[https://github.com/juspay/decision-engine/blob/main/crates/open-router/routing-config/priority_logic.txt]

```groovy
def priorities = ['A','B','C','D','E'] // Default priorities if no rule matches
def systemtimemills = System.currentTimeMillis() % 100
def enforceFlag = false

if ((payment.paymentMethodType)=='UPI' && (txn.sourceObject)=='UPI_COLLECT') {
    priorities = ['A','B']
    enforceFlag = true
}
else {
    if (['UPI'].contains(payment.paymentMethodType)) {
        if (order.udf1=="LOB1") {
            if (payment.paymentSource?.indexOf("ABC") >= 0 || 
                payment.paymentSource?.indexOf("DEF") >= 0) {
                priorities = ['B','C']
            }
            else if (systemtimemills < 50) {
                priorities = ['D','E']
            }
            else {
                priorities = ['E','D']
            }
        }
    }
}
```

### 2. SR and ER routing configs to be update in this (file)[https://github.com/juspay/decision-engine/blob/main/crates/open-router/routing-config/config.yaml]

```yaml
merchant_id: test_merchant1
priority_logic:
  script: priority_logic.txt
  tag: PL_TEST
elimination_config:
  threshold: 0.35
sr_routing_config:
  defaultBucketSize: 50
  defaultHedgingPercent: 5
  subLevelInputConfig:
    - paymentMethodType: UPI
      paymentMethod: UPI_COLLECT
      bucketSize: 100
      hedgingPercent: 1
    - paymentMethodType: UPI
      paymentMethod: UPI_PAY
      bucketSize: 500
      hedgingPercent: 1
    - paymentMethodType: UPI
      paymentMethod: UPI_QR
      bucketSize: 1000
      hedgingPercent: 1
    - paymentMethodType: NB
      bucketSize: 50
      hedgingPercent: 1
    - paymentMethodType: CARD
      bucketSize: 200
      hedgingPercent: 1
    - paymentMethodType: WALLET
      bucketSize: 50
      hedgingPercent: 1
```

After modifying the configurations, use the following command to push them to the DB:

```bash
make update-config
```

## Glossary

### Gateway Decision API Parameters

| Parameter | Description |
|-----------|-------------|
| `merchantId` | Unique identifier assigned to the merchant using the API |
| `eligibleGatewayList` | List of gateways eligible to process the transaction |
| `rankingAlgorithm` | Specifies the routing algorithm to use (`SR_BASED_ROUTING` or `PL_BASED_ROUTING`) |
| `eliminationEnabled` | Boolean flag to enable/disable downtime detection in routing decisions |

#### Payment Info Parameters

| Parameter | Description |
|-----------|-------------|
| `paymentId` | Unique identifier for the transaction (mandatory) |
| `amount` | Transaction amount to be processed |
| `currency` | Currency code for the transaction (e.g., INR, USD) |
| `paymentType` | Indicates payment purpose (e.g., `ORDER_PAYMENT`, `MANDATE_REGISTER`, `EMANDATE_REGISTER`) |
| `paymentMethodType` | Type of payment method (e.g., `CARD`, `UPI`, `WALLET`, `NET BANKING`) |
| `paymentMethod` | Specific subcategory within the chosen paymentMethodType |

### Response Fields

| Field | Description |
|-------|-------------|
| `decided_gateway` | The gateway chosen by the decision engine for routing the transaction |
| `gateway_priority_map` | Scores for each gateway used in making the routing decision |
| `filter_wise_gateways` | List of eligible connectors (if Eligibility Check/Orchestration is used) |
| `priority_logic_tag` | Unique identifier for the specific Static Rule defined in the YAML file |
| `routing_approach` | The specific routing approach used for processing the transaction |
| `gateway_before_evaluation` | The gateway decided before downtime evaluation |
| `routing_dimension` | The dimensions on which routing decisions are made |
| `routing_dimension_level` | The level at which routing decisions are made (e.g., `PM_LEVEL`) |
| `is_scheduled_outage` | Returns true if the routing decision is impacted by scheduled outages |

### Update Gateway Score Parameters

| Parameter | Description |
|-----------|-------------|
| `merchantId` | Unique identifier assigned to the merchant using the API |
| `gateway` | The gateway to which the transaction was routed |
| `gatewayReferenceId` | Reference ID from the gateway |
| `status` | Transaction status used to update the routing score |
| `paymentId` | Unique identifier for the transaction |

### Configuration YAML Parameters

| Parameter | Description |
|-----------|-------------|
| `merchant_id` | Unique identifier assigned to the merchant |
| `priority_logic.script` | The file name in which static rules are defined |
| `priority_logic.tag` | Unique identifier for a static rule defined |
| `elimination_config.threshold` | PG health threshold (PGs below this are deprioritized) |
| `defaultBucketSize` | Last 'n' transactions to consider for computing SR scores |
| `defaultHedgingPercent` | Percentage of traffic for exploration of lower-ranked gateways |
| `subLevelInputConfig` | Define granular configs at PMT/PM level |
