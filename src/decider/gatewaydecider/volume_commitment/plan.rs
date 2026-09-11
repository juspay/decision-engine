//! Works out which commitments we still chase, and which PSPs need extra volume.

use serde::{Deserialize, Serialize};

use super::inputs::TrafficRates;
use super::math;

/// One PSP's position against its commitment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PspPlan {
    pub connector: String,
    /// What the merchant earns if this commitment is met.
    pub reward: f64,
    /// What this commitment promises over the whole cycle.
    #[serde(default)]
    pub goal: f64,
    /// What has landed on it so far this cycle.
    #[serde(default)]
    pub achieved: f64,
    /// Volume still owed.
    pub remaining: f64,
    /// Volume needed each day from now on.
    pub needed_daily: f64,
    /// Volume normal routing already sends here each day.
    pub routing_gives_daily: f64,
    /// True when normal routing is not sending enough — only these PSPs get extra volume.
    pub needs_steering: bool,
    /// Share of eligible payments to divert here, 0..=1. Recomputed every forecast from what has
    /// actually been delivered, which is what lets the payment path decide without counting.
    pub steer_rate: f64,
    /// Instant this commitment's cycle opened — contract days are counted from here.
    pub period_start_ms: i64,
    /// Instant it closes. Past this the commitment is settled, and no payment may be steered to
    /// it: the volume would land in the next cycle, not the one it was owed to.
    pub period_end_ms: i64,
    /// How long one contract day lasts, so the nudge paces on the same unit the plan does.
    pub day_secs: u64,
}

/// A commitment we stopped chasing, and why.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DroppedPsp {
    pub connector: String,
    pub remaining: f64,
    pub reward: f64,
    pub reason: String,
}

/// What the background loop hands to the routing path. Rebuilt each tick, read on every payment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SteeringPlan {
    pub merchant_id: String,
    /// Which execution of the contract this plan belongs to — one cycle, start to close. Every
    /// forecast, steer and elimination it produces is filed under this.
    pub run_id: String,
    /// Anchor of the contract this plan was built from; a mismatch means the plan is for a replaced contract.
    pub contract_anchor_ms: i64,
    pub computed_at_epoch_secs: i64,
    /// Epoch second after which this plan may no longer steer — a dead scheduler fails safe.
    pub stale_after_epoch_secs: i64,
    /// How much approval rate we are willing to give up to win volume, as a fraction.
    pub tolerance: f64,
    /// Commitments we are still chasing, best reward first.
    pub psps: Vec<PspPlan>,
    /// Commitments we gave up on.
    pub dropped: Vec<DroppedPsp>,
    /// Commitments that read unreachable on this forecast, whether or not they were dropped for
    /// it. A drop is only recorded when the verdict repeats, so this is what the next forecast
    /// checks against — see `compute_plan`.
    #[serde(default)]
    pub flagged_unreachable: Vec<String>,
    /// Commitments already dropped that this forecast read as clearly reachable again. Reversing
    /// a drop takes two agreeing forecasts, exactly as ordering one does, so this is what the
    /// next forecast checks a recovery against.
    #[serde(default)]
    pub flagged_reachable: Vec<String>,
}

impl SteeringPlan {
    /// The PSPs that currently need extra volume.
    pub fn needing_steering(&self) -> impl Iterator<Item = &PspPlan> {
        self.psps.iter().filter(|p| p.needs_steering)
    }

    /// True once the plan has outlived its forecast cadence and must stop steering.
    pub fn is_stale(&self, now_epoch_secs: i64) -> bool {
        now_epoch_secs > self.stale_after_epoch_secs
    }
}

