//! Stops a variant whose auth rate falls too far below control, and completes an experiment once
//! it has routed its sample target (`min_sample_size`) of payments. All state is in Redis, shared by every instance,
//! per arm and observation period:
//!
//! - a set of the payments the experiment routed to the arm, added to when it routes them;
//! - HyperLogLogs of those payments with an outcome, and of those that succeeded.
//!
//! An outcome counts only for a payment in its arm's set, whenever its feedback arrives within the
//! keys' TTL. A payment's arm is recomputed from its id. An observation period is identified by
//! when it started, in Unix milliseconds, and a new period starts with empty sets and counts.

use super::arms::{self, ArmSide};
use super::config::{self, AbTestConfig};
use crate::app::get_tenant_app_state;
use crate::euclid::types::{ABTestData, ExperimentEndpoint};
use crate::logger;

// Share of the merchant's sample target that must resolve before the guardrail can stop a
// variant. A safety stop should react long before a significance verdict, but not on the first
// few payments, where one binary outcome swings an arm's rate by tens of points.
const GUARDRAIL_SAMPLE_PCT: u32 = 10;
/// How long an arm's routed payments and counts are kept after their last write.
const COUNTS_TTL_SECS: i64 = 30 * 24 * 60 * 60;

type RedisResult<T> = Result<T, error_stack::Report<redis_interface::errors::RedisError>>;

/// What an outcome requires of the experiment, in the observation period `run`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GuardrailAction {
    /// The variant fell too far below control: stop routing to it.
    StopVariant {
        experiment_id: String,
        variant_id: String,
        run: u64,
    },
    /// The experiment routed its sample target of payments: end it.
    Complete { experiment_id: String, run: u64 },
}

/// Distinct payments with an outcome, and those among them that succeeded, for one arm.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ArmCounts {
    transactions: u64,
    successes: u64,
}

/// An arm's keys in one observation period.
struct ArmKeys {
    /// Set of the payments the experiment routed to the arm.
    routed: String,
    /// HyperLogLog of the routed payments with an outcome.
    transactions: String,
    /// HyperLogLog of the routed payments that succeeded.
    successes: String,
}

/// Every key of one experiment shares the `{merchant_experiment}` hash tag, so they sit in one
/// cluster slot and are read and written in one pipeline.
fn run_key(merchant_id: &str, experiment_id: &str) -> String {
    format!("ab_guardrail_{{{merchant_id}_{experiment_id}}}_run")
}

fn arm_keys(merchant_id: &str, experiment_id: &str, run: u64, side: &ArmSide) -> ArmKeys {
    let prefix = format!(
        "ab_guardrail_{{{merchant_id}_{experiment_id}}}_{run}_{}",
        side.as_str()
    );
    ArmKeys {
        routed: format!("{prefix}_routed"),
        transactions: format!("{prefix}_txns"),
        successes: format!("{prefix}_succ"),
    }
}

/// The payment's arm, when the experiment splits it on any endpoint. The arm is a hash of the
/// payment id over the variants' slices, so every instance computes the one routing used.
fn tracked_side(data: &ABTestData, payment_id: &str) -> Option<ArmSide> {
    ExperimentEndpoint::ALL
        .iter()
        .map(|endpoint| arms::plan(data, *endpoint, payment_id))
        .find(|plan| plan.in_experiment)
        .map(|plan| plan.side)
}

/// Resolved payments, control's plus a variant's, from which the guardrail judges that variant: a
/// tenth of the sample target. The experiment results apply the same minimum.
pub fn min_resolved(min_sample_size: u32) -> u64 {
    u64::from(min_sample_size / GUARDRAIL_SAMPLE_PCT)
}

/// Every arm that can have routed payments: control and each variant, stopped ones included.
fn all_arms(data: &ABTestData) -> Vec<ArmSide> {
    std::iter::once(ArmSide::Control)
        .chain(
            arms::resolved_variants(data)
                .into_iter()
                .map(|variant| ArmSide::Variant(variant.id)),
        )
        .collect()
}

