use std::collections::{hash_map::Entry, HashMap, HashSet, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use once_cell::sync::Lazy;

use super::arms::{self, ArmSide, EndpointPlan};
use crate::euclid::types::{ABTestData, ExperimentEndpoint};
use crate::logger;

static GUARDRAILS: Lazy<Mutex<Guardrails>> = Lazy::new(|| Mutex::new(Guardrails::default()));
// Pruned on traffic, not by a worker, so a long experiment doesn't retain every payment id.
const PAYMENT_RETENTION: Duration = Duration::from_secs(3600);
// Share of the merchant's sample target that must resolve before the guardrail can stop a
// variant. A safety stop should react long before a significance verdict, but not on the first
// few payments, where one binary outcome swings an arm's rate by tens of points.
const GUARDRAIL_SAMPLE_PCT: u32 = 10;

#[derive(Default)]
struct Guardrails {
    merchants: HashMap<String, HashMap<String, Experiment>>,
    // Identifies one observation period. A stop request carries the run it was raised for, so a
    // stop that lands after a reset cannot silence the run that replaced it.
    next_run: u64,
}

struct Experiment {
    run: u64,
    threshold_pp: f64,
    min_resolved: u64,
    /// Variants whose auth rate fell below control by more than the threshold.
    breached: HashSet<String>,
    /// Breached variants whose durable stop has landed.
    stop_completed: HashSet<String>,
    payments: HashMap<String, Payment>,
    payment_order: VecDeque<(Instant, String)>,
    endpoints: HashMap<ExperimentEndpoint, EndpointCounts>,
}

impl Experiment {
    fn new(data: &ABTestData, run: u64) -> Self {
        Self {
            run,
            threshold_pp: data.guardrail_threshold_pp,
            min_resolved: (data.min_sample_size / GUARDRAIL_SAMPLE_PCT) as u64,
            breached: HashSet::new(),
            stop_completed: HashSet::new(),
            payments: HashMap::new(),
            payment_order: VecDeque::new(),
            endpoints: HashMap::new(),
        }
    }

    fn prune(&mut self, now: Instant) {
        while self
            .payment_order
            .front()
            .is_some_and(|(at, _)| now.duration_since(*at) >= PAYMENT_RETENTION)
        {
            if let Some((_, payment_id)) = self.payment_order.pop_front() {
                self.payments.remove(&payment_id);
            }
        }
    }
}

struct Payment {
    side: ArmSide,
    // None is routed without an outcome, Some(false) failed but may retry, Some(true) succeeded.
    endpoints: HashMap<ExperimentEndpoint, Option<bool>>,
}

/// Outcome counts per arm on one endpoint.
#[derive(Default)]
struct EndpointCounts {
    arms: HashMap<ArmSide, ArmCounts>,
}

impl EndpointCounts {
    fn arm_mut(&mut self, side: &ArmSide) -> &mut ArmCounts {
        self.arms.entry(side.clone()).or_default()
    }

    /// Whether `variant` fell more than `threshold_pp` below control on this endpoint. Each
    /// variant is judged against control alone, so one variant's results never stop another.
    fn breached(&self, variant: &ArmSide, threshold_pp: f64, min_resolved: u64) -> bool {
        let (Some(control), Some(variant)) =
            (self.arms.get(&ArmSide::Control), self.arms.get(variant))
        else {
            return false;
        };
        // Both arms need a rate to compare, and the merchant's share of the target has to have
        // resolved before that comparison means anything.
        if control.transactions == 0
            || variant.transactions == 0
            || control.transactions + variant.transactions < min_resolved
        {
            return false;
        }
        let control_rate = control.successes as f64 / control.transactions as f64;
        let variant_rate = variant.successes as f64 / variant.transactions as f64;
        (control_rate - variant_rate) * 100.0 > threshold_pp
    }
}

#[derive(Default)]
struct ArmCounts {
    transactions: u64,
    successes: u64,
}

fn lock() -> std::sync::MutexGuard<'static, Guardrails> {
    GUARDRAILS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Plan the payment on `endpoint` and, when it is split there, remember its arm until the
/// experiment is deactivated so its outcome can be counted.
pub fn plan(
    merchant_id: &str,
    experiment_id: &str,
    data: &ABTestData,
    endpoint: ExperimentEndpoint,
    payment_id: &str,
) -> EndpointPlan {
    let planned = arms::plan(data, endpoint, payment_id);
    if !planned.in_experiment || payment_id.is_empty() {
        return planned;
    }
    let mut guardrails = lock();
    // Split the borrow so a new observation period can take its run from the same registry.
    let guardrails = &mut *guardrails;
    let next_run = &mut guardrails.next_run;
    let experiment = guardrails
        .merchants
        .entry(merchant_id.to_owned())
        .or_default()
        .entry(experiment_id.to_owned())
        .or_insert_with(|| {
            *next_run += 1;
            Experiment::new(data, *next_run)
        });
    experiment.prune(Instant::now());
    if let Entry::Vacant(entry) = experiment.payments.entry(payment_id.to_owned()) {
        entry.insert(Payment {
            side: planned.side.clone(),
            endpoints: HashMap::new(),
        });
        experiment
            .payment_order
            .push_back((Instant::now(), payment_id.to_owned()));
    }
    planned
}

/// Associate a payment with an endpoint only when a real routing decision was recorded.
/// Repeated calls with the same payment and endpoint preserve its existing outcome.
pub fn record_decision(
    merchant_id: &str,
    experiment_id: &str,
    payment_id: &str,
    endpoint: ExperimentEndpoint,
) {
    let mut guardrails = lock();
    let Some(experiment) = guardrails
        .merchants
        .get_mut(merchant_id)
        .and_then(|merchant| merchant.get_mut(experiment_id))
    else {
        return;
    };
    let Some(payment) = experiment.payments.get_mut(payment_id) else {
        return;
    };
    if let Entry::Vacant(entry) = payment.endpoints.entry(endpoint) {
        entry.insert(None);
    }
}

/// One breached variant and the observation period it breached in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreachedRun {
    pub experiment_id: String,
    pub variant_id: String,
    pub run: u64,
}

