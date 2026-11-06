#!/usr/bin/env python3
"""Turn the results directory of ci_benchmark.sh into a Markdown report on stdout.

    python3 scripts/load-test/summarize_ci_benchmark.py <results dir> [environment note]

Latency is the round-trip of the routing call alone (k6 `decide_gateway_latency`, which excludes
the feedback call that follows it). Throughput counts routing calls per second; SR, hybrid and
decide iterations each also send one /update-gateway-score call, so the server handles twice
that many HTTP requests.
"""

import json
import statistics
import sys
from pathlib import Path

ENDPOINT = {
    "DECIDE": "/decide",
    "HYBRID_ROUTING": "/routing/hybrid",
    "SR_BASED_ROUTING": "/decide-gateway",
    "RULE_BASED_ROUTING": "/routing/evaluate",
}
SENDS_FEEDBACK = {"DECIDE", "HYBRID_ROUTING", "SR_BASED_ROUTING"}


def seconds(duration: str) -> float:
    if duration.endswith("ms"):
        return float(duration[:-2]) / 1000
    unit = {"s": 1, "m": 60, "h": 3600}[duration[-1]]
    return float(duration[:-1]) * unit


def load(path: Path) -> dict:
    metrics = json.loads(path.read_text())["metrics"]

    def value(name, key, default=0.0):
        return metrics.get(name, {}).get("values", {}).get(key, default)

    return {
        "count": value("decide_reqs", "count"),
        "p50": value("decide_gateway_latency", "med"),
        "p95": value("decide_gateway_latency", "p(95)"),
        "p99": value("decide_gateway_latency", "p(99)"),
        "max": value("decide_gateway_latency", "max"),
        "checks": value("checks", "rate", 0.0),
        # Routing and feedback calls only: http_req_failed also counts the setup requests.
        "errors": value("decide_gateway_errors", "count") + value("feedback_errors", "count"),
        "dropped": value("dropped_iterations", "count"),
    }


def results(directory: Path) -> dict:
    """{algorithm: [run, ...]} for every load_test_results_<ALGORITHM>.json under directory."""
    found = {}
    for path in sorted(directory.glob("*/load_test_results_*.json")):
        algo = path.stem.removeprefix("load_test_results_")
        found.setdefault(algo, []).append((path.parent.name, load(path)))
    return found


def ms(x: float) -> str:
    return f"{x:.2f}"


def main() -> None:
    out = Path(sys.argv[1])
    note = sys.argv[2] if len(sys.argv) > 2 else ""
    params = json.loads((out / "params.json").read_text())
    order = [a for a in params["endpoints"].split(",") if a in ENDPOINT]

    print("# Decision engine benchmark\n")
    if note:
        print(f"{note}\n")

    latency = results(out / "latency")
    print(
        f"## Latency at {params['lat_rate']} req/s\n\n"
        f"{params['rounds']} rounds of {params['lat_duration']} per endpoint, endpoints alternating. "
        "Round-trip of the routing call in ms; p50/p95/p99 are medians across rounds.\n"
    )
    print("| Endpoint | p50 | p95 | p99 (median) | p99 range | max (median) | checks passed |")
    print("|---|--:|--:|--:|--:|--:|--:|")
    for algo in order:
        runs = [r for _, r in latency.get(algo, [])]
        if not runs:
            continue
        med = lambda k: statistics.median(r[k] for r in runs)
        p99s = [r["p99"] for r in runs]
        print(
            f"| `{ENDPOINT[algo]}` | {ms(med('p50'))} | {ms(med('p95'))} | **{ms(med('p99'))}** "
            f"| {ms(min(p99s))}–{ms(max(p99s))} | {ms(med('max'))} "
            f"| {min(r['checks'] for r in runs) * 100:.2f}% |"
        )

    throughput = results(out / "throughput")
    duration = seconds(params["tps_duration"])
    print(
        f"\n## Throughput\n\n"
        f"Each step holds a fixed arrival rate for {params['tps_duration']}. A step is sustained "
        "when k6 dropped no iterations, every check passed and no routing or feedback call errored.\n"
    )
    print("| Endpoint | Sustained TPS | Server HTTP req/s | p99 at that rate (ms) | First failing step |")
    print("|---|--:|--:|--:|---|")
    detail = []
    for algo in order:
        steps = sorted(
            ((int(d.removeprefix("rate_")), r) for d, r in throughput.get(algo, [])),
            key=lambda s: s[0],
        )
        best = None
        first_fail = "—"
        for rate, r in steps:
            achieved = r["count"] / duration
            ok = r["dropped"] == 0 and r["errors"] == 0 and r["checks"] >= 1.0
            detail.append(
                f"| `{ENDPOINT[algo]}` | {rate} | {achieved:.0f} | {ms(r['p50'])} | {ms(r['p99'])} "
                f"| {int(r['dropped'])} | {int(r['errors'])} | {r['checks'] * 100:.2f}% |"
            )
            if ok and first_fail == "—":
                best = (achieved, r)
            elif not ok and first_fail == "—":
                first_fail = f"{rate} req/s"
        if best is None:
            print(f"| `{ENDPOINT[algo]}` | none sustained | — | — | {first_fail} |")
            continue
        achieved, r = best
        http_rate = achieved * (2 if algo in SENDS_FEEDBACK else 1)
        # Nothing failed: the ceiling lies above the highest rate tried.
        floor = "≥ " if first_fail == "—" else ""
        print(
            f"| `{ENDPOINT[algo]}` | **{floor}{achieved:.0f}** | {floor}{http_rate:.0f} "
            f"| {ms(r['p99'])} | {first_fail} |"
        )
    print(
        "\nIf no step failed, the ceiling is above the highest rate tried: raise `tps_rates`.\n"
    )
    print("<details><summary>Every throughput step</summary>\n")
    print("| Endpoint | Target req/s | Achieved | p50 | p99 | Dropped | Errors | Checks |")
    print("|---|--:|--:|--:|--:|--:|--:|--:|")
    print("\n".join(detail))
    print("\n</details>")


if __name__ == "__main__":
    main()
