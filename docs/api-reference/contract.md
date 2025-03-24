# Contract Routing API Contracts

## Overview

The Contract Score Calculator service provides endpoints for routing payments based on contractual obligations and targets with payment processors. This service is a key component of the Dynamo routing system, helping ensure that payment volumes are distributed according to business agreements while maintaining efficient processing.

## Service Definition

```protobuf
service ContractScoreCalculator {
    rpc FetchContractScore (CalContractScoreRequest) returns (CalContractScoreResponse);
    rpc UpdateContract (UpdateContractRequest) returns (UpdateContractResponse);
    rpc InvalidateContract (InvalidateContractRequest) returns (InvalidateContractResponse);
}
```

## Authentication

All endpoints require authentication. Authentication is handled via metadata in the gRPC request. The service supports multi-tenancy, which means contract data is isolated per tenant.

- Authentication requires an `x-api-key` header with a valid API key
- The `x-tenant-id` header is required to specify the tenant context
- Tenant ID is extracted from request metadata
- All operations verify tenant permissions before processing
- Multi-tenancy can be enabled/disabled via configuration

## Endpoints

### 1. FetchContractScore

Calculates and returns contract-based routing scores for the provided processors.

#### Request: `CalContractScoreRequest`

```protobuf
message CalContractScoreRequest {
    string id = 1;                   // Entity identifier
    string params = 2;               // Additional parameters for contract calculation
    repeated string labels = 3;      // Labels (processors) to calculate scores for
    CalContractScoreConfig config = 4; // Configuration for calculation
}

message CalContractScoreConfig {
    repeated double constants = 1;   // Constants used in score calculation algorithm
    TimeScale time_scale = 2;        // Time scale for contract calculation
}

message TimeScale {
    enum Scale {
        Day = 0;    // Daily time scale
        Month = 1;  // Monthly time scale
    }
    Scale time_scale = 1;  // Selected time scale
}
```

#### Response: `CalContractScoreResponse`

```protobuf
message CalContractScoreResponse {
    repeated ScoreData labels_with_score = 1;  // Contract scores for each label
}

message ScoreData {
    double score = 1;         // Contract score (higher values indicate higher priority)
    string label = 2;         // Label (processor) identifier
    uint64 current_count = 3; // Current transaction count for this processor
}
```

#### Behavior

- Calculates contract scores based on processor targets and current usage
- Scores are influenced by how far each processor is from meeting its target
- Higher scores indicate processors that need more transactions to meet targets
- Additional parameters can influence the scoring algorithm
- Returns current transaction counts alongside scores for transparency

#### Process Flow

1. Extract tenant ID from request metadata
2. Validate the request parameters
3. Extract entity ID, parameters, and processor labels
4. Convert the configuration to internal contract score settings
5. Calculate contract scores for each processor based on current transaction counts and targets
6. Return detailed score data for each processor

---

### 2. UpdateContract

Updates the contract information for specific processors, affecting future contract-based routing decisions.

#### Request: `UpdateContractRequest`

```protobuf
message UpdateContractRequest {
    string id = 1;                       // Entity identifier
    string params = 2;                   // Additional parameters
    repeated LabelInformation labels_information = 3; // Contract information for processors
}

message LabelInformation {
    string label = 1;         // Processor identifier
    uint64 target_count = 2;  // Target transaction count in contract
    uint64 target_time = 3;   // Time period for the target (in seconds)
    uint64 current_count = 4; // Current transaction count
}
```

#### Response: `UpdateContractResponse`

```protobuf
message UpdateContractResponse {
   enum UpdationStatus {
      CONTRACT_UPDATION_SUCCEEDED = 0;
      CONTRACT_UPDATION_FAILED = 1;
   }
   UpdationStatus status = 1; // Status of the update operation
}
```

#### Behavior

- Updates contract information for the specified processors
- Stores target counts, time periods, and current transaction counts
- Enables the system to calculate routing scores based on contract fulfillment
- Contract data is tenant-specific for multi-tenant deployments
- Returns the status of the update operation

#### Process Flow

1. Extract tenant ID from request metadata
2. Validate the request parameters
3. Extract entity ID, parameters, and contract information for processors
4. Convert the provided data to internal contract map format
5. Update contract information for all specified processors
6. Return success/failure status of the operation

---

### 3. InvalidateContract

Invalidates all contract data for a specific entity, effectively resetting its contract-based routing state.

#### Request: `InvalidateContractRequest`

```protobuf
message InvalidateContractRequest {
    string id = 1;  // Entity identifier to invalidate
}
```

#### Response: `InvalidateContractResponse`

```protobuf
message InvalidateContractResponse {
   enum InvalidationStatus {
      CONTRACT_INVALIDATION_SUCCEEDED = 0;
      CONTRACT_INVALIDATION_FAILED = 1;
   }
   InvalidationStatus status = 1; // Status of the invalidation operation
}
```

#### Behavior

- Removes all contract data for the specified entity
- This effectively resets the contract-based routing for this entity
- Useful when contracts are renewed or significantly changed
- Does not affect contract data for other entities
- Returns the status of the invalidation operation

#### Process Flow

1. Extract tenant ID from request metadata
2. Validate the request parameters
3. Extract entity ID to invalidate
4. Remove all contract data for the entity within the tenant context
5. Return success/failure status of the operation

## Error Handling

All endpoints return standard gRPC status codes:

