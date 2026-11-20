#!/usr/bin/env python3
"""Simulates a three-way routing comparison (the merchant's own system, this engine and an
external engine) under each feedback-sharing scheme, and checks which scheme's result matches
how each engine performs on its own.

    scripts/routing-comparison-sim/run.py
    scripts/routing-comparison-sim/run.py --scenarios late_recovery,hard_outage --seeds 200
    scripts/routing-comparison-sim/run.py --split 34,33,33 --payments-per-day 500 --days 28

See README.md in this directory for the method and how to read the output.
"""

import argparse
import json
import multiprocessing
import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import analysis  # noqa: E402
import policies  # noqa: E402
import simulate  # noqa: E402
from scenarios import SCENARIOS  # noqa: E402


def compact(result, arm_names=None):
    return {
        "arms": {
            (arm_names or {}).get(arm, arm): {
                "payments": s.payments,
                "approved": s.approved,
                "expected": s.expected,
                "day_payments": s.day_payments,
                "day_approved": s.day_approved,
            }
            for arm, s in result.arms.items()
        },
        "incident_failures": result.incident_failures,
        "regret": result.regret,
    }


def simulate_seed(job):
    """Every run for one (scenario, seed): each system alone, then each external stand-in under
    each scheme. One seed's runs share a payment stream, so they are paired."""
    scenario, seed, cfg = job
    common = dict(seed=seed, payments_per_day=cfg["payments_per_day"], days=cfg["days"],
                  feedback_delay=cfg["feedback_delay"])
    policy_of = lambda label: cfg["ours"] if label == "ours_clone" else label  # noqa: E731
    solo = {}
    for name, policy in [("merchant", cfg["merchant"]), ("ours", cfg["ours"])] + [
        (e, policy_of(e)) for e in cfg["externals"]
    ]:
        result = simulate.run(
            scenario, {"solo": policy}, {"solo": 1.0}, {"solo": ("solo",)}, **common
        )
        solo[name] = compact(result)
    tests = {}
    for external in cfg["externals"]:
        systems = {"merchant": cfg["merchant"], "ours": cfg["ours"], "external": policy_of(external)}
        tests[external] = {
            scheme: compact(
                simulate.run(scenario, systems, cfg["shares"], simulate.SCHEMES[scheme], **common)
            )
            for scheme in cfg["schemes"]
        }
    return scenario, seed, {"solo": solo, "tests": tests}


def pct(x):
    return "n/a" if x != x else f"{100 * x:.2f}%"


def pp(x, ci=None):
    s = f"{x:+.2f}"
    return s if ci is None else f"{s} ±{ci:.2f}"


