//! What one routing evaluation resolved to: the rule layer's result, the success-rate layer's,
//! and the connectors they settled on. `/decide` writes it as a
//! [`crate::types::decide::response::DecideResponse`]; the audit record, which `/routing/hybrid`
//! also returns, writes it as `static_routing`, `dynamic_routing` and `evaluated_connectors`.

use crate::decider::gatewaydecider::types::DecidedGateway;
use crate::euclid::ast::ConnectorInfo;
use crate::euclid::types::RoutingEvaluateResponse;

#[derive(Debug)]
pub struct RoutingEvaluation {
    pub static_routing: Option<RoutingEvaluateResponse>,
    /// The rule layer's error when it failed and the candidates answered instead.
    pub static_error: Option<String>,
    pub success_rate: SuccessRateOutcome,
    /// `None` when neither layer produced connectors; the audit record then omits the field.
    pub evaluated_connectors: Option<Vec<ConnectorInfo>>,
}

/// What the success-rate layer did.
#[derive(Debug)]
pub enum SuccessRateOutcome {
    Applied(Box<DecidedGateway>),
    /// The decider failed and the rule output, or the candidates, answered unranked.
    Failed {
        error_message: String,
        fallback_connectors: Vec<ConnectorInfo>,
    },
    /// The merchant's SR setting, or its A/B arm, left the layer off.
    Skipped,
}
