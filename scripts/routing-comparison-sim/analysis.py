"""Turns raw runs into the comparison: how far each sharing scheme's measured gap sits from the
standalone truth, how often its significance test names the right winner, and what the test
costs the merchant.

Standalone truth: a system's expected approval rate when it routes 100% of the traffic alone,
which is what the merchant gets after picking it. It is estimated from runs on the same seeds
as the tests, using expected (not realised) approvals, so its noise is small.
"""

import math
import statistics

Z_95 = 1.959963984540054


def mean_ci(values):
    """Mean and the half-width of its 95% confidence interval."""
    if not values:
        return float("nan"), float("nan")
    m = statistics.fmean(values)
    if len(values) < 2:
        return m, float("nan")
    return m, Z_95 * statistics.stdev(values) / math.sqrt(len(values))


def t_critical_975(df):
    """Two-sided 95% critical value of Student's t (Cornish-Fisher expansion; within 0.1% for
    df >= 5, which a test of at least a week always has)."""
    z = Z_95
    return (
        z
        + (z**3 + z) / (4 * df)
        + (5 * z**5 + 16 * z**3 + 3 * z) / (96 * df**2)
        + (3 * z**7 + 19 * z**5 + 17 * z**3 - 15 * z) / (384 * df**3)
    )


def z_test(a, b):
    """Payment-level two-proportion z-test: +1 if `a` is significantly higher, -1 if `b` is,
    0 for no call. Treats every payment as independent."""
    n1, n2 = a["payments"], b["payments"]
    if not n1 or not n2:
        return 0
    p1, p2 = a["approved"] / n1, b["approved"] / n2
    pooled = (a["approved"] + b["approved"]) / (n1 + n2)
    se = math.sqrt(pooled * (1 - pooled) * (1 / n1 + 1 / n2))
    if se == 0:
        return 0
    z = (p1 - p2) / se
    return 1 if z > Z_95 else -1 if z < -Z_95 else 0


def day_t_test(a, b):
    """Day-level paired t-test on the two arms' daily approval rates: +1, -1 or 0 as `z_test`.

    Payments on the same day share processor incidents and a learner's current beliefs, so
    they are not independent; taking the day as the unit keeps that correlation out of the
    error estimate."""
    diffs = [
        aa / an - ba / bn
        for an, aa, bn, ba in zip(
            a["day_payments"], a["day_approved"], b["day_payments"], b["day_approved"]
        )
        if an and bn
    ]
    if len(diffs) < 3:
        return 0
    sd = statistics.stdev(diffs)
    if sd == 0:
        return 0
    t = statistics.fmean(diffs) / (sd / math.sqrt(len(diffs)))
    critical = t_critical_975(len(diffs) - 1)
    return 1 if t > critical else -1 if t < -critical else 0


TESTS = {"day": day_t_test, "payment": z_test}


def expected_rate(arm):
    return arm["expected"] / arm["payments"] if arm["payments"] else float("nan")


def realised_rate(arm):
    return arm["approved"] / arm["payments"] if arm["payments"] else float("nan")


def winner(gap_pp, ci_pp, margin_pp):
    """'ours', 'external' or 'tie': a side wins when it is ahead by at least the margin and
    the interval excludes zero."""
    if abs(gap_pp) < margin_pp or abs(gap_pp) <= ci_pp:
        return "tie"
    return "ours" if gap_pp > 0 else "external"


