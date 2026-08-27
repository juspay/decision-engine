//! Per-payment decision: a behind-pace PSP takes the payment if the plan is fresh, its cycle is
//! open, it is within tolerance, and it wins a roll against its steer rate — stateless by design.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::plan::SteeringPlan;

/// Whether this payment was moved.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VolumeSteerOutcome {
    /// A PSP behind on its commitment took the payment.
    Steered,
    /// Normal routing kept the payment.
    SrPrevailed,
}

/// Why this payment was or was not moved. Shown on the decide response.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct VolumeSteerInfo {
    pub outcome: VolumeSteerOutcome,
    pub reason: String,
    /// The PSP normal routing picked.
    pub sr_head: Option<String>,
    /// The PSP we actually chose. Same as `sr_head` when nothing moved.
    pub chosen: Option<String>,
    /// Approval rate given up to win the volume, as a fraction. None when nothing moved.
    pub sr_gap_conceded: Option<f64>,
    /// The share of eligible payments this PSP was set to receive when the roll happened.
    /// None when nothing moved.
    pub steer_rate: Option<f64>,
    /// How many PSPs needed extra volume when this payment arrived.
    pub steering_count: usize,
    /// The contract execution this steer belongs to, so it files under the right run.
    pub run_id: Option<String>,
}

/// The decision for one payment.
#[derive(Debug, Clone)]
pub struct NudgeOutcome {
    /// Set only when we moved the payment. None means routing keeps its own choice.
    pub chosen: Option<String>,
    /// Backup PSPs to try, best-approving first.
    pub fallbacks: Vec<String>,
    pub info: VolumeSteerInfo,
}

/// Decide whether a PSP behind on its commitment should take this payment. `now` is injected so
/// the windowing is testable; callers pass `Utc::now()`.
pub fn choose(
    scores: &HashMap<String, f64>,
    plan: &SteeringPlan,
    now: DateTime<Utc>,
    roll: &mut impl FnMut() -> f64,
) -> NudgeOutcome {
    let steering_count = plan.needing_steering().count();

    let Some((best_psp, best_score)) = highest_scoring(scores) else {
        return keep_routing_choice(None, "No PSPs to choose from.".to_string(), steering_count);
    };

    // A dead scheduler fails safe: the plan expires and normal routing carries on unaided.
    if plan.is_stale(now.timestamp()) {
        return keep_routing_choice(
            Some(best_psp),
            "The pacing plan has expired without a fresh forecast; normal routing keeps this \
             payment."
                .to_string(),
            steering_count,
        );
    }

    // The list is ordered by reward, so the first PSP that passes every check is the best one.
    for psp in plan.needing_steering() {
        // Not in the score map means routing already ruled it out for this payment.
        let Some(&score) = scores.get(&psp.connector) else {
            continue;
        };

        // Routing already picked it — nothing to move.
        if psp.connector == best_psp {
            continue;
        }

        // The cycle this commitment was owed to has closed; volume sent now counts toward the
        // next one, so there is nothing left to rescue here.
        if now.timestamp_millis() >= psp.period_end_ms {
            continue;
        }

        // Approves too much worse than the best PSP.
        let approval_given_up = (best_score - score).max(0.0);
        if approval_given_up > plan.tolerance {
            continue;
        }

        // The forecast already decided how much of the eligible flow this PSP should take. All
        // that is left is to roll for it — no counter to read, nothing to write back.
        if roll() >= psp.steer_rate {
            continue;
        }

        return NudgeOutcome {
            chosen: Some(psp.connector.clone()),
            fallbacks: others_by_score(scores, &psp.connector),
            info: VolumeSteerInfo {
                outcome: VolumeSteerOutcome::Steered,
                reason: format!(
                    "Sent to {} to help meet its volume commitment (worth {:.0}). It is taking \
                     {:.1}% of eligible payments; gave up {:.4} approval rate versus {}, within \
                     the {:.4} allowed.",
                    psp.connector,
                    psp.reward,
                    psp.steer_rate * 100.0,
                    approval_given_up,
                    best_psp,
                    plan.tolerance
                ),
                sr_head: Some(best_psp),
                chosen: Some(psp.connector.clone()),
                sr_gap_conceded: Some(approval_given_up),
                steer_rate: Some(psp.steer_rate),
                steering_count,
                run_id: Some(plan.run_id.clone()),
            },
        };
    }

    let reason = if steering_count == 0 {
        "Every commitment is on track; normal routing keeps this payment.".to_string()
    } else {
        "No PSP behind on its commitment was both close enough on approval and selected by its \
         steering rate; normal routing keeps this payment."
            .to_string()
    };
    keep_routing_choice(Some(best_psp), reason, steering_count)
}

