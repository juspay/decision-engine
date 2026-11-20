#!/usr/bin/env python3
"""
Greedy SR vs Autopilot: a reproducible A/B demo on Decision Engine.

WHAT IT SHOWS
  A success-rate (SR) router with 0% hedging is "greedy": it always sends a payment to the
  connector with the best score. When a connector has an outage, greedy moves away from it, and
  because it never sends that connector another payment, it never notices when the connector
  recovers. Its score for that connector stays frozen at its outage level.

  Autopilot keeps a small, self-tuned share of traffic exploring the other connectors (hedging).
  That costs a little in normal times, but it sees the recovery and moves back.

THE EXPERIMENT (create and activate it on the A/B Testing page before running this)
  Control    volume split, 50/50 across the two connectors      (baseline, never adapts)
  Variant A  SR, manual tuning, 0% hedging                       (greedy)
  Variant B  SR, auto-tuned (Autopilot)                          (explores)
  SR feedback: "Control + own". Endpoint: Decide.
  Merchant features: SR routing and Autopilot on. SR config default hedging: 0%.
  Use a freshly activated experiment for every run: each variant keeps its own scores, and a
  rerun on the same experiment would start from where the last run left them.

THE SCENARIO (connector success rates the simulated outcomes are drawn from)
  1. Normal          stripe 95%, adyen 90%   everyone learns stripe is best
  2. Stripe outage   stripe 40%, adyen 90%   SR arms move to adyen
  3. Stripe recovers stripe 98%, adyen 93%   greedy stays stuck on adyen, Autopilot moves back

  Why adyen moves from 90% to 93% in phase 3: greedy stops sending payments to stripe the moment
  stripe's score falls below adyen's, so its stripe score freezes at about adyen's level then
  (~90%). While adyen stays above that frozen value, greedy never sends stripe another payment
  and cannot see stripe's recovery. With adyen at exactly 90%, ordinary ups and downs in adyen's
  score sometimes dip below the frozen value and greedy stumbles back onto stripe by luck; the
  small move to 93% removes that luck so the run is the same every time.

HOW IT WORKS
  For every payment the script calls POST /decide, uses the first connector it returns, draws the
  payment's outcome from that connector's success rate for the current phase, and reports it with
  POST /update-gateway-score, exactly as a merchant integration would.

  Outcomes are drawn evenly rather than at random (a connector at 90% fails exactly once in every
  ten payments it gets), so two runs give the same picture and differences between arms are not
  noise.

  Decision Engine assigns each payment to an arm by hashing its payment id. The response does not
  name the arm, so the script repeats the same hash to report results per arm as it goes. The
  A/B Testing page shows the same experiment's official results.

RUN
  python3 greedy_vs_autopilot_demo.py --base-url https://<host> --api-key <key>
  (locally: --base-url http://localhost:8080 --admin-secret <secret> --merchant-id <id>)

  Every payment is written to a CSV (see --csv) so the run can be analysed afterwards.
"""

import argparse
import csv
import json
import sys
import threading
import time
import urllib.error
import urllib.request
from concurrent.futures import ThreadPoolExecutor

# ── Scenario ──────────────────────────────────────────────────────────────────────────────────
CONNECTORS = ["stripe", "adyen"]

PHASES = [
    {
        "name": "1. Normal",
        "payments": 1500,
        "success_rates": {"stripe": 95, "adyen": 90},
        "expect": "Both SR arms send nearly everything to stripe (95%). Control splits 50/50.",
    },
    {
        "name": "2. Stripe outage",
        "payments": 1500,
        "success_rates": {"stripe": 40, "adyen": 90},
        "expect": "Both SR arms move to adyen (90%). Control keeps sending half to stripe.",
    },
    {
        # Autopilot only reaches stripe through exploration: its hedging % (up to 20-30%), half of
        # it to stripe, on its 20% traffic share, about 2% of all payments. Its window grows with
        # traffic (up to a few hundred outcomes), so refreshing it takes on the order of 10,000
        # payments.
        "name": "3. Stripe recovers",
        "payments": 15000,
        "success_rates": {"stripe": 98, "adyen": 93},
        "expect": "Greedy (Variant A) stays on adyen (93%): its stripe score is frozen at its outage "
                  "level and it never tries stripe again. Autopilot (Variant B) explores, sees stripe "
                  "is back at 98% and moves to it.",
    },
]

