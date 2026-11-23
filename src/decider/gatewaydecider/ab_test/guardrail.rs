//! Stops a variant whose auth rate falls too far below control. Outcomes are counted in Redis
//! HyperLogLogs shared by every instance, one pair per arm and observation period, so feedback is
//! counted whichever instance receives it and however late it arrives within the keys' TTL. A
//! payment's arm is recomputed from its id, so nothing is stored per payment. An observation
//! period is identified by when it started, in Unix milliseconds, and a payment routed before
//! then is not counted in it.

use super::arms::{self, ArmSide};
use super::config::{self, AbTestConfig};
use crate::app::get_tenant_app_state;
use crate::euclid::types::{ABTestData, ExperimentEndpoint};
use crate::logger;

// Share of the merchant's sample target that must resolve before the guardrail can stop a
// variant. A safety stop should react long before a significance verdict, but not on the first
// few payments, where one binary outcome swings an arm's rate by tens of points.
const GUARDRAIL_SAMPLE_PCT: u32 = 10;
/// How long an arm's counts are kept after its last outcome.
const COUNTS_TTL_SECS: i64 = 30 * 24 * 60 * 60;

/// One breached variant and the observation period it breached in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreachedRun {
    pub experiment_id: String,
    pub variant_id: String,
    pub run: u64,
}

/// Distinct payments with an outcome, and those among them that succeeded, for one arm.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ArmCounts {
    transactions: u64,
    successes: u64,
}

/// The start of the experiment's current observation period, in Unix milliseconds.
fn run_key(merchant_id: &str, experiment_id: &str) -> String {
    format!("ab_guardrail_run_{merchant_id}_{experiment_id}")
}

