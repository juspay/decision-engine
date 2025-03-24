# Success Rate API Contracts

## Overview

The Success Rate Calculator service provides endpoints for calculating, updating, and managing success rates for payment routing decisions. This service is a core component of the Dynamo routing system, helping optimize payment flows by directing transactions to processors with the highest historical success rates.

## Service Definition

```protobuf
service SuccessRateCalculator {
    rpc FetchSuccessRate (CalSuccessRateRequest) returns (CalSuccessRateResponse);
    rpc UpdateSuccessRateWindow (UpdateSuccessRateWindowRequest) returns (UpdateSuccessRateWindowResponse);
    rpc InvalidateWindows (InvalidateWindowsRequest) returns (InvalidateWindowsResponse);
    rpc FetchEntityAndGlobalSuccessRate (CalGlobalSuccessRateRequest) returns (CalGlobalSuccessRateResponse);
}
```

## Authentication

All endpoints require authentication. Authentication is handled via metadata in the gRPC request. The service supports multi-tenancy, which means data and routing decisions are isolated per tenant.

- Authentication is performed using the `Authenticate` trait
- Authentication requires an `x-api-key` header with a valid API key
- The `x-tenant-id` header is required to specify the tenant context
- Tenant and merchant IDs are extracted from authentication info
- All operations verify authentication before processing

## Endpoints

### 1. FetchSuccessRate

Calculates and returns success rates for a specific entity and its associated labels.

#### Request: `CalSuccessRateRequest`

```protobuf
message CalSuccessRateRequest {
    string id = 1;                 // Entity identifier
    string params = 2;             // Additional parameters for success rate calculation
    repeated string labels = 3;     // Labels to calculate success rates for
    CalSuccessRateConfig config = 4; // Configuration for calculation
}

message CalSuccessRateConfig {
    uint32 min_aggregates_size = 1;   // Minimum number of data points required
    double default_success_rate = 2;   // Default rate to use if insufficient data
    optional SuccessRateSpecificityLevel specificity_level = 3; // ENTITY or GLOBAL level
}

enum SuccessRateSpecificityLevel {
    ENTITY = 0;
    GLOBAL = 1;
}
```

#### Response: `CalSuccessRateResponse`

```protobuf
message CalSuccessRateResponse {
    repeated LabelWithScore labels_with_score = 1; // Success rates for each label
}

message LabelWithScore {
    double score = 1;   // Success rate score (0.0 to 1.0)
    string label = 2;   // Label identifier
}
```

#### Behavior

- Returns calculated success rates for requested labels based on historical data
- If insufficient data exists for a label, uses the default success rate
- Applied specificity level determines if entity-specific or global data is used
- Success rates are returned as scores between 0.0 and 1.0 (or as percentages)
- Labels are returned in descending order of scores

#### Process Flow

1. Authenticate the request and extract tenant/merchant information
2. Fetch configuration settings for the tenant
3. Extract tenant ID from request (for multi-tenancy support)
4. Validate the request parameters
5. Extract entity ID, parameters, and labels
6. Calculate success rates based on historical data
7. Return the scores with associated labels

#### Metrics

- `SUCCESS_BASED_ROUTING_REQUEST`: Counts total requests
- `SUCCESS_BASED_ROUTING_DECISION_REQUEST_TIME`: Measures processing time
- `SUCCESS_BASED_ROUTING_SUCCESSFUL_RESPONSE_COUNT`: Counts successful responses

---

### 2. UpdateSuccessRateWindow

Updates the success/failure status for a set of labels, affecting future routing decisions.

#### Request: `UpdateSuccessRateWindowRequest`

