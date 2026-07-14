#!/usr/bin/env python3
"""
certify_integration.py — contract-level certification of a live Decision Engine.

Exercises /decide-gateway and /update-gateway-score against a running instance
and asserts the invariants an orchestrator integration relies on:

  1. decide-gateway returns a decided_gateway that is IN the eligible list.
  2. The same paymentId is accepted by update-gateway-score.
  3. A malformed request is rejected (documented error shape), not silently
     accepted — so the orchestrator's fail-open branch is actually reachable.
  4. update-gateway-score is idempotent-safe for the same paymentId (a second
     call does not error).

This certifies the ENGINE side of the contract. The orchestrator-side behavior
(fail-open, BIN-not-PAN, paymentId reuse) is certified by the behavioral test
matrix in references/testing-and-evals.md.

Usage:
  DECISION_ENGINE_URL=http://localhost:8080 \
  API_KEY=DE_xxx \
  MERCHANT_ID=merchant_demo \
  python3 certify_integration.py

Only depends on the Python standard library.
"""
import json
import os
import sys
import time
import urllib.request
import urllib.error

BASE = os.environ.get("DECISION_ENGINE_URL", "http://localhost:8080").rstrip("/")
API_KEY = os.environ.get("API_KEY", "")
MERCHANT_ID = os.environ.get("MERCHANT_ID", "merchant_demo")
ELIGIBLE = json.loads(os.environ.get("ELIGIBLE", '["stripe","adyen","checkout"]'))

results = []


def record(name, ok, detail=""):
    results.append((name, ok, detail))
    print(f"  [{'PASS' if ok else 'FAIL'}] {name}" + (f" — {detail}" if detail else ""))


def call(path, body, extra_headers=None):
    headers = {"Content-Type": "application/json"}
    if API_KEY:
        headers["x-api-key"] = API_KEY
    if extra_headers:
        headers.update(extra_headers)
    data = json.dumps(body).encode()
    req = urllib.request.Request(BASE + path, data=data, headers=headers, method="POST")
    try:
        with urllib.request.urlopen(req, timeout=10) as r:
            raw = r.read().decode()
            try:
                return r.status, json.loads(raw)
            except json.JSONDecodeError:
                return r.status, raw
    except urllib.error.HTTPError as e:
        raw = e.read().decode()
        try:
            return e.code, json.loads(raw)
        except json.JSONDecodeError:
            return e.code, raw
    except urllib.error.URLError as e:
        # Engine unreachable (connection refused / DNS / TLS). Surface a clean
        # sentinel instead of a stack trace — a certifier that can't reach the
        # engine should report FAIL, not crash.
        return 0, {"error": f"unreachable: {e.reason}"}


def main():
    if not API_KEY:
        print("API_KEY is required (x-api-key). Set API_KEY=DE_...", file=sys.stderr)
        sys.exit(2)

    payment_id = f"cert_{int(time.time())}"
    print(f"Certifying {BASE} for merchant '{MERCHANT_ID}' (paymentId={payment_id})\n")

    # 1. decide-gateway happy path -------------------------------------------
    status, body = call("/decide-gateway", {
        "merchantId": MERCHANT_ID,
        "eligibleGatewayList": ELIGIBLE,
        "rankingAlgorithm": "SR_BASED_ROUTING",
        "eliminationEnabled": True,
        "paymentInfo": {
            "paymentId": payment_id, "amount": 1000, "currency": "USD",
            "country": "US", "paymentType": "ORDER_PAYMENT",
            "paymentMethodType": "CARD", "paymentMethod": "CREDIT",
            "authType": "THREE_DS", "cardIsin": "424242",
        },
    })
    decided = body.get("decided_gateway") if isinstance(body, dict) else None
    record("decide-gateway returns 200", status == 200, f"status={status}")
    record("decided_gateway present", bool(decided), f"decided_gateway={decided}")
    record("decided_gateway is eligible", decided in ELIGIBLE if decided else False,
           f"{decided} in {ELIGIBLE}")

    gateway = decided if decided in ELIGIBLE else ELIGIBLE[0]

    # 2. update-gateway-score with the SAME paymentId ------------------------
    status, body = call("/update-gateway-score", {
        "merchantId": MERCHANT_ID, "gateway": gateway,
        "gatewayReferenceId": None, "status": "CHARGED",
        "paymentId": payment_id, "enforceDynamicRoutingFailure": None,
    })
    record("update-gateway-score accepts same paymentId", status == 200, f"status={status}")

    # 3. idempotency-safe second score ---------------------------------------
    status2, _ = call("/update-gateway-score", {
        "merchantId": MERCHANT_ID, "gateway": gateway,
        "gatewayReferenceId": None, "status": "CHARGED",
        "paymentId": payment_id, "enforceDynamicRoutingFailure": None,
    })
    record("repeat score call does not error", status2 == 200, f"status={status2}")

    # 4. malformed request is rejected (fail-open path is reachable) ---------
    status, body = call("/decide-gateway", {"merchantId": MERCHANT_ID})  # missing paymentInfo
    is_error = status >= 400
    record("malformed decide-gateway rejected (>=400)", is_error, f"status={status}")

    # summary ----------------------------------------------------------------
    failed = [n for n, ok, _ in results if not ok]
    print()
    if failed:
        print(f"CERTIFY: FAIL ({len(failed)} failing) -> {', '.join(failed)}")
        sys.exit(1)
    print(f"CERTIFY: PASS ({len(results)} checks)")


if __name__ == "__main__":
    main()