# The payment every request describes. One segment, so all arms learn about the same payments.
PAYMENT = {
    "amount": 100,
    "currency": "USD",
    "payment_method_type": "CARD",
    "payment_method_sub_type": "DEBIT",
    "authentication_type": "THREE_DS",
    "card": {"card_network": "VISA", "card_type": "DEBIT"},
}

# Stop the run when this many requests in a row fail: Decision Engine is unreachable.
MAX_CONSECUTIVE_FAILURES = 20

ARM_LABELS = {
    "control": "Control (volume split)",
    "variant_a": "Variant A (greedy, 0% hedging)",
    "variant_b": "Variant B (Autopilot)",
}


# ── API ───────────────────────────────────────────────────────────────────────────────────────
class Api:
    def __init__(self, base_url, headers, merchant_id):
        self.base_url = base_url.rstrip("/")
        self.headers = {"content-type": "application/json", **headers}
        self.merchant_id = merchant_id

    def call(self, method, path, body=None):
        data = json.dumps(body).encode() if body is not None else None
        req = urllib.request.Request(self.base_url + path, data=data, method=method, headers=self.headers)
        try:
            with urllib.request.urlopen(req, timeout=30) as resp:
                raw = resp.read()
        except urllib.error.HTTPError as err:
            raise RuntimeError(f"{method} {path} -> {err.code}: {err.read().decode(errors='replace')[:300]}")
        try:
            return json.loads(raw) if raw else None
        except json.JSONDecodeError:
            return raw.decode(errors="replace")

    def active_experiment(self):
        """The merchant's running A/B experiment, or None."""
        for algo in self.call("GET", f"/routing/list/active/{self.merchant_id}") or []:
            data = algo.get("algorithm_data") or algo.get("algorithm") or {}
            if data.get("type") == "ab_test":
                return algo["id"], algo.get("name", ""), data["data"]
        return None

    def decide(self, payment_id):
        body = {
            "payment_id": payment_id,
            **PAYMENT,
            "routing_config": {
                "algorithm_for": "payment",
                "ranking_algorithm": "SR_BASED_ROUTING",
                "elimination_enabled": False,
                "eligible_connectors": [{"connector": c} for c in CONNECTORS],
            },
        }
        return self.call("POST", "/decide", body)

    def report_outcome(self, payment_id, connector, success):
        self.call("POST", "/update-gateway-score", {
            "merchantId": self.merchant_id,
            "gateway": connector,
            "gatewayReferenceId": None,
            "status": "CHARGED" if success else "FAILURE",
            "paymentId": payment_id,
            "enforceDynamicRoutingFailure": None,
        })


# ── Arm assignment (mirrors Decision Engine) ──────────────────────────────────────────────────
def arm_of(payment_id, variants):
    """The arm Decision Engine serves this payment on: a djb2 hash of the payment id, mod 100.
    Variants own consecutive slices in order (the first [0, split1), the next after it), control
    the rest."""
    h = 5381
    for byte in payment_id.encode():
        h = (h * 33 + byte) & 0xFFFFFFFFFFFFFFFF
    bucket, upper = h % 100, 0
    for variant in variants:
        upper += variant["split_pct"]
        if bucket < upper:
            return variant["id"]
    return "control"


# ── Outcomes ──────────────────────────────────────────────────────────────────────────────────
class Outcomes:
    """Evenly spaced outcomes per connector: at 90%, exactly 9 of every 10 payments succeed."""

    def __init__(self):
        self.credit = {c: 0.5 for c in CONNECTORS}
        self.lock = threading.Lock()

    def draw(self, connector, success_rate_pct):
        with self.lock:
            self.credit[connector] += success_rate_pct / 100
            if self.credit[connector] >= 1:
                self.credit[connector] -= 1
                return True
            return False


