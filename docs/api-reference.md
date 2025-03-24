# Dynamo API Reference

## Overview

Dynamo provides both gRPC and HTTP APIs for its dynamic payment routing services. This reference documents the available endpoints, their parameters, and example usage.

---

## Authentication and Headers

All API requests require appropriate authentication:

| Header | Required | Description |
|:-------|:--------:|:------------|
| `x-api-key` | ✅ | API key for authentication |
| `x-tenant-id` | ✅ | Tenant identifier (*required if multi-tenancy is enabled) |

---

## Available Services

Dynamo offers three main routing services:

### 1. **[Success Rate Calculator](api-reference/success-rate.md)**

Routes payments to processors with the highest historical success rates.

| Endpoint | Description |
|:---------|:------------|
| **`FetchSuccessRate`** | Get success rates for processors |
| **`UpdateSuccessRateWindow`** | Update success/failure data |
| **`InvalidateWindows`** | Reset success rate data |
| **`FetchEntityAndGlobalSuccessRate`** | Get both entity and global success rates |

### 2. **[Elimination Analyser](api-reference/elimination.md)**

Prevents routing to processors that meet failure criteria.

| Endpoint | Description |
|:---------|:------------|
| **`GetEliminationStatus`** | Check if processors should be eliminated |
| **`UpdateEliminationBucket`** | Update failure data |
| **`InvalidateBucket`** | Reset elimination data |

### 3. **[Contract Score Calculator](api-reference/contract.md)**

Routes based on contractual obligations and targets.

| Endpoint | Description |
|:---------|:------------|
| **`FetchContractScore`** | Get contract-based scores |
| **`UpdateContract`** | Update contract fulfillment data |
| **`InvalidateContract`** | Reset contract data |

### 4. **Health**

System health checks.

| Endpoint | Description |
|:---------|:------------|
| **`Check`** | Verify system health status |

**gRPC Method**: `grpc.health.v1.Health/Check`

**HTTP Endpoint**: `POST /grpc.health.v1.Health/Check`

**Request**:

```json
{
  "service": "dynamo"
}
```

**Response**:

```json
{
  "status": 1  // 1 = SERVING
}
```

---

## Protocol Support

All services are available via both:

<table>
  <tr>
    <td width="150px"><strong>gRPC</strong></td>
    <td>Efficient binary protocol for direct integration</td>
  </tr>
  <tr>
    <td><strong>HTTP/JSON</strong></td>
    <td>RESTful interface for broader compatibility</td>
  </tr>
</table>

---

## Quick Start Examples

### 🔹 gRPC Example (using grpcurl)

```bash
grpcurl -d '{
  "id": "merchant_123",
  "params": "{\"payment_method\":\"card\"}",
  "labels": ["processor_A", "processor_B"],
  "config": {
    "min_aggregates_size": 10,
    "default_success_rate": 0.5
  }
}' \
-H 'x-api-key: YOUR_API_KEY' \
-H 'x-tenant-id: tenant_001' \
-plaintext localhost:9000 success_rate.SuccessRateCalculator/FetchSuccessRate
```

### 🔹 HTTP Example (using curl)

```bash
curl -X POST \
  -H "Content-Type: application/json" \
  -H "x-api-key: YOUR_API_KEY" \
  -H "x-tenant-id: tenant_001" \
  -d '{
    "id": "merchant_123",
    "params": "{\"payment_method\":\"card\"}",
    "labels": ["processor_A", "processor_B"],
    "config": {
      "min_aggregates_size": 10,
      "default_success_rate": 0.5
    }
  }' \
  http://localhost:8080/success_rate.SuccessRateCalculator/FetchSuccessRate
```

---

## Error Handling

All services use standard gRPC status codes:

| Status Code | Description | Common Causes |
|:------------|:------------|:--------------|
| <code>OK (0)</code> | ✅ Operation completed successfully | Normal successful operation |
| <code>INVALID_ARGUMENT (3)</code> | ⚠️ Validation failed | Missing required fields, invalid parameters |
| <code>NOT_FOUND (5)</code> | 🔍 Required resource not found | Entity or configuration not found |
| <code>UNAUTHENTICATED (16)</code> | 🔒 Authentication failed | Invalid API key |
| <code>PERMISSION_DENIED (7)</code> | 🚫 Unauthorized access | Insufficient permissions |
| <code>INTERNAL (13)</code> | ❌ Internal server error | Unexpected server-side errors |

---

## Related Documentation

📥 [**Installation**](installation.md)  ⚙️ [**Configuration**](configuration.md)  🔄 [**Dual Protocol**](dual-protocol-layer.md)

