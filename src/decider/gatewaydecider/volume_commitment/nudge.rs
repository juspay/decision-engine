//! Per-payment decision: a behind-pace PSP takes the payment if the plan is fresh, its cycle is
//! open, it is within tolerance, and it wins a roll against its steer rate — stateless by design.
//! A behind-pace PSP that is already the routing head cannot be steered to (it has the payment),
//! but it can be steered *from*: it rolls first, and a win keeps the payment out of the reach of
//! every lower-reward commitment behind it in the plan.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::plan::{PspPlan, SteeringPlan};

/// Whether this payment was moved.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VolumeSteerOutcome {
    /// A PSP behind on its commitment took the payment.
    Steered,
    /// Normal routing kept the payment.
    SrPrevailed,
}

/// Why a commitment that wanted this payment did not get it.
///
/// A steer rate is a share of the payments a commitment is *allowed* to take, and most payments
/// reach none of them: routing has already chosen the commitment, or never offered it, or the
/// approval gap costs more than the commitment is worth. Without a name for each of those, a plan
/// set to divert most of what it may looks identical to one diverting nothing.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SteerBlock {
    /// Routing had already picked this PSP, so there was nothing to move to it. It keeps the
    /// payment — this is a commitment being served, not one being denied.
    AlreadyChosen,
    /// Routing did not offer this PSP for this payment at all.
    NotOffered,
    /// Its cycle closed; volume sent now would land in the next period.
    CycleClosed,
    /// It approves too much worse than the best PSP for what the commitment is worth. The
    /// document's tolerance is a ceiling; a thin reward affords far less of it.
    OutsideTolerance,
    /// Eligible, but the plan only takes `steer_rate` of these and this one lost the draw.
    LostRoll,
    /// A gate written by a newer version than this one is reading.
    #[serde(other)]
    Unknown,
}

/// One commitment that wanted this payment, and the gate that stopped it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BlockedCommitment {
    pub connector: String,
    pub gate: SteerBlock,
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
    /// Every commitment that wanted this payment and did not get it, with the gate that stopped
    /// it. Empty when nothing was behind, or when the first commitment considered took it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocked: Vec<BlockedCommitment>,
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

    // Why each commitment that wanted this payment did not get it, in the order they were asked.
    let mut blocked: Vec<BlockedCommitment> = Vec::new();
    let block = |connector: &str, gate: SteerBlock| BlockedCommitment {
        connector: connector.to_string(),
        gate,
    };

    // The list is ordered by reward, so the first PSP that passes every check is the best one.
    for psp in plan.needing_steering() {
        // Not in the score map means routing already ruled it out for this payment.
        let Some(&score) = scores.get(&psp.connector) else {
            blocked.push(block(&psp.connector, SteerBlock::NotOffered));
            continue;
        };

        // Routing already picked it, so there is nothing to move *to* it — but it is behind, and
        // the list is ordered by reward, so every commitment still to come is worth less. It keeps
        // the payment outright.
        //
        // Not a roll: `steer_rate` is the share of eligible flow a commitment must *divert* when
        // it is not the head, and a head is already receiving these payments for nothing. Rolling
        // it here would ration a claim that costs nothing to honour, and hand the remainder to a
        // commitment worth less — which is how a 2bps rebate came to take volume from a 3750bps
        // one. Richer commitments are unaffected: they sit earlier in the list and have had their
        // turn before this point.
        if psp.connector == best_psp {
            // Its cycle has closed, so extra volume cannot rescue it: it has no claim left, and
            // the commitments below it may still bid for the payment.
            if now.timestamp_millis() >= psp.period_end_ms {
                blocked.push(block(&psp.connector, SteerBlock::CycleClosed));
                continue;
            }
            blocked.push(block(&psp.connector, SteerBlock::AlreadyChosen));
            return keep_routing_choice_with(
                Some(best_psp.clone()),
                format!(
                    "Kept with {}, which routing already chose and which is behind on its own \
                     volume commitment (worth {:.0}); no lower-reward commitment may steer this \
                     payment away from it.",
                    psp.connector, psp.reward
                ),
                steering_count,
                blocked,
            );
        }

        // The cycle this commitment was owed to has closed; volume sent now counts toward the
        // next one, so there is nothing left to rescue here.
        if now.timestamp_millis() >= psp.period_end_ms {
            blocked.push(block(&psp.connector, SteerBlock::CycleClosed));
            continue;
        }

        // Approves too much worse than the best PSP — or worse than this commitment can afford.
        // Conceding `a` of approval costs about `a * amount` of expected authorized volume, while
        // landing the commitment pays `reward / remaining` of that same amount, so the two are
        // directly comparable. The document's tolerance is the ceiling; reward density is what
        // this commitment is actually worth paying, and for a thin rebate that is far less.
        let approval_given_up = (best_score - score).max(0.0);
        let affordable = plan.tolerance.min(reward_density(psp));
        if approval_given_up > affordable {
            blocked.push(block(&psp.connector, SteerBlock::OutsideTolerance));
            continue;
        }

        // The forecast already decided how much of the eligible flow this PSP should take. All
        // that is left is to roll for it — no counter to read, nothing to write back.
        if roll() >= psp.steer_rate {
            blocked.push(block(&psp.connector, SteerBlock::LostRoll));
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
                    affordable
                ),
                sr_head: Some(best_psp),
                chosen: Some(psp.connector.clone()),
                sr_gap_conceded: Some(approval_given_up),
                steer_rate: Some(psp.steer_rate),
                steering_count,
                run_id: Some(plan.run_id.clone()),
                // The richer commitments asked before this one, and why each missed.
                blocked,
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
    keep_routing_choice_with(Some(best_psp), reason, steering_count, blocked)
}