# ── Run ───────────────────────────────────────────────────────────────────────────────────────
def run_payment(api, outcomes, run_id, phase_index, i, rates, variants):
    payment_id = f"demo_{run_id}_p{phase_index + 1}_{i}"
    decision = api.decide(payment_id)
    connector = decision["connectors"][0]["connector"]
    success = outcomes.draw(connector, rates[connector])
    api.report_outcome(payment_id, connector, success)
    sr = decision.get("success_rate") or {}
    return {
        "payment_id": payment_id,
        "phase": phase_index + 1,
        "arm": arm_of(payment_id, variants),
        "connector": connector,
        "success": success,
        "explored": "hedging" in (sr.get("approach") or "").lower(),
        "strategy": (decision.get("outcome") or {}).get("strategy"),
    }


def summarize(rows):
    """Per-arm success rate, share sent to stripe and exploration picks."""
    out = {}
    for arm in ARM_LABELS:
        mine = [r for r in rows if r["arm"] == arm]
        if not mine:
            continue
        n = len(mine)
        out[arm] = {
            "payments": n,
            "sr": 100 * sum(r["success"] for r in mine) / n,
            "to_stripe": 100 * sum(r["connector"] == "stripe" for r in mine) / n,
            "explored": sum(r["explored"] for r in mine),
        }
    return out


def print_table(title, stats):
    print(f"\n  {title}")
    print(f"  {'Arm':<34}{'Payments':>9}{'Success':>10}{'To stripe':>11}{'Explored':>10}")
    for arm, s in stats.items():
        print(f"  {ARM_LABELS[arm]:<34}{s['payments']:>9}{s['sr']:>9.1f}%{s['to_stripe']:>10.0f}%{s['explored']:>10}")


def print_progress(rows, block=2500):
    """Share of each SR arm's payments sent to stripe, block by block: shows when an arm switched."""
    print(f"\n  Share sent to stripe, per {block} payments of the phase")
    print(f"  {'Payments':<14}" + "".join(f"{ARM_LABELS[a]:>34}" for a in ("variant_a", "variant_b")))
    for start in range(0, len(rows), block):
        chunk = rows[start:start + block]
        cells = []
        for arm in ("variant_a", "variant_b"):
            mine = [r for r in chunk if r["arm"] == arm]
            share = 100 * sum(r["connector"] == "stripe" for r in mine) / len(mine) if mine else 0
            cells.append(f"{share:>33.0f}%")
        print(f"  {start + 1:>5}-{start + len(chunk):<8}" + "".join(cells))


