"""Checks the simulator itself.  python3 -m unittest discover scripts/routing-comparison-sim"""

import os
import random
import statistics
import sys
import unittest

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import policies  # noqa: E402
import simulate  # noqa: E402
from scenarios import SCENARIOS, Event, Scenario  # noqa: E402

PROCESSORS = ("P1", "P2", "P3")


class SrV3ModelTest(unittest.TestCase):
    def test_an_unseen_processor_scores_one(self):
        model = policies.SrV3(PROCESSORS, random.Random(0))
        self.assertEqual(model.scores("visa"), {p: 1.0 for p in PROCESSORS})

    def test_the_window_keeps_the_last_bucket_size_outcomes(self):
        model = policies.SrV3(PROCESSORS, random.Random(0), bucket_size=4)
        for approved in (False, False, True, False, False):
            model.observe("visa", "P1", approved, True)
        # The last four outcomes: F, T, F, F.
        self.assertEqual(model.scores("visa")["P1"], 0.25)
        self.assertEqual(model.scores("mastercard")["P1"], 1.0)

    def test_windows_ignore_the_segment_when_not_segment_aware(self):
        model = policies.SrV3(PROCESSORS, random.Random(0), bucket_size=4, segment_aware=False)
        model.observe("visa", "P1", False, True)
        self.assertEqual(model.scores("mastercard")["P1"], 0.75)

    def test_without_hedging_the_leader_always_wins(self):
        model = policies.SrV3(PROCESSORS, random.Random(0), bucket_size=10, hedge_mode="off")
        model.observe("visa", "P1", False, True)
        model.observe("visa", "P2", False, True)
        self.assertEqual({model.decide("visa") for _ in range(500)}, {"P3"})

    def test_explore_exploit_hedging_picks_uniformly_at_the_hedging_rate(self):
        model = policies.SrV3(PROCESSORS, random.Random(1), bucket_size=10, hedging_percent=30)
        for _ in range(10):
            model.observe("visa", "P1", False, True)
            model.observe("visa", "P2", False, True)
        picks = [model.decide("visa") for _ in range(20000)]
        # Exploring picks each processor with probability 0.3 / 3.
        self.assertAlmostEqual(picks.count("P1") / len(picks), 0.10, delta=0.01)
        self.assertAlmostEqual(picks.count("P3") / len(picks), 0.80, delta=0.01)

    def test_random_traffic_hedging_never_explores_the_leader(self):
        model = policies.SrV3(PROCESSORS, random.Random(2), bucket_size=10,
                              hedging_percent=30, hedge_mode="random_traffic")
        for _ in range(10):
            model.observe("visa", "P1", False, True)
            model.observe("visa", "P2", False, True)
        picks = [model.decide("visa") for _ in range(20000)]
        self.assertAlmostEqual(picks.count("P3") / len(picks), 0.70, delta=0.01)

    def test_explore_only_learns_from_exploring_decisions_alone(self):
        model = policies.SrV3(PROCESSORS, random.Random(3), bucket_size=1000,
                              hedging_percent=20, learn_from="explore_only")
        explored = 0
        for _ in range(2000):
            processor = model.decide("visa")
            was_explore = model.explored[-1]
            explored += was_explore
            model.observe("visa", processor, False, True)
        failures = sum(1000 - model.sums[("visa", p)] for p in PROCESSORS)
        self.assertEqual(failures, explored)

    def test_explore_only_counts_another_systems_payment_at_the_hedging_rate(self):
        model = policies.SrV3(PROCESSORS, random.Random(4), bucket_size=10000,
                              hedging_percent=20, learn_from="explore_only")
        for _ in range(5000):
            model.observe("visa", "P1", False, False)
        self.assertAlmostEqual((10000 - model.sums[("visa", "P1")]) / 5000, 0.20, delta=0.02)

    def test_reset_lifts_a_processor_far_below_the_leader(self):
        model = policies.SrV3(PROCESSORS, random.Random(0), bucket_size=100, reset=True,
                              hedge_mode="off")
        for _ in range(100):
            model.observe("visa", "P2", False, True)
        model.decide("visa")
        # Reset to just under the leader (1.0) minus the lower reset band.
        self.assertGreater(model.scores("visa")["P2"], 0.8)
        self.assertLess(model.scores("visa")["P2"], 1.0)


