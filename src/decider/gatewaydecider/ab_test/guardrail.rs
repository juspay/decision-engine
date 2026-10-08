use std::collections::{hash_map::Entry, HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use once_cell::sync::Lazy;

use super::arms::{self, ArmSide, EndpointPlan};
use crate::euclid::types::{ABTestData, ExperimentEndpoint};
use crate::logger;

static GUARDRAILS: Lazy<Mutex<Guardrails>> = Lazy::new(|| Mutex::new(Guardrails::default()));
// Match the existing per-payment in-flight retention. Expiration is checked on traffic, so no
// worker is needed and running experiments do not retain every payment ID indefinitely.
const PAYMENT_RETENTION: Duration = Duration::from_secs(3600);

#[derive(Default)]
struct Guardrails {
    merchants: HashMap<String, HashMap<String, Experiment>>,
}

struct Experiment {
    threshold_pp: f64,
    breached: bool,
    stop_completed: bool,
    payments: HashMap<String, Payment>,
    payment_order: VecDeque<(Instant, String)>,
    endpoints: HashMap<ExperimentEndpoint, EndpointCounts>,
}

impl Experiment {
    fn new(data: &ABTestData) -> Self {
        Self {
            threshold_pp: data.guardrail_threshold_pp,
            breached: false,
            stop_completed: false,
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
    // None: routed, no outcome. Some(false): failed; a retry may succeed. Some(true): succeeded.
    endpoints: HashMap<ExperimentEndpoint, Option<bool>>,
}

#[derive(Default)]
struct EndpointCounts {
    control: ArmCounts,
    variant: ArmCounts,
}

impl EndpointCounts {
    fn arm_mut(&mut self, side: ArmSide) -> &mut ArmCounts {
        match side {
            ArmSide::Control => &mut self.control,
            ArmSide::Variant => &mut self.variant,
        }
    }

    fn breached(&self, threshold_pp: f64) -> bool {
        if self.control.transactions == 0 || self.variant.transactions == 0 {
            return false;
        }
        let control_rate = self.control.successes as f64 / self.control.transactions as f64;
        let variant_rate = self.variant.successes as f64 / self.variant.transactions as f64;
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

/// Remember a payment's arm for every endpoint of this experiment until it is deactivated.
pub fn plan(
    merchant_id: &str,
    experiment_id: &str,
    data: &ABTestData,
    endpoint: ExperimentEndpoint,
    payment_id: &str,
) -> EndpointPlan {
    if !arms::splits_on(data, endpoint) || payment_id.is_empty() {
        return arms::plan(data, endpoint, payment_id);
    }
    let mut guardrails = lock();
    let experiment = guardrails
        .merchants
        .entry(merchant_id.to_owned())
        .or_default()
        .entry(experiment_id.to_owned())
        .or_insert_with(|| Experiment::new(data));
    experiment.prune(Instant::now());
    let side = experiment
        .payments
        .get(payment_id)
        .map(|payment| payment.side)
        .unwrap_or_else(|| super::assign_arm(payment_id, data.variant_split_pct));
    if let Entry::Vacant(entry) = experiment.payments.entry(payment_id.to_owned()) {
        entry.insert(Payment {
            side,
            endpoints: HashMap::new(),
        });
        experiment
            .payment_order
            .push_back((Instant::now(), payment_id.to_owned()));
    }
    EndpointPlan {
        side,
        projection: arms::project(&arms::resolved_arm(data, side), endpoint),
        in_experiment: true,
    }
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

/// Update net auth rate on every feedback and return experiments that need durable deactivation.
/// Duplicate callbacks and a failure followed by a success count at most one success per payment
/// and endpoint. A breached experiment remains pending until deactivation succeeds.
pub fn record_outcome(merchant_id: &str, payment_id: &str, is_success: bool) -> Vec<String> {
    let mut guardrails = lock();
    let Some(experiments) = guardrails.merchants.get_mut(merchant_id) else {
        return Vec::new();
    };
    for (experiment_id, experiment) in experiments.iter_mut() {
        experiment.prune(Instant::now());
        if let Some(payment) = experiment.payments.get_mut(payment_id) {
            for (endpoint, outcome) in &mut payment.endpoints {
                let counts = experiment.endpoints.entry(*endpoint).or_default();
                let arm = counts.arm_mut(payment.side);
                if outcome.is_none() {
                    arm.transactions += 1;
                    *outcome = Some(false);
                }
                if is_success && *outcome != Some(true) {
                    arm.successes += 1;
                    *outcome = Some(true);
                }
                if !experiment.breached && counts.breached(experiment.threshold_pp) {
                    experiment.breached = true;
                    logger::warn!(merchant_id = %merchant_id, experiment_id = %experiment_id,
                        endpoint = endpoint.as_str(), threshold_pp = experiment.threshold_pp,
                        "A/B guardrail breached; stopping the experiment");
                }
            }
        }
    }
    experiments
        .iter()
        .filter(|(_, experiment)| experiment.breached && !experiment.stop_completed)
        .map(|(experiment_id, _)| experiment_id.clone())
        .collect()
}

/// A successful stop ends deactivation retries until a deliberate new activation.
pub fn mark_stopped(merchant_id: &str, experiment_id: &str) {
    let mut guardrails = lock();
    if let Some(experiment) = guardrails
        .merchants
        .get_mut(merchant_id)
        .and_then(|merchant| merchant.get_mut(experiment_id))
    {
        experiment.stop_completed = true;
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

    fn payment_for(side: ArmSide, used: &[String]) -> String {
        (0..1000)
            .map(|i| format!("guardrail_payment_{i}"))
            .find(|payment_id| {
                !used.contains(payment_id) && super::super::assign_arm(payment_id, 20) == side
            })
            .unwrap()
    }

    #[test]
    fn feedback_requests_stop_without_double_counting_or_changing_arm_assignments() {
        let merchant = "guardrail_unit_merchant";
        let experiment = "guardrail_unit_experiment";
        let mut config = data();
        config.min_sample_size = 1_000;
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
                let id = payment_for(ArmSide::Variant, &used);
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
            assert_eq!(planned.side, super::super::assign_arm(payment_id, 20));
        }
        assert!(record_outcome(merchant, &controls[0], true).is_empty());
        assert_eq!(
            record_outcome(merchant, &variants[0], false),
            vec![experiment]
        );
        assert_eq!(
            record_outcome(merchant, &variants[0], false),
            vec![experiment]
        );
        assert_eq!(
            record_outcome(merchant, &controls[1], true),
            vec![experiment]
        );
        assert_eq!(
            record_outcome(merchant, &variants[1], false),
            vec![experiment]
        );

        let new_variant = payment_for(ArmSide::Variant, &used);
        assert_eq!(
            plan(
                merchant,
                experiment,
                &config,
                ExperimentEndpoint::HybridRouting,
                &new_variant
            )
            .side,
            ArmSide::Variant
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
            ArmSide::Variant
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
            ArmSide::Variant
        );
        assert_eq!(
            record_outcome(merchant, &variants[0], true),
            vec![experiment]
        );
        mark_stopped(merchant, experiment);
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
                .control
                .transactions;
            observed.prune(Instant::now() + PAYMENT_RETENTION + Duration::from_secs(1));
            assert!(observed.payments.is_empty());
            assert_eq!(
                observed
                    .endpoints
                    .get(&ExperimentEndpoint::HybridRouting)
                    .unwrap()
                    .control
                    .transactions,
                counted
            );
            assert!(observed.breached);
            assert!(observed.stop_completed);
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
            ArmSide::Variant
        );
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
            payment_for(ArmSide::Variant, &[]),
            payment_for(ArmSide::Variant, &[payment_for(ArmSide::Variant, &[])]),
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
            let stop = record_outcome(scope, &variants[1], false);
            if scope == merchant {
                assert!(stop.is_empty()); // 50pp drop is within the saved 60pp limit.
            } else {
                assert_eq!(stop, vec![experiment]); // The other merchant saved 10pp.
            }
        }
        reset(merchant, experiment);
        reset(other, experiment);
    }
}