/// Drop the unreachable, then rank by reward and shed the tail until period and daily budgets fit;
/// dropped PSPs still get normal traffic.
pub fn choose_commitments_to_keep(
    psps: Vec<PspPlan>,
    traffic: TrafficRates,
    days_left: f64,
) -> (Vec<PspPlan>, Vec<DroppedPsp>) {
    // The period budget is the daily rate over the horizon, derived here rather than passed in:
    // handed the two separately, a caller can pass a total that no longer matches the rate it was
    // computed from, and the pass then sheds commitments against a budget that never existed.
    let traffic_left = traffic.left_over(days_left);
    let daily_traffic = traffic.forward_daily;

    // Pass 1: the certifiably lost.
    let (mut kept, mut dropped) = drop_unreachable(psps, traffic);

    // Pass 2: reward-ranked, giving up the tail until the period and daily budgets both fit.
    kept.sort_by(by_reward_desc);
    let mut total_remaining: f64 = kept.iter().map(|p| p.remaining).sum();
    let mut total_daily: f64 = kept.iter().map(|p| p.needed_daily).sum();

    let mut budget_dropped = Vec::new();
    while total_remaining > traffic_left || total_daily > daily_traffic {
        let Some(psp) = kept.pop() else { break };
        total_remaining -= psp.remaining;
        total_daily -= psp.needed_daily;
        budget_dropped.push(DroppedPsp {
            reason: format!(
                "needs {:.0} more volume for a reward of only {:.0} — the lowest-ranked \
                 commitment still standing when the traffic ran short, so it was given up to \
                 leave room for the ones worth more",
                psp.remaining, psp.reward
            ),
            connector: psp.connector,
            remaining: psp.remaining,
            reward: psp.reward,
        });
    }

    // Popped cheapest-first; report best-reward-first, matching the kept list.
    budget_dropped.reverse();
    dropped.extend(budget_dropped);
    (kept, dropped)
}

/// Drop only commitments whose daily need exceeds total daily traffic — measured traffic where
/// there is any, else the contract's declared figure. Used alone through the first contract day,
/// when the budget pass would be noise.
pub fn drop_unreachable(
    psps: Vec<PspPlan>,
    traffic: TrafficRates,
) -> (Vec<PspPlan>, Vec<DroppedPsp>) {
    // Feasibility asks what will arrive before the cycle closes, never how big the merchant is.
    let daily_traffic = traffic.forward_daily;
    let (unreachable, kept): (Vec<PspPlan>, Vec<PspPlan>) = psps
        .into_iter()
        .partition(|psp| psp.needed_daily > daily_traffic);
    let dropped: Vec<DroppedPsp> = unreachable
        .into_iter()
        .map(|psp| DroppedPsp {
            reason: format!(
                "needs {:.0} a day but only {:.0} a day is flowing across all PSPs, \
                 so this commitment cannot be met and is not chased",
                psp.needed_daily, daily_traffic
            ),
            connector: psp.connector,
            remaining: psp.remaining,
            reward: psp.reward,
        })
        .collect();
    (kept, dropped)
}

/// Biggest reward first. Same reward falls back to name, so the order never changes randomly.
fn by_reward_desc(a: &PspPlan, b: &PspPlan) -> std::cmp::Ordering {
    b.reward
        .partial_cmp(&a.reward)
        .unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| a.connector.cmp(&b.connector))
}

impl PspPlan {
    /// Volume each remaining day must bring for this commitment to land inside the cycle's
    /// finishing margin — the one answer to "what does this need right now".
    ///
    /// Read by both halves of the decision: whether the commitment is behind, and what share of
    /// the flow steering then takes. They were separate sums over the same quantities, and a
    /// margin added to one and not the other is a commitment steered at a rate that does not
    /// match the urgency it was marked with.
    ///
    /// Higher than `needed_daily`, which paces to the cycle's closing instant and is what the
    /// plan reports and feasibility is judged on. Nothing is written off for needing more than
    /// this asks — see `math::pacing_days_left` for why the margin exists.
    pub fn paced_daily_need(&self, now_ms: i64) -> f64 {
        math::needed_daily(
            self.remaining,
            math::pacing_days_left(
                math::days_left(self.period_end_ms, now_ms, self.day_secs),
                math::cycle_days(self.period_start_ms, self.period_end_ms, self.day_secs),
            ),
        )
    }

    /// Of that need, what steering has to add: the rest routing delivers unaided.
    pub fn steering_shortfall_daily(&self, now_ms: i64) -> f64 {
        math::daily_shortfall(self.paced_daily_need(now_ms), self.routing_gives_daily)
    }