def main():
    parser = argparse.ArgumentParser(description="Greedy SR vs Autopilot A/B demo")
    parser.add_argument("--base-url", required=True, help="Decision Engine base URL")
    parser.add_argument("--api-key", help="merchant API key (sent as x-api-key)")
    parser.add_argument("--admin-secret", help="admin secret, for local runs instead of an API key")
    parser.add_argument("--merchant-id", required=True, help="merchant id the experiment belongs to")
    parser.add_argument("--workers", type=int, default=4, help="payments in flight at once (default 4)")
    parser.add_argument("--scale", type=float, default=1.0, help="multiply every phase's payment count")
    parser.add_argument("--csv", default=None, help="where to write every payment (default demo_<run>.csv)")
    args = parser.parse_args()

    if not (args.api_key or args.admin_secret):
        sys.exit("Pass --api-key (or --admin-secret for a local run).")
    headers = {"x-connected-merchant-id": args.merchant_id}
    if args.api_key:
        headers["x-api-key"] = args.api_key
    else:
        headers["x-admin-secret"] = args.admin_secret
    api = Api(args.base_url, headers, args.merchant_id)

    experiment = api.active_experiment()
    if not experiment:
        sys.exit("No active A/B experiment for this merchant. Create and activate it first (see the top of this file).")
    experiment_id, experiment_name, data = experiment
    variants = data.get("variants") or []
    print(f"Experiment: {experiment_name} ({experiment_id})")
    print(f"  SR feedback: {data.get('feedback_sharing', 'shared')}")
    print(f"  Control {100 - sum(v['split_pct'] for v in variants)}%  " +
          "  ".join(f"{v.get('name', v['id'])} {v['split_pct']}%" for v in variants))

    # Each variant's scores are tagged with the experiment id; existing ones mean an earlier run.
    try:
        tag = f"ab_{experiment_id.removeprefix('routing_')}_"
        scores = api.call("GET", f"/gateway-score/{args.merchant_id}") or {}
        if any(tag in w.get("series", "") for w in scores.get("windows", [])):
            print("  Warning: this experiment already has scores from an earlier run. For a clean run, "
                  "duplicate and activate the experiment, then run again.")
    except RuntimeError:
        pass  # the scores endpoint is optional; the run doesn't depend on it

    run_id = str(int(time.time()))
    csv_path = args.csv or f"demo_{run_id}.csv"
    outcomes = Outcomes()
    all_rows = []

    for index, phase in enumerate(PHASES):
        n = max(1, int(phase["payments"] * args.scale))
        rates = phase["success_rates"]
        print(f"\n━━ Phase {phase['name']}  ({n} payments)  " +
              "  ".join(f"{c} {rates[c]}%" for c in CONNECTORS))
        print(f"   Expect: {phase['expect']}")

        started = time.time()
        rows, errors, in_a_row = [], 0, 0
        with ThreadPoolExecutor(args.workers) as pool:
            futures = [pool.submit(run_payment, api, outcomes, run_id, index, i, rates, variants) for i in range(n)]
            for done, future in enumerate(futures, 1):
                try:
                    rows.append(future.result())
                    in_a_row = 0
                except Exception as err:  # a few failed calls don't change the picture
                    errors += 1
                    in_a_row += 1
                    if errors <= 3:
                        print(f"   request failed: {err}")
                    if in_a_row >= MAX_CONSECUTIVE_FAILURES:
                        for pending in futures:
                            pending.cancel()
                        sys.exit(f"\n{in_a_row} requests in a row failed: Decision Engine at {args.base_url} "
                                 "is not answering. Stopping; start a fresh experiment before running again.")
                if done % 2500 == 0:
                    print(f"   {done}/{n} payments")
        all_rows.extend(rows)

        print(f"   Done in {time.time() - started:.0f}s" + (f", {errors} failed requests" if errors else ""))
        print_table("Whole phase", summarize(rows))
        tail = rows[-max(1, len(rows) // 3):]
        print_table(f"Last {len(tail)} payments of the phase (where each arm settled)", summarize(tail))
        if index == len(PHASES) - 1:
            print_progress(rows)

    with open(csv_path, "w", newline="") as f:
        writer = csv.DictWriter(f, fieldnames=list(all_rows[0].keys()))
        writer.writeheader()
        writer.writerows(all_rows)

    last = [r for r in all_rows if r["phase"] == len(PHASES)]
    settled = summarize(last[-max(1, len(last) // 3):])
    a, b = settled.get("variant_a"), settled.get("variant_b")
    print("\n━━ Result")
    if a and b:
        print(f"   After stripe recovered, greedy (Variant A) sent {a['to_stripe']:.0f}% of its payments to stripe "
              f"and succeeded {a['sr']:.1f}% of the time.")
        print(f"   Autopilot (Variant B) sent {b['to_stripe']:.0f}% to stripe and succeeded {b['sr']:.1f}% "
              f"({b['sr'] - a['sr']:+.1f} points).")
    print(f"   Every payment is in {csv_path}. Official per-arm results: A/B Testing page, experiment "
          f"'{experiment_name}'.")


if __name__ == "__main__":
    main()
