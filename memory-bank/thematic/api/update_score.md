# Update Gateway Score API

## Overview

The Update Gateway Score API is a critical component of the Decision Engine's feedback loop. It allows reporting of transaction outcomes back to the system, enabling the engine to learn from past experiences and optimize future routing decisions. By continuously updating gateway performance metrics, the system can adapt to changing conditions and improve overall transaction success rates.

## Endpoint

```
POST /update-gateway-score
```

## Request Format

The API accepts a JSON payload with the following structure:

```json
{
    "merchantId": "test_merchant1",
    "gateway": "PAYU",
    "gatewayReferenceId": null,
    "status": "PENDING_VBV",
    "paymentId": "123",
    "enforceDynamicRoutingFailure": null
}
```

### Request Parameters

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `merchantId` | String | Yes | Unique identifier for the merchant making the request |
| `gateway` | String | Yes | Identifier of the gateway that processed the transaction |
| `gatewayReferenceId` | String | No | Reference ID provided by the gateway for the transaction |
| `status` | String | Yes | Outcome status of the transaction |
| `paymentId` | String | Yes | Unique identifier for the payment transaction (must match the ID used in the original decide-gateway request) |
| `enforceDynamicRoutingFailure` | Boolean | No | Flag to explicitly mark a transaction as failed for routing purposes regardless of status |

### Status Values

The `status` parameter accepts various values representing different transaction outcomes:

#### Success States
- `CHARGED` - Transaction successfully completed
- `AUTHENTICATION_SUCCESSFUL` - Authentication step completed successfully
- `AUTHORIZED` - Transaction was authorized but not yet captured

#### Pending States
- `PENDING` - Transaction is in a pending state
- `PENDING_VBV` - Pending 3D Secure verification
- `PENDING_CONFIRMATION` - Awaiting confirmation

#### Failure States
- `FAILED` - Transaction failed
- `AUTHENTICATION_FAILED` - Authentication step failed
- `AUTHORIZATION_FAILED` - Authorization step failed
- `CAPTURE_FAILED` - Capture step failed
- `EXPIRED` - Transaction expired before completion
- `CANCELLED` - Transaction was cancelled
- `REFUNDED` - Transaction was refunded

## Response Format

The API responds with a simple success message when the score update is processed successfully:

```
Success
```

In case of errors, the API returns an appropriate error response.

## Error Responses

### 400 Bad Request

Returned when the request is invalid:

```json
{
    "status": "400",
    "error_code": "400",
    "error_message": "Error parsing request",
    "error_info": {
        "code": "INVALID_INPUT",
        "user_message": "Invalid request params. Please verify your input.",
        "developer_message": "Detailed error message here"
    }
}
```

### 404 Not Found

Returned when the referenced merchant or payment is not found:

```json
{
    "status": "404",
    "error_code": "404",
    "error_message": "Resource not found",
    "error_info": {
        "code": "RESOURCE_NOT_FOUND",
        "user_message": "The requested resource was not found.",
        "developer_message": "Detailed error message here"
    }
}
```

### 500 Internal Server Error

Returned when there's a server-side error:

```json
{
    "status": "500",
    "error_code": "500",
    "error_message": "Internal server error",
    "error_info": {
        "code": "INTERNAL_ERROR",
        "user_message": "An unexpected error occurred. Please try again later.",
        "developer_message": "Detailed error message here"
    }
}
```

## Usage Examples

### Basic Success Update

```bash
curl --location 'http://localhost:8080/update-gateway-score' \
--header 'Content-Type: application/json' \
--data '{
    "merchantId": "test_merchant1",
    "gateway": "PAYU",
    "gatewayReferenceId": "pay_ref_123",
    "status": "CHARGED",
    "paymentId": "PAY12345"
}'
```

### Failure Update

```bash
curl --location 'http://localhost:8080/update-gateway-score' \
--header 'Content-Type: application/json' \
--data '{
    "merchantId": "test_merchant1",
    "gateway": "RAZORPAY",
    "gatewayReferenceId": "rzp_ref_456",
    "status": "FAILED",
    "paymentId": "PAY12346"
}'
```

### Pending State Update

```bash
curl --location 'http://localhost:8080/update-gateway-score' \
--header 'Content-Type: application/json' \
--data '{
    "merchantId": "test_merchant1",
    "gateway": "PAYTM_V2",
    "gatewayReferenceId": "ptm_ref_789",
    "status": "PENDING_VBV",
    "paymentId": "PAY12347"
}'
```

## How the Feedback Loop Works

1. **Transaction Processing**:
   - A transaction is routed to a specific gateway using the Decide Gateway API
   - The gateway processes the transaction and returns an outcome

2. **Outcome Reporting**:
   - The transaction outcome is reported to the Decision Engine using this API
   - The status reflects whether the transaction succeeded, failed, or is in a pending state

3. **Metric Calculation**:
   - The system records the outcome for the specific gateway
   - Success rates are recalculated across various dimensions (payment method, card type, etc.)

4. **Cache Invalidation**:
   - Relevant cached metrics are invalidated to ensure fresh data for future decisions
   - This ensures that recent transaction outcomes influence future routing decisions

5. **Future Routing Optimization**:
   - Gateways with higher success rates for specific transaction types receive higher priority
   - Gateways with persistent failures may be temporarily excluded via elimination logic

## Integration Notes

1. **Timely Updates**:
   - Report transaction outcomes as soon as they are known
   - For pending states, send updates when the final state is determined

2. **Gateway Reference IDs**:
   - Include gateway reference IDs when available for better tracking
   - This helps correlate transactions across systems

3. **Status Mapping**:
   - Map your internal transaction statuses to the accepted status values
   - Ensure consistent status reporting for accurate metrics

4. **Multiple Updates**:
   - It's acceptable to send multiple updates for the same transaction as its status changes
   - The system will use the most recent status for metric calculations

5. **Batch Processing**:
   - For high-volume systems, consider implementing batch updates
   - However, timely updates are preferred for real-time optimization

## Performance Impact

The Update Gateway Score API is designed to be lightweight and fast, but it triggers several important background processes:

1. **Database Operations**:
   - The transaction outcome is recorded in the database
   - Success rate metrics are updated

2. **Cache Management**:
   - Cached metrics affected by the update are invalidated
   - This ensures future routing decisions use the most recent data

3. **Elimination Evaluation**:
   - Gateway elimination criteria are evaluated
   - Consistently failing gateways may be temporarily removed from routing

## Security Considerations

1. **Authentication**:
   - Ensure proper authentication for this endpoint
   - Unauthorized updates could manipulate routing decisions

2. **Data Integrity**:
   - Validate that the reported status is legitimate
   - Consider implementing idempotency to prevent duplicate updates

3. **Audit Trail**:
   - Maintain a record of status updates for auditing purposes
   - This helps track routing decision evolution over time