/// The running variants an outcome on `side` can tip: a control outcome moves the baseline every
/// variant is judged against, and a variant outcome moves only that variant.
fn candidates(side: &ArmSide, running: &[String]) -> Vec<String> {
    match side {
        ArmSide::Control => running.to_vec(),
        ArmSide::Variant(id) => running.iter().filter(|r| *r == id).cloned().collect(),
    }
}

/// Whether `variant` fell more than `threshold_pp` below `control`, once both arms have outcomes
/// and together reach `min_resolved`.
fn breached(control: ArmCounts, variant: ArmCounts, threshold_pp: f64, min_resolved: u64) -> bool {
    if control.transactions == 0
        || variant.transactions == 0
        || control.transactions + variant.transactions < min_resolved
    {
        return false;
    }
    let rate =
        |arm: ArmCounts| arm.successes.min(arm.transactions) as f64 / arm.transactions as f64;
    (rate(control) - rate(variant)) * 100.0 > threshold_pp
}

/// Whether every running comparison, control's routed payments plus a variant's, has reached the
/// sample target: the point its results report a verdict at. `routed` holds each arm's routed
/// payments by arm id.
fn reached_sample_target(data: &ABTestData, routed: &[(ArmSide, u64)]) -> bool {
    let routed_to = |side: &ArmSide| {
        routed
            .iter()
            .find(|(arm, _)| arm == side)
            .map_or(0, |(_, count)| *count)
    };
    let control = routed_to(&ArmSide::Control);
    let running = arms::running_variant_ids(data);
    !running.is_empty()
        && running
            .into_iter()
            .all(|id| control + routed_to(&ArmSide::Variant(id)) >= u64::from(data.min_sample_size))
}

/// What the experiment's counts require: completion once every comparison has routed the sample
/// target, otherwise a stop for each of `variants` that breached against `control`.
fn judge(
    data: &ABTestData,
    experiment_id: &str,
    run: u64,
    routed: &[(ArmSide, u64)],
    control: ArmCounts,
    variants: &[(String, ArmCounts)],
) -> Vec<GuardrailAction> {
    if reached_sample_target(data, routed) {
        return vec![GuardrailAction::Complete {
            experiment_id: experiment_id.to_owned(),
            run,
        }];
    }
    let min_resolved = min_resolved(data.min_sample_size);
    variants
        .iter()
        .filter(|(_, counts)| breached(control, *counts, data.guardrail_threshold_pp, min_resolved))
        .map(|(variant_id, _)| GuardrailAction::StopVariant {
            experiment_id: experiment_id.to_owned(),
            variant_id: variant_id.clone(),
            run,
        })
        .collect()
}

/// The experiment's current observation period: its start in Unix milliseconds, or 0 for an
/// experiment activated before periods were recorded.
pub async fn current_run(merchant_id: &str, experiment_id: &str) -> RedisResult<u64> {
    let run = get_tenant_app_state()
        .await
        .redis_conn
        .get_counter(&run_key(merchant_id, experiment_id))
        .await?;
    Ok(run.and_then(|run| u64::try_from(run).ok()).unwrap_or(0))
}

/// Records that the experiment routed the payment to `side` in its current observation period, so
/// the payment's outcome counts whenever it arrives.
pub async fn record_routed(
    merchant_id: String,
    experiment_id: String,
    side: ArmSide,
    payment_id: String,
) {
    if payment_id.is_empty() {
        return;
    }
    let recorded = async {
        let run = current_run(&merchant_id, &experiment_id).await?;
        let keys = arm_keys(&merchant_id, &experiment_id, run, &side);
        get_tenant_app_state()
            .await
            .redis_conn
            .sadd_with_ttl(&keys.routed, &payment_id, COUNTS_TTL_SECS)
            .await
    };
    if let Err(error) = recorded.await {
        logger::warn!(merchant_id = %merchant_id, experiment_id = %experiment_id,
            payment_id = %payment_id, error = ?error,
            "A/B guardrail could not record the routed payment");
    }
}

