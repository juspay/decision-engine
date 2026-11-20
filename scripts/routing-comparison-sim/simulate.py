"""One simulated test: payments are split across systems, each system routes its share, and
outcomes are fed back according to a sharing scheme.

Common random numbers: for a given seed, every run sees the same payments (segment, arm
assignment) and the same outcome draws. Payment i sent to processor g is approved when
u[i][g] < rate(g, segment, day). Two runs with the same seed therefore differ only in the routing
choices, which is what makes paired comparisons between schemes and standalone runs fair.
"""

import collections
import functools
import random
from dataclasses import dataclass, field

import policies
from scenarios import SCENARIOS

ARMS = ("merchant", "ours", "external")

# Which arms' outcomes each system receives. The merchant's own system sees all of its own
# payments in every scheme; it is the merchant's data.
SCHEMES = {
    "full": {
        "merchant": ARMS,
        "ours": ARMS,
        "external": ARMS,
    },
    "own_plus_merchant": {
        "merchant": ARMS,
        "ours": ("ours", "merchant"),
        "external": ("external", "merchant"),
    },
    "own_only": {
        "merchant": ARMS,
        "ours": ("ours",),
        "external": ("external",),
    },
}


@dataclass
class ArmStats:
    payments: int = 0
    approved: int = 0
    # Sum of the true approval rate of each chosen processor: the arm's expected approvals,
    # free of outcome noise. Used to measure bias; decisions use the realised outcomes.
    expected: float = 0.0
    day_payments: list = field(default_factory=list)
    day_approved: list = field(default_factory=list)
    # day_picks[day][processor]: how often the arm chose each processor that day.
    day_picks: list = field(default_factory=list)

    def rate(self):
        return self.approved / self.payments if self.payments else float("nan")

    def expected_rate(self):
        return self.expected / self.payments if self.payments else float("nan")


@dataclass
class RunResult:
    arms: dict
    # Failed payments while some processor was in an incident, across all arms.
    incident_failures: int
    # Expected approvals lost against always picking the best processor for each payment.
    regret: float
    payments: int


@functools.lru_cache(maxsize=None)
def rate_table(scenario_name, payments_per_day, days):
    """rates[segment][processor] is the list of approval rates for every payment index, and
    incident[segment] marks the payments made while an event was active for that segment."""
    scenario = SCENARIOS[scenario_name]
    n = payments_per_day * days
    rates, incident = {}, {}
    for segment in scenario.segments:
        rates[segment] = {
            p: [scenario.rate(p, segment, i / payments_per_day) for i in range(n)]
            for p in scenario.processors
        }
        incident[segment] = [scenario.in_event(i / payments_per_day, segment) for i in range(n)]
    return rates, incident


@functools.lru_cache(maxsize=4)
def payment_stream(scenario_name, payments_per_day, days, seed):
    """Segments, arm-assignment draws and outcome draws for one seed."""
    scenario = SCENARIOS[scenario_name]
    rng = random.Random(f"payments:{seed}")
    n = payments_per_day * days
    segments = rng.choices(scenario.segments, weights=scenario.segment_mix, k=n)
    assignment = [rng.random() for _ in range(n)]
    outcomes = [{p: rng.random() for p in scenario.processors} for _ in range(n)]
    return segments, assignment, outcomes


def run(
    scenario_name,
    systems,
    shares,
    receives,
    seed,
    payments_per_day,
    days,
    feedback_delay=0,
):
    """Runs one test.

    systems:  arm -> policy name.
    shares:   arm -> traffic share (sums to 1), in a fixed arm order so the same seed assigns
              the same payments to the same arm in every scheme.
    receives: arm -> the arms whose outcomes that arm's system learns from.
    feedback_delay: payments that pass before an outcome reaches the systems.
    """
    scenario = SCENARIOS[scenario_name]
    processors = scenario.processors
    rates, incident = rate_table(scenario_name, payments_per_day, days)
    segments, assignment, outcomes = payment_stream(scenario_name, payments_per_day, days, seed)
    n = len(segments)

    arm_order = [a for a in ARMS if a in systems] + [a for a in systems if a not in ARMS]
    bounds, total = [], 0.0
    for arm in arm_order:
        total += shares[arm]
        bounds.append((total, arm))
    if abs(total - 1.0) > 1e-9:
        raise ValueError(f"shares sum to {total}, not 1")

    live = {
        arm: policies.build(name, processors, f"policy:{seed}:{arm}")
        for arm, name in systems.items()
    }
    # source arm -> (policy, whether the payment is that policy's own) for each recipient.
    recipients = {
        source: [(live[arm], arm == source) for arm in systems if source in receives[arm]]
        for source in systems
    }
    stats = {arm: ArmStats() for arm in systems}
    for s in stats.values():
        s.day_payments = [0] * days
        s.day_approved = [0] * days
        s.day_picks = [collections.Counter() for _ in range(days)]

    pending = collections.deque()
    incident_failures = 0
    regret = 0.0

    for i in range(n):
        while pending and pending[0][0] <= i:
            _, targets, segment, processor, approved = pending.popleft()
            for policy, own in targets:
                policy.observe(segment, processor, approved, own)

        segment = segments[i]
        draw = assignment[i]
        arm = bounds[-1][1]
        for bound, candidate in bounds:
            if draw < bound:
                arm = candidate
                break

        processor = live[arm].decide(segment)
        segment_rates = rates[segment]
        rate = segment_rates[processor][i]
        approved = outcomes[i][processor] < rate

        s = stats[arm]
        day = i // payments_per_day
        s.payments += 1
        s.expected += rate
        s.day_payments[day] += 1
        s.day_picks[day][processor] += 1
        if approved:
            s.approved += 1
            s.day_approved[day] += 1
        elif incident[segment][i]:
            incident_failures += 1
        regret += max(segment_rates[p][i] for p in processors) - rate

        targets = recipients[arm]
        if feedback_delay <= 0:
            for policy, own in targets:
                policy.observe(segment, processor, approved, own)
        else:
            pending.append((i + feedback_delay, targets, segment, processor, approved))

    return RunResult(stats, incident_failures, regret, n)
