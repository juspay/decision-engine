#!/usr/bin/env python3
"""Checks the simulator's model of this engine (`policies.SrV3`) against a running engine.

Replays a scenario with the real engine in the `ours` seat, calling /decide-gateway and
/update-gateway-score for every payment, then runs the model on the same payments over many
seeds and compares: the real run's approval rate should sit inside the model's spread, and its
day-by-day processor mix should follow the model's.

For payments another system routed, the engine is called the way a merchant integration would
call it to learn from them: /decide-gateway first (so the engine holds the payment's scoring
context), then /update-gateway-score with the processor that was actually used.

    DECISION_ENGINE_URL=http://localhost:8099 scripts/routing-comparison-sim/validate_engine.py
    ... --scenario late_recovery --scheme full --payments-per-day 300 --days 21 --seeds 3

Each run creates a fresh merchant: sr-routing on, SR v3 with the engine's default window and
hedging, keyed by card network, elimination off. `--mode ours_ee` turns explore-exploit-srv3 on;
`--mode ours` leaves it off, so hedging comes from the deployment's global random-traffic flag
(ENABLE_MERCHANT_ON_VOLUME_DISTRIBUTION_FEATURE_SR_V3), which must be on.
"""

import argparse
import collections
import http.client
import json
import os
import statistics
import sys
import time
import urllib.parse
import uuid

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import policies  # noqa: E402
import simulate  # noqa: E402

BASE = os.environ.get("DECISION_ENGINE_URL", "http://localhost:8099").rstrip("/")
ADMIN_SECRET = os.environ.get("ADMIN_SECRET", "test_admin")
TENANT = os.environ.get("TENANT_ID", "public")

# Processor names in the scenarios -> connector names the engine is given.
CONNECTORS = {"P1": "stripe", "P2": "adyen", "P3": "checkout"}
FROM_CONNECTOR = {v: k for k, v in CONNECTORS.items()}
NETWORKS = {"visa": "VISA", "mastercard": "MASTERCARD"}


_connection = None


def call(method, path, body=None, attempts=5):
    """One request over a kept-alive connection; a long replay makes tens of thousands of
    calls, too many to open a connection for each. Retries on a dropped connection."""
    global _connection
    url = urllib.parse.urlsplit(BASE)
    data = None if body is None else json.dumps(body)
    headers = {
        "Content-Type": "application/json",
        "x-admin-secret": ADMIN_SECRET,
        "x-tenant-id": TENANT,
    }
    for attempt in range(attempts):
        try:
            if _connection is None:
                _connection = http.client.HTTPConnection(url.hostname, url.port or 80, timeout=30)
            _connection.request(method, url.path + path, body=data, headers=headers)
            response = _connection.getresponse()
            raw = response.read().decode()
            try:
                return response.status, (json.loads(raw) if raw else None)
            except json.JSONDecodeError:
                return response.status, raw
        except (OSError, http.client.HTTPException):
            if _connection is not None:
                _connection.close()
            _connection = None
            if attempt == attempts - 1:
                raise
            time.sleep(1 + attempt)


def must(status_body, what):
    status, body = status_body
    if status >= 400:
        raise SystemExit(f"{what} failed ({status}): {body}")
    return body


def create_merchant(bucket_size, hedging_percent, explore_exploit):
    merchant_id = f"sim_validate_{uuid.uuid4().hex[:12]}"
    must(call("POST", "/merchant-account/create",
              {"merchant_id": merchant_id, "gateway_success_rate_based_decider_input": None}),
         "merchant create")
    must(call("POST", "/rule/create", {
        "merchant_id": merchant_id,
        "config": {"type": "successRate", "data": {
            "defaultLatencyThreshold": 90,
            "defaultSuccessRate": 0.5,
            "defaultBucketSize": bucket_size,
            "defaultHedgingPercent": hedging_percent,
            "subLevelInputConfig": [],
        }},
    }), "SR config")
    must(call("POST", "/config-sr-dimension", {
        "merchant_id": merchant_id,
        "paymentInfo": {"udfs": [], "fields": ["card_network"]},
    }), "SR dimension config")
    for feature, enabled in (("sr-routing", True), ("explore-exploit-srv3", explore_exploit)):
        must(call("POST", f"/merchant-account/{merchant_id}/features/{feature}",
                  {"enabled": enabled}), f"feature {feature}")
    return merchant_id


class RealEngine:
    """A policy backed by the running engine; plugs into `simulate.run` like the model."""

    def __init__(self, merchant_id, processors):
        self.merchant_id = merchant_id
        self.eligible = [CONNECTORS[p] for p in processors]
        # Payment ids of this engine's own decisions awaiting their outcome, oldest first.
        self.own_payments = collections.deque()
        self.calls = 0

    def _decide(self, segment):
        payment_id = f"sim_{uuid.uuid4().hex}"
        body = must(call("POST", "/decide-gateway", {
            "merchantId": self.merchant_id,
            "eligibleGatewayList": self.eligible,
            "rankingAlgorithm": "SR_BASED_ROUTING",
            "eliminationEnabled": False,
            "paymentInfo": {
                "paymentId": payment_id,
                "amount": 100.0,
                "currency": "USD",
                "customerId": "sim_customer",
                "udfs": None,
                "preferredConnectors": None,
                "paymentType": "ORDER_PAYMENT",
                "metadata": None,
                "internalMetadata": None,
                "isEmi": False,
                "emiBank": None,
                "emiTenure": None,
                "paymentMethod": "CARD",
                "paymentMethodType": "CREDIT",
                "paymentSource": None,
                "authType": None,
                "cardIssuerBankName": None,
                "cardIsin": None,
                "cardType": "CREDIT",
                "cardSwitchProvider": NETWORKS[segment],
            },
        }), "decide-gateway")
        self.calls += 1
        return payment_id, FROM_CONNECTOR[body["decided_gateway"]]

    def decide(self, segment):
        payment_id, processor = self._decide(segment)
        self.own_payments.append(payment_id)
        return processor

    def observe(self, segment, processor, approved, own):
        if own:
            payment_id = self.own_payments.popleft()
        else:
            # Another system routed this payment: give the engine its context first.
            payment_id, _ = self._decide(segment)
        must(call("POST", "/update-gateway-score", {
            "merchantId": self.merchant_id,
            "gateway": CONNECTORS[processor],
            "gatewayReferenceId": None,
            "status": "CHARGED" if approved else "AUTHORIZATION_FAILED",
            "paymentId": payment_id,
            "enforceDynamicRoutingFailure": None,
            "txnLatency": {"gatewayLatency": 3000},
        }), "update-gateway-score")
        self.calls += 1