/// Update net auth rate on feedback and return the variants needing a durable stop. At most one
/// success counts per payment and endpoint. A breached variant stays pending until stopped.
pub fn record_outcome(merchant_id: &str, payment_id: &str, is_success: bool) -> Vec<BreachedRun> {
    let mut guardrails = lock();
    let Some(experiments) = guardrails.merchants.get_mut(merchant_id) else {
        return Vec::new();
    };
    for (experiment_id, experiment) in experiments.iter_mut() {
        experiment.prune(Instant::now());
        let Some(payment) = experiment.payments.get_mut(payment_id) else {
            continue;
        };
        for (endpoint, outcome) in &mut payment.endpoints {
            let counts = experiment.endpoints.entry(*endpoint).or_default();
            let arm = counts.arm_mut(&payment.side);
            if outcome.is_none() {
                arm.transactions += 1;
                *outcome = Some(false);
            }
            if is_success && *outcome != Some(true) {
                arm.successes += 1;
                *outcome = Some(true);
            }
            // A control outcome moves the baseline every variant is judged against.
            let candidates: Vec<ArmSide> = match &payment.side {
                ArmSide::Control => counts
                    .arms
                    .keys()
                    .filter(|side| **side != ArmSide::Control)
                    .cloned()
                    .collect(),
                variant => vec![variant.clone()],
            };
            for side in candidates {
                let ArmSide::Variant(variant_id) = &side else {
                    continue;
                };
                if !experiment.breached.contains(variant_id)
                    && counts.breached(&side, experiment.threshold_pp, experiment.min_resolved)
                {
                    experiment.breached.insert(variant_id.clone());
                    logger::warn!(merchant_id = %merchant_id, experiment_id = %experiment_id,
                        variant_id = %variant_id, endpoint = endpoint.as_str(),
                        threshold_pp = experiment.threshold_pp,
                        "A/B guardrail breached; stopping the variant");
                }
            }
        }
    }
    let mut pending: Vec<BreachedRun> = experiments
        .iter()
        .flat_map(|(experiment_id, experiment)| {
            experiment
                .breached
                .iter()
                .filter(|variant_id| !experiment.stop_completed.contains(*variant_id))
                .map(|variant_id| BreachedRun {
                    experiment_id: experiment_id.clone(),
                    variant_id: variant_id.clone(),
                    run: experiment.run,
                })
        })
        .collect();
    pending
        .sort_by(|a, b| (&a.experiment_id, &a.variant_id).cmp(&(&b.experiment_id, &b.variant_id)));
    pending
}