/// A payment in the merchant's running experiment, with the arm routing assigned it.
pub struct TrackedPayment {
    merchant_id: String,
    payment_id: String,
    experiment_id: String,
    data: ABTestData,
    side: ArmSide,
}

/// The payment, when live-traffic A/B testing is on for the merchant and its running experiment
/// splits the payment on any endpoint.
pub async fn track(merchant_id: &str, payment_id: &str) -> Option<TrackedPayment> {
    if payment_id.is_empty() || !config::is_enabled(merchant_id).await {
        return None;
    }
    let AbTestConfig {
        experiment_id,
        data,
    } = config::load_active_ab_test(merchant_id).await?;
    let side = tracked_side(&data, payment_id)?;
    Some(TrackedPayment {
        merchant_id: merchant_id.to_owned(),
        payment_id: payment_id.to_owned(),
        experiment_id,
        data,
        side,
    })
}

impl TrackedPayment {
    /// Counts the payment's outcome for its arm, if the experiment routed it in the current
    /// observation period, and returns what the counts require. A payment counts once as a
    /// transaction, and once as a success if any of its outcomes succeeded. A required stop or
    /// completion is returned on each outcome until it lands.
    pub async fn record_outcome(&self, is_success: bool) -> Vec<GuardrailAction> {
        match self.count_and_judge(is_success).await {
            Ok(actions) => actions,
            Err(error) => {
                logger::warn!(merchant_id = %self.merchant_id,
                    experiment_id = %self.experiment_id, payment_id = %self.payment_id,
                    error = ?error, "A/B guardrail could not count the outcome");
                Vec::new()
            }
        }
    }

    async fn count_and_judge(&self, is_success: bool) -> RedisResult<Vec<GuardrailAction>> {
        let (merchant_id, experiment_id) = (self.merchant_id.as_str(), self.experiment_id.as_str());
        let redis = &get_tenant_app_state().await.redis_conn;
        let run = current_run(merchant_id, experiment_id).await?;
        let keys = arm_keys(merchant_id, experiment_id, run, &self.side);
        // Payments routed outside the experiment, or before this period, aren't in the set.
        if !redis.sismember(&keys.routed, &self.payment_id).await? {
            return Ok(Vec::new());
        }
        let added = if is_success {
            vec![keys.transactions, keys.successes]
        } else {
            vec![keys.transactions]
        };
        redis
            .pfadd_all_with_ttl(&added, &self.payment_id, COUNTS_TTL_SECS)
            .await?;

        // Control's counts, then each candidate variant's, then every arm's routed payments, in
        // one round trip.
        let candidates = candidates(&self.side, &arms::running_variant_ids(&self.data));
        let judged: Vec<ArmSide> = std::iter::once(ArmSide::Control)
            .chain(candidates.iter().cloned().map(ArmSide::Variant))
            .collect();
        let hyperloglogs: Vec<String> = judged
            .iter()
            .flat_map(|side| {
                let keys = arm_keys(merchant_id, experiment_id, run, side);
                [keys.transactions, keys.successes]
            })
            .collect();
        let every_arm = all_arms(&self.data);
        let sets: Vec<String> = every_arm
            .iter()
            .map(|side| arm_keys(merchant_id, experiment_id, run, side).routed)
            .collect();
        let (hyperloglog_counts, routed_counts) =
            redis.pfcount_and_scard(&hyperloglogs, &sets).await?;
        let mut counts = hyperloglog_counts.chunks(2).map(|pair| ArmCounts {
            transactions: pair.first().copied().unwrap_or(0),
            successes: pair.get(1).copied().unwrap_or(0),
        });
        let control = counts.next().unwrap_or_default();
        let variants: Vec<(String, ArmCounts)> = candidates.into_iter().zip(counts).collect();
        let routed: Vec<(ArmSide, u64)> = every_arm.into_iter().zip(routed_counts).collect();

        let actions = judge(&self.data, experiment_id, run, &routed, control, &variants);
        for action in &actions {
            match action {
                GuardrailAction::StopVariant { variant_id, .. } => {
                    logger::warn!(merchant_id = %merchant_id, experiment_id = %experiment_id,
                        variant_id = %variant_id, threshold_pp = self.data.guardrail_threshold_pp,
                        "A/B guardrail breached; stopping the variant");
                }
                GuardrailAction::Complete { .. } => {
                    logger::info!(merchant_id = %merchant_id, experiment_id = %experiment_id,
                        sample_target = self.data.min_sample_size,
                        "A/B experiment reached its sample target; completing it");
                }
            }
        }
        Ok(actions)
    }
}