    /// Share of the divertible flow to take, 0..=1 — what the payment path rolls against.
    ///
    /// The denominator is the flow left in *this contract day*, so a rate set early in a day and
    /// read late in it still asks for the whole of what the day still owes; `already_today` is
    /// what steering has delivered since the day opened, which is how the loop closes without a
    /// shared counter.
    pub fn steer_rate_for(&self, now_ms: i64, already_today: f64, divertible_daily: f64) -> f64 {
        let day_ms = math::day_ms(self.day_secs) as f64;
        let elapsed_in_day = ((now_ms - self.period_start_ms).max(0) as f64) % day_ms;
        let day_remaining = ((day_ms - elapsed_in_day) / day_ms).clamp(0.0, 1.0);
        math::steer_rate(
            (self.steering_shortfall_daily(now_ms) - already_today).max(0.0),
            divertible_daily * day_remaining,
        )
    }

    /// Volume the promise expects to have landed by `now_ms`, pacing evenly across the cycle.
    fn promised_by(&self, now_ms: i64) -> f64 {
        let span = (self.period_end_ms - self.period_start_ms).max(1);
        let elapsed = (now_ms - self.period_start_ms).clamp(0, span);
        self.goal * (elapsed as f64 / span as f64)
    }

    /// Whether a whole contract day has passed, so `routing_gives_daily` is a rate rather than a
    /// fraction of one presented as a rate.
    fn rate_is_measurable(&self, now_ms: i64) -> bool {
        now_ms.saturating_sub(self.period_start_ms) >= math::day_ms(self.day_secs)
    }

    /// Whether normal routing is leaving this commitment short.
    ///
    /// The settled answer is the forward-looking rate test: at what routing sends here unaided,
    /// does the remainder arrive in time? "In time" is `paced_daily_need` — the same target the
    /// steer rate is a share of, so being marked behind and being steered hard enough to fix it
    /// are one decision.
    ///
    /// Inside the first contract day it cannot be asked. `routing_gives_daily` is measured over
    /// the window since the cycle opened and divided by a *whole* contract day — the divisor has
    /// a floor of one, because a smaller one would turn a few seconds of traffic into an enormous
    /// extrapolated rate. The cost is the other direction: minutes of real traffic are reported as
    /// a day's worth, so every commitment reads as starved however busy the merchant is, and the
    /// engine spends approval-rate tolerance closing a gap that does not exist. It corrects itself
    /// within the day, which is what made it easy to miss.
    ///
    /// So until there is a day to divide by, ask what the evidence can answer: has less arrived
    /// than the promise expects by now? That still catches a commitment receiving nothing — its
    /// achieved stays at zero while the promise's expectation climbs — without inventing a
    /// shortfall for one that is merely young.
    fn is_behind(&self, now_ms: i64) -> bool {
        if self.rate_is_measurable(now_ms) {
            self.steering_shortfall_daily(now_ms) > 0.0
        } else {
            self.achieved < self.promised_by(now_ms)
        }
    }
}

/// Mark who is short, and order by reward so the biggest wins a contested payment.
pub fn mark_who_needs_steering(psps: &mut [PspPlan], now_ms: i64) {
    for psp in psps.iter_mut() {
        psp.needs_steering = psp.is_behind(now_ms);
    }

    psps.sort_by(by_reward_desc);
}

#[cfg(test)]
mod steering_marks {
    use super::*;

    const DAY_MS: i64 = math::SECS_PER_DAY as i64 * 1_000;

    /// A commitment on a ten-contract-day cycle opening at epoch zero.
    fn paced(goal: f64, achieved: f64, routing_gives_daily: f64, needed_daily: f64) -> PspPlan {
        PspPlan {
            connector: "adyen".to_string(),
            reward: 100.0,
            goal,
            achieved,
            remaining: math::remaining(goal, achieved),
            needed_daily,
            routing_gives_daily,
            needs_steering: false,
            steer_rate: 0.0,
            period_start_ms: 0,
            period_end_ms: 10 * DAY_MS,
            day_secs: math::SECS_PER_DAY,
        }
    }