```protobuf
message UpdateSuccessRateWindowRequest {
    string id = 1;                   // Entity identifier
    string params = 2;               // Additional parameters
    repeated LabelWithStatus labels_with_status = 3; // Entity-specific labels with success/failure status
    UpdateSuccessRateWindowConfig config = 4;        // Update configuration
    repeated LabelWithStatus global_labels_with_status = 5; // Global labels with success/failure status
}

message LabelWithStatus {
    string label = 1;   // Label identifier
    bool status = 2;    // Success (true) or failure (false) status
}

message UpdateSuccessRateWindowConfig {
    uint32 max_aggregates_size = 1;               // Maximum size of aggregation window
    CurrentBlockThreshold current_block_threshold = 2; // Threshold configuration
}

message CurrentBlockThreshold {
    optional uint64 duration_in_mins = 1; // Duration-based threshold in minutes
    uint64 max_total_count = 2;          // Count-based threshold
}
```

#### Response: `UpdateSuccessRateWindowResponse`

```protobuf
message UpdateSuccessRateWindowResponse {
   enum UpdationStatus {
      WINDOW_UPDATION_SUCCEEDED = 0;
      WINDOW_UPDATION_FAILED = 1;
   }
   UpdationStatus status = 1; // Status of the update operation
}
```

#### Behavior

- Updates both entity-specific and global success rate windows
- Applies the provided status (success/failure) to each label
- Maintains a sliding window of transaction results based on configuration
- Will return failure status if either entity or global updates fail
- Uses configured thresholds to determine when to rotate windows

#### Process Flow

1. Authenticate the request and extract tenant/merchant information
2. Fetch configuration settings for the tenant
3. Extract tenant ID from request (for multi-tenancy support)
4. Validate the request parameters
5. Extract entity ID, parameters, and label status information
6. Update the entity-specific success rate window
7. Set the specificity level to Global
8. Update the global success rate window
9. Return success/failure status of the operation

#### Metrics

- `SUCCESS_BASED_ROUTING_UPDATE_WINDOW_DECISION_REQUEST_TIME`: Measures processing time
- `SUCCESS_BASED_ROUTING_UPDATE_WINDOW_COUNT`: Counts window update requests

---

### 3. InvalidateWindows

Invalidates success rate windows for a specific entity, effectively resetting its success rate history.

#### Request: `InvalidateWindowsRequest`

```protobuf
message InvalidateWindowsRequest {
    string id = 1;  // Entity identifier to invalidate
}
```

#### Response: `InvalidateWindowsResponse`

```protobuf
message InvalidateWindowsResponse {
   enum InvalidationStatus {
      WINDOW_INVALIDATION_SUCCEEDED = 0;
      WINDOW_INVALIDATION_FAILED = 1;
   }
   InvalidationStatus status = 1; // Status of the invalidation operation
}
```

#### Behavior

- Removes all success rate window data for the specified entity
- This effectively resets the success rate calculation for this entity
- Useful for handling significant processor changes or clearing problematic data
- Does not affect global success rate data

#### Process Flow

1. Authenticate the request
2. Extract tenant ID from request (for multi-tenancy support)
3. Validate the request parameters
4. Extract entity ID to invalidate
5. Remove all success rate window data for the entity
6. Return success/failure status of the operation

---

### 4. FetchEntityAndGlobalSuccessRate

Fetches both entity-specific and global success rates in a single request.

#### Request: `CalGlobalSuccessRateRequest`

```protobuf
message CalGlobalSuccessRateRequest {
    string entity_id = 1;          // Entity identifier
    string entity_params = 2;      // Entity-specific parameters
    repeated string entity_labels = 3; // Labels for entity-specific calculation
    repeated string global_labels = 4; // Labels for global calculation
    CalGlobalSuccessRateConfig config = 5; // Configuration
}

message CalGlobalSuccessRateConfig {
    uint32 entity_min_aggregates_size = 1;   // Minimum aggregates for entity calculation
    double entity_default_success_rate = 2;  // Default success rate for entity
}
```

#### Response: `CalGlobalSuccessRateResponse`

```protobuf
message CalGlobalSuccessRateResponse {
    repeated LabelWithScore entity_scores_with_labels = 1; // Entity-specific success rates
    repeated LabelWithScore global_scores_with_labels = 2; // Global success rates
}
```

#### Behavior