/// A deliberate activation or deactivation starts a new observation period, so payments routed
/// and outcomes counted before it no longer judge the experiment.
pub async fn reset(merchant_id: &str, experiment_id: &str) {
    let started_ms =
        i64::try_from(time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000)
            .unwrap_or(i64::MAX);
    if let Err(error) = get_tenant_app_state()
        .await
        .redis_conn
        .set_counter(&run_key(merchant_id, experiment_id), started_ms)
        .await
    {
        logger::warn!(merchant_id = %merchant_id, experiment_id = %experiment_id, error = ?error,
            "A/B guardrail could not start a new observation period");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data() -> ABTestData {
        serde_json::from_value(serde_json::json!({
            "control": { "rule_algorithm_id": "control_rule" },
            "variant": { "rule_algorithm_id": "variant_rule" },
            "variant_split_pct": 20,
            "min_sample_size": 200,
            "guardrail_threshold_pp": 10.0
        }))
        .unwrap()
    }

    fn counts(transactions: u64, successes: u64) -> ArmCounts {
        ArmCounts {
            transactions,
            successes,
        }
    }

    #[test]
    fn breach_needs_both_arms_and_the_minimum_sample() {
        // 80% against 50% is 30pp behind, over the 10pp threshold.
        assert!(breached(counts(10, 8), counts(10, 5), 10.0, 20));
        // Same gap, but only 19 resolved.
        assert!(!breached(counts(10, 8), counts(9, 5), 10.0, 20));
        assert!(!breached(counts(0, 0), counts(30, 0), 10.0, 20));
        assert!(!breached(counts(30, 30), counts(0, 0), 10.0, 20));
        // 80% against 75% is within the threshold.
        assert!(!breached(counts(20, 16), counts(20, 15), 10.0, 20));
        // A variant ahead of control is never breached.
        assert!(!breached(counts(20, 10), counts(20, 20), 10.0, 20));
    }

    #[test]
    fn hyperloglog_overcount_of_successes_is_capped() {
        // PFCOUNT is approximate, so successes can read above transactions.
        assert!(!breached(counts(10, 11), counts(10, 10), 10.0, 20));
    }

    #[test]
    fn control_outcomes_judge_every_running_variant_and_variant_outcomes_only_their_own() {
        let running = vec!["v1".to_string(), "v2".to_string()];
        assert_eq!(candidates(&ArmSide::Control, &running), running);
        assert_eq!(
            candidates(&ArmSide::Variant("v2".into()), &running),
            vec!["v2".to_string()]
        );
        // A stopped variant is no longer judged.
        assert!(candidates(&ArmSide::Variant("v3".into()), &running).is_empty());
    }

    #[test]
    fn judging_stops_breached_variants_until_the_sample_target_completes_the_experiment() {
        // A target of 200 judges a breach from 20 resolved payments on.
        let config = data();
        let variants = [("variant".to_string(), counts(10, 5))];
        let routed = |control, variant| {
            vec![
                (ArmSide::Control, control),
                (ArmSide::Variant("variant".into()), variant),
            ]
        };
        assert_eq!(
            judge(
                &config,
                "exp1",
                7,
                &routed(160, 39),
                counts(10, 8),
                &variants
            ),
            vec![GuardrailAction::StopVariant {
                experiment_id: "exp1".into(),
                variant_id: "variant".into(),
                run: 7,
            }]
        );
        // Having routed its target, the experiment completes, which ends every variant.
        assert_eq!(
            judge(
                &config,
                "exp1",
                7,
                &routed(160, 40),
                counts(10, 8),
                &variants
            ),
            vec![GuardrailAction::Complete {
                experiment_id: "exp1".into(),
                run: 7,
            }]
        );
    }

    #[test]
    fn with_several_variants_every_running_comparison_must_reach_the_target() {
        let mut config = data();
        config.variant = None;
        config.variants = Some(
            serde_json::from_value(serde_json::json!([
                { "id": "a", "split_pct": 20, "arm": { "rule_algorithm_id": "rule_a" } },
                { "id": "b", "split_pct": 20, "arm": { "rule_algorithm_id": "rule_b" } },
                { "id": "c", "split_pct": 10, "stopped": true,
                  "arm": { "rule_algorithm_id": "rule_c" } }
            ]))
            .unwrap(),
        );
        let routed = |b| {
            vec![
                (ArmSide::Control, 150),
                (ArmSide::Variant("a".into()), 60),
                (ArmSide::Variant("b".into()), b),
                (ArmSide::Variant("c".into()), 0),
            ]
        };
        // Control + a has 210 of 200, but control + b only 199; the stopped c isn't waited on.
        assert!(!reached_sample_target(&config, &routed(49)));
        assert!(reached_sample_target(&config, &routed(50)));
    }

    #[test]
    fn the_guardrail_judges_from_a_tenth_of_the_sample_target() {
        assert_eq!(min_resolved(50_000), 5_000);
        assert_eq!(min_resolved(200), 20);
    }

    #[test]
    fn tracked_side_matches_the_arm_routing_assigned() {
        let config = data();
        let variants = arms::resolved_variants(&config);
        for i in 0..200 {
            let payment_id = format!("guardrail_payment_{i}");
            assert_eq!(
                tracked_side(&config, &payment_id),
                Some(super::super::assign_arm(&payment_id, &variants))
            );
        }
    }

    #[test]
    fn a_stopped_variants_payments_are_not_tracked() {
        let mut config = data();
        config.variant = None;
        config.variants = Some(
            serde_json::from_value(serde_json::json!([
                { "id": "v1", "split_pct": 50, "stopped": true,
                  "arm": { "rule_algorithm_id": "variant_rule" } }
            ]))
            .unwrap(),
        );
        let variants = arms::resolved_variants(&config);
        let stopped_payment = (0..1000)
            .map(|i| format!("guardrail_payment_{i}"))
            .find(|id| super::super::assign_arm(id, &variants) == ArmSide::Variant("v1".into()))
            .unwrap();
        assert_eq!(tracked_side(&config, &stopped_payment), None);
        // Its routed payments still count toward the sample target.
        assert_eq!(
            all_arms(&config),
            vec![ArmSide::Control, ArmSide::Variant("v1".into())]
        );
    }

    #[test]
    fn each_period_and_arm_has_its_own_keys() {
        let keys = arm_keys("m1", "exp1", 3, &ArmSide::Variant("v1".into()));
        assert_eq!(keys.routed, "ab_guardrail_{m1_exp1}_3_v1_routed");
        assert_eq!(keys.transactions, "ab_guardrail_{m1_exp1}_3_v1_txns");
        assert_eq!(keys.successes, "ab_guardrail_{m1_exp1}_3_v1_succ");
        assert_eq!(run_key("m1", "exp1"), "ab_guardrail_{m1_exp1}_run");
        assert_ne!(
            arm_keys("m1", "exp1", 4, &ArmSide::Control).routed,
            arm_keys("m1", "exp1", 3, &ArmSide::Control).routed
        );
    }
}