/// Reward earned per unit of volume still owed — the most approval a commitment can give up on a
/// payment before the steer costs more than the reward it is chasing. Rises as the goal nears,
/// which is right: the last volume before a threshold is what actually wins the reward.
fn reward_density(psp: &PspPlan) -> f64 {
    if psp.remaining <= 0.0 {
        return 0.0;
    }
    psp.reward / psp.remaining
}

/// Leave the payment where normal routing put it.
fn keep_routing_choice(
    best_psp: Option<String>,
    reason: String,
    steering_count: usize,
) -> NudgeOutcome {
    keep_routing_choice_with(best_psp, reason, steering_count, Vec::new())
}

/// As `keep_routing_choice`, carrying the commitments that wanted the payment and why each missed.
fn keep_routing_choice_with(
    best_psp: Option<String>,
    reason: String,
    steering_count: usize,
    blocked: Vec<BlockedCommitment>,
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
            blocked,
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
            goal: 1_000.0,
            achieved: 0.0,
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
            flagged_unreachable: Vec::new(),
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

    /// Every gate has to be named, or a plan diverting most of what it may looks the same as one
    /// diverting nothing — which is exactly how "Steering · 62% of eligible" came to sit beside
    /// zero steered payments with no way to tell why.
    mod gates {
        use super::*;

        fn gate_of(outcome: &NudgeOutcome, connector: &str) -> Option<SteerBlock> {
            outcome
                .info
                .blocked
                .iter()
                .find(|b| b.connector == connector)
                .map(|b| b.gate)
        }

        /// The commitment routing already picked keeps the payment. Nothing was denied it — this
        /// is a commitment being served, and counting it as blocked would read as a failure.
        #[test]
        fn the_routing_head_is_recorded_as_already_chosen() {
            let plan = fresh_plan(vec![psp("behind", 1_000.0, 1.0)], 0.05, noon());
            let scores = scores(&[("behind", 0.95), ("other", 0.90)]);

            let outcome = choose(&scores, &plan, noon(), &mut always());

            assert_eq!(outcome.chosen, None);
            assert_eq!(gate_of(&outcome, "behind"), Some(SteerBlock::AlreadyChosen));
        }

        #[test]
        fn an_approval_gap_wider_than_the_budget_is_recorded_as_tolerance() {
            let plan = fresh_plan(vec![psp("behind", 1_000.0, 1.0)], 0.05, noon());
            let scores = scores(&[("best", 0.95), ("behind", 0.89)]); // gap 0.06 > 0.05

            let outcome = choose(&scores, &plan, noon(), &mut always());

            assert_eq!(gate_of(&outcome, "behind"), Some(SteerBlock::OutsideTolerance));
        }

        /// Eligible in every respect and simply unlucky — the one gate that says the plan is
        /// working and the rate is the only thing holding volume back.
        #[test]
        fn losing_the_draw_is_recorded_as_such() {
            let plan = fresh_plan(vec![psp("behind", 1_000.0, 0.2)], 0.05, noon());
            let scores = scores(&[("best", 0.95), ("behind", 0.92)]);

            let outcome = choose(&scores, &plan, noon(), &mut never());

            assert_eq!(gate_of(&outcome, "behind"), Some(SteerBlock::LostRoll));
        }

        #[test]
        fn a_psp_routing_never_offered_is_recorded_as_not_offered() {
            let plan = fresh_plan(vec![psp("behind", 1_000.0, 1.0)], 0.05, noon());
            let scores = scores(&[("best", 0.95), ("other", 0.90)]);

            let outcome = choose(&scores, &plan, noon(), &mut always());

            assert_eq!(gate_of(&outcome, "behind"), Some(SteerBlock::NotOffered));
        }

        /// A payment that *was* steered still reports the commitments asked before the winner, so
        /// a richer one losing out is visible rather than silent.
        #[test]
        fn a_steered_payment_still_names_who_was_passed_over() {
            let rich = psp("rich", 1_000.0, 1.0);
            // Reward density 0.05 — it can afford the 0.01 gap below; `rich` cannot afford 0.15.
            let mut poor = psp("poor", 50.0, 1.0);
            poor.remaining = 1_000.0;
            let plan = fresh_plan(vec![rich, poor], 0.05, noon());
            // `rich` is far enough off the best score to be unaffordable; `poor` is not.
            let scores = scores(&[("best", 0.95), ("rich", 0.80), ("poor", 0.94)]);

            let outcome = choose(&scores, &plan, noon(), &mut always());

            assert_eq!(outcome.chosen.as_deref(), Some("poor"));
            assert_eq!(gate_of(&outcome, "rich"), Some(SteerBlock::OutsideTolerance));
            assert_eq!(gate_of(&outcome, "poor"), None, "the winner is not blocked");
        }
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

    /// The head is behind on the richer commitment: the payment stays with it,
    /// and the cheaper commitment further down the plan may not take it away.
    #[test]
    fn a_behind_head_keeps_the_payment_from_cheaper_commitments() {
        let plan = fresh_plan(
            vec![psp("adyen", 12_000.0, 0.6), psp("stripe", 2_000.0, 1.0)],
            0.05,
            noon(),
        );
        let scores = scores(&[("adyen", 0.91), ("stripe", 0.91)]);

        let outcome = choose(&scores, &plan, noon(), &mut always());
        assert_eq!(outcome.chosen, None);
        assert_eq!(outcome.info.outcome, VolumeSteerOutcome::SrPrevailed);
        assert_eq!(outcome.info.chosen.as_deref(), Some("adyen"));
        assert!(outcome.info.reason.contains("Kept with adyen"));
    }

    /// A behind head is not rationed by its own steer rate. That rate is the share of flow it
    /// would have to *divert* if it were not the head; payments it already holds cost nothing to
    /// keep, so a low rate must not hand them to a commitment worth less.
    #[test]
    fn a_behind_head_keeps_the_payment_however_the_roll_falls() {
        let plan = fresh_plan(
            vec![psp("adyen", 12_000.0, 0.01), psp("stripe", 2_000.0, 1.0)],
            0.05,
            noon(),
        );
        let scores = scores(&[("adyen", 0.91), ("stripe", 0.91)]);

        // Even on a roll that loses every rate, the cheaper commitment gets nothing.
        let outcome = choose(&scores, &plan, noon(), &mut never());
        assert_eq!(outcome.chosen, None);
        assert_eq!(outcome.info.chosen.as_deref(), Some("adyen"));
        assert!(outcome.info.reason.contains("Kept with adyen"));
    }

    /// A commitment paying 2bps of the volume it still owes must not buy that volume with 1pp of
    /// approval: the steer would cost about fifty times the rebate it is chasing. This is the
    /// trade that let a $12 commitment take payments from a $15,000 one.
    #[test]
    fn a_thin_rebate_will_not_pay_for_approval() {
        let mut thin = psp("thin", 0.0, 1.0);
        thin.remaining = 6_000_000.0;
        thin.reward = 1_200.0; // 2 bps of what is still owed
        let plan = fresh_plan(vec![thin], 0.05, noon());
        let scores = scores(&[("best", 0.95), ("thin", 0.94)]); // gives up 1pp

        let outcome = choose(&scores, &plan, noon(), &mut always());
        assert_eq!(outcome.chosen, None);
        assert_eq!(outcome.info.outcome, VolumeSteerOutcome::SrPrevailed);
    }

    /// The same 1pp is cheap for a commitment whose rebate is worth a large share of the volume,
    /// so a rich commitment still spends the tolerance it is given.
    #[test]
    fn a_rich_rebate_still_pays_the_tolerance() {
        let mut rich = psp("rich", 0.0, 1.0);
        rich.remaining = 4_000_000.0;
        rich.reward = 1_500_000.0; // 3750 bps of what is still owed
        let plan = fresh_plan(vec![rich], 0.05, noon());
        let scores = scores(&[("best", 0.95), ("rich", 0.94)]);

        let outcome = choose(&scores, &plan, noon(), &mut always());
        assert_eq!(outcome.chosen.as_deref(), Some("rich"));
    }

    /// A richer commitment that is *not* the head still outranks the head's own claim: the plan
    /// order decides, and the head only defends against commitments below it.
    #[test]
    fn a_richer_non_head_commitment_still_wins_over_a_behind_head() {
        let plan = fresh_plan(
            vec![psp("richest", 20_000.0, 1.0), psp("head", 12_000.0, 1.0)],
            0.05,
            noon(),
        );
        let scores = scores(&[("head", 0.95), ("richest", 0.92)]);

        let outcome = choose(&scores, &plan, noon(), &mut always());
        assert_eq!(outcome.chosen.as_deref(), Some("richest"));
    }

    /// A behind head whose cycle has closed has nothing to defend; it does not consume a roll.
    #[test]
    fn a_behind_head_past_its_cycle_end_does_not_hold_the_payment() {
        let mut head = psp("head", 12_000.0, 1.0);
        head.period_end_ms = noon().timestamp_millis() - 1;
        let plan = fresh_plan(vec![head, psp("poor", 2_000.0, 1.0)], 0.05, noon());
        let scores = scores(&[("head", 0.91), ("poor", 0.91)]);

        let outcome = choose(&scores, &plan, noon(), &mut always());
        assert_eq!(outcome.chosen.as_deref(), Some("poor"));
    }
}
