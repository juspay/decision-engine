//! The arithmetic. Plain numbers in, plain numbers out.

/// How many days of history we average to work out a PSP's recent daily volume.
pub const PACE_WINDOW_DAYS: u32 = 7;

/// Seconds in a calendar day — the contract-day length for every non-test billing cycle.
pub const SECS_PER_DAY: u64 = 86_400;

/// Smallest slice of a day we will divide by — about a second of one. Below this the cycle is
/// effectively over, and the large `needed_daily` that follows reads as unreachable, which is the
/// honest answer rather than a division by zero.
const MIN_DAYS_LEFT: f64 = 1.0 / 86_400.0;

/// Contract days left in the billing period, as a fraction: 1.5 means a day and a half.
///
/// Fractional, because rounding a day and a half down to one is wrong in both directions at once —
/// it inflates the daily rate a commitment needs *and* shrinks the traffic left to fund it, so a
/// commitment still comfortably reachable is written off as lost. The error is worst on short
/// cycles, where a single day is a large share of the period, but it bites real calendar contracts
/// too: on the last day of a month you have hours left, not a day.
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

/// Identifier for one execution of a contract — one billing cycle, start to close.
///
/// A contract does not run once; it runs every month, quarter or (under a test cycle) every few
/// minutes, and each pass is judged on its own delivery. Naming the pass lets a merchant look back
/// at "the March run" and see the forecasts, steering and eliminations that belonged to it, rather
/// than one undifferentiated stream. The cycle's opening instant is what makes it unique, and
/// keeps runs sorting in the order they happened.
pub fn run_id(cycle_start_ms: i64) -> String {
    format!("vcr_{cycle_start_ms}")
}

/// Volume still owed on a commitment. Never negative — an over-delivered PSP owes nothing.
pub fn remaining(goal: f64, achieved: f64) -> f64 {
    (goal - achieved).max(0.0)
}

/// Volume a PSP must receive each day from now on to still hit its goal.
pub fn needed_daily(remaining: f64, days_left: f64) -> f64 {
    remaining / days_left.max(MIN_DAYS_LEFT)
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

/// The share of eligible payments to divert to a PSP, as a fraction of 0..=1.
///
/// This is what the forecast publishes instead of a volume budget, and it is the reason no counter
/// exists. Delivering a *quantity* forces you to sum what you have delivered — across pods that
/// sum has to be shared, and it is lost whenever a pod restarts. A rate makes every payment
/// independent: roll, act, forget. It is also self-pacing, because it applies to arriving traffic
/// rather than to a bucket, so it cannot be spent in one burst and needs no release window.
///
/// The forecast measures what actually landed and recomputes this each run, so the loop closes:
/// under-delivering raises the rate, over-delivering lowers it.
pub fn steer_rate(remaining_shortfall: f64, expected_remaining_traffic: f64) -> f64 {
    if expected_remaining_traffic <= 0.0 {
        // No traffic left to steer into; whatever is owed cannot be delivered this cycle.
        return 0.0;
    }
    (remaining_shortfall / expected_remaining_traffic).clamp(0.0, 1.0)
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

    /// The rate is simply what is owed over what is still coming.
    #[test]
    fn the_rate_is_the_share_of_remaining_traffic_still_owed() {
        assert_eq!(steer_rate(25_000.0, 100_000.0), 0.25);
        assert_eq!(steer_rate(0.0, 100_000.0), 0.0);
    }

    /// Owing more than the traffic that remains means taking all of it, never more.
    #[test]
    fn the_rate_never_exceeds_all_of_the_traffic() {
        assert_eq!(steer_rate(500_000.0, 100_000.0), 1.0);
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
