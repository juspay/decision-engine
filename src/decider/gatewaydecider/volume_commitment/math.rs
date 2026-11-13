//! The arithmetic. Plain numbers in, plain numbers out.

/// How many days of history we average to work out a PSP's recent daily volume.
pub const PACE_WINDOW_DAYS: u32 = 7;

/// Seconds in a calendar day — the contract-day length for every non-test billing cycle.
pub const SECS_PER_DAY: u64 = 86_400;

/// Contract-day length a `test_minutes` cycle falls back on when a deployment's config names
/// none: half a minute per contract day, so a demo watches its promises pace, fall behind and be
/// rescued at twice the speed of the clock. `volume_commitment.test_day_secs` overrides it.
pub const DEFAULT_TEST_DAY_SECS: u64 = 30;

/// Fewest contract days a `test_minutes` cycle may declare — its `anchor` counts contract days,
/// and a cycle of one has no pace to fall behind.
pub const MIN_TEST_CYCLE_DAYS: u32 = 2;

/// Forecasts a contract gets per contract day when it names no cadence of its own.
///
/// A cadence in wall-clock seconds does not survive a compressed day: an hourly forecast is 24 a
/// day on a calendar cycle, and the same number of seconds on a `test_minutes` cycle is a forecast
/// every few contract days. A demo would then re-plan far less often, relative to its cycle, than
/// production does — reacting more slowly to exactly the drifts it exists to show. Held per
/// contract day, the resolution is the same on both.
pub const FORECASTS_PER_CONTRACT_DAY: u64 = 24;

/// Floor for a derived cadence. Below it the scheduler's own tick and the run round trip dominate,
/// so asking for more forecasts buys none.
pub const MIN_FORECAST_INTERVAL_SECS: u64 = 2;

/// Seconds between forecasts for a contract whose day lasts `day_secs`.
pub fn forecast_interval_for_day(day_secs: u64) -> u64 {
    (day_secs / FORECASTS_PER_CONTRACT_DAY).max(MIN_FORECAST_INTERVAL_SECS)
}

/// Prefix of every run id; the rest is the cycle's opening instant in epoch ms.
pub const RUN_ID_PREFIX: &str = "vcr_";

/// Floor for days-left (~1s) so an ended cycle yields a huge `needed_daily` instead of a division by zero.
const MIN_DAYS_LEFT: f64 = 1.0 / SECS_PER_DAY as f64;

/// Fractional contract days left; flooring would overstate the needed rate and understate the traffic left.
pub fn days_left(period_end_ms: i64, now_ms: i64, day_secs: u64) -> f64 {
    let remaining_ms = (period_end_ms - now_ms).max(0) as f64;
    (remaining_ms / day_ms(day_secs) as f64).max(MIN_DAYS_LEFT)
}

/// One contract day in milliseconds.
pub fn day_ms(day_secs: u64) -> i64 {
    (day_secs.max(1) as i64).saturating_mul(1000)
}

/// Which contract day of the cycle `now` falls in, counting from 0.
pub fn day_index(period_start_ms: i64, now_ms: i64, day_secs: u64) -> i64 {
    (now_ms - period_start_ms).max(0) / day_ms(day_secs)
}

/// Whole contract days in a cycle, never below one — the x-axis span a promise line runs to.
pub fn days_total(period_start_ms: i64, period_end_ms: i64, day_secs: u64) -> u32 {
    days_left(period_end_ms, period_start_ms, day_secs)
        .round()
        .max(1.0) as u32
}

/// Run id for one billing cycle, keyed on its opening instant so runs sort chronologically.
pub fn run_id(cycle_start_ms: i64) -> String {
    format!("{RUN_ID_PREFIX}{cycle_start_ms}")
}

/// The cycle-opening instant a run id names, or None for anything that is not a run id.
pub fn run_start_ms(run_id: &str) -> Option<i64> {
    run_id.strip_prefix(RUN_ID_PREFIX)?.parse().ok()
}

/// Volume still owed on a commitment. Never negative — an over-delivered PSP owes nothing.
pub fn remaining(goal: f64, achieved: f64) -> f64 {
    (goal - achieved).max(0.0)
}