def summarise(scenario_runs, externals, schemes, test, margin_pp):
    """scenario_runs: the per-seed results of one scenario (see run.py `simulate_seed`)."""
    seeds = len(scenario_runs)
    pp = 100.0
    solo = lambda run, name: expected_rate(run["solo"][name]["arms"]["solo"])  # noqa: E731

    merchant_solo = [solo(r, "merchant") for r in scenario_runs]
    ours_solo = [solo(r, "ours") for r in scenario_runs]
    rows = []
    for external in externals:
        ext_solo = [solo(r, external) for r in scenario_runs]
        truth_seed = [(o - e) * pp for o, e in zip(ours_solo, ext_solo)]
        truth_gap, truth_ci = mean_ci(truth_seed)
        truth_vs_merchant, _ = mean_ci([(o - m) * pp for o, m in zip(ours_solo, merchant_solo)])
        truth = winner(truth_gap, truth_ci, margin_pp)

        for scheme in schemes:
            measured, errors, calls = [], [], {1: 0, -1: 0, 0: 0}
            overall, cost, incident_failures = [], [], []
            ours_rate, ext_rate = [], []
            for r, truth_s, merchant_s in zip(scenario_runs, truth_seed, merchant_solo):
                result = r["tests"][external][scheme]
                arms = result["arms"]
                gap = (expected_rate(arms["ours"]) - expected_rate(arms["external"])) * pp
                measured.append(gap)
                errors.append(gap - truth_s)
                calls[TESTS[test](arms["ours"], arms["external"])] += 1
                ours_rate.append(realised_rate(arms["ours"]))
                ext_rate.append(realised_rate(arms["external"]))
                payments = sum(a["payments"] for a in arms.values())
                approved = sum(a["approved"] for a in arms.values())
                expected = sum(a["expected"] for a in arms.values())
                overall.append(approved / payments)
                cost.append((expected / payments - merchant_s) * pp)
                incident_failures.append(result["incident_failures"])

            bias, bias_ci = mean_ci(errors)
            wrong_side = {"ours": -1, "external": 1}.get(truth)
            rows.append(
                {
                    "external": external,
                    "scheme": scheme,
                    "truth": truth,
                    "standalone_gap_pp": truth_gap,
                    "standalone_gap_ci_pp": truth_ci,
                    "standalone_ours_vs_merchant_pp": truth_vs_merchant,
                    "ours_sr": statistics.fmean(ours_rate),
                    "external_sr": statistics.fmean(ext_rate),
                    "measured_gap_pp": statistics.fmean(measured),
                    "bias_pp": bias,
                    "bias_ci_pp": bias_ci,
                    "called_ours": calls[1] / seeds,
                    "called_external": calls[-1] / seeds,
                    "no_call": calls[0] / seeds,
                    # Naming the system that is truly worse; when the truth is a tie, any call.
                    "wrong_call": (
                        (calls[1] + calls[-1]) / seeds
                        if truth == "tie"
                        else calls[wrong_side] / seeds
                    ),
                    "right_call": 0.0 if truth == "tie" else calls[-wrong_side] / seeds,
                    "merchant_overall_sr": statistics.fmean(overall),
                    "cost_vs_merchant_alone_pp": statistics.fmean(cost),
                    "incident_failures": statistics.fmean(incident_failures),
                }
            )
    return {
        "standalone": {
            "merchant": statistics.fmean(merchant_solo),
            "ours": statistics.fmean(ours_solo),
            **{
                e: statistics.fmean(solo(r, e) for r in scenario_runs)
                for e in externals
            },
        },
        "rows": rows,
    }


def calibration(scenario_runs, scheme, external="ours_clone"):
    """False-call rate of each test in an A/A run, where both seats run the same algorithm and
    any call is wrong. A well-calibrated 95% test calls about 5% of the time."""
    out = {}
    for name, test in TESTS.items():
        calls = 0
        for r in scenario_runs:
            arms = r["tests"][external][scheme]["arms"]
            calls += test(arms["ours"], arms["external"]) != 0
        out[name] = calls / len(scenario_runs)
    return out


def scheme_scorecard(summaries, schemes, exclude=("ours_clone",)):
    """One line per scheme across every scenario and external stand-in: how far off the
    measured gap is, how often the test names the wrong system, and what the test costs the
    merchant."""
    card = {}
    for scheme in schemes:
        rows = [
            row
            for summary in summaries
            for row in summary["rows"]
            if row["scheme"] == scheme and row["external"] not in exclude
        ]
        decided = [r for r in rows if r["truth"] != "tie"]
        card[scheme] = {
            "cases": len(rows),
            "mean_abs_bias_pp": statistics.fmean(abs(r["bias_pp"]) for r in rows),
            "worst_bias_pp": max((r["bias_pp"] for r in rows), key=abs),
            "wrong_call": statistics.fmean(r["wrong_call"] for r in rows),
            "right_call_when_real_gap": (
                statistics.fmean(r["right_call"] for r in decided) if decided else float("nan")
            ),
            "cost_vs_merchant_alone_pp": statistics.fmean(
                r["cost_vs_merchant_alone_pp"] for r in rows
            ),
            "incident_failures": statistics.fmean(r["incident_failures"] for r in rows),
        }
    return card
