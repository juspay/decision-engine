"""Synthetic processor environments: who approves what, and how that changes over the test.

A scenario is a set of processors with a base approval rate per payment segment, plus events
that change those rates for a window of time. Time is measured in days; a payment's day is its
index divided by the daily volume, so the same scenario works at any traffic level.
"""

from dataclasses import dataclass, field

SEGMENTS = ("visa", "mastercard")
SEGMENT_MIX = (0.6, 0.4)

# Base approval rate per processor and segment. The gaps are a few points, as between real
# card processors, and no processor is best in every segment.
BASE_RATES = {
    "P1": {"visa": 0.90, "mastercard": 0.86},
    "P2": {"visa": 0.87, "mastercard": 0.89},
    "P3": {"visa": 0.85, "mastercard": 0.85},
}


@dataclass(frozen=True)
class Event:
    """Sets `processor`'s approval rate to `rate` from `start_day` until `end_day`.

    `segments` limits the event to those segments (all when empty). `repeat_every_days`
    repeats the window, for flapping outages; the window then lasts `end_day - start_day`
    days and starts every `repeat_every_days` from `start_day`. `incident=False` marks a
    lasting change of rate rather than an outage, so its payments are not counted as failures
    during an incident.
    """

    processor: str
    rate: float
    start_day: float
    end_day: float
    segments: tuple = ()
    repeat_every_days: float = 0.0
    incident: bool = True

    def active(self, day: float, segment: str) -> bool:
        if self.segments and segment not in self.segments:
            return False
        if day < self.start_day:
            return False
        if self.repeat_every_days > 0:
            offset = (day - self.start_day) % self.repeat_every_days
            return offset < self.end_day - self.start_day
        return day < self.end_day


@dataclass(frozen=True)
class Scenario:
    name: str
    description: str
    events: tuple = ()
    base_rates: dict = field(default_factory=lambda: BASE_RATES)
    segments: tuple = SEGMENTS
    segment_mix: tuple = SEGMENT_MIX

    @property
    def processors(self) -> tuple:
        return tuple(self.base_rates)

    def rate(self, processor: str, segment: str, day: float) -> float:
        rate = self.base_rates[processor][segment]
        # Later events override earlier ones, so a scenario can layer a window over a long
        # change.
        for event in self.events:
            if event.processor == processor and event.active(day, segment):
                rate = event.rate
        return rate

    def in_event(self, day: float, segment: str) -> bool:
        return any(event.incident and event.active(day, segment) for event in self.events)


SCENARIOS = {
    s.name: s
    for s in (
        Scenario(
            "steady",
            "No incidents. Every comparison should be unbiased here; it checks the harness.",
        ),
        Scenario(
            "hard_outage",
            "The best visa processor (P1) approves 5% for 12 hours on day 7.",
            events=(Event("P1", 0.05, 7.0, 7.5),),
        ),
        Scenario(
            "degradation",
            "The best mastercard processor (P2) loses 12 points on mastercard from day 5 to 12.",
            events=(Event("P2", 0.77, 5.0, 12.0, segments=("mastercard",)),),
        ),
        Scenario(
            "late_recovery",
            "P3 starts degraded (70%) and from day 8 becomes the best processor (95%).",
            events=(
                Event("P3", 0.70, 0.0, 8.0, incident=False),
                Event("P3", 0.95, 8.0, 1e9, incident=False),
            ),
        ),
        Scenario(
            "flapping",
            "P1 approves 30% for 2 hours every day.",
            events=(Event("P1", 0.30, 0.5, 0.5 + 2 / 24, repeat_every_days=1.0),),
        ),
        Scenario(
            "segment_outage",
            "P2 approves 20% on mastercard only, for 1 day from day 10.",
            events=(Event("P2", 0.20, 10.0, 11.0, segments=("mastercard",)),),
        ),
        Scenario(
            "mixed",
            "A degraded processor that recovers, a hard outage and a flapping one in the same test.",
            events=(
                Event("P3", 0.72, 0.0, 6.0, incident=False),
                Event("P3", 0.93, 6.0, 1e9, incident=False),
                Event("P1", 0.05, 11.0, 11.5),
                Event("P2", 0.40, 3.25, 3.25 + 3 / 24, repeat_every_days=4.0),
            ),
        ),
    )
}
