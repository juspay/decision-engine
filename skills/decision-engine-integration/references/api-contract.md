# Decision Engine API Contract

Everything the orchestrator client needs for the two integration endpoints.
All bodies are JSON.

**Casing matters and is not uniform.** Request bodies use **camelCase**
(`merchantId`, `paymentInfo`, `cardIsin`, `rankingAlgorithm`). The
`/decide-gateway` **response uses snake_case** (`decided_gateway`,
`gateway_priority_map`, `routing_approach`, `priority_logic_output`) — with the
exception of the nested `priority_logic_output` object, whose inner keys are
camelCase (`isEnforcement`, `gws`, `priorityLogicTag`, `gatewayReferenceIds`).
Field names below are given exactly as they appear on the wire; deserialize
against these, not a normalized guess.

## Table of contents

- [Auth & base URL](#auth--base-url)
- [POST /decide-gateway](#post-decide-gateway)
  - [Request](#decide-gateway-request)
  - [paymentInfo fields](#paymentinfo-fields)
  - [Response](#decide-gateway-response)
  - [Ranking algorithms](#ranking-algorithms)
  - [routing_approach values](#routing_approach-values)
- [POST /update-gateway-score](#post-update-gateway-score)
  - [TxnStatus values](#txnstatus-values)
- [Debit / network routing & the BIN metadata](#debit--network-routing--the-bin-metadata)
- [Error shape](#error-shape)
- [Where these come from in the source](#where-these-come-from-in-the-source)

## Auth & base URL

- Base URL of a local/self-hosted instance: `http://localhost:8080`.
- Both endpoints are **protected**. Send one of:
  - `x-api-key: DE_<key>` (server-to-server — use this for the orchestrator), or
  - `Authorization: Bearer <jwt>` (dashboard sessions).
- Content type is `application/json`.
- If routing through Hyperswitch sandbox, also add `x-feature: decision-engine`.

## POST /decide-gateway

Picks the gateway for one payment attempt. Call it at the PSP-selection point,
before Authorize/Charge.

### decide-gateway request

```json
{
  "merchantId": "merchant_demo",
  "eligibleGatewayList": ["stripe", "adyen", "checkout"],
  "rankingAlgorithm": "SR_BASED_ROUTING",
  "eliminationEnabled": true,
  "paymentInfo": {
    "paymentId": "attempt_abc123",
    "amount": 1000,
    "currency": "USD",
    "country": "US",
    "paymentType": "ORDER_PAYMENT",
    "paymentMethodType": "CARD",
    "paymentMethod": "CREDIT",
    "authType": "THREE_DS",
    "cardIsin": "424242",
    "metadata": null
  }
}
```

Top-level fields:

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `merchantId` | string | yes | Your merchant id in Decision Engine. |
| `eligibleGatewayList` | string[] | recommended | The PSPs this payment may use. If omitted, DE uses the merchant's configured set. Pass the *actual* eligible list per payment. |
| `rankingAlgorithm` | enum | yes | See [ranking algorithms](#ranking-algorithms). Default `SR_BASED_ROUTING`. |
| `eliminationEnabled` | bool | optional | When true, gateways in detected downtime are deprioritized/excluded. |
| `paymentInfo` | object | yes | The payment context, below. |

### paymentInfo fields

These map 1:1 to the `PaymentInfo` struct. Only a few are required; send what
you have. The BIN (`cardIsin`) is what makes card-aware routing work.

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `paymentId` | string | yes | Your attempt id. **Reuse verbatim in `update-gateway-score`.** |
| `amount` | number | yes | Order amount (major units, e.g. `1000` = 1000.00). |
| `currency` | enum | yes | ISO code, e.g. `USD`, `INR`. |
| `country` | string | optional | ISO-2, e.g. `US`. |
| `customerId` | string | optional | |
| `preferredGateway` | string | optional | Hint a specific gateway. |
| `paymentType` | enum | yes | Usually `ORDER_PAYMENT`. Also `MANDATE_PAYMENT`, `MANDATE_REGISTER`, `TPV_PAYMENT`, etc. |
| `paymentMethodType` | string | yes | `CARD` for card flows. |
| `paymentMethod` | string | yes | `CREDIT` or `DEBIT` (or `NET_BANKING`, `WALLET`, …). |
| `authType` | enum | optional | `THREE_DS`, `NO_THREE_DS`, `OTP`, … |
| `cardIsin` | string | **strongly recommended** | The **card BIN** (first 6–8 digits) from the Vault token metadata. Enables BIN/network-level routing. Never send the full PAN. |
| `cardIssuerBankName` | string | optional | If Vault metadata provides it. |
| `cardType` | string | optional | e.g. `CREDIT`, `DEBIT`. |
| `metadata` | string (JSON) | optional | A **stringified** JSON blob. Required for debit/network routing (`co_badged_card_data`) — see [debit section](#debit--network-routing--the-bin-metadata). |
| `internalMetadata` | string | optional | |
| `isEmi` / `emiBank` / `emiTenure` | bool/string/int | optional | EMI payments. |
| `udfs` | string[] | optional | User-defined fields. |
| `cardSwitchProvider` | string | optional | |

### decide-gateway response

```json
{
  "decided_gateway": "stripe",
  "gateway_priority_map": { "stripe": 0.94, "adyen": 0.91, "checkout": 0.88 },
  "routing_approach": "SR_SELECTION_V3_ROUTING",
  "gateway_before_evaluation": "stripe",
  "priority_logic_output": {
    "isEnforcement": false,
    "gws": ["stripe", "adyen", "checkout"],
    "priorityLogicTag": null,
    "gatewayReferenceIds": {}
  },
  "reset_approach": "NO_RESET",
  "routing_dimension": "ORDER_PAYMENT,CARD,CREDIT,UNKNOWN",
  "routing_dimension_level": "CARD_LEVEL",
  "debit_routing_output": null,
  "is_scheduled_outage": false,
  "is_rust_based_decider": true
}
```

How the orchestrator uses it:

- **`decided_gateway`** — the PSP to authorize with. This is your answer.
- **`gateway_priority_map`** — score per gateway (higher = better). Use it, or
  `priority_logic_output.gws`, as the **fallback order** if the primary
  Authorize fails and you retry another PSP.
- **`routing_approach`** — why this gateway was chosen (SR selection, downtime
  hedging, priority logic, network-based). Useful to log for debugging.
- **`debit_routing_output`** — present only for debit/network routing.

Always validate that `decided_gateway` is in your `eligibleGatewayList` before
acting on it; if not (or the field is absent), fall back to legacy routing.

### Ranking algorithms

| Value | Meaning |
| --- | --- |
| `SR_BASED_ROUTING` | Success-rate based. The default for smart card routing. Requires a success-rate config (see local-setup). |
| `PL_BASED_ROUTING` | Priority-logic based: evaluates the merchant's active priority rule and returns the ordered connectors. |
| `NTW_BASED_ROUTING` | Debit-network based; needs `co_badged_card_data` in `metadata` and the merchant debit-routing flag enabled. |
| `NTW_SR_HYBRID_ROUTING` | Combines debit/network metadata with SR scoring. |

### routing_approach values

Returned for SR routing — log them, they explain the decision:

- `SR_SELECTION_V3_ROUTING` — best eligible gateway by SR score.
- `SR_V3_DOWNTIME_ROUTING` — some gateways deprioritized due to downtime.
- `SR_V3_ALL_DOWNTIME_ROUTING` — all eligible down; best degraded option chosen.
- `SR_V3_HEDGING` / `SR_V3_DOWNTIME_HEDGING` / `SR_V3_ALL_DOWNTIME_HEDGING` —
  exploration modes.
- `PRIORITY_LOGIC` — chosen by priority rule (`PL_BASED_ROUTING`).
- `NTW_BASED_ROUTING` — chosen by debit-network routing.

## POST /update-gateway-score

Reports the terminal outcome so the SR model learns. Call after Authorize/Charge
returns a final status.

### update-gateway-score request

```json
{
  "merchantId": "merchant_demo",
  "gateway": "stripe",
  "gatewayReferenceId": null,
  "status": "CHARGED",
  "paymentId": "attempt_abc123",
  "enforceDynamicRoutingFailure": null
}
```

| Field | Type | Required | Notes |
| --- | --- | --- | --- |
| `merchantId` | string | yes | Same merchant as the decide call. |
| `gateway` | string | yes | The PSP you actually charged. |
| `paymentId` | string | yes | **Must equal** the `paymentInfo.paymentId` sent to `decide-gateway`. |
| `status` | enum | yes | Terminal `TxnStatus` — see below. |
| `gatewayReferenceId` | string | optional | If you route by a specific gateway MID/reference. |
| `enforceDynamicRoutingFailure` | bool | optional | Force-count as a routing failure; usually `null`. |
| `txnLatency` | object | optional | `{ "gatewayLatency": <ms> }` if you measure PSP latency. |

Success response body is the literal:

```
Success
```

(A JSON `{message, merchantId, gateway, paymentId}` may be returned depending on
version; treat HTTP 200 as success.)

### TxnStatus values

Map your PSP's terminal outcome to one of these. The ones that matter for card
auth feedback:

| Category | Values | Send when |
| --- | --- | --- |
| Success | `CHARGED`, `AUTHORIZED`, `PARTIAL_CHARGED` | Payment authorized/captured. `CHARGED` = captured, `AUTHORIZED` = auth-only. |
| Failure | `AUTHORIZATION_FAILED`, `FAILURE`, `DECLINED`, `JUSPAY_DECLINED`, `AUTHENTICATION_FAILED` | PSP declined / auth failed / 3DS failed. `AUTHORIZATION_FAILED` is the common card-decline case. |
| **Do NOT send (non-terminal)** | `STARTED`, `PENDING`, `PENDING_VBV`, `AUTHORIZING`, `TO_BE_CHARGED`, `CAPTURE_INITIATED`, `VOID_INITIATED` | Wait for a terminal state first. |

Full enum (for reference): `STARTED`, `AUTHENTICATION_FAILED`,
`JUSPAY_DECLINED`, `PENDING_VBV`, `VBV_SUCCESSFUL`, `AUTHORIZED`,
`AUTHORIZATION_FAILED`, `CHARGED`, `AUTHORIZING`, `COD_INITIATED`, `VOIDED`,
`VOID_INITIATED`, `NOP`, `CAPTURE_INITIATED`, `CAPTURE_FAILED`, `VOID_FAILED`,
`AUTO_REFUNDED`, `PARTIAL_CHARGED`, `TO_BE_CHARGED`, `PENDING`, `FAILURE`,
`DECLINED`.

## Debit / network routing & the BIN metadata

For `NTW_BASED_ROUTING` / `NTW_SR_HYBRID_ROUTING`, the debit inputs go inside
`paymentInfo.metadata` as a **stringified JSON**, and the merchant debit-routing
flag must be enabled first (`POST /merchant-account/:merchant-id/debit-routing`).

```json
"metadata": "{\"merchant_category_code\":\"merchant_category_code_0001\",\"acquirer_country\":\"US\",\"co_badged_card_data\":{\"co_badged_card_networks\":[\"VISA\",\"NYCE\",\"PULSE\",\"STAR\"],\"issuer_country\":\"US\",\"is_regulated\":false,\"regulated_name\":null,\"card_type\":\"debit\"}}"
```

The response then carries `debit_routing_output` with per-network
`saving_percentage`. **Important feedback rule:** for `NTW_BASED_ROUTING` runs
where `decided_gateway` is the selected *network* (not the PSP you charged), do
**not** call `update-gateway-score` for that run — it would corrupt SR data.

## Error shape

On a bad request the endpoints return an `ErrorResponse`:

```json
{
  "status": "400",
  "error_code": "400",
  "error_message": "Error parsing request",
  "error_info": {
    "code": "INVALID_INPUT",
    "user_message": "Invalid request params. Please verify your input.",
    "developer_message": "<detail>"
  },
  "priority_logic_output": null,
  "is_dynamic_mga_enabled": false
}
```

Treat any non-2xx (or a body without a usable `decided_gateway`) as a signal to
**fail open** to legacy routing rather than blocking the payment.

## Where these come from in the source

If you have the Decision Engine repo checked out and need to verify a field:

- Routes registered in `src/app.rs` (`/decide-gateway`, `/update-gateway-score`).
- `decide-gateway` request: `DomainDeciderRequestForApiCallV2` / `PaymentInfo`
  in `src/decider/gatewaydecider/types.rs`.
- `update-gateway-score` request: `UpdateScorePayload` in `src/feedback/types.rs`.
- `TxnStatus` enum: `src/types/txn_details/types.rs`.
- Worked curl examples: `docs/api-refs/decide-gateway-*.mdx` and
  `docs/api-refs/update-gateway-score.mdx`.
