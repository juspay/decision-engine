# Elimination Routing API Contracts

## Overview

The Elimination Analyser service provides endpoints for identifying and filtering out underperforming payment processors. This service is a critical component of the Dynamo routing system, helping improve payment success rates by preventing routing to processors with consistently high failure rates.

## Service Definition

```protobuf
service EliminationAnalyser {
    rpc GetEliminationStatus (EliminationRequest) returns (EliminationResponse);
    rpc UpdateEliminationBucket (UpdateEliminationBucketRequest) returns (UpdateEliminationBucketResponse);
    rpc InvalidateBucket (InvalidateBucketRequest) returns (InvalidateBucketResponse);
}
```

## Authentication

All endpoints require authentication. Authentication is handled via metadata in the gRPC request. The service supports multi-tenancy, which means elimination data is isolated per tenant.

- Authentication requires an `x-api-key` header with a valid API key
- The `x-tenant-id` header is required to specify the tenant context
- Tenant ID is extracted from request metadata
- All operations verify tenant permissions before processing
- Multi-tenancy can be enabled/disabled via configuration

## Endpoints

### 1. GetEliminationStatus

Determines which processors should be eliminated from routing consideration based on historical performance data.

#### Request: `EliminationRequest`

```protobuf
message EliminationRequest {
    string id = 1;                   // Entity identifier
    string params = 2;               // Additional parameters for elimination analysis
    repeated string labels = 3;      // Labels (processors) to check for elimination
    EliminationBucketConfig config = 4; // Configuration for elimination buckets
}

message EliminationBucketConfig {
    uint64 bucket_size = 1;             // Maximum failures allowed before elimination
    uint64 bucket_leak_interval_in_secs = 2; // Time interval after which failures are "forgotten"
}
```

#### Response: `EliminationResponse`

```protobuf
message EliminationResponse {
    repeated LabelWithStatus labels_with_status = 1; // Elimination status for each label
}

message LabelWithStatus {
    string label = 1;                       // Label identifier
    EliminationInformation elimination_information = 2; // Elimination details
}

message EliminationInformation {
    BucketInformation entity = 1;  // Entity-specific elimination information
    BucketInformation global = 2;  // Global elimination information
}

message BucketInformation {
    bool is_eliminated = 1;        // Whether the processor should be eliminated
    repeated string bucket_name = 2; // Bucket identifiers that triggered elimination
}
```

#### Behavior

- Evaluates each processor (label) against failure thresholds at both entity and global levels
- Returns elimination status for each processor along with the specific buckets triggering elimination
- A processor can be eliminated at entity level, global level, or both
- Bucket information provides context about which failure conditions were met

#### Process Flow

1. Extract tenant ID from request metadata
2. Validate the request parameters
3. Extract entity ID, parameters, and processor labels
4. Convert the configuration to internal elimination bucket settings
5. Perform elimination analysis for each processor
6. Return detailed elimination status for each processor

---

### 2. UpdateEliminationBucket

Updates the failure records for specific processors, affecting future elimination decisions.

#### Request: `UpdateEliminationBucketRequest`

```protobuf
message UpdateEliminationBucketRequest {
    string id = 1;                       // Entity identifier
    string params = 2;                   // Additional parameters
    repeated LabelWithBucketName labels_with_bucket_name = 3; // Processors with bucket information
    EliminationBucketConfig config = 4;  // Configuration for elimination buckets
}

message LabelWithBucketName {
    string label = 1;       // Processor identifier
    string bucket_name = 2; // Bucket to update (failure type)
}

message EliminationBucketConfig {
    uint64 bucket_size = 1;             // Maximum failures allowed before elimination
    uint64 bucket_leak_interval_in_secs = 2; // Time interval after which failures are "forgotten"
}
```

#### Response: `UpdateEliminationBucketResponse`

```protobuf
message UpdateEliminationBucketResponse {
   enum UpdationStatus {
      BUCKET_UPDATION_SUCCEEDED = 0;
      BUCKET_UPDATION_FAILED = 1;
   }
   UpdationStatus status = 1; // Status of the update operation
}
```

#### Behavior

- Updates failure records for the specified processors and buckets
- Each bucket represents a specific failure type or condition
- Uses a "leaky bucket" algorithm where failures are counted until thresholds are met
- After the specified leak interval, failures are gradually removed from consideration
- Failure to update any processor's bucket will result in a failed status

#### Process Flow

1. Extract tenant ID from request metadata
2. Validate the request parameters
3. Extract entity ID, parameters, and processor-bucket mapping
4. Convert the configuration to internal elimination bucket settings
5. Update the elimination buckets for each processor
6. Return success/failure status of the operation

---

### 3. InvalidateBucket

Invalidates all elimination bucket data for a specific entity, effectively resetting its processor elimination history.

#### Request: `InvalidateBucketRequest`

```protobuf
message InvalidateBucketRequest {
    string id = 1;  // Entity identifier to invalidate
}
```

#### Response: `InvalidateBucketResponse`

```protobuf
message InvalidateBucketResponse {
    enum InvalidationStatus {
        BUCKET_INVALIDATION_SUCCEEDED = 0;
        BUCKET_INVALIDATION_FAILED = 1;
    }
    InvalidationStatus status = 1; // Status of the invalidation operation
}
```

#### Behavior