/// A successful stop ends a variant's deactivation retries until a deliberate new activation. A
/// stop raised for an earlier run is ignored, so a reset plus fresh routing is never silenced by
/// it.
pub fn mark_stopped(merchant_id: &str, experiment_id: &str, variant_id: &str, run: u64) {
    let mut guardrails = lock();
    if let Some(experiment) = guardrails
        .merchants
        .get_mut(merchant_id)
        .and_then(|merchant| merchant.get_mut(experiment_id))
    {
        if experiment.run == run {
            experiment.stop_completed.insert(variant_id.to_owned());
        }
    }
}

/// A deliberate activation/deactivation starts a new observation period.
pub fn reset(merchant_id: &str, experiment_id: &str) {
    let mut guardrails = lock();
    if let Some(merchant) = guardrails.merchants.get_mut(merchant_id) {
        merchant.remove(experiment_id);
        if merchant.is_empty() {
            guardrails.merchants.remove(merchant_id);
        }
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
            "min_sample_size": 4,
            "guardrail_threshold_pp": 10.0
        }))
        .unwrap()
    }

    fn stopped(breached: Vec<BreachedRun>) -> Vec<String> {
        breached.into_iter().map(|b| b.experiment_id).collect()
    }

    fn variant() -> ArmSide {
        ArmSide::Variant(crate::euclid::types::LEGACY_VARIANT_ARM_ID.to_string())
    }

    fn side_of(config: &ABTestData, payment_id: &str) -> ArmSide {
        super::super::assign_arm(payment_id, &arms::resolved_variants(config))
    }

    fn payment_in(config: &ABTestData, side: &ArmSide, used: &[String]) -> String {
        (0..1000)
            .map(|i| format!("guardrail_payment_{i}"))
            .find(|payment_id| !used.contains(payment_id) && side_of(config, payment_id) == *side)
            .unwrap()
    }

    fn payment_for(side: ArmSide, used: &[String]) -> String {
        payment_in(&data(), &side, used)
    }

    #[test]
    fn feedback_requests_stop_without_double_counting_or_changing_arm_assignments() {
        let merchant = "guardrail_unit_merchant";
        let experiment = "guardrail_unit_experiment";
        let mut config = data();
        config.min_sample_size = 20;
        let mut used = Vec::new();
        let controls: Vec<_> = (0..2)
            .map(|_| {
                let id = payment_for(ArmSide::Control, &used);
                used.push(id.clone());
                id
            })
            .collect();
        let variants: Vec<_> = (0..2)
            .map(|_| {
                let id = payment_for(variant(), &used);
                used.push(id.clone());
                id
            })
            .collect();
        for payment_id in &used {
            let planned = plan(
                merchant,
                experiment,
                &config,
                ExperimentEndpoint::HybridRouting,
                payment_id,
            );
            record_decision(
                merchant,
                experiment,
                payment_id,
                ExperimentEndpoint::HybridRouting,
            );
            record_decision(
                merchant,
                experiment,
                payment_id,
                ExperimentEndpoint::HybridRouting,
            );
            assert_eq!(planned.side, side_of(&config, payment_id));
        }
        assert!(record_outcome(merchant, &controls[0], true).is_empty());
        assert_eq!(
            stopped(record_outcome(merchant, &variants[0], false)),
            vec![experiment]
        );
        assert_eq!(
            stopped(record_outcome(merchant, &variants[0], false)),
            vec![experiment]
        );
        assert_eq!(
            stopped(record_outcome(merchant, &controls[1], true)),
            vec![experiment]
        );
        assert_eq!(
            stopped(record_outcome(merchant, &variants[1], false)),
            vec![experiment]
        );

        let new_variant = payment_for(variant(), &used);
        assert_eq!(
            plan(
                merchant,
                experiment,
                &config,
                ExperimentEndpoint::HybridRouting,
                &new_variant
            )
            .side,
            variant()
        );
        assert_eq!(
            plan(
                merchant,
                experiment,
                &config,
                ExperimentEndpoint::Evaluate,
                &new_variant
            )
            .side,
            variant()
        );
        assert_eq!(
            plan(
                merchant,
                experiment,
                &config,
                ExperimentEndpoint::Evaluate,
                &variants[0]
            )
            .side,
            variant()
        );
        assert_eq!(
            stopped(record_outcome(merchant, &variants[0], true)),
            vec![experiment]
        );
        let run = lock().merchants[merchant][experiment].run;
        mark_stopped(merchant, experiment, "variant", run);
        assert!(record_outcome(merchant, &variants[0], true).is_empty());

        {
            let mut registry = lock();
            let observed = registry
                .merchants
                .get_mut(merchant)
                .unwrap()
                .get_mut(experiment)
                .unwrap();
            let counted = observed
                .endpoints
                .get(&ExperimentEndpoint::HybridRouting)
                .unwrap()
                .arms[&ArmSide::Control]
                .transactions;
            observed.prune(Instant::now() + PAYMENT_RETENTION + Duration::from_secs(1));
            assert!(observed.payments.is_empty());
            assert_eq!(
                observed
                    .endpoints
                    .get(&ExperimentEndpoint::HybridRouting)
                    .unwrap()
                    .arms[&ArmSide::Control]
                    .transactions,
                counted
            );
            assert!(observed.breached.contains("variant"));
            assert!(observed.stop_completed.contains("variant"));
        }

        reset(merchant, experiment);
        assert_eq!(
            plan(
                merchant,
                experiment,
                &config,
                ExperimentEndpoint::HybridRouting,
                &new_variant
            )
            .side,
            variant()
        );
        reset(merchant, experiment);
    }

    /// A stop raised before a reset must not silence the run that replaced it.
    #[test]
    fn stop_completion_is_scoped_to_its_run() {
        let merchant = "guardrail_run_merchant";
        let experiment = "guardrail_run_experiment";
        let config = data();
        let mut used = Vec::new();
        let control = payment_for(ArmSide::Control, &used);
        used.push(control.clone());
        let variant = payment_for(variant(), &used);
        used.push(variant.clone());

        let route = |payment_id: &str| {
            plan(
                merchant,
                experiment,
                &config,
                ExperimentEndpoint::HybridRouting,
                payment_id,
            );
            record_decision(
                merchant,
                experiment,
                payment_id,
                ExperimentEndpoint::HybridRouting,
            );
        };

        // First run breaches and a stop is raised for it, but the stop has not landed yet.
        for payment_id in &used {
            route(payment_id);
        }
        record_outcome(merchant, &control, true);
        let raised = record_outcome(merchant, &variant, false);
        let stale_run = raised[0].run;
        assert_eq!(stopped(raised), vec![experiment]);

        // The merchant reactivates, and fresh routing starts a new observation period.
        reset(merchant, experiment);
        for payment_id in &used {
            route(payment_id);
        }
        let live_run = lock().merchants[merchant][experiment].run;
        assert_ne!(stale_run, live_run);

        // The old stop lands now. It must leave the new run alone.
        mark_stopped(merchant, experiment, "variant", stale_run);
        record_outcome(merchant, &control, true);
        assert_eq!(
            stopped(record_outcome(merchant, &variant, false)),
            vec![experiment]
        );

        // A stop naming the live run still ends its retries.
        mark_stopped(merchant, experiment, "variant", live_run);
        assert!(record_outcome(merchant, &variant, false).is_empty());
        reset(merchant, experiment);
    }

    #[test]
    fn saved_threshold_and_merchant_scope_control_the_cutoff() {
        let merchant = "guardrail_threshold_merchant";
        let other = "guardrail_other_merchant";
        let experiment = "guardrail_threshold_experiment";
        let mut config = data();
        config.guardrail_threshold_pp = 60.0;
        let other_config = data();
        let controls = [
            payment_for(ArmSide::Control, &[]),
            payment_for(ArmSide::Control, &[payment_for(ArmSide::Control, &[])]),
        ];
        let variants = [
            payment_for(variant(), &[]),
            payment_for(variant(), &[payment_for(variant(), &[])]),
        ];
        for scope in [merchant, other] {
            let selected_config = if scope == merchant {
                &config
            } else {
                &other_config
            };
            for payment_id in controls.iter().chain(variants.iter()) {
                plan(
                    scope,
                    experiment,
                    selected_config,
                    ExperimentEndpoint::Evaluate,
                    payment_id,
                );
                record_decision(scope, experiment, payment_id, ExperimentEndpoint::Evaluate);
            }
            for payment_id in &controls {
                record_outcome(scope, payment_id, true);
            }
            record_outcome(scope, &variants[0], true);
            let stop = stopped(record_outcome(scope, &variants[1], false));
            if scope == merchant {
                assert!(stop.is_empty()); // 50pp drop is within the saved 60pp limit.
            } else {
                assert_eq!(stop, vec![experiment]); // The other merchant saved 10pp.
            }
        }
        reset(merchant, experiment);
        reset(other, experiment);
    }

    /// Several variants: each is judged against control alone, and the stop names the variant.
    #[test]
    fn a_breach_stops_only_the_variant_that_fell_behind() {
        let merchant = "guardrail_multi_merchant";
        let experiment = "guardrail_multi_experiment";
        let config: ABTestData = serde_json::from_value(serde_json::json!({
            "control": { "rule_algorithm_id": "control_rule" },
            "variants": [
                { "id": "good", "split_pct": 20, "rule_algorithm_id": "good_rule" },
                { "id": "bad", "split_pct": 20, "rule_algorithm_id": "bad_rule" }
            ],
            "min_sample_size": 4,
            "guardrail_threshold_pp": 10.0
        }))
        .unwrap();
        let good = ArmSide::Variant("good".into());
        let bad = ArmSide::Variant("bad".into());
        let mut used = Vec::new();
        let mut pick = |side: &ArmSide| {
            let id = payment_in(&config, side, &used);
            used.push(id.clone());
            id
        };
        let control_payment = pick(&ArmSide::Control);
        let good_payment = pick(&good);
        let bad_payment = pick(&bad);
        for payment_id in [&control_payment, &good_payment, &bad_payment] {
            plan(
                merchant,
                experiment,
                &config,
                ExperimentEndpoint::Evaluate,
                payment_id,
            );
            record_decision(
                merchant,
                experiment,
                payment_id,
                ExperimentEndpoint::Evaluate,
            );
        }
        assert!(record_outcome(merchant, &control_payment, true).is_empty());
        assert!(record_outcome(merchant, &good_payment, true).is_empty());
        let breached = record_outcome(merchant, &bad_payment, false);
        assert_eq!(
            breached,
            vec![BreachedRun {
                experiment_id: experiment.to_string(),
                variant_id: "bad".to_string(),
                run: breached[0].run,
            }]
        );
        mark_stopped(merchant, experiment, "bad", breached[0].run);
        assert!(record_outcome(merchant, &good_payment, true).is_empty());
        reset(merchant, experiment);
    }
}