| Status Code | Description | When Used |
|-------------|-------------|-----------|
| `OK (0)` | Operation completed successfully | Normal successful operation |
| `NOT_FOUND (5)` | Required resource not found | Missing configuration, entity not found |
| `INVALID_ARGUMENT (3)` | Validation failed | Invalid or missing required parameters |
| `UNAUTHENTICATED (16)` | Authentication failed | Invalid or missing authentication credentials |
| `PERMISSION_DENIED (7)` | Authenticated user lacks permission | User not authorized for the operation |
| `INTERNAL (13)` | Internal server error | Unexpected errors during processing |

## Multi-tenancy Support

The service supports multi-tenancy with the following behavior:

- All contract data is isolated per tenant with no cross-tenant data access
- Tenant ID is extracted from request metadata
- Each tenant has its own contract settings and transaction counts
- Multi-tenancy can be disabled in configuration via `is_multi_tenancy_enabled` flag
- When disabled, a default tenant ID is used

## Performance Considerations

- **Efficient Scoring Algorithm**: The contract score calculation is optimized for performance
- **Persistent Storage**: Contract data is stored efficiently for quick access
- **Parallel Processing**: The system can handle multiple contract score calculations simultaneously
- **Request Validation**: Validates requests early to fail fast and save processing resources
- **Rust Implementation**: Implementation is in Rust for maximum efficiency and safety
- **Error Handling**: Comprehensive error handling with detailed context

## Example Usage

### Fetch Contract Score Example

```json
// Request
{
  "id": "merchant_123",
  "params": "{\"payment_type\":\"card\"}",
  "labels": ["processor_A", "processor_B", "processor_C"],
  "config": {
    "constants": [0.75, 1.25, 0.5],
    "time_scale": {
      "time_scale": "Month"
    }
  }
}

// Response
{
  "labels_with_score": [
    {
      "score": 0.85,
      "label": "processor_B",
      "current_count": 7520
    },
    {
      "score": 0.65,
      "label": "processor_A",
      "current_count": 12450
    },
    {
      "score": 0.42,
      "label": "processor_C",
      "current_count": 9870
    }
  ]
}
```

### Update Contract Example

```json
// Request
{
  "id": "merchant_123",
  "params": "{\"payment_type\":\"card\"}",
  "labels_information": [
    {
      "label": "processor_A",
      "target_count": 15000,
      "target_time": 2592000,
      "current_count": 12450
    },
    {
      "label": "processor_B",
      "target_count": 10000,
      "target_time": 2592000,
      "current_count": 7520
    },
    {
      "label": "processor_C",
      "target_count": 12000,
      "target_time": 2592000,
      "current_count": 9870
    }
  ]
}

// Response
{
  "status": "CONTRACT_UPDATION_SUCCEEDED"
}
```

### Invalidate Contract Example

```json
// Request
{
  "id": "merchant_123"
}

// Response
{
  "status": "CONTRACT_INVALIDATION_SUCCEEDED"
}
```

## Integration Notes

- All requests should include appropriate authentication metadata
- JSON parameters in the `params` field should be properly escaped
- Contract targets should be set realistically based on historical processing volumes
- Target times should align with billing cycles (typically monthly)
- Contract-based routing works best when combined with success rate and elimination routing
- Maintain proper error handling for all API calls
- Consider resetting contracts at the beginning of each billing cycle

## gRPCurl Examples

The following examples demonstrate how to call the Contract Score Calculator API endpoints using gRPCurl. These examples assume the service is running on localhost at port 9000.

### FetchContractScore

```bash
grpcurl -plaintext -d '{
  "id": "merchant_123",
  "params": "{\"payment_type\":\"card\"}",
  "labels": ["processor_A", "processor_B", "processor_C"],
  "config": {
    "constants": [0.75, 1.25, 0.5],
    "time_scale": {
      "time_scale": "Month"
    }
  }
}' \
-H 'x-api-key: YOUR_API_KEY' \
-H 'x-tenant-id: tenant_001' \
localhost:9000 contract_routing.ContractScoreCalculator/FetchContractScore
```

### UpdateContract

```bash
grpcurl -plaintext -d '{
  "id": "merchant_123",
  "params": "{\"payment_type\":\"card\"}",
  "labels_information": [
    {
      "label": "processor_A",
      "target_count": 15000,
      "target_time": 2592000,
      "current_count": 12450
    },
    {
      "label": "processor_B",
      "target_count": 10000,
      "target_time": 2592000,
      "current_count": 7520
    },
    {
      "label": "processor_C",
      "target_count": 12000,
      "target_time": 2592000,
      "current_count": 9870
    }
  ]
}' \
-H 'x-api-key: YOUR_API_KEY' \
-H 'x-tenant-id: tenant_001' \
localhost:9000 contract_routing.ContractScoreCalculator/UpdateContract
```

### InvalidateContract

```bash
grpcurl -plaintext -d '{
  "id": "merchant_123"
}' \
-H 'x-api-key: YOUR_API_KEY' \
-H 'x-tenant-id: tenant_001' \
localhost:9000 contract_routing.ContractScoreCalculator/InvalidateContract
```

### Listing Available Methods

To discover available methods on the service:

```bash
grpcurl -plaintext localhost:9000 list contract_routing.ContractScoreCalculator
```

### Viewing Method Details

To see detailed information about a specific method:

```bash
grpcurl -plaintext localhost:9000 describe contract_routing.ContractScoreCalculator.FetchContractScore
```