/// Leave the payment where normal routing put it.
fn keep_routing_choice(
    best_psp: Option<String>,
    reason: String,
    steering_count: usize,
) -> NudgeOutcome {
    NudgeOutcome {
        chosen: None,
        fallbacks: Vec::new(),
        info: VolumeSteerInfo {
            outcome: VolumeSteerOutcome::SrPrevailed,
            reason,
            sr_head: best_psp.clone(),
            chosen: best_psp,
            sr_gap_conceded: None,
            steer_rate: None,
            steering_count,
            run_id: None,
        },
    }
}

/// Highest score first; equal scores fall back to name, so the order is deterministic.
fn by_score_desc(a: &(&String, &f64), b: &(&String, &f64)) -> std::cmp::Ordering {
    b.1.partial_cmp(a.1)
        .unwrap_or(std::cmp::Ordering::Equal)
        .then_with(|| a.0.cmp(b.0))
}

/// The best-approving PSP, ties broken by name for determinism.
fn highest_scoring(scores: &HashMap<String, f64>) -> Option<(String, f64)> {
    scores
        .iter()
        .filter(|(_, score)| score.is_finite())
        .min_by(by_score_desc)
        .map(|(psp, score)| (psp.clone(), *score))
}

/// The remaining PSPs, best-approving first.
fn others_by_score(scores: &HashMap<String, f64>, chosen: &str) -> Vec<String> {
    let mut rest: Vec<(&String, &f64)> = scores.iter().filter(|(psp, _)| *psp != chosen).collect();
    rest.sort_by(by_score_desc);
    rest.into_iter().map(|(psp, _)| psp.clone()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decider::gatewaydecider::volume_commitment::math;
    use crate::decider::gatewaydecider::volume_commitment::plan::PspPlan;

    /// A PSP behind pace, taking `steer_rate` of the eligible flow.
    fn psp(connector: &str, reward: f64, steer_rate: f64) -> PspPlan {
        PspPlan {
            connector: connector.to_string(),
            reward,
            remaining: 1_000.0,
            needed_daily: 200.0,
            routing_gives_daily: 50.0,
            needs_steering: true,
            steer_rate,
            period_start_ms: 0,
            period_end_ms: i64::MAX,
            day_secs: math::SECS_PER_DAY,
        }
    }

    fn fresh_plan(psps: Vec<PspPlan>, tolerance: f64, now: DateTime<Utc>) -> SteeringPlan {
        SteeringPlan {
            merchant_id: "m1".to_string(),
            run_id: "vcr_test".to_string(),
            contract_anchor_ms: 0,
            computed_at_epoch_secs: now.timestamp(),
            stale_after_epoch_secs: now.timestamp() + 3_600,
            tolerance,
            psps,
            dropped: Vec::new(),
        }
    }

    fn scores(entries: &[(&str, f64)]) -> HashMap<String, f64> {
        entries
            .iter()
            .map(|(name, score)| (name.to_string(), *score))
            .collect()
    }

    fn noon() -> DateTime<Utc> {
        "2026-08-20T12:00:00Z".parse().expect("valid instant")
    }

    /// Rolls are injected rather than random, so every test below is deterministic. `always()`
    /// rolls 0.0 — under any positive rate; `never()` rolls 1.0 — under none.
    fn always() -> impl FnMut() -> f64 {
        || 0.0
    }
    fn never() -> impl FnMut() -> f64 {
        || 1.0
    }

    #[test]
    fn steers_a_behind_psp_that_approves_within_tolerance() {
        let plan = fresh_plan(vec![psp("behind", 1_000.0, 0.5)], 0.05, noon());
        let scores = scores(&[("best", 0.95), ("behind", 0.92)]);

        let outcome = choose(&scores, &plan, noon(), &mut always());
        assert_eq!(outcome.chosen.as_deref(), Some("behind"));
        assert_eq!(outcome.info.outcome, VolumeSteerOutcome::Steered);
        assert_eq!(outcome.info.sr_head.as_deref(), Some("best"));
        assert_eq!(outcome.info.steer_rate, Some(0.5));
    }

    #[test]
    fn respects_the_tolerance() {
        let plan = fresh_plan(vec![psp("behind", 1_000.0, 1.0)], 0.05, noon());
        let scores = scores(&[("best", 0.95), ("behind", 0.89)]); // gap 0.06 > 0.05

        // Even at a rate of 1.0, the approval gap keeps this payment where routing put it.
        let outcome = choose(&scores, &plan, noon(), &mut always());
        assert_eq!(outcome.chosen, None);
        assert_eq!(outcome.info.outcome, VolumeSteerOutcome::SrPrevailed);
    }

    /// The roll is what replaces the old daily counter: a payment that loses it stays put.
    #[test]
    fn a_losing_roll_leaves_the_payment_alone() {
        let plan = fresh_plan(vec![psp("behind", 1_000.0, 0.2)], 0.05, noon());
        let scores = scores(&[("best", 0.95), ("behind", 0.92)]);

        let outcome = choose(&scores, &plan, noon(), &mut never());
        assert_eq!(outcome.chosen, None);
    }

    /// A rate of zero means the forecast decided this PSP needs nothing more.
    #[test]
    fn a_zero_rate_never_steers() {
        let plan = fresh_plan(vec![psp("behind", 1_000.0, 0.0)], 0.05, noon());
        let scores = scores(&[("best", 0.95), ("behind", 0.92)]);

        let outcome = choose(&scores, &plan, noon(), &mut always());
        assert_eq!(outcome.chosen, None);
    }

    #[test]
    fn an_expired_plan_steers_nothing() {
        let mut plan = fresh_plan(vec![psp("behind", 1_000.0, 1.0)], 0.05, noon());
        plan.stale_after_epoch_secs = noon().timestamp() - 1;
        let scores = scores(&[("best", 0.95), ("behind", 0.92)]);

        let outcome = choose(&scores, &plan, noon(), &mut always());
        assert_eq!(outcome.chosen, None);
        assert!(outcome.info.reason.contains("expired"));
    }

    /// Past its cycle end a commitment is settled: steering there would credit the next period.
    #[test]
    fn a_closed_cycle_steers_nothing() {
        let mut behind = psp("behind", 1_000.0, 1.0);
        behind.period_end_ms = noon().timestamp_millis() - 1;
        let plan = fresh_plan(vec![behind], 0.05, noon());
        let scores = scores(&[("best", 0.95), ("behind", 0.92)]);

        let outcome = choose(&scores, &plan, noon(), &mut always());
        assert_eq!(outcome.chosen, None);
    }

    /// The plan is reward-ordered; the first PSP passing every check wins a contested payment.
    #[test]
    fn the_higher_reward_wins_a_contested_payment() {
        let plan = fresh_plan(
            vec![psp("rich", 5_000.0, 1.0), psp("poor", 1_000.0, 1.0)],
            0.05,
            noon(),
        );
        let scores = scores(&[("best", 0.95), ("rich", 0.92), ("poor", 0.93)]);

        let outcome = choose(&scores, &plan, noon(), &mut always());
        assert_eq!(outcome.chosen.as_deref(), Some("rich"));
    }

    /// A PSP routing already picked needs no nudge, even when it is behind.
    #[test]
    fn the_sr_head_itself_is_never_nudged() {
        let plan = fresh_plan(vec![psp("behind", 1_000.0, 1.0)], 0.05, noon());
        let scores = scores(&[("behind", 0.95), ("other", 0.90)]);

        let outcome = choose(&scores, &plan, noon(), &mut always());
        assert_eq!(outcome.chosen, None);
        assert_eq!(outcome.info.sr_head.as_deref(), Some("behind"));
    }
}
