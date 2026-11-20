# Routing comparison simulator

A merchant evaluating this engine may split live traffic across three systems: its own routing,
this engine, and another engine. Each system routes its share, and the outcomes are fed back
to the systems. This simulator answers two questions about that set-up before any real traffic
is spent:

1. **Which feedback-sharing scheme gives a trustworthy comparison?** Does each engine learn from
   every payment, or only from its own and the merchant's?
2. **What does the test cost the merchant**, in approval rate and in payments failed during
   processor incidents?

Everything is synthetic: processors, approval rates and incidents are made up, so the results
show how the comparison behaves, not how any real processor performs.

## Run it

Standard library only (Python 3.9+).

```sh
scripts/routing-comparison-sim/run.py                      # every scenario, 100 seeds, ~3 min on 10 cores
scripts/routing-comparison-sim/run.py --list               # scenarios and policies
scripts/routing-comparison-sim/run.py --scenarios late_recovery --seeds 200
scripts/routing-comparison-sim/run.py --split 34,33,33 --payments-per-day 500 --days 28
python3 -m unittest discover scripts/routing-comparison-sim
```

`--split` is the traffic percentage for merchant, this engine and the external engine.
`--feedback-delay` holds each outcome back for that many payments.

## Method

### The benchmark

The merchant will route all its traffic through whichever system wins. So the truth a test is
judged against is each system's **expected approval rate when it routes 100% of the traffic
alone** (its *standalone* rate), measured on the same seeds as the tests. A test is good when
the gap it measures between two systems matches the gap between their standalone rates.

### Schemes

| scheme | this engine learns from | the external engine learns from |
|---|---|---|
| `full` | every payment | every payment |
| `own_plus_merchant` | its own and the merchant's payments | its own and the merchant's payments |
| `own_only` | its own payments | its own payments |

The merchant's system sees every payment in every scheme, because it is the merchant's own data.

### Paired runs

Each seed fixes the payments: their card network, which system each payment goes to, and one
outcome draw per payment and processor. A payment sent to processor `g` is approved when its
draw for `g` is below `g`'s approval rate at that moment. Every scheme and every standalone run
on a seed sees the same payments and draws, so two runs differ only in routing choices. That is
what makes per-seed differences meaningful with a modest number of seeds.

### What is reported

- **bias**: measured gap minus standalone gap (this engine minus the external engine), averaged
  over seeds with a 95% interval. It is computed from expected approvals (the true rate of each
  chosen processor), which removes outcome noise. Bias is the systematic error of the scheme.
- **calls**: what a significance test on the realised outcomes concludes in each run. The
  default `day` test is a paired t-test on the two arms' daily approval rates. The `payment`
  test treats every payment as independent. A call that names the system with the lower
  standalone rate is a **wrong call**. When the standalone gap is under `--margin-pp`, the truth
  is a tie and any call is wrong.
- **A/A calibration**: the external seat runs an identical copy of this engine (`ours_clone`),
  so any call is false. A trustworthy 95% test calls about 5% of the time. Higher rates mean the
  test mistakes the luck of one learning run for a real difference.
- **merchant SR** and **cost vs merchant alone**: the approval rate across all three arms, and
  how far that is from the merchant routing everything with its own system.
- **incident failures**: payments that failed while a processor was in an incident, across all
  arms.

### Policies

This engine is modelled by `policies.SrV3`:
- a window of the last 125 outcomes per card network and processor, starting full of successes;
- the highest window rate wins, and ties break at random;
- 5% of payments explore, in one of two ways depending on the merchant's flags.

| policy | merchant flags | how it explores | which outcomes update scores |
|---|---|---|---|
| `ours` (default, `--ours`) | `explore-exploit-srv3` off, global random-traffic flag on | to a processor other than the leader | every outcome it receives |
| `ours_ee` | `explore-exploit-srv3` on | uniformly over all processors | only outcomes of its own exploring payments |

Under `explore-exploit-srv3` the feedback flow updates scores only for payments whose decision
explored (`is_routing_approach_in_explore`). So the engine learns from about 5% of its own
traffic. To learn from a payment another system routed, the merchant has to ask the engine to
decide it first, and that payment counts only when that decision happened to explore. With
neither flag on, the engine does not explore and routes like `greedy`.

The model can also run with reset, or without per-network windows.

The external engine's algorithm is unknown, so it is played by stand-ins that learn in different
ways:

| policy | what it represents |
|---|---|
| `greedy` | an engine that never explores once it has a favourite |
| `heavy_explorer` | SR v3 with 15% hedging: learns fast and pays for it |
| `thompson` | discounted Thompson sampling, which explores according to its uncertainty |
| `ours_clone` | an identical copy of this engine, for the A/A check |

The merchant is played by static rules (`merchant_rules`, or `merchant_split`, which spreads its
traffic).

### Checking the model against the engine

`validate_engine.py` replays a scenario with a running engine in this engine's seat and compares
the result with the model over many seeds:

```sh
DECISION_ENGINE_URL=http://localhost:8099 scripts/routing-comparison-sim/validate_engine.py \
  --scenario late_recovery --scheme solo --payments-per-day 300 --seeds 3
```

For payments another system routed, it calls `/decide-gateway` before
`/update-gateway-score`, because the engine only updates scores for payments it holds scoring
context for. That is the call pattern a merchant integration needs if the engine is to learn
from other systems' traffic.

`--mode` picks the engine configuration (`ours` or `ours_ee`, as above). The validator prints
whether the real run's approval rate falls inside the model's 95% range, and each day's
processor mix against the band the model's runs span. A single run follows one learning path,
and a learner's daily mix swings widely from run to run, so the overall rate is the firmer
check.

## Limits

- Outcomes depend only on processor, card network and time. Real payments differ by issuer,
  amount and more, and systems may route on fields this model leaves out.
- The external stand-ins bracket plausible behaviour; they are not the external engine.
- First-attempt approval only; retries are not modelled.
- Feedback is immediate unless `--feedback-delay` is set.
