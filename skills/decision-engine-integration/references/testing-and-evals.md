# Certifying the integration: tests + AI evals

A Decision Engine integration touches the live money path, so "it compiled" is
not the bar. Certify it two ways:

1. **Behavioral tests** — deterministic tests you add to the orchestrator's own
   suite that prove the integration behaves correctly, including when Decision
   Engine misbehaves.
2. **AI evals** — an LLM-as-judge rubric that grades the *integration code* for
   the correctness and safety properties that are easy to get subtly wrong.

Do both. The tests catch regressions; the eval catches design mistakes a passing
test wouldn't (e.g. blocking payments when DE is down).

## Part 1 — Behavioral test matrix

Implement each of these against the orchestrator's payment flow, mocking the
Decision Engine HTTP client and the PSP client. Names are suggestions; adapt to
the repo's test framework.

| # | Test | Setup | Assert |
| --- | --- | --- | --- |
| T1 | `decide_gateway_chooses_returned_psp` | DE returns `decided_gateway: "adyen"` | Orchestrator authorizes against **adyen**. |
| T2 | `decide_gateway_sends_card_bin` | Capture the outgoing DE request | `paymentInfo.cardIsin` equals the BIN from the Vault token metadata; **no PAN / full token** anywhere in the body. |
| T3 | `fallback_order_used_on_primary_failure` | DE returns priority `["stripe","adyen"]`; stripe auth fails | Orchestrator retries **adyen** (if retry is in scope) per the returned order. |
| T4 | `fail_open_when_de_unreachable` | DE client throws / times out | Payment still authorizes via **legacy routing**; no exception surfaces to the customer. |
| T5 | `fail_open_on_de_error_response` | DE returns HTTP 400/500 or a body with no `decided_gateway` | Same as T4 — legacy routing, payment proceeds. |
| T6 | `fail_open_on_ineligible_gateway` | DE returns a `decided_gateway` **not** in `eligibleGatewayList` | Orchestrator ignores it and falls back. |
| T7 | `score_reports_terminal_success` | PSP returns captured | `update-gateway-score` called once with matching `paymentId`, `gateway`, and `status: CHARGED` (or `AUTHORIZED`). |
| T8 | `score_reports_terminal_failure` | PSP declines | Score called with `status: AUTHORIZATION_FAILED` (or mapped failure). |
| T9 | `score_not_sent_for_pending` | PSP returns pending/authorizing | **No** score call until a terminal outcome. |
| T10 | `score_paymentid_matches_decide` | Full round trip | The `paymentId` in the score call is byte-identical to the one in the decide call. |
| T11 | `score_failure_does_not_fail_payment` | `update-gateway-score` throws | Payment result is unaffected (score is out-of-band / best-effort). |
| T12 | `kill_switch_disables_integration` | Feature flag off | Neither DE endpoint is called; legacy routing used. |
| T13 | `debit_network_skip` (only if debit routing used) | `NTW_BASED_ROUTING` run where `decided_gateway` is a network | No `update-gateway-score` call for that run. |
| T14 | `decide_timeout_is_bounded` | DE delays beyond the timeout | Client aborts within the configured budget and falls back (no unbounded wait in the auth path). |

The non-negotiables are **T4/T5/T6 (fail-open)**, **T2 (BIN not PAN)**, and
**T10 (paymentId attribution)**. If any of those fail, the integration is not
ready regardless of the rest.

## Part 2 — Live round-trip check

Against a real/local Decision Engine, run [../scripts/smoke_test.sh](../scripts/smoke_test.sh)
(bootstrap → decide → score) and
[../scripts/certify_integration.py](../scripts/certify_integration.py), which
asserts contract invariants: `decided_gateway ∈ eligibleGatewayList`, decide is
idempotent-safe, score accepts the same `paymentId`, and a malformed request is
rejected with the documented error shape.

## Part 3 — AI eval rubric (LLM-as-judge)

Use this to grade the integration diff/code. Point an agent at the changed files
and the rubric below; have it return a per-criterion PASS/FAIL with a one-line
evidence quote (file:line) for each. This catches design-level issues that unit
tests may not.

Score each criterion 0/1. **A failure on any "critical" item = the integration
does not pass, regardless of total score.**

```
CRITICAL (any fail blocks release):
[C1] Fail-open: every path where decide-gateway errors, times out, or returns an
     unusable/ineligible gateway falls back to legacy routing and lets the
     payment proceed. No path lets a routing-brain problem block or error a
     payment. (evidence: file:line)
[C2] No PAN / no raw token is sent to Decision Engine. Only the BIN (cardIsin)
     and non-sensitive metadata cross the boundary. (evidence)
[C3] The decide-gateway call has a bounded timeout appropriate for the live auth
     path (roughly ≤300ms), not a default/infinite client timeout. (evidence)
[C4] update-gateway-score uses the SAME paymentId that was sent to
     decide-gateway, and the same gateway that was actually charged. (evidence)
[C5] A failing/slow update-gateway-score call cannot fail or delay the payment
     (it is async / fire-and-forget / queued / best-effort). (evidence)

IMPORTANT (fails count against the grade):
[I1] update-gateway-score is only sent on TERMINAL outcomes; non-terminal states
     (PENDING/AUTHORIZING/etc.) are not reported. (evidence)
[I2] Success/failure are mapped to valid TxnStatus values
     (CHARGED/AUTHORIZED vs AUTHORIZATION_FAILED/FAILURE/…). (evidence)
[I3] There is a kill-switch / feature flag to disable the integration and revert
     to legacy routing without a code change. (evidence)
[I4] The returned decided_gateway is validated against eligibleGatewayList before
     use. (evidence)
[I5] Config (URL, api key, merchantId, timeout) is externalized, not hard-coded;
     the api key is handled as a secret. (evidence)
[I6] eligibleGatewayList reflects the gateways actually valid for THIS payment,
     not a stale global list.

HYGIENE (nice to have):
[H1] routing_approach / decision is logged for observability and debugging.
[H2] The DE client matches the codebase's existing HTTP/client conventions
     (retries, tracing, error types) rather than a bespoke one-off.
[H3] The debit-network skip rule is honored if debit routing is in scope.
[H4] Tests from the matrix above exist and cover at least the CRITICAL items.

Output format, one line per item:
  [C1] PASS — <quote> (path/to/file.ext:NN)
  [C2] FAIL — <what's missing or wrong> (path/to/file.ext:NN)
Then: VERDICT: PASS only if all CRITICAL pass; otherwise FAIL, listing the
critical failures.
```

### Running the eval as a subagent (optional)

If the coding agent supports subagents, spawn a fresh one with the rubric and the
list of changed files so the judgment is independent of the author. Otherwise run
the rubric inline as a final self-review pass before declaring the integration
done. Report the verdict to the user with the evidence lines — don't just say
"looks good."

## What "certified" means

Report the integration as ready only when: all CRITICAL rubric items pass, the
fail-open + BIN + paymentId-attribution tests (T2, T4, T5, T6, T10) pass, and the
live smoke test round-trips successfully. Anything short of that, say so
explicitly and list what's outstanding.