def daily_mix(arm, processors):
    return [
        {p: arm.day_picks[d][p] / arm.day_payments[d] for p in processors}
        if arm.day_payments[d] else {p: 0.0 for p in processors}
        for d in range(len(arm.day_payments))
    ]


def main():
    parser = argparse.ArgumentParser(description=__doc__,
                                     formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--scenario", default="late_recovery")
    parser.add_argument("--mode", choices=["ours", "ours_ee"], default="ours",
                        help="Which engine configuration to check; see the module docstring.")
    parser.add_argument("--scheme", default="solo",
                        choices=["solo", *simulate.SCHEMES],
                        help="solo: the engine routes all traffic; otherwise the 50/25/25 test "
                             "against greedy, with the engine learning per the scheme.")
    parser.add_argument("--payments-per-day", type=int, default=300)
    parser.add_argument("--days", type=int, default=21)
    parser.add_argument("--seeds", type=int, default=2, help="Real-engine runs.")
    parser.add_argument("--model-seeds", type=int, default=200)
    parser.add_argument("--feedback-delay", type=int, default=3,
                        help="For the model: the engine caches scores for 75 ms, so a fast "
                             "replay decides on scores a few payments old.")
    args = parser.parse_args()

    status, _ = call("GET", "/health")
    if status != 200:
        raise SystemExit(f"no engine at {BASE} (health returned {status})")

    if args.scheme == "solo":
        systems, shares, receives = {"ours": args.mode}, {"ours": 1.0}, {"ours": ("ours",)}
    else:
        systems = {"merchant": "merchant_rules", "ours": args.mode, "external": "greedy"}
        shares = {"merchant": 0.5, "ours": 0.25, "external": 0.25}
        receives = simulate.SCHEMES[args.scheme]
    processors = simulate.SCENARIOS[args.scenario].processors
    common = dict(payments_per_day=args.payments_per_day, days=args.days)

    model_rates, model_mix = [], []
    for seed in range(args.model_seeds):
        result = simulate.run(args.scenario, systems, shares, receives, seed,
                              feedback_delay=args.feedback_delay, **common)
        model_rates.append(result.arms["ours"].expected_rate())
        model_mix.append(daily_mix(result.arms["ours"], processors))
    model_rates.sort()
    lo = model_rates[int(0.025 * len(model_rates))]
    hi = model_rates[int(0.975 * len(model_rates)) - 1]
    print(f"model ({args.model_seeds} seeds): ours expected SR mean "
          f"{100 * statistics.fmean(model_rates):.2f}%, 95% of seeds in "
          f"[{100 * lo:.2f}%, {100 * hi:.2f}%]")

    real_build = policies.build

    for seed in range(args.seeds):
        merchant_id = create_merchant(policies.DEFAULT_BUCKET_SIZE,
                                      policies.DEFAULT_HEDGING_PERCENT,
                                      explore_exploit=args.mode == "ours_ee")
        engine = None

        def build(name, procs, policy_seed, _real_seed=seed):
            nonlocal engine
            if str(policy_seed).endswith(":ours"):
                engine = RealEngine(merchant_id, procs)
                return engine
            return real_build(name, procs, policy_seed)

        policies.build = build
        started = time.time()
        try:
            result = simulate.run(args.scenario, systems, shares, receives, seed, **common)
        finally:
            policies.build = real_build
        arm = result.arms["ours"]
        rate = arm.expected_rate()
        inside = "inside" if lo <= rate <= hi else "OUTSIDE"
        print(f"\nreal engine, seed {seed} (merchant {merchant_id}, {engine.calls} calls, "
              f"{time.time() - started:.0f}s): expected SR {100 * rate:.2f}% — {inside} the "
              f"model's 95% range")
        real = daily_mix(arm, processors)
        # One run follows one learning path, and a learner's daily mix swings from run to run,
        # so the real day is compared with the band the model's runs span, not their mean.
        print("Share of the day's payments per processor: real, then the model's 10th-90th "
              "percentile across seeds.")
        print("day  " + "  ".join(f"{p:>19}" for p in processors))
        for day, picks in enumerate(real):
            cells = []
            for p in processors:
                spread = sorted(m[day][p] for m in model_mix)
                lo_p = spread[int(0.1 * (len(spread) - 1))]
                hi_p = spread[int(0.9 * (len(spread) - 1))]
                flag = " " if lo_p <= picks[p] <= hi_p else "*"
                cells.append(f"{100 * picks[p]:4.0f}{flag}[{100 * lo_p:3.0f}-{100 * hi_p:3.0f}]")
            print(f"{day:3}  " + "  ".join(f"{c:>19}" for c in cells))
        print("* outside the model's band; expect about one cell in five by chance.")


if __name__ == "__main__":
    main()
