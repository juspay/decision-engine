//! Attribution types for real-payment A/B decisions.
//!
//! A payment's outcome is not tracked here: the experiment results join each routing event
//! (`emit_routing_event`) to the merchant's score updates for the same payment in ClickHouse, so
//! an outcome reported days after routing is still attributed to its arm.

use crate::euclid::types::ExperimentEndpoint;

/// Which experiment, arm and endpoint a payment's decision is attributed to.
#[derive(Debug, Clone)]
pub struct ExperimentAssignment {
    pub experiment_id: String,
    pub side: super::ArmSide,
    pub endpoint: ExperimentEndpoint,
}

/// Multi-objective cost outcome of an SR-arm decision. All `None` when the arm ran auth-only.
#[derive(Debug, Clone, Default)]
pub struct CostOutcome {
    pub cost_saved_bps: Option<f64>,
    pub chosen_cost_bps: Option<f64>,
    pub margin: Option<f64>,
}