- Performs parallel calculation of both entity and global success rates
- Allows for comparison between entity-specific and global performance
- Useful for analytics and decision-making about routing strategies
- Returns both sets of scores ordered by success rate

#### Process Flow

1. Authenticate the request
2. Extract tenant ID from request (for multi-tenancy support)
3. Validate the request parameters
4. Extract entity ID, parameters, and labels for both entity and global calculations
5. Create entity-specific and global configurations
6. Perform both calculations in parallel using tokio::try_join!
7. Format the results into the response structure
8. Return both sets of success rates

#### Metrics

- `SUCCESS_BASED_ROUTING_METRICS_REQUEST`: Counts metrics requests
- `SUCCESS_BASED_ROUTING_METRICS_DECISION_REQUEST_TIME`: Measures processing time
- `SUCCESS_BASED_ROUTING__METRICS_SUCCESSFUL_RESPONSE_COUNT`: Counts successful responses

## Error Handling

All endpoints return standard gRPC status codes:

| Status Code             | Description                         | When Used                                     |
| ----------------------- | ----------------------------------- | --------------------------------------------- |
| `OK (0)`                | Operation completed successfully    | Normal successful operation                   |
| `NOT_FOUND (5)`         | Required resource not found         | Missing configuration, entity not found       |
| `INVALID_ARGUMENT (3)`  | Validation failed                   | Invalid or missing required parameters        |
| `UNAUTHENTICATED (16)`  | Authentication failed               | Invalid or missing authentication credentials |
| `PERMISSION_DENIED (7)` | Authenticated user lacks permission | User not authorized for the operation         |
| `INTERNAL (13)`         | Internal server error               | Unexpected errors during processing           |

## Multi-tenancy Support

The service supports multi-tenancy with the following behavior:

- All data is isolated per tenant with no cross-tenant data access
- Tenant ID is extracted from request metadata using authentication info
- Each tenant has its own success rate windows and calculations
- Multi-tenancy can be disabled in configuration via `is_multi_tenancy_enabled` flag
- When disabled, a default tenant ID is used

## Performance Considerations

- **Optimized Processing**: The service is optimized for high throughput and low latency
- **Caching**: Success rate calculations are cached for performance
- **Efficient Storage**: Windows are stored efficiently to minimize memory usage
- **Metrics Collection**: Comprehensive metrics are collected for performance monitoring
- **Rust Implementation**: Implementation is in Rust for maximum efficiency and safety
- **Parallel Processing**: Uses concurrent processing where appropriate (e.g., in FetchEntityAndGlobalSuccessRate)
- **Request Validation**: Validates requests early to fail fast and save processing resources

## Example Usage

### Fetch Success Rate Example

```json
// Request
{
  "id": "merchant_123",
  "params": "{\"payment_method\":\"card\"}",
  "labels": ["processor_A", "processor_B", "processor_C"],
  "config": {
    "min_aggregates_size": 10,
    "default_success_rate": 0.5,
    "specificity_level": "ENTITY"
  }
}

// Response
{
  "labels_with_score": [
    {"score": 0.95, "label": "processor_A"},
    {"score": 0.82, "label": "processor_B"},
    {"score": 0.73, "label": "processor_C"}
  ]
}
```

### Update Success Rate Window Example

```json
// Request
{
  "id": "merchant_123",
  "params": "{\"payment_method\":\"card\"}",
  "labels_with_status": [
    {"label": "processor_A", "status": true},
    {"label": "processor_B", "status": false}
  ],
  "global_labels_with_status": [
    {"label": "processor_A", "status": true},
    {"label": "processor_B", "status": false}
  ],
  "config": {
    "max_aggregates_size": 100,
    "current_block_threshold": {
      "duration_in_mins": 60,
      "max_total_count": 1000
    }
  }
}

// Response
{
  "status": "WINDOW_UPDATION_SUCCEEDED"
}
```

### Invalidate Windows Example

```json
// Request
{
  "id": "merchant_123"
}

// Response
{
  "status": "WINDOW_INVALIDATION_SUCCEEDED"
}
```