/// Volume a PSP must receive each day from now on to still hit its goal.
pub fn needed_daily(remaining: f64, days_left: f64) -> f64 {
    remaining / days_left.max(MIN_DAYS_LEFT)
}

/// The share of a cycle steering aims to have the goal delivered inside; the rest is margin.
pub const PACE_WITHIN_CYCLE: f64 = 0.9;

/// Contract days steering paces against: the days the cycle has left, less the margin it keeps at
/// the end.
///
/// A commitment pays nothing at 99% of its goal, and volume arrives one payment at a time through
/// a roll, so a plan that aims to land exactly on the line lands under it about half the time. It
/// also spends the whole cycle believing routing will keep delivering its recent average unaided,
/// which on the last day is a single sample rather than an average — one quiet day, or one spent
/// outside the approval tolerance, and there is nothing left to make it up with.
///
/// Pacing to finish inside `PACE_WITHIN_CYCLE` of the cycle buys that room back: a little more is
/// asked for each day, the goal is reached with time in hand, and inside the final margin what is
/// still owed is asked for at once.
pub fn pacing_days_left(days_left: f64, cycle_days: f64) -> f64 {
    (days_left - cycle_days * (1.0 - PACE_WITHIN_CYCLE)).max(MIN_DAYS_LEFT)
}

/// Contract days one cycle runs, never below one.
pub fn cycle_days(period_start_ms: i64, period_end_ms: i64, day_secs: u64) -> f64 {
    ((period_end_ms - period_start_ms) as f64 / day_ms(day_secs) as f64).max(1.0)
}

/// Total traffic we expect for the rest of the period — the budget every commitment competes for.
pub fn traffic_left(expected_daily_traffic: f64, days_left: f64) -> f64 {
    expected_daily_traffic * days_left
}

/// Where a PSP lands at its current rate. Informational only; does not drive steering.
pub fn forecast(achieved: f64, pace: f64, days_left: f64) -> f64 {
    achieved + pace * days_left
}

/// Day-0 pace guess: the day's traffic split evenly across the PSPs.
pub fn starting_pace(expected_daily_traffic: f64, psp_count: usize) -> f64 {
    if psp_count == 0 {
        0.0
    } else {
        expected_daily_traffic / psp_count as f64
    }
}

/// Volume steering must *add* each day: the daily target less what routing already delivers,
/// never negative. Capping on `needed_daily` instead would overshoot by routing's own share.
pub fn daily_shortfall(needed_daily: f64, routing_gives_daily: f64) -> f64 {
    (needed_daily - routing_gives_daily).max(0.0)
}

/// How much more than a commitment needs must be flowing before a drop is reversed.
///
/// Dropping and re-entering on the same threshold flaps. The estimate behind both follows a
/// measured rate, and a rate wandering a few percent either side of the line gives up on a
/// commitment, chases it, and gives up again — each cycle spending approval-rate tolerance on a
/// race the previous forecast had conceded, and leaving a dashboard whose "eliminated" is
/// contradicted by the steering underneath it.
///
/// The gap between the two thresholds is what makes a re-entry mean "the traffic really came
/// back" rather than "the estimate moved". A genuine spike clears 25% headroom easily; noise
/// around the drop threshold does not.
pub const REENTRY_HEADROOM: f64 = 1.25;

/// Whether a commitment already given up on should be chased again: what it needs each day now —
/// which has grown while it was dropped — fits inside the flow with headroom to spare.
pub fn reclaimable(needed_daily: f64, forward_daily_traffic: f64) -> bool {
    needed_daily * REENTRY_HEADROOM <= forward_daily_traffic
}

/// The traffic a steer can actually draw on: everything except what routing already gives this
/// PSP. A payment routing has already sent it is kept, never rolled for — there is nothing to
/// move — so that volume is no part of the flow a rate is a share of.
pub fn divertible_daily(daily_traffic: f64, routing_gives_daily: f64) -> f64 {
    (daily_traffic - routing_gives_daily).max(0.0)
}

