"""Routing policies: a model of the engine's SR v3 routing, stand-ins for an external engine,
and static merchant rules.

Every policy answers `decide(segment)` with a processor and learns through
`observe(segment, processor, approved, own)`, where `own` says whether the payment was one this
policy routed. A policy never sees approval rates directly, only the outcomes it is given, so
what it learns depends on the feedback scheme it runs under.
"""

import collections
import math
import random

# Engine defaults (src/decider/gatewaydecider/constants.rs).
DEFAULT_BUCKET_SIZE = 125
DEFAULT_HEDGING_PERCENT = 5.0
DEFAULT_RESET_FACTOR = 3.0


class SrV3:
    """The engine's SR v3 routing for one merchant.

    Mirrors `get_cached_scores_based_on_srv3` and the SR v3 feedback flow:

    * Each (segment, processor) keeps a window of its last `bucket_size` outcomes. A new window
      starts full of successes, so an unseen processor scores 1.0.
    * The score is the window's approval rate; the highest score wins and ties break at random.
    * `hedge_mode="explore_exploit"` (the `explore-exploit-srv3` merchant feature): with
      probability `hedging_percent`, every processor scores 1.0, so the pick is uniform.
    * `hedge_mode="random_traffic"` (the volume-distribution feature): with that probability,
      the pick is uniform over every processor except the current leader.
    * `learn_from="explore_only"` (what the engine does when `explore-exploit-srv3` is on): only
      outcomes of exploring payments update the windows (`is_routing_approach_in_explore` in the
      feedback flow). For a payment another system routed, the engine's own decision for it is
      made when the merchant asks for it to learn from that payment, so it counts only when that
      decision happened to explore. `learn_from="all"` counts every outcome.
    * `reset=True` (the SR v3 reset feature): a processor scoring more than
      `upper_reset_factor` sigmas below the leader has its window rewritten to just under the
      leader's rate.
    * `segment_aware=False` keys windows by processor only, as when the merchant's SR config
      sets no card-network dimension.
    """

    def __init__(
        self,
        processors,
        rng,
        bucket_size=DEFAULT_BUCKET_SIZE,
        hedging_percent=DEFAULT_HEDGING_PERCENT,
        hedge_mode="explore_exploit",
        learn_from="all",
        reset=False,
        upper_reset_factor=DEFAULT_RESET_FACTOR,
        lower_reset_factor=DEFAULT_RESET_FACTOR,
        segment_aware=True,
    ):
        if hedge_mode not in ("explore_exploit", "random_traffic", "off"):
            raise ValueError(f"unknown hedge_mode {hedge_mode!r}")
        if learn_from not in ("all", "explore_only"):
            raise ValueError(f"unknown learn_from {learn_from!r}")
        self.processors = tuple(processors)
        self.rng = rng
        self.bucket_size = bucket_size
        self.hedge_probability = hedging_percent / 100.0 if hedge_mode != "off" else 0.0
        self.hedge_mode = hedge_mode
        self.learn_from = learn_from
        # Whether each of this policy's own decisions explored, oldest first; outcomes of its
        # own payments arrive in decision order.
        self.explored = collections.deque()
        self.reset = reset
        self.upper_reset_factor = upper_reset_factor
        self.lower_reset_factor = lower_reset_factor
        self.segment_aware = segment_aware
        self.windows = {}
        self.sums = {}

    def _key(self, segment, processor):
        return (segment if self.segment_aware else None, processor)

    def _window(self, key):
        window = self.windows.get(key)
        if window is None:
            window = collections.deque([1] * self.bucket_size, maxlen=self.bucket_size)
            self.windows[key] = window
            self.sums[key] = self.bucket_size
        return window

    def scores(self, segment):
        scores = {}
        for processor in self.processors:
            key = self._key(segment, processor)
            self._window(key)
            scores[processor] = self.sums[key] / self.bucket_size
        return scores

    def _apply_reset(self, segment, scores):
        best = max(scores.values())
        n = float(self.bucket_size)
        sigma = math.sqrt(max(best * (1.0 - best), 0.09) / n)
        threshold = best - self.upper_reset_factor * sigma
        zeros = int(math.floor((1.0 - best + self.lower_reset_factor * sigma) * n))
        zeros = min(max(zeros, 2), self.bucket_size)
        reset_successes = self.bucket_size - zeros
        for processor, score in scores.items():
            if score < threshold:
                key = self._key(segment, processor)
                # Spread the failures evenly, as `reset_sr_v3_score` does; only the count
                # matters to the score, but the order decides which outcomes age out first.
                interval = (n - 1.0) / (zeros - 1)
                pattern, placed = [], 0
                for i in range(self.bucket_size, 0, -1):
                    if n - i >= placed * interval:
                        pattern.append(0)
                        placed += 1
                    else:
                        pattern.append(1)
                pattern.reverse()
                self.windows[key] = collections.deque(pattern, maxlen=self.bucket_size)
                self.sums[key] = reset_successes
                scores[processor] = reset_successes / n

    def decide(self, segment):
        processor, explored = self._decide(segment)
        if self.learn_from == "explore_only":
            self.explored.append(explored)
        return processor

    def _decide(self, segment):
        rng = self.rng
        if self.hedge_mode == "explore_exploit" and rng.random() < self.hedge_probability:
            return rng.choice(self.processors), True
        scores = self.scores(segment)
        if self.reset:
            self._apply_reset(segment, scores)
        best = max(scores.values())
        leaders = [p for p in self.processors if scores[p] == best]
        leader = leaders[0] if len(leaders) == 1 else rng.choice(leaders)
        if self.hedge_mode == "random_traffic" and rng.random() < self.hedge_probability:
            others = [p for p in self.processors if p != leader]
            if others:
                return rng.choice(others), True
        return leader, False

    def observe(self, segment, processor, approved, own):
        if self.learn_from == "explore_only":
            if own:
                if not self.explored.popleft():
                    return
            elif not (
                self.hedge_mode == "explore_exploit"
                and self.rng.random() < self.hedge_probability
            ):
                return
        key = self._key(segment, processor)
        window = self._window(key)
        self.sums[key] -= window[0]
        window.append(1 if approved else 0)
        self.sums[key] += window[-1]