### Fetch Entity and Global Success Rate Example

```json
// Request
{
  "entity_id": "merchant_123",
  "entity_params": "{\"payment_method\":\"card\"}",
  "entity_labels": ["processor_A", "processor_B", "processor_C"],
  "global_labels": ["processor_A", "processor_B", "processor_C", "processor_D"],
  "config": {
    "entity_min_aggregates_size": 10,
    "entity_default_success_rate": 0.5
  }
}

// Response
{
  "entity_scores_with_labels": [
    {"score": 0.95, "label": "processor_A"},
    {"score": 0.82, "label": "processor_B"},
    {"score": 0.73, "label": "processor_C"}
  ],
  "global_scores_with_labels": [
    {"score": 0.92, "label": "processor_A"},
    {"score": 0.85, "label": "processor_C"},
    {"score": 0.78, "label": "processor_B"},
    {"score": 0.70, "label": "processor_D"}
  ]
}
```

## Integration Notes

- All requests should include appropriate authentication metadata
- JSON parameters in the `params` field should be properly escaped
- Review metrics to monitor system performance
- Consider implementing client-side retries for transient failures
- Cache routing decisions where appropriate to improve performance
- Maintain proper error handling for all API calls

## gRPCurl Examples

The following examples demonstrate how to call the Success Rate API endpoints using gRPCurl. These examples assume the service is running on localhost at port 9000.
We wouldn't require configs as Dynamo will automatically fetch that from he specified profile.

### FetchSuccessRate

```bash
grpcurl -plaintext -d '{
  "id": "merchant_123",
  "params": "{\"payment_method\":\"card\"}",
  "labels": ["processor_A", "processor_B", "processor_C"],
}' \
-H 'x-api-key: YOUR_API_KEY' \
-H 'x-tenant-id: tenant_001' \
-H 'x-profile-id: profile_id' \
localhost:9000 success_rate.SuccessRateCalculator/FetchSuccessRate
```

### UpdateSuccessRateWindow

```bash
grpcurl -plaintext -d '{
  "id": "merchant_123",
  "params": "{\"payment_method\":\"card\"}",
  "labels_with_status": [
    {"label": "processor_A", "status": true},
    {"label": "processor_B", "status": false}
  ],
  "global_labels_with_status": [
    {"label": "processor_A", "status": true},
    {"label": "processor_B", "status": false}
  ]
}' \
-H 'x-api-key: YOUR_API_KEY' \
-H 'x-tenant-id: tenant_001' \
-H 'x-profile-id: profile_id' \
localhost:9000 success_rate.SuccessRateCalculator/UpdateSuccessRateWindow
```

### InvalidateWindows

```bash
grpcurl -plaintext -d '{
  "id": "merchant_123"
}' \
-H 'x-api-key: YOUR_API_KEY' \
-H 'x-tenant-id: tenant_001' \
localhost:9000 success_rate.SuccessRateCalculator/InvalidateWindows
```

### FetchEntityAndGlobalSuccessRate

```bash
grpcurl -plaintext -d '{
  "entity_id": "merchant_123",
  "entity_params": "{\"payment_method\":\"card\"}",
  "entity_labels": ["processor_A", "processor_B", "processor_C"],
  "global_labels": ["processor_A", "processor_B", "processor_C", "processor_D"]
}' \
-H 'x-api-key: YOUR_API_KEY' \
-H 'x-tenant-id: tenant_001' \
-H 'x-profile-id: profile_id' \
localhost:9000 success_rate.SuccessRateCalculator/FetchEntityAndGlobalSuccessRate
```

### Listing Available Methods

To discover available methods on the service:

```bash
grpcurl -plaintext localhost:9000 list success_rate.SuccessRateCalculator
```

### Viewing Method Details

To see detailed information about a specific method:

```bash
grpcurl -plaintext localhost:9000 describe success_rate.SuccessRateCalculator.FetchSuccessRate
```