/// The HyperLogLogs of one arm in one observation period: (transactions, successes).
fn arm_keys(merchant_id: &str, experiment_id: &str, run: u64, side: &ArmSide) -> (String, String) {
    let prefix = format!(
        "ab_guardrail_{merchant_id}_{experiment_id}_{run}_{}",
        side.as_str()
    );
    (format!("{prefix}_txns"), format!("{prefix}_succ"))
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

/// The running variants an outcome on `side` can tip: a control outcome moves the baseline every
/// variant is judged against, and a variant outcome moves only that variant.
fn candidates(side: &ArmSide, running: &[String]) -> Vec<String> {
    match side {
        ArmSide::Control => running.to_vec(),
        ArmSide::Variant(id) => running.iter().filter(|r| *r == id).cloned().collect(),
    }
}

/// Whether a payment routed at `routed_at` predates the period that started at `run_started_ms`.
/// A payment without a scoring context has no routing time and is counted.
fn routed_before(routed_at: Option<time::OffsetDateTime>, run_started_ms: u64) -> bool {
    routed_at.is_some_and(|at| at.unix_timestamp_nanos() / 1_000_000 < i128::from(run_started_ms))
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

/// The experiment's current observation period: its start in Unix milliseconds, or 0 for an
/// experiment activated before periods were recorded.
pub async fn current_run(
    merchant_id: &str,
    experiment_id: &str,
) -> Result<u64, error_stack::Report<redis_interface::errors::RedisError>> {
    let run = get_tenant_app_state()
        .await
        .redis_conn
        .get_counter(&run_key(merchant_id, experiment_id))
        .await?;
    Ok(run.and_then(|run| u64::try_from(run).ok()).unwrap_or(0))
}

async fn arm_counts(
    merchant_id: &str,
    experiment_id: &str,
    run: u64,
    side: &ArmSide,
) -> Result<ArmCounts, error_stack::Report<redis_interface::errors::RedisError>> {
    let redis = &get_tenant_app_state().await.redis_conn;
    let (txns, succ) = arm_keys(merchant_id, experiment_id, run, side);
    Ok(ArmCounts {
        transactions: redis.pfcount(&txns).await?,
        successes: redis.pfcount(&succ).await?,
    })
}

/// Counts the payment's outcome for its arm and returns the variants needing a durable stop. A
/// payment counts once as a transaction, and once as a success if any of its outcomes succeeded.
/// A breached variant is returned on each outcome until its stop lands.
pub async fn record_outcome(
    merchant_id: &str,
    payment_id: &str,
    is_success: bool,
) -> Vec<BreachedRun> {
    if payment_id.is_empty() || !config::is_enabled(merchant_id).await {
        return Vec::new();
    }
    let Some(AbTestConfig {
        experiment_id,
        data,
    }) = config::load_active_ab_test(merchant_id).await
    else {
        return Vec::new();
    };
    let Some(side) = tracked_side(&data, payment_id) else {
        return Vec::new();
    };
    match count_and_judge(
        merchant_id,
        &experiment_id,
        &data,
        payment_id,
        &side,
        is_success,
    )
    .await
    {
        Ok(breached) => breached,
        Err(error) => {
            logger::warn!(merchant_id = %merchant_id, experiment_id = %experiment_id,
                payment_id = %payment_id, error = ?error,
                "A/B guardrail could not count the outcome");
            Vec::new()
        }
    }
}

async fn count_and_judge(
    merchant_id: &str,
    experiment_id: &str,
    data: &ABTestData,
    payment_id: &str,
    side: &ArmSide,
    is_success: bool,
) -> Result<Vec<BreachedRun>, error_stack::Report<redis_interface::errors::RedisError>> {
    let redis = &get_tenant_app_state().await.redis_conn;
    let run = current_run(merchant_id, experiment_id).await?;
    if run > 0 {
        let routed_at =
            crate::feedback::gateway_scoring_service::routed_at(merchant_id, payment_id).await;
        if routed_before(routed_at, run) {
            return Ok(Vec::new());
        }
    }
    let (txns, succ) = arm_keys(merchant_id, experiment_id, run, side);
    redis
        .pfadd_with_ttl(&txns, payment_id, COUNTS_TTL_SECS)
        .await?;
    if is_success {
        redis
            .pfadd_with_ttl(&succ, payment_id, COUNTS_TTL_SECS)
            .await?;
    }

    let candidates = candidates(side, &arms::running_variant_ids(data));
    if candidates.is_empty() {
        return Ok(Vec::new());
    }
    let min_resolved = u64::from(data.min_sample_size / GUARDRAIL_SAMPLE_PCT);
    let control = arm_counts(merchant_id, experiment_id, run, &ArmSide::Control).await?;
    let mut breached_runs = Vec::new();
    for variant_id in candidates {
        let variant = ArmSide::Variant(variant_id.clone());
        let counts = arm_counts(merchant_id, experiment_id, run, &variant).await?;
        if breached(control, counts, data.guardrail_threshold_pp, min_resolved) {
            logger::warn!(merchant_id = %merchant_id, experiment_id = %experiment_id,
                variant_id = %variant_id, threshold_pp = data.guardrail_threshold_pp,
                control_transactions = control.transactions, control_successes = control.successes,
                variant_transactions = counts.transactions, variant_successes = counts.successes,
                "A/B guardrail breached; stopping the variant");
            breached_runs.push(BreachedRun {
                experiment_id: experiment_id.to_owned(),
                variant_id,
                run,
            });
        }
    }
    Ok(breached_runs)
}

/// A deliberate activation or deactivation starts a new observation period, so outcomes counted
/// before it no longer judge the experiment.
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
            "min_sample_size": 40,
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
    }

    #[test]
    fn payments_routed_before_the_period_are_not_counted() {
        let started = time::macros::datetime!(2026-11-20 10:00 UTC);
        let started_ms = u64::try_from(started.unix_timestamp() * 1000).unwrap();
        let before = started - time::Duration::milliseconds(1);
        assert!(routed_before(Some(before), started_ms));
        assert!(!routed_before(Some(started), started_ms));
        assert!(!routed_before(
            Some(started + time::Duration::hours(40)),
            started_ms
        ));
        // Without a scoring context the routing time is unknown, so the payment counts.
        assert!(!routed_before(None, started_ms));
    }

    #[test]
    fn each_period_and_arm_has_its_own_keys() {
        let (txns, succ) = arm_keys("m1", "exp1", 3, &ArmSide::Variant("v1".into()));
        assert_eq!(txns, "ab_guardrail_m1_exp1_3_v1_txns");
        assert_eq!(succ, "ab_guardrail_m1_exp1_3_v1_succ");
        assert_ne!(
            arm_keys("m1", "exp1", 4, &ArmSide::Control).0,
            arm_keys("m1", "exp1", 3, &ArmSide::Control).0
        );
    }
}