    /// The case that spent approval-rate tolerance for nothing. Seven tenths into the first
    /// contract day, `routing_gives_daily` is the traffic so far divided by a *whole* day, so a
    /// PSP running four times ahead of its promise still reads as delivering almost none of it.
    #[test]
    fn a_busy_psp_is_not_steered_to_inside_its_first_contract_day() {
        let now = DAY_MS * 7 / 10;
        // $20k has landed against the $4.55k the promise expects by now — comfortably ahead. The
        // measured rate says otherwise only because it is not yet a rate.
        let mut psps = vec![paced(65_000.0, 20_000.0, 900.0, 6_500.0)];
        mark_who_needs_steering(&mut psps, now);
        assert!(!psps[0].needs_steering);
    }

    /// Genuine starvation still shows inside the first day: nothing has arrived while the promise
    /// expects some. Suppressing the rate test must not suppress the engine.
    #[test]
    fn a_psp_receiving_nothing_is_steered_to_inside_its_first_contract_day() {
        let now = DAY_MS * 7 / 10;
        let mut psps = vec![paced(65_000.0, 0.0, 0.0, 6_500.0)];
        mark_who_needs_steering(&mut psps, now);
        assert!(psps[0].needs_steering);
    }

    /// Once a contract day has passed the rate governs, and it looks forward: a PSP ahead on
    /// cumulative volume whose traffic has dried up is still short of what the rest needs. The
    /// cumulative test alone would not catch this, which is why it does not replace the rate one.
    #[test]
    fn the_rate_test_governs_once_a_contract_day_has_passed() {
        let now = DAY_MS * 3;
        // $25k against the $19.5k expected by day three — ahead. But routing now sends a tenth of
        // the $6.5k a day the remainder needs.
        let mut psps = vec![paced(65_000.0, 25_000.0, 650.0, 6_500.0)];
        mark_who_needs_steering(&mut psps, now);
        assert!(psps[0].needs_steering);
    }

    #[test]
    fn a_psp_routing_already_covers_is_left_alone() {
        let now = DAY_MS * 3;
        let mut psps = vec![paced(65_000.0, 25_000.0, 9_000.0, 6_500.0)];
        mark_who_needs_steering(&mut psps, now);
        assert!(!psps[0].needs_steering);
    }

    /// The switch is the first contract day, not the first day of the axis: a commitment whose own
    /// cycle opened later is still inside its first day when older ones are past theirs.
    #[test]
    fn the_switch_follows_each_commitments_own_cycle() {
        let now = DAY_MS * 3;
        let mut late = paced(65_000.0, 20_000.0, 900.0, 6_500.0);
        late.period_start_ms = DAY_MS * 3 - DAY_MS / 2;
        late.period_end_ms = late.period_start_ms + 10 * DAY_MS;
        let mut psps = vec![late];
        mark_who_needs_steering(&mut psps, now);
        assert!(
            !psps[0].needs_steering,
            "half a day in, the rate is not a rate"
        );
    }

    /// The last day of the cycle, with the goal within reach: the rate takes everything it may.
    /// Paced to the cycle's closing instant instead, this commitment asked for a third of the
    /// flow, lost the rolls and finished at 99% of its goal — which pays nothing.
    #[test]
    fn the_final_margin_takes_every_payment_it_can() {
        let now = DAY_MS * 9;
        // $67k of $70k delivered, $3k owed, routing sending $2.4k a day of its own.
        let psp = paced(70_000.0, 67_000.0, 2_400.0, 3_000.0);
        assert!(psp.paced_daily_need(now) > 3_000.0);
        assert_eq!(psp.steer_rate_for(now, 0.0, 7_000.0), 1.0);
    }

    /// Mid-cycle it stays a share, not a sweep: the margin tightens the pace, it does not turn
    /// every commitment urgent.
    #[test]
    fn mid_cycle_the_rate_is_still_a_share() {
        let now = DAY_MS * 5;
        let psp = paced(70_000.0, 35_000.0, 2_400.0, 7_000.0);
        let rate = psp.steer_rate_for(now, 0.0, 7_000.0);
        assert!(rate > 0.0 && rate < 1.0, "rate was {rate}");
    }

