# Decide Gateway API

## Overview

The Decide Gateway API is the core endpoint of the Decision Engine responsible for determining the optimal payment gateway for a transaction in real-time. It evaluates transaction details against configured routing strategies to provide an ordered list of payment gateways, with the most suitable gateway marked as the primary choice.

## Endpoint

```
POST /decide-gateway
```

## Request Format

The API accepts a JSON payload with the following structure:

```json
{
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
}
```

### Request Parameters

#### Top-Level Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `merchantId` | String | Yes | Unique identifier for the merchant making the request |
| `eligibleGatewayList` | Array of Strings | Yes | List of gateway identifiers that are eligible for this transaction |
| `rankingAlgorithm` | String | Yes | Routing algorithm to use (`SR_BASED_ROUTING` or `PL_BASED_ROUTING`) |
| `eliminationEnabled` | Boolean | Yes | Whether to enable the elimination of underperforming gateways |
| `paymentInfo` | Object | Yes | Detailed information about the payment transaction |

#### Payment Info Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `paymentId` | String | Yes | Unique identifier for the payment transaction |
| `amount` | Number | Yes | Transaction amount |
| `currency` | String | Yes | Three-letter currency code (e.g., USD, INR) |
| `customerId` | String | No | Identifier for the customer making the payment |
| `udfs` | Object | No | User-defined fields for additional transaction data |
| `preferredGateway` | String | No | Gateway to prefer if available and eligible |
| `paymentType` | String | Yes | Type of payment (e.g., `ORDER_PAYMENT`, `MANDATE_REGISTER`) |
| `metadata` | Object | No | Additional transaction metadata |
| `internalMetadata` | Object | No | Internal metadata not exposed to external systems |
| `isEmi` | Boolean | No | Whether the transaction is an EMI payment |
| `emiBank` | String | No | Bank providing the EMI for the transaction |
| `emiTenure` | Number | No | Tenure period for the EMI in months |
| `paymentMethodType` | String | Yes | Type of payment method used (e.g., `CARD`, `UPI`, `WALLET`) |
| `paymentMethod` | String | Yes | Specific payment method within the type (e.g., `UPI_PAY`, `UPI_COLLECT`) |
| `paymentSource` | String | No | Source of the payment (e.g., app name, website) |
| `authType` | String | No | Authentication type for the transaction |
| `cardIssuerBankName` | String | No | Name of the bank that issued the card (for card transactions) |
| `cardIsin` | String | No | ISIN code for the card (for card transactions) |
| `cardType` | String | No | Type of card (e.g., `CREDIT`, `DEBIT`) |
| `cardSwitchProvider` | String | No | Card network or switch provider |

## Response Format

The API responds with a JSON payload containing the routing decision:

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

### Response Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `decided_gateway` | String | The gateway selected as optimal for this transaction |
| `gateway_priority_map` | Object | Map of gateways to their priority scores |
| `filter_wise_gateways` | Object/null | List of gateways that passed each filter step |
| `priority_logic_tag` | String/null | Tag of the priority logic rule that was applied |
| `routing_approach` | String | The routing approach used for the decision (e.g., `PRIORITY_LOGIC`, `SR_SELECTION_V3_ROUTING`) |
| `gateway_before_evaluation` | String | Gateway selected before downtime evaluation |
| `priority_logic_output` | Object | Details of the priority logic evaluation |
| `reset_approach` | String | Approach used if reset was needed |
| `routing_dimension` | String | The dimension used for routing (e.g., `ORDER_PAYMENT, UPI, UPI_PAY`) |
| `routing_dimension_level` | String | Level at which routing was performed (e.g., `PM_LEVEL`) |
| `is_scheduled_outage` | Boolean | Whether a scheduled outage affected the routing decision |
| `gateway_mga_id_map` | Object/null | Mapping of gateways to merchant gateway account IDs |

#### Priority Logic Output Parameters

| Parameter | Type | Description |
|-----------|------|-------------|
| `isEnforcement` | Boolean | Whether the rule strictly enforces the priority list |
| `gws` | Array | List of gateway identifiers in priority order |
| `priorityLogicTag` | String | Identifier for the priority logic rule |
| `gatewayReferenceIds` | Object | Mapping of gateways to reference IDs |
| `primaryLogic` | Object | Information about the primary logic used |
| `fallbackLogic` | Object/null | Information about any fallback logic used |

## Error Responses

### 400 Bad Request

Returned when the request is invalid:

```json
{
    "status": "400",
    "error_code": "400",
    "error_message": "Error parsing request",
    "priority_logic_tag": null,
    "routing_approach": null,
    "filter_wise_gateways": null,
    "error_info": {
        "code": "INVALID_INPUT",
        "user_message": "Invalid request params. Please verify your input.",
        "developer_message": "Detailed error message here"
    },
    "priority_logic_output": null,
    "is_dynamic_mga_enabled": false
}
```

### 500 Internal Server Error

Returned when there's a server-side error:

```json
{
    "status": "500",
    "error_code": "500",
    "error_message": "Internal server error",
    "priority_logic_tag": null,
    "routing_approach": null,
    "filter_wise_gateways": null,
    "error_info": {
        "code": "INTERNAL_ERROR",
        "user_message": "An unexpected error occurred. Please try again later.",
        "developer_message": "Detailed error message here"
    },
    "priority_logic_output": null,
    "is_dynamic_mga_enabled": false
}
```

## Usage Examples

### Basic Request

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
        "paymentType": "ORDER_PAYMENT",
        "paymentMethodType": "UPI",
        "paymentMethod": "UPI_PAY"
    }
}'
```

### Card Payment Request

```bash
curl --location 'http://localhost:8080/decide-gateway' \
--header 'Content-Type: application/json' \
--data '{
    "merchantId": "test_merchant1",
    "eligibleGatewayList": ["PAYU", "RAZORPAY", "PAYTM_V2"],
    "rankingAlgorithm": "PL_BASED_ROUTING",
    "eliminationEnabled": true,
    "paymentInfo": {
        "paymentId": "PAY12346",
        "amount": 500.00,
        "currency": "INR",
        "customerId": "CUST12345",
        "paymentType": "ORDER_PAYMENT",
        "paymentMethodType": "CARD",
        "paymentMethod": "CREDIT",
        "cardType": "CREDIT",
        "cardIssuerBankName": "HDFC",
        "cardIsin": "123456"
    }
}'
```

## Integration Notes

1. **Ranking Algorithm Choice**:
   - Use `SR_BASED_ROUTING` for success rate-based routing that adapts based on historical performance
   - Use `PL_BASED_ROUTING` for priority logic-based routing that follows predefined rules

2. **Elimination Logic**:
   - Set `eliminationEnabled` to `true` to enable the elimination of underperforming gateways
   - This helps avoid gateways experiencing high failure rates or outages

3. **Transaction Dimensions**:
   - The combination of `paymentType`, `paymentMethodType`, and `paymentMethod` defines the routing dimension
   - More specific dimensions may have different routing configurations

4. **Gateway Selection Process**:
   1. The system evaluates eligible gateways against merchant configuration
   2. It applies the specified ranking algorithm to determine optimal order
   3. It checks for gateway outages or performance issues
   4. It returns the final decision with priority information

5. **Feedback Loop**:
   - After processing the transaction, use the Update Gateway Score API to provide outcome feedback
   - This helps the system improve future routing decisions
