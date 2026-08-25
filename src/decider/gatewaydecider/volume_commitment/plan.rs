//! Works out which commitments we still chase, and which PSPs need extra volume.

use serde::{Deserialize, Serialize};

/// One PSP's position against its commitment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PspPlan {
    pub connector: String,
    /// What the merchant earns if this commitment is met.
    pub reward: f64,
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
    /// The contract document this plan was built from. Activating a different one leaves the old
    /// plan sitting in the store until the next forecast; comparing this tells a reader the plan
    /// belongs to a contract that is no longer active, rather than showing its stale verdicts.
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

/// Keep the best-rewarding commitments that can still be met with the traffic we have left.
///
/// Two passes. A commitment whose daily need exceeds the whole day's traffic is lost whatever we
/// do, so it is dropped outright — its reward rank must not crowd out commitments that can still
/// land. Whatever remains is ranked by reward and dropped from the tail until both budgets fit,
/// so who survives is predictable straight from the reward column.
///
/// A dropped PSP is not blocked — normal routing still sends it whatever it would have sent.
pub fn choose_commitments_to_keep(
    psps: Vec<PspPlan>,
    traffic_left: f64,
    daily_traffic: f64,
) -> (Vec<PspPlan>, Vec<DroppedPsp>) {
    // Pass 1: the certifiably lost.
    let (unreachable, mut kept): (Vec<PspPlan>, Vec<PspPlan>) = psps
        .into_iter()
        .partition(|psp| psp.needed_daily > daily_traffic);
    let mut dropped: Vec<DroppedPsp> = unreachable
        .into_iter()
        .map(|psp| DroppedPsp {
            reason: format!(
                "needs {:.0} a day but the merchant only expects {:.0} a day in total, \
                 so this commitment cannot be met and is not chased",
                psp.needed_daily, daily_traffic
            ),
            connector: psp.connector,
            remaining: psp.remaining,
            reward: psp.reward,
        })
        .collect();

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

/// Biggest reward first. Same reward falls back to name, so the order never changes randomly.
fn by_reward_desc(a: &PspPlan, b: &PspPlan) -> std::cmp::Ordering {
    b.reward
        .partial_cmp(&a.reward)
        .unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| a.connector.cmp(&b.connector))
}

/// Mark who is short, and order by reward so the biggest wins a contested payment.
pub fn mark_who_needs_steering(psps: &mut [PspPlan]) {
    for psp in psps.iter_mut() {
        psp.needs_steering = psp.routing_gives_daily < psp.needed_daily;
    }

    psps.sort_by(by_reward_desc);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn psp(connector: &str, reward: f64, remaining: f64, needed_daily: f64) -> PspPlan {
        PspPlan {
            connector: connector.to_string(),
            reward,
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

    fn names(psps: &[PspPlan]) -> Vec<&str> {
        psps.iter().map(|p| p.connector.as_str()).collect()
    }

    #[test]
    fn keeps_everything_when_it_all_fits() {
        let psps = vec![
            psp("psp_a", 20_000.0, 4_400_000.0, 244_444.0),
            psp("psp_b", 12_000.0, 3_450_000.0, 191_667.0),
        ];
        let (kept, dropped) = choose_commitments_to_keep(psps, 50_000_000.0, 1_000_000.0);
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
        let (kept, dropped) = choose_commitments_to_keep(psps, 11_160_000.0, 620_000.0);

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
        let (kept, dropped) = choose_commitments_to_keep(psps, 10_000_000.0, 620_000.0);
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
        let (kept, dropped) = choose_commitments_to_keep(psps, 6_000_000.0, 1_000_000.0);
        assert_eq!(names(&kept), ["alpha"]);
        assert_eq!(dropped[0].connector, "zeta");
    }

    /// Nothing fits at all: everything is given up rather than looping forever.
    #[test]
    fn drops_everything_when_nothing_fits() {
        let psps = vec![psp("psp_a", 20_000.0, 9_000_000.0, 500_000.0)];
        let (kept, dropped) = choose_commitments_to_keep(psps, 1_000.0, 1_000.0);
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
        let (kept, dropped) = choose_commitments_to_keep(psps, 11_160_000.0, 620_000.0);

        // Without the pre-pass, psp_doomed's reward rank would keep it and evict both others.
        assert_eq!(names(&kept), ["psp_b", "psp_c"]);
        assert_eq!(dropped.len(), 1);
        assert_eq!(dropped[0].connector, "psp_doomed");
        assert!(dropped[0].reason.contains("cannot be met"));
    }

    #[test]
    fn empty_input_is_no_op() {
        let (kept, dropped) = choose_commitments_to_keep(Vec::new(), 0.0, 0.0);
        assert!(kept.is_empty() && dropped.is_empty());
    }
}