- Removes all elimination bucket data for the specified entity
- This effectively resets the elimination status for all processors associated with the entity
- Useful for handling significant processor changes or clearing problematic data
- Does not affect global elimination data

#### Process Flow

1. Extract tenant ID from request metadata
2. Validate the request parameters
3. Extract entity ID to invalidate
4. Remove all elimination bucket data for the entity
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

- All elimination data is isolated per tenant with no cross-tenant data access
- Tenant ID is extracted from request metadata
- Each tenant has its own elimination buckets and configurations
- Multi-tenancy can be disabled in configuration via `is_multi_tenancy_enabled` flag
- When disabled, a default tenant ID is used

## Performance Considerations

- **Leaky Bucket Algorithm**: Efficiently tracks failures without unlimited growth of data
- **Time-based Processing**: Automatically ages out old failures based on configuration
- **Optimized Storage**: Buckets are stored efficiently to minimize memory usage
- **Concurrent Processing**: Handles multiple elimination checks in parallel
- **Rust Implementation**: Implementation is in Rust for maximum efficiency and safety
- **Request Validation**: Validates requests early to fail fast and save processing resources

## Example Usage

### Get Elimination Status Example

```json
// Request
{
  "id": "merchant_123",
  "params": "{\"payment_method\":\"card\"}",
  "labels": ["processor_A", "processor_B", "processor_C"],
  "config": {
    "bucket_size": 5,
    "bucket_leak_interval_in_secs": 3600
  }
}

// Response
{
  "labels_with_status": [
    {
      "label": "processor_A",
      "elimination_information": {
        "entity": {
          "is_eliminated": false,
          "bucket_name": []
        },
        "global": {
          "is_eliminated": false,
          "bucket_name": []
        }
      }
    },
    {
      "label": "processor_B",
      "elimination_information": {
        "entity": {
          "is_eliminated": true,
          "bucket_name": ["authentication_failure"]
        },
        "global": {
          "is_eliminated": false,
          "bucket_name": []
        }
      }
    },
    {
      "label": "processor_C",
      "elimination_information": {
        "entity": {
          "is_eliminated": true,
          "bucket_name": ["network_error"]
        },
        "global": {
          "is_eliminated": true,
          "bucket_name": ["network_error", "timeout"]
        }
      }
    }
  ]
}
```

### Update Elimination Bucket Example

```json
// Request
{
  "id": "merchant_123",
  "params": "{\"payment_method\":\"card\"}",
  "labels_with_bucket_name": [
    {
      "label": "processor_A",
      "bucket_name": "authentication_failure"
    },
    {
      "label": "processor_B",
      "bucket_name": "network_error"
    }
  ],
  "config": {
    "bucket_size": 5,
    "bucket_leak_interval_in_secs": 3600
  }
}

// Response
{
  "status": "BUCKET_UPDATION_SUCCEEDED"
}
```

### Invalidate Bucket Example

```json
// Request
{
  "id": "merchant_123"
}

// Response
{
  "status": "BUCKET_INVALIDATION_SUCCEEDED"
}
```

## Integration Notes

- All requests should include appropriate authentication metadata
- JSON parameters in the `params` field should be properly escaped
- Bucket names should represent meaningful failure categories for better analytics
- Consider the bucket_size and leak interval carefully based on traffic patterns
- Very small bucket sizes may lead to premature elimination
- Very large leak intervals may keep processors eliminated for too long
- Maintain proper error handling for all API calls

## gRPCurl Examples

The following examples demonstrate how to call the Elimination Analyser API endpoints using gRPCurl. These examples assume the service is running on localhost at port 9000.

### GetEliminationStatus

```bash
grpcurl -plaintext -d '{
  "id": "merchant_123",
  "params": "{\"payment_method\":\"card\"}",
  "labels": ["processor_A", "processor_B", "processor_C"],
  "config": {
    "bucket_size": 5,
    "bucket_leak_interval_in_secs": 3600
  }
}' \
-H 'x-api-key: YOUR_API_KEY' \
-H 'x-tenant-id: tenant_001' \
localhost:9000 elimination.EliminationAnalyser/GetEliminationStatus
```

### UpdateEliminationBucket

```bash
grpcurl -plaintext -d '{
  "id": "merchant_123",
  "params": "{\"payment_method\":\"card\"}",
  "labels_with_bucket_name": [
    {
      "label": "processor_A",
      "bucket_name": "authentication_failure"
    },
    {
      "label": "processor_B",
      "bucket_name": "network_error"
    }
  ],
  "config": {
    "bucket_size": 5,
    "bucket_leak_interval_in_secs": 3600
  }
}' \
-H 'x-api-key: YOUR_API_KEY' \
-H 'x-tenant-id: tenant_001' \
localhost:9000 elimination.EliminationAnalyser/UpdateEliminationBucket
```

### InvalidateBucket

```bash
grpcurl -plaintext -d '{
  "id": "merchant_123"
}' \
-H 'x-api-key: YOUR_API_KEY' \
-H 'x-tenant-id: tenant_001' \
localhost:9000 elimination.EliminationAnalyser/InvalidateBucket
```

### Listing Available Methods

To discover available methods on the service:

```bash
grpcurl -plaintext localhost:9000 list elimination.EliminationAnalyser
```

### Viewing Method Details

To see detailed information about a specific method:

```bash
grpcurl -plaintext localhost:9000 describe elimination.EliminationAnalyser.GetEliminationStatus
```