    /// Volume already steered this contract day is what the rate is net of — the loop closes on
    /// the measurement rather than on a counter the payment path would have to keep.
    #[test]
    fn the_rate_is_net_of_what_today_already_delivered() {
        let now = DAY_MS * 5;
        let psp = paced(70_000.0, 35_000.0, 2_400.0, 7_000.0);
        let fresh = psp.steer_rate_for(now, 0.0, 7_000.0);
        let after = psp.steer_rate_for(now, 3_000.0, 7_000.0);
        assert!(after < fresh);
        // Today's share already delivered: nothing more is owed until tomorrow.
        assert_eq!(psp.steer_rate_for(now, 1_000_000.0, 7_000.0), 0.0);
    }

    /// Being marked behind and being steered hard enough to fix it read the same target: a PSP
    /// routing already covers is left alone *and* asks for nothing.
    #[test]
    fn the_mark_and_the_rate_agree() {
        let now = DAY_MS * 3;
        let covered = paced(65_000.0, 25_000.0, 9_000.0, 6_500.0);
        let mut psps = vec![covered.clone()];
        mark_who_needs_steering(&mut psps, now);
        assert!(!psps[0].needs_steering);
        assert_eq!(covered.steer_rate_for(now, 0.0, 7_000.0), 0.0);

        let starved = paced(65_000.0, 25_000.0, 650.0, 6_500.0);
        let mut psps = vec![starved.clone()];
        mark_who_needs_steering(&mut psps, now);
        assert!(psps[0].needs_steering);
        assert!(starved.steer_rate_for(now, 0.0, 7_000.0) > 0.0);
    }