def print_scenario(name, summary, cfg, file=sys.stdout):
    scenario = SCENARIOS[name]
    print(f"\n## {name}\n\n{scenario.description}\n", file=file)
    alone = ", ".join(f"{k} {pct(v)}" for k, v in summary["standalone"].items())
    print(f"Standalone SR (each system alone on 100% of traffic): {alone}\n", file=file)
    print(
        "| external | scheme | truth (alone, ours − ext) | measured gap | bias | ours called "
        "| ext called | no call | wrong call | merchant SR | cost vs merchant alone "
        "| incident failures |",
        file=file,
    )
    print("|" + "---|" * 12, file=file)
    for row in summary["rows"]:
        truth = f"{pp(row['standalone_gap_pp'], row['standalone_gap_ci_pp'])} ({row['truth']})"
        print(
            f"| {row['external']} | {row['scheme']} | {truth} | {pp(row['measured_gap_pp'])} "
            f"| {pp(row['bias_pp'], row['bias_ci_pp'])} | {pct(row['called_ours'])} "
            f"| {pct(row['called_external'])} | {pct(row['no_call'])} "
            f"| {pct(row['wrong_call'])} | {pct(row['merchant_overall_sr'])} "
            f"| {pp(row['cost_vs_merchant_alone_pp'])} | {row['incident_failures']:.0f} |",
            file=file,
        )


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--scenarios", default=",".join(SCENARIOS))
    parser.add_argument("--externals", default="ours_clone,greedy,heavy_explorer,thompson",
                        help="Stand-ins for the external engine. ours_clone is a copy of --ours: "
                             "the A/A check.")
    parser.add_argument("--ours", choices=["ours", "ours_ee"], default="ours",
                        help="This engine's mode: ours (random-traffic hedging, learns from "
                             "every outcome) or ours_ee (explore-exploit-srv3 on).")
    parser.add_argument("--merchant", default="merchant_rules")
    parser.add_argument("--schemes", default=",".join(simulate.SCHEMES))
    parser.add_argument("--split", default="50,25,25",
                        help="Traffic percent for merchant,ours,external.")
    parser.add_argument("--payments-per-day", type=int, default=2000)
    parser.add_argument("--days", type=int, default=21)
    parser.add_argument("--seeds", type=int, default=100)
    parser.add_argument("--feedback-delay", type=int, default=0,
                        help="Payments that pass before an outcome reaches the systems.")
    parser.add_argument("--test", choices=sorted(analysis.TESTS), default="day",
                        help="Significance test that makes each run's call.")
    parser.add_argument("--margin-pp", type=float, default=0.25,
                        help="A standalone gap smaller than this counts as a tie.")
    parser.add_argument("--workers", type=int, default=os.cpu_count())
    parser.add_argument("--json", help="Write every summary row here.")
    parser.add_argument("--list", action="store_true", help="List scenarios and policies.")
    args = parser.parse_args()

    if args.list:
        print("Scenarios:")
        for s in SCENARIOS.values():
            print(f"  {s.name:16} {s.description}")
        print("Policies:")
        for name, (description, _) in policies.POLICIES.items():
            print(f"  {name:16} {description}")
        return

    split = [float(x) for x in args.split.split(",")]
    if len(split) != 3 or abs(sum(split) - 100) > 1e-6:
        parser.error("--split takes three percentages that sum to 100")
    cfg = {
        "ours": args.ours,
        "externals": args.externals.split(","),
        "merchant": args.merchant,
        "schemes": args.schemes.split(","),
        "shares": dict(zip(simulate.ARMS, (x / 100 for x in split))),
        "payments_per_day": args.payments_per_day,
        "days": args.days,
        "feedback_delay": args.feedback_delay,
    }
    for name in [args.merchant, *cfg["externals"]]:
        if name != "ours_clone" and name not in policies.POLICIES:
            parser.error(f"unknown policy {name!r}")
    scenarios = args.scenarios.split(",")
    for name in scenarios:
        if name not in SCENARIOS:
            parser.error(f"unknown scenario {name!r}")
    for scheme in cfg["schemes"]:
        if scheme not in simulate.SCHEMES:
            parser.error(f"unknown scheme {scheme!r}")

    jobs = [(s, seed, cfg) for s in scenarios for seed in range(args.seeds)]
    started = time.time()
    runs = {s: [None] * args.seeds for s in scenarios}
    with multiprocessing.Pool(args.workers) as pool:
        for done, (scenario, seed, result) in enumerate(
            pool.imap_unordered(simulate_seed, jobs), start=1
        ):
            runs[scenario][seed] = result
            if done % max(1, len(jobs) // 20) == 0:
                print(f"  {done}/{len(jobs)} seeds ({time.time() - started:.0f}s)", file=sys.stderr)

    print("# Routing comparison simulation\n")
    print(
        f"{args.seeds} seeds per scenario, {args.payments_per_day} payments/day for "
        f"{args.days} days, split merchant/ours/external = {args.split}, this engine "
        f"`{args.ours}`, merchant system "
        f"`{args.merchant}`, feedback delay {args.feedback_delay} payments, `{args.test}` test, "
        f"tie margin {args.margin_pp} pp."
    )
    print(
        "\nbias = measured gap − standalone gap (ours − external, in pp, expected approvals, "
        "±95% CI over seeds). Calls come from the significance test on realised approvals."
    )

    out = {"config": {**cfg, "seeds": args.seeds, "test": args.test}, "scenarios": {}}
    if "ours_clone" in cfg["externals"]:
        print("\n## A/A calibration (both seats run the same algorithm; any call is false)\n")
        print("| scenario | scheme | day-level test | payment-level test |")
        print("|---|---|---|---|")
        for s in scenarios:
            for scheme in cfg["schemes"]:
                c = analysis.calibration(runs[s], scheme)
                print(f"| {s} | {scheme} | {pct(c['day'])} | {pct(c['payment'])} |")
                out.setdefault("calibration", {}).setdefault(s, {})[scheme] = c

    for s in scenarios:
        out["scenarios"][s] = analysis.summarise(
            runs[s], cfg["externals"], cfg["schemes"], args.test, args.margin_pp
        )

    card = analysis.scheme_scorecard(out["scenarios"].values(), cfg["schemes"])
    out["scorecard"] = card
    print("\n## Scorecard per scheme (all scenarios and external stand-ins except the A/A clone)\n")
    print("| scheme | mean abs bias | worst bias | wrong call | right call when a real gap "
          "exists | cost vs merchant alone | incident failures per test |")
    print("|---|---|---|---|---|---|---|")
    for scheme, c in card.items():
        print(f"| {scheme} | {c['mean_abs_bias_pp']:.2f} pp | {pp(c['worst_bias_pp'])} pp "
              f"| {pct(c['wrong_call'])} | {pct(c['right_call_when_real_gap'])} "
              f"| {pp(c['cost_vs_merchant_alone_pp'])} pp | {c['incident_failures']:.0f} |")

    for s in scenarios:
        print_scenario(s, out["scenarios"][s], cfg)

    if args.json:
        with open(args.json, "w") as f:
            json.dump(out, f, indent=2)
    print(f"\n_{time.time() - started:.0f}s_", file=sys.stderr)


if __name__ == "__main__":
    main()