class DiscountedThompson:
    """Thompson sampling over a Beta posterior per (segment, processor), discounted so it
    forgets: each outcome for a processor shrinks that processor's earlier evidence by
    `discount`. Explores in proportion to its uncertainty rather than at a fixed rate."""

    def __init__(self, processors, rng, discount=0.995):
        self.processors = tuple(processors)
        self.rng = rng
        self.discount = discount
        self.alpha = collections.defaultdict(lambda: 1.0)
        self.beta = collections.defaultdict(lambda: 1.0)

    def decide(self, segment):
        betavariate = self.rng.betavariate
        draws = {
            p: betavariate(self.alpha[(segment, p)], self.beta[(segment, p)])
            for p in self.processors
        }
        return max(draws, key=draws.get)

    def observe(self, segment, processor, approved, own):
        key = (segment, processor)
        d = self.discount
        self.alpha[key] = d * self.alpha[key] + (1.0 if approved else 0.0)
        self.beta[key] = d * self.beta[key] + (0.0 if approved else 1.0)


class StaticRules:
    """Fixed per-segment routing weights, as a rules-based merchant system. Does not learn."""

    def __init__(self, weights, rng):
        self.rng = rng
        self.table = {
            segment: (tuple(choices), tuple(w for w in choices.values()))
            for segment, choices in weights.items()
        }

    def decide(self, segment):
        processors, weights = self.table[segment]
        if len(processors) == 1:
            return processors[0]
        return self.rng.choices(processors, weights=weights)[0]

    def observe(self, segment, processor, approved, own):
        pass


# Built-in policies. Each factory takes (processors, rng). The external engine's algorithm is
# unknown, so the comparison runs against several stand-ins that bracket how it might learn.
POLICIES = {
    "ours": (
        "The engine with explore-exploit off and the global random-traffic flag on: window 125 "
        "per card network, 5% of payments go to a processor other than the leader, and every "
        "outcome updates scores.",
        lambda processors, rng: SrV3(processors, rng, hedge_mode="random_traffic"),
    ),
    "ours_ee": (
        "The engine with explore-exploit-srv3 on: window 125 per card network, 5% of payments "
        "explore uniformly, and only exploring payments update scores.",
        lambda processors, rng: SrV3(processors, rng, learn_from="explore_only"),
    ),
    "greedy": (
        "Same windows as `ours` but never explores once it has a favourite.",
        lambda processors, rng: SrV3(processors, rng, hedging_percent=0.0, hedge_mode="off"),
    ),
    "heavy_explorer": (
        "SR v3 windows with 15% uniform exploration, learning from every outcome: learns fast, "
        "pays for it on its own traffic.",
        lambda processors, rng: SrV3(processors, rng, hedging_percent=15.0),
    ),
    "thompson": (
        "Discounted Thompson sampling (memory of about 200 outcomes per processor).",
        lambda processors, rng: DiscountedThompson(processors, rng),
    ),
    "merchant_rules": (
        "Static rules: visa to P1, mastercard to P2 (the best processors on base rates).",
        lambda processors, rng: StaticRules({"visa": {"P1": 1.0}, "mastercard": {"P2": 1.0}}, rng),
    ),
    "merchant_split": (
        "Static rules that spread traffic: 60/30/10 on visa, 30/60/10 on mastercard.",
        lambda processors, rng: StaticRules(
            {
                "visa": {"P1": 0.6, "P2": 0.3, "P3": 0.1},
                "mastercard": {"P1": 0.3, "P2": 0.6, "P3": 0.1},
            },
            rng,
        ),
    ),
}


def build(name, processors, seed):
    """A fresh policy with its own random stream, so its exploration and tie-breaks don't
    depend on how many random draws any other part of the simulation made."""
    try:
        _, factory = POLICIES[name]
    except KeyError:
        raise SystemExit(f"unknown policy {name!r}; choose from {', '.join(POLICIES)}")
    return factory(processors, random.Random(seed))