class ScenarioTest(unittest.TestCase):
    def test_a_repeating_event_is_active_only_in_its_window(self):
        event = Event("P1", 0.3, 0.5, 0.6, repeat_every_days=1.0)
        self.assertTrue(event.active(2.55, "visa"))
        self.assertFalse(event.active(2.65, "visa"))
        self.assertFalse(event.active(0.4, "visa"))

    def test_a_segment_event_leaves_other_segments_alone(self):
        scenario = SCENARIOS["segment_outage"]
        self.assertEqual(scenario.rate("P2", "mastercard", 10.5), 0.20)
        self.assertEqual(scenario.rate("P2", "visa", 10.5), 0.87)

    def test_lasting_changes_are_not_incidents(self):
        self.assertFalse(SCENARIOS["late_recovery"].in_event(12.0, "visa"))
        self.assertTrue(SCENARIOS["hard_outage"].in_event(7.2, "visa"))


THREE_SYSTEMS = {"merchant": "merchant_rules", "ours": "ours", "external": "greedy"}
SPLIT = {"merchant": 0.5, "ours": 0.25, "external": 0.25}


class RunTest(unittest.TestCase):
    def run_test(self, scenario, systems, scheme, seed, **kw):
        return simulate.run(scenario, systems, SPLIT, simulate.SCHEMES[scheme], seed,
                            payments_per_day=kw.get("ppd", 400), days=kw.get("days", 10))

    def test_a_seed_gives_every_scheme_the_same_payments(self):
        # Static merchant rules don't learn, so with common random numbers their arm must come
        # out identical whatever the others learn.
        a = self.run_test("hard_outage", THREE_SYSTEMS, "full", 3).arms["merchant"]
        b = self.run_test("hard_outage", THREE_SYSTEMS, "own_only", 3).arms["merchant"]
        self.assertEqual((a.payments, a.approved, a.expected), (b.payments, b.approved, b.expected))

    def test_identical_systems_sharing_all_feedback_measure_the_same(self):
        systems = {"merchant": "merchant_rules", "ours": "ours", "external": "ours"}
        gaps = [
            r.arms["ours"].expected_rate() - r.arms["external"].expected_rate()
            for r in (self.run_test("steady", systems, "full", s) for s in range(20))
        ]
        self.assertLess(abs(statistics.fmean(gaps)), 0.002)

    def test_full_sharing_lets_a_greedy_system_learn_from_an_explorers_traffic(self):
        # P2 starts worse and recovers; the merchant only uses P1.
        # A greedy system finds the recovery only from someone else's exploration.
        SCENARIOS["_recovery_example"] = Scenario(
            "_recovery_example",
            "P2 80% then 95%; P1 90%.",
            base_rates={"P1": {"visa": 0.90, "mastercard": 0.90},
                        "P2": {"visa": 0.80, "mastercard": 0.80}},
            events=(Event("P2", 0.95, 5.0, 1e9, incident=False),),
            # Visa only, which the merchant's rules send to P1.
            segment_mix=(1.0, 0.0),
        )
        systems = {"merchant": "merchant_rules", "ours": "heavy_explorer", "external": "greedy"}
        try:
            def greedy_share_of_p2_late(scheme):
                arms = [
                    self.run_test("_recovery_example", systems, scheme, s, days=16)
                    .arms["external"]
                    for s in range(10)
                ]
                late = range(12, 16)
                return sum(a.day_picks[d]["P2"] for a in arms for d in late) / sum(
                    a.day_payments[d] for a in arms for d in late
                )
            # It moves to the recovered P2 when it sees the explorer's traffic, and mostly
            # stays put when it doesn't.
            self.assertGreater(greedy_share_of_p2_late("full"), 0.6)
            self.assertLess(greedy_share_of_p2_late("own_plus_merchant"), 0.35)
        finally:
            del SCENARIOS["_recovery_example"]
            simulate.rate_table.cache_clear()
            simulate.payment_stream.cache_clear()


if __name__ == "__main__":
    unittest.main()
