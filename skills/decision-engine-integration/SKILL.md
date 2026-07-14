---
name: decision-engine-integration
description: >-
  Wire a self-hosted Juspay Decision Engine into a merchant's own payment
  orchestrator so PSP/gateway selection is delegated to Decision Engine. Use this
  skill whenever the user is working in their orchestration / payment-routing /
  checkout backend and wants to call Decision Engine's `decide-gateway` (to pick
  the best PSP before authorizing) and `update-gateway-score` (to feed the
  auth/charge outcome back), or mentions "Decision Engine", "decide gateway",
  "gateway score", "PSP routing", "dynamic routing", "success-rate routing",
  "which gateway should I use", or hooking a Vault/tokenized-card checkout up to
  smart routing. Trigger this even when the user only describes the goal
  ("route this payment to the best acquirer", "add smart routing before we
  charge the PSP") without naming the APIs. This skill covers discovering the
  authorize/charge path in the orchestrator, inserting the two calls in the
  right place, mapping the tokenized-card + BIN metadata into the request,
  failing open safely, and certifying the integration with runnable tests and
  AI evals. It can also stand up Decision Engine locally if it is not running.
---

# Decision Engine Integration

You are integrating a **self-hosted Juspay Decision Engine** into a merchant's
**own orchestrator** — the service that already decides which PSP/acquirer to
send a payment to and then makes the Authorize/Charge call. After this
integration, that PSP choice is delegated to Decision Engine.

Decision Engine is a stateless HTTP routing brain. It never sees the PAN. Your
orchestrator asks it *"given this payment, which of my gateways should I use?"*
(`POST /decide-gateway`) and later tells it *"here's how that attempt turned
out"* (`POST /update-gateway-score`) so its success-rate model keeps improving.

```
Vault checkout (tokenizes card)                 Decision Engine (self-hosted)
        │  token + metadata (incl. card BIN)          ▲            │
        ▼                                             │ decide     │ decision
  Merchant server ──────────────────────────────────┘            ▼
        │                                        ┌── decided_gateway + order
        ▼                                        │
  Orchestrator ── Authorize/Charge ──▶ chosen PSP
        │                                        │
        └──────────── update-gateway-score ◀─────┘  (auth outcome feedback)
```

## The critical mental model

Two calls, placed around the code that already picks + charges a PSP:

1. **`decide-gateway`** goes in **right before** the orchestrator selects the
   PSP for the Authorize/Charge call. It *replaces or informs* whatever static
   priority list is there today. Input = the payment context (amount, currency,
   payment method, **card BIN**, eligible gateways). Output = the ordered
   gateway the orchestrator should try.

2. **`update-gateway-score`** goes **after** the PSP returns a terminal
   Authorize/Charge result. It reports the outcome (`CHARGED`, `AUTHORIZED`,
   `AUTHORIZATION_FAILED`, …) keyed by the *same* `paymentId` and `gateway` you
   just used, so the success-rate model learns.

Get these two placements right and the integration is 90% done. Everything else
(config, fail-open, mapping) is in service of these two.

## Workflow

Work through these phases in order. Do not skip discovery — placing the calls in
the wrong function is the most common way this integration goes wrong.

### Phase 0 — Confirm Decision Engine is reachable

Ask the user (or check env/config) for the base URL and API key. Then:

```bash
curl -sS "$DECISION_ENGINE_URL/health"          # expect {"status":"UP"} style body, 200
curl -sS "$DECISION_ENGINE_URL/health/ready"     # expect 200 when DB/Redis are wired
```

- If it responds → record `DECISION_ENGINE_URL`, the `x-api-key`, and the
  `merchantId`, and go to Phase 1.
- If the user does **not** have Decision Engine running → read
  [references/local-setup.md](references/local-setup.md) and stand it up locally
  (Docker Compose one-liner + merchant/API-key bootstrap). **Skip this if they
  already have a running instance** (self-hosted, sandbox, or otherwise).

### Phase 1 — Discover the orchestrator's payment path

Before writing anything, map the existing flow. Search the repo for:

- The **PSP/gateway selection** point — where the code picks an acquirer/connector
  (look for a hard-coded priority list, a `switch`/`match` on gateway, a
  "connectors"/"gateways"/"acquirers" config, a `select_gateway`-style function).
- The **Authorize/Charge call site** — where the chosen PSP's API is invoked.
- The **card/token intake** — where the Vault checkout token + metadata land on
  the merchant server. This is where the **card BIN** (`cardIsin`, first 6–8
  digits) lives. Decision Engine needs the BIN, never the PAN or the token.
- The **outcome handler** — where the PSP's Authorize/Charge response is parsed
  into success/failure.

Report back a short map: *"selection happens in X, authorize in Y, outcome in Z,
BIN is available at W."* Confirm with the user if any of these are ambiguous —
guessing here produces a broken integration.

### Phase 2 — Insert `decide-gateway` before PSP selection

Add a client call that, at the selection point, sends the payment context to
`decide-gateway` and uses `decided_gateway` as the PSP to authorize with (and
`gateway_priority_map` / `priorityLogicOutput.gws` as the fallback order).

Map the orchestrator's data into the request. **Full field reference and every
enum value is in [references/api-contract.md](references/api-contract.md)** —
read it before writing the client. The essential mapping:

| Decision Engine field | Source in the orchestrator |
| --- | --- |
| `merchantId` | your configured merchant id |
| `eligibleGatewayList` | the PSPs this payment is actually allowed to use |
| `rankingAlgorithm` | `SR_BASED_ROUTING` for success-rate routing (default) |
| `paymentInfo.paymentId` | your payment/attempt id (**reuse in the score call**) |
| `paymentInfo.amount` / `currency` / `country` | the order |
| `paymentInfo.paymentMethodType` | `CARD` |
| `paymentInfo.paymentMethod` | `CREDIT` or `DEBIT` |
| `paymentInfo.authType` | `THREE_DS` / `NO_THREE_DS` |
| `paymentInfo.cardIsin` | **the card BIN from the Vault token metadata** |

Critical implementation rules — explain these in code comments so future
maintainers understand the *why*:

- **Fail open.** `decide-gateway` sits in the live authorization path. If it
  errors, times out (set a tight timeout, e.g. 150–300 ms), or returns a gateway
  not in your eligible list, **fall back to the orchestrator's existing routing**
  and continue. A routing brain outage must never block payments.
- **Never send the PAN or the raw token.** Only the BIN (`cardIsin`) plus
  non-sensitive metadata go to Decision Engine. This keeps it out of PCI scope.
- **Keep `paymentId` stable** for this attempt — you must send the same value to
  `update-gateway-score` or the feedback won't attribute to the right decision.

### Phase 3 — Insert `update-gateway-score` after the outcome

After the PSP returns a **terminal** Authorize/Charge result, report it:

- Send `merchantId`, `gateway` (the PSP you actually used), `paymentId` (the
  same one from Phase 2), and `status` mapped to a Decision Engine `TxnStatus`.
- Success → `CHARGED` (captured) or `AUTHORIZED`. Failure →
  `AUTHORIZATION_FAILED` (most common), `FAILURE`, `AUTHENTICATION_FAILED`, or
  `JUSPAY_DECLINED`. **Do not report non-terminal states** like `PENDING` /
  `AUTHORIZING` — wait for the final outcome. Full status table is in the API
  contract reference.
- Make this call **out of the critical path** (fire-and-forget / async / queued)
  and idempotent per `paymentId` so retries don't double-count. A failed score
  update must not fail the payment.
- **Skip the score call** for `NTW_BASED_ROUTING` debit runs where
  `decided_gateway` is a network rather than the PSP you charged — see the
  contract reference note.

### Phase 4 — Configuration & wiring

Externalize `DECISION_ENGINE_URL`, the `x-api-key`, `merchantId`, the timeout,
and a **feature flag / kill-switch** to disable the whole integration and revert
to legacy routing instantly. Match the surrounding codebase's config idiom
(env vars, config service, secrets manager) — don't invent a new one.

### Phase 5 — Certify the integration

An integration that compiles is not a working integration. Certify it with the
runnable tests and AI evals in
[references/testing-and-evals.md](references/testing-and-evals.md). At minimum:

- Run [scripts/smoke_test.sh](scripts/smoke_test.sh) against a live/local
  Decision Engine to prove the round trip (bootstrap → decide → charge → score).
- Add the integration test cases (happy path, fallback ordering, **fail-open when
  Decision Engine is down**, BIN propagation, feedback attribution, debit-skip)
  to the orchestrator's own test suite.
- Run the AI eval rubric in the reference to grade the integration code for
  correctness and safety before shipping.

## Reference material

Read these as needed — don't load them all upfront:

- **[references/api-contract.md](references/api-contract.md)** — exact
  request/response schemas, every enum value, error shapes, auth headers, and
  the debit/BIN metadata format. Read before writing the client (Phase 2/3).
- **[references/local-setup.md](references/local-setup.md)** — download, run,
  and bootstrap Decision Engine locally, including merchant + API key + SR config
  creation. Read only if the user needs a local instance (Phase 0).
- **[references/testing-and-evals.md](references/testing-and-evals.md)** — the
  test matrix and the AI eval rubric to certify the integration (Phase 5).
- **[scripts/smoke_test.sh](scripts/smoke_test.sh)** — end-to-end round-trip
  check against a running Decision Engine.
- **[scripts/certify_integration.py](scripts/certify_integration.py)** — a
  contract-level certifier that exercises decide + score and checks invariants.