/// Share of divertible traffic to take (0..=1) = shortfall / the divertible traffic still to
/// come; a rate needs no shared counter and self-paces.
///
/// The denominator is what the roll is offered, not what the merchant does. Given all of the
/// merchant's flow instead, the rate comes out short by exactly the fraction routing already
/// sends this PSP: rolled only against the payments it does not already hold, it then delivers
/// `shortfall × divertible / total` a day and the commitment lands under its goal.
pub fn steer_rate(remaining_shortfall: f64, divertible_traffic: f64) -> f64 {
    if divertible_traffic <= 0.0 {
        // Routing already sends this PSP everything; there is nothing left to divert to it.
        return 0.0;
    }
    (remaining_shortfall / divertible_traffic).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortfall_is_the_target_less_what_routing_already_gives() {
        // The fixture's psp_b: needs ~192k/day, routing supplies 158k.
        assert!((daily_shortfall(191_667.0, 158_000.0) - 33_667.0).abs() < 1.0);
    }

    #[test]
    fn a_psp_routing_already_feeds_needs_no_help() {
        assert_eq!(daily_shortfall(100_000.0, 158_000.0), 0.0);
    }

    /// The rate is simply what is owed over the divertible traffic still to come.
    #[test]
    fn the_rate_is_the_share_of_divertible_traffic_still_owed() {
        assert_eq!(steer_rate(25_000.0, 100_000.0), 0.25);
        assert_eq!(steer_rate(0.0, 100_000.0), 0.0);
    }

    /// Owing more than the divertible traffic means taking all of it, never more.
    #[test]
    fn the_rate_never_exceeds_all_of_the_divertible_traffic() {
        assert_eq!(steer_rate(500_000.0, 100_000.0), 1.0);
    }

    /// The dead band: the rate that dropped a commitment does not immediately un-drop it, and a
    /// real recovery does.
    #[test]
    fn re_entry_asks_for_more_than_the_drop_gave_up_on() {
        // Dropped needing 200k a day against 190k flowing. The wobble that used to resurrect it:
        // the same rate, read a few percent higher.
        assert!(!reclaimable(200_000.0, 205_000.0));
        assert!(!reclaimable(200_000.0, 240_000.0));
        // Traffic that genuinely came back.
        assert!(reclaimable(200_000.0, 250_000.0));
        assert!(reclaimable(200_000.0, 1_000_000.0));
    }

    /// What the roll is offered is the traffic routing is not already giving this PSP.
    #[test]
    fn only_the_traffic_routing_sends_elsewhere_can_be_diverted() {
        // The case the demo lost: $9.4k a day flowing, $3.5k of it already landing on adyen.
        assert_eq!(divertible_daily(9_400.0, 3_500.0), 5_900.0);
        // A PSP routing already sends everything to has nothing left to be given.
        assert_eq!(divertible_daily(9_400.0, 9_400.0), 0.0);
        assert_eq!(divertible_daily(9_400.0, 12_000.0), 0.0);
    }

    /// Rolling the shortfall against the divertible flow delivers the shortfall. Against all of
    /// the merchant's traffic it delivers only 63% of it here, which is a commitment missed.
    #[test]
    fn the_rate_covers_the_shortfall_out_of_the_flow_it_is_rolled_against() {
        let (traffic, unaided, shortfall) = (9_400.0, 3_500.0, 3_000.0);
        let divertible = divertible_daily(traffic, unaided);

        let rate = steer_rate(shortfall, divertible);
        assert!((rate * divertible - shortfall).abs() < 1e-9);

        // The denominator that under-delivered: same roll, same eligible payments, less volume.
        let over_all_traffic = steer_rate(shortfall, traffic);
        assert!(over_all_traffic * divertible < shortfall * 0.7);
    }

    /// The last tenth of a cycle is margin: what is owed inside it is asked for at once, rather
    /// than paced against days that are about to run out.
    #[test]
    fn the_final_margin_asks_for_what_is_left_at_once() {
        // Day 9 of ten, $3k still owed. Paced against the day that remains it reads as $3k a day,
        // which routing's own average appears to cover — and the commitment missed by $1k.
        assert_eq!(needed_daily(3_000.0, 1.0), 3_000.0);
        // Inside the margin the same $3k is what the rest of the cycle has to deliver.
        let pacing = pacing_days_left(1.0, 10.0);
        assert!(pacing < 0.01);
        assert!(needed_daily(3_000.0, pacing) > 300_000.0);
    }

    /// Mid-cycle it only tightens the pace; it does not turn a comfortable commitment urgent.
    #[test]
    fn the_margin_tightens_the_pace_it_does_not_replace_it() {
        // Day 5 of ten with half the goal to go: four days to deliver it in rather than five.
        assert!((pacing_days_left(5.0, 10.0) - 4.0).abs() < 1e-9);
        assert!((needed_daily(35_000.0, pacing_days_left(5.0, 10.0)) - 8_750.0).abs() < 1e-9);
    }

    /// Contract days, not calendar ones — a test cycle's day is however long the config says.
    #[test]
    fn a_cycles_length_is_counted_in_its_own_days() {
        assert_eq!(cycle_days(0, 100_000, 10), 10.0);
        assert_eq!(cycle_days(0, 30 * 86_400_000, SECS_PER_DAY), 30.0);
        // Never zero: a closed or malformed window still divides safely.
        assert_eq!(cycle_days(100, 0, SECS_PER_DAY), 1.0);
    }

    /// The demo and the real contract re-plan equally often *relative to their own cycle*, which
    /// is the only sense in which a demo predicts production.
    #[test]
    fn the_cadence_is_the_same_share_of_a_day_whatever_the_day_is() {
        // A calendar day, forecast hourly — the production default this replaces.
        assert_eq!(forecast_interval_for_day(SECS_PER_DAY), 3_600);
        assert_eq!(SECS_PER_DAY / forecast_interval_for_day(SECS_PER_DAY), 24);
        // A five-minute contract day gets its twenty-four too.
        assert_eq!(forecast_interval_for_day(300), 12);
    }

    /// Past the floor the scheduler cannot keep up, so the cadence stops shrinking rather than
    /// promising a resolution nothing delivers.
    #[test]
    fn a_very_short_day_stops_at_the_floor() {
        assert_eq!(forecast_interval_for_day(30), MIN_FORECAST_INTERVAL_SECS);
        assert_eq!(forecast_interval_for_day(1), MIN_FORECAST_INTERVAL_SECS);
        assert_eq!(forecast_interval_for_day(0), MIN_FORECAST_INTERVAL_SECS);
    }

    /// A cycle with no traffic left cannot deliver, and must not divide by zero.
    #[test]
    fn no_remaining_traffic_yields_no_steering() {
        assert_eq!(steer_rate(50_000.0, 0.0), 0.0);
        assert_eq!(steer_rate(50_000.0, -1.0), 0.0);
    }

    #[test]
    fn days_left_counts_contract_days_not_calendar_days() {
        let day_ms = 60_000; // one-minute contract days
                             // Ten minutes of cycle remaining = ten contract days.
        assert_eq!(days_left(10 * day_ms, 0, 60), 10.0);
        // Past the end nothing is left, but the value stays safe to divide by.
        assert!(days_left(0, 10 * day_ms, 60) > 0.0);
        assert!(days_left(0, 10 * day_ms, 60) < 0.001);
    }

    /// Half a contract day is half a day, not a whole one. Flooring here understated the traffic
    /// left to fund a commitment and overstated the rate it needed, writing off reachable goals.
    #[test]
    fn a_part_day_is_counted_as_a_fraction() {
        let day_ms = 60_000;
        assert_eq!(days_left(3 * day_ms / 2, 0, 60), 1.5);

        // The case that misfired: a 2-day cycle, 30s in, one PSP owing 600k against 500k/day.
        let remaining_ms = 2 * day_ms - 30_000;
        let left = days_left(remaining_ms, 0, 60);
        assert_eq!(left, 1.5);
        assert_eq!(needed_daily(600_000.0, left), 400_000.0); // was 600_000 when floored
        assert_eq!(traffic_left(500_000.0, left), 750_000.0); // was 500_000 when floored
    }
}