    /// The richest commitment sorts first, so it wins a payment two of them could take.
    #[test]
    fn the_marker_orders_by_reward() {
        let now = DAY_MS * 3;
        let mut cheap = paced(65_000.0, 0.0, 0.0, 6_500.0);
        cheap.connector = "stripe".to_string();
        cheap.reward = 10.0;
        let rich = paced(65_000.0, 0.0, 0.0, 6_500.0);
        let mut psps = vec![cheap, rich];
        mark_who_needs_steering(&mut psps, now);
        assert_eq!(psps[0].connector, "adyen");
        assert!(psps.iter().all(|p| p.needs_steering));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn psp(connector: &str, reward: f64, remaining: f64, needed_daily: f64) -> PspPlan {
        PspPlan {
            connector: connector.to_string(),
            reward,
            // A goal already delivered in full, so the cumulative cold-start test never fires and
            // these cases exercise the rate test they were written for.
            goal: remaining,
            achieved: remaining,
            remaining,
            needed_daily,
            routing_gives_daily: 0.0,
            needs_steering: false,
            steer_rate: 0.0,
            period_start_ms: 0,
            period_end_ms: i64::MAX,
            day_secs: crate::decider::gatewaydecider::volume_commitment::math::SECS_PER_DAY,
        }
    }

    /// A merchant flowing `forward_daily` a day, which is the only rate feasibility reads.
    fn flowing(forward_daily: f64) -> TrafficRates {
        TrafficRates {
            typical_daily: forward_daily,
            forward_daily,
        }
    }

    fn names(psps: &[PspPlan]) -> Vec<&str> {
        psps.iter().map(|p| p.connector.as_str()).collect()
    }

    /// The first-day pass never trades on reward: an over-promised pair both survive it, and only
    /// a commitment that is unreachable on its own is dropped.
    #[test]
    fn the_first_day_pass_drops_only_the_unreachable() {
        let psps = vec![
            psp("adyen", 13_000.0, 900_000.0, 300_000.0),
            psp("stripe", 5_000.0, 1_150_000.0, 380_000.0),
            psp("checkout", 1_000.0, 3_000_000.0, 750_000.0),
        ];
        let (kept, dropped) = drop_unreachable(psps, flowing(500_000.0));
        assert_eq!(names(&kept), vec!["adyen", "stripe"]);
        assert_eq!(names_dropped(&dropped), vec!["checkout"]);
    }

    fn names_dropped(psps: &[DroppedPsp]) -> Vec<&str> {
        psps.iter().map(|p| p.connector.as_str()).collect()
    }

    #[test]
    fn keeps_everything_when_it_all_fits() {
        let psps = vec![
            psp("psp_a", 20_000.0, 4_400_000.0, 244_444.0),
            psp("psp_b", 12_000.0, 3_450_000.0, 191_667.0),
        ];
        let (kept, dropped) = choose_commitments_to_keep(psps, flowing(1_000_000.0), 50.0);
        assert_eq!(names(&kept), ["psp_a", "psp_b"]);
        assert!(dropped.is_empty());
    }

    /// Four PSPs with 18 days left at 620k/day: 11.16M of traffic against 13.4M of commitments.
    /// The weakest reward is the one to go.
    #[test]
    fn drops_the_weakest_reward_first() {
        let psps = vec![
            psp("psp_a", 20_000.0, 4_400_000.0, 244_444.0),
            psp("psp_b", 12_000.0, 3_450_000.0, 191_667.0),
            psp("psp_c", 6_000.0, 3_050_000.0, 169_444.0),
            psp("psp_d", 15_000.0, 2_500_000.0, 138_889.0),
        ];
        let (kept, dropped) = choose_commitments_to_keep(psps, flowing(620_000.0), 18.0);

        assert_eq!(names(&kept), ["psp_a", "psp_d", "psp_b"]);
        assert_eq!(dropped.len(), 1);
        assert_eq!(dropped[0].connector, "psp_c");
        assert_eq!(dropped[0].reward, 6_000.0);
    }

    /// Only the day's traffic is short — the period total fits. That alone must force a drop.
    #[test]
    fn daily_traffic_alone_can_force_a_drop() {
        let psps = vec![
            psp("psp_a", 20_000.0, 100.0, 400_000.0),
            psp("psp_b", 5_000.0, 100.0, 400_000.0),
        ];
        let (kept, dropped) = choose_commitments_to_keep(psps, flowing(620_000.0), 16.13);
        assert_eq!(names(&kept), ["psp_a"]);
        assert_eq!(dropped.len(), 1);
        assert_eq!(dropped[0].connector, "psp_b");
    }

    /// Equal rewards must not resolve differently from one run to the next.
    #[test]
    fn equal_rewards_break_by_name() {
        let psps = vec![
            psp("zeta", 10_000.0, 6_000_000.0, 100.0),
            psp("alpha", 10_000.0, 6_000_000.0, 100.0),
        ];
        let (kept, dropped) = choose_commitments_to_keep(psps, flowing(1_000_000.0), 6.0);
        assert_eq!(names(&kept), ["alpha"]);
        assert_eq!(dropped[0].connector, "zeta");
    }

    /// Nothing fits at all: everything is given up rather than looping forever.
    #[test]
    fn drops_everything_when_nothing_fits() {
        let psps = vec![psp("psp_a", 20_000.0, 9_000_000.0, 500_000.0)];
        let (kept, dropped) = choose_commitments_to_keep(psps, flowing(1_000.0), 1.0);
        assert!(kept.is_empty());
        assert_eq!(dropped.len(), 1);
    }

    /// An impossible commitment must not ride its reward rank: it is lost either way, and keeping
    /// it would push out commitments that can still be landed.
    #[test]
    fn an_unreachable_commitment_cannot_crowd_out_achievable_ones() {
        let psps = vec![
            // Highest reward, but needs 900k/day against 620k/day of total traffic: hopeless.
            psp("psp_doomed", 50_000.0, 9_000_000.0, 900_000.0),
            psp("psp_b", 12_000.0, 3_450_000.0, 191_667.0),
            psp("psp_c", 6_000.0, 3_050_000.0, 169_444.0),
        ];
        let (kept, dropped) = choose_commitments_to_keep(psps, flowing(620_000.0), 18.0);

        // Without the pre-pass, psp_doomed's reward rank would keep it and evict both others.
        assert_eq!(names(&kept), ["psp_b", "psp_c"]);
        assert_eq!(dropped.len(), 1);
        assert_eq!(dropped[0].connector, "psp_doomed");
        assert!(dropped[0].reason.contains("cannot be met"));
    }

    #[test]
    fn empty_input_is_no_op() {
        let (kept, dropped) = choose_commitments_to_keep(Vec::new(), flowing(0.0), 0.0);
        assert!(kept.is_empty() && dropped.is_empty());
    }
}
