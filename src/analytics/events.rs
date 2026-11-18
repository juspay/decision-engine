use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::analytics::flow::{AnalyticsFlowContext, AnalyticsRoute};
use crate::analytics::flow::{ApiFlow, FlowType};
use crate::analytics::flow::{
    PAYMENT_AUDIT_HYBRID_FLOW_TYPES, PAYMENT_AUDIT_MULTI_OBJECTIVE_FLOW_TYPES,
    PAYMENT_AUDIT_PREVIEW_FLOW_TYPES, SUMMARY_KIND_DYNAMIC, SUMMARY_KIND_HYBRID,
    SUMMARY_KIND_PREVIEW,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainAnalyticsEvent {
    pub event_id: String,
    pub api_flow: ApiFlow,
    pub flow_type: FlowType,
    pub summary_kind: Option<String>,
    pub merchant_id: Option<String>,
    pub payment_id: Option<String>,
    pub request_id: Option<String>,
    pub lookup_key: Option<String>,
    pub global_request_id: Option<String>,
    pub trace_id: Option<String>,
    pub payment_method_type: Option<String>,
    pub payment_method: Option<String>,
    pub card_network: Option<String>,
    pub card_is_in: Option<String>,
    pub currency: Option<String>,
    pub country: Option<String>,
    pub auth_type: Option<String>,
    pub gateway: Option<String>,
    pub event_stage: Option<String>,
    pub routing_approach: Option<String>,
    pub rule_name: Option<String>,
    pub status: Option<String>,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
    pub score_value: Option<f64>,
    pub sigma_factor: Option<f64>,
    pub average_latency: Option<f64>,
    pub tp99_latency: Option<f64>,
    pub transaction_count: Option<i64>,
    pub route: Option<String>,
    pub details: Option<String>,
    pub created_at_ms: i64,
}

/// Built with struct update syntax: a constructor fills the fields its kind always carries and the
/// caller names the rest, then sends it with `emit`.
///
/// ```ignore
/// DomainAnalyticsEvent {
///     merchant_id: Some(merchant_id),
///     payment_id: Some(payment_id),
///     ..DomainAnalyticsEvent::request_hit(flow, route)
/// }
/// .emit();
/// ```
impl DomainAnalyticsEvent {
    /// An event of `flow` on `route`, stamped now, with every other field empty.
    pub fn new(flow: AnalyticsFlowContext, route: AnalyticsRoute) -> Self {
        Self {
            event_id: next_event_id(),
            api_flow: flow.api_flow,
            flow_type: flow.flow_type,
            summary_kind: derive_payment_audit_summary_kind(route, flow.flow_type),
            merchant_id: None,
            payment_id: None,
            request_id: None,
            lookup_key: None,
            global_request_id: None,
            trace_id: None,
            payment_method_type: None,
            payment_method: None,
            card_network: None,
            card_is_in: None,
            currency: None,
            country: None,
            auth_type: None,
            gateway: None,
            event_stage: None,
            routing_approach: None,
            rule_name: None,
            status: None,
            error_code: None,
            error_message: None,
            score_value: None,
            sigma_factor: None,
            average_latency: None,
            tp99_latency: None,
            transaction_count: None,
            route: Some(route.as_str().to_string()),
            details: None,
            created_at_ms: crate::analytics::now_ms(),
        }
    }

    /// A request that reached its handler.
    pub fn request_hit(flow: AnalyticsFlowContext, route: AnalyticsRoute) -> Self {
        Self {
            event_stage: Some("request_received".to_string()),
            status: Some("received".to_string()),
            ..Self::new(flow, route)
        }
    }

    /// A request that failed with `error_code`.
    pub fn error(
        flow: AnalyticsFlowContext,
        route: AnalyticsRoute,
        error_code: String,
        error_message: String,
    ) -> Self {
        Self {
            status: Some("failure".to_string()),
            error_code: Some(error_code),
            error_message: Some(error_message),
            ..Self::new(flow, route)
        }
    }

    /// A priority-logic rule that matched. Send it with `emit_rule_hit`, which counts it.
    pub fn rule_hit(flow: AnalyticsFlowContext, route: AnalyticsRoute, rule_name: String) -> Self {
        Self {
            rule_name: Some(rule_name),
            status: Some("hit".to_string()),
            ..Self::new(flow, route)
        }
    }

    /// A gateway's score after feedback.
    pub fn score_snapshot(flow: AnalyticsFlowContext, route: AnalyticsRoute) -> Self {
        Self {
            status: Some("snapshot".to_string()),
            ..Self::new(flow, route)
        }
    }

    /// A rule evaluated without routing a payment.
    pub fn rule_evaluation_preview(flow: AnalyticsFlowContext) -> Self {
        Self {
            event_stage: Some("preview_evaluated".to_string()),
            routing_approach: Some("RULE_EVALUATE_PREVIEW".to_string()),
            ..Self::new(flow, AnalyticsRoute::RoutingEvaluate)
        }
    }

    /// A calibration retune emitted by the SR auto-calibrator. Numeric knobs ride typed
    /// columns (`score_value`/`sigma_factor` = new/prev hedging %, `transaction_count` =
    /// new bucket, `average_latency` = prev bucket) so the routing-events detector can read
    /// them back without JSON parsing. `event_stage` tags the row for that query.
    pub fn autopilot_calibration(
        flow: AnalyticsFlowContext,
        route: AnalyticsRoute,
        new_bucket: i32,
        previous_bucket: Option<i32>,
        new_hedge: f64,
        previous_hedge: Option<f64>,
    ) -> Self {
        Self {
            event_stage: Some(crate::analytics::models::AUTOPILOT_CALIBRATION_STAGE.to_string()),
            status: Some("applied".to_string()),
            score_value: Some(new_hedge),
            sigma_factor: previous_hedge,
            transaction_count: Some(i64::from(new_bucket)),
            average_latency: previous_bucket.map(f64::from),
            ..Self::new(flow, route)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiEvent {
    pub event_id: String,
    pub merchant_id: Option<String>,
    pub payment_id: Option<String>,
    pub api_flow: ApiFlow,
    pub flow_type: FlowType,
    pub created_at_timestamp: i64,
    pub request_id: String,
    pub global_request_id: Option<String>,
    pub trace_id: Option<String>,
    pub latency: u64,
    pub status_code: u16,
    pub request: String,
    pub user_agent: Option<String>,
    pub ip_addr: Option<String>,
    pub url_path: String,
    pub response: Option<String>,
    pub error: Option<String>,
    pub http_method: String,
}

/// Generates a sortable internal analytics row id used for Kafka key fallback,
/// ClickHouse storage, and stable timeline ordering.
pub fn next_event_id() -> String {
    Uuid::now_v7().to_string()
}

pub fn derive_lookup_key(payment_id: Option<&str>, request_id: Option<&str>) -> Option<String> {
    payment_id
        .filter(|value| !value.is_empty())
        .or(request_id.filter(|value| !value.is_empty()))
        .map(str::to_string)
}

pub fn derive_payment_audit_summary_kind(
    route: AnalyticsRoute,
    flow_type: FlowType,
) -> Option<String> {
    if route == AnalyticsRoute::RoutingEvaluate
        && PAYMENT_AUDIT_PREVIEW_FLOW_TYPES.contains(&flow_type)
    {
        return Some(SUMMARY_KIND_PREVIEW.to_string());
    }

    if PAYMENT_AUDIT_HYBRID_FLOW_TYPES.contains(&flow_type) {
        return Some(SUMMARY_KIND_HYBRID.to_string());
    }

    if PAYMENT_AUDIT_MULTI_OBJECTIVE_FLOW_TYPES.contains(&flow_type) {
        return Some(SUMMARY_KIND_DYNAMIC.to_string());
    }

    None
}

#[cfg(test)]
mod tests {
    use uuid::{Uuid, Version};

    use crate::analytics::flow::{AnalyticsRoute, FlowType};

    use super::{derive_lookup_key, derive_payment_audit_summary_kind, next_event_id};

    #[test]
    fn lookup_key_prefers_payment_id() {
        assert_eq!(
            derive_lookup_key(Some("pay_123"), Some("req_123")),
            Some("pay_123".to_string())
        );
    }

    #[test]
    fn lookup_key_falls_back_to_request_id() {
        assert_eq!(
            derive_lookup_key(None, Some("req_123")),
            Some("req_123".to_string())
        );
    }

    #[test]
    fn lookup_key_ignores_empty_values() {
        assert_eq!(derive_lookup_key(Some(""), Some("")), None);
        assert_eq!(
            derive_lookup_key(Some(""), Some("req_123")),
            Some("req_123".to_string())
        );
    }

    #[test]
    fn summary_kind_uses_preview_for_routing_evaluate_preview_flows() {
        assert_eq!(
            derive_payment_audit_summary_kind(
                AnalyticsRoute::RoutingEvaluate,
                FlowType::RoutingEvaluatePreview,
            ),
            Some("preview".to_string())
        );
    }

    #[test]
    fn summary_kind_uses_dynamic_for_runtime_decision_flows() {
        assert_eq!(
            derive_payment_audit_summary_kind(
                AnalyticsRoute::DecideGateway,
                FlowType::DecideGatewayDecision,
            ),
            Some("dynamic".to_string())
        );
    }

    #[test]
    fn summary_kind_uses_hybrid_for_hybrid_routing_flows() {
        assert_eq!(
            derive_payment_audit_summary_kind(
                AnalyticsRoute::DecideGateway,
                FlowType::RoutingHybridDecision,
            ),
            Some("hybrid".to_string())
        );
        assert_eq!(
            derive_payment_audit_summary_kind(
                AnalyticsRoute::DecideGateway,
                FlowType::RoutingHybridError,
            ),
            Some("hybrid".to_string())
        );
    }

    #[test]
    fn summary_kind_is_none_for_untracked_flows() {
        assert_eq!(
            derive_payment_audit_summary_kind(
                AnalyticsRoute::RuleConfigCreate,
                FlowType::RuleConfigCreate,
            ),
            None
        );
    }

    #[test]
    fn next_event_id_uses_uuid_v7() {
        let event_id = next_event_id();
        let parsed = Uuid::parse_str(&event_id).expect("event id should be a valid uuid");

        assert_eq!(parsed.get_version(), Some(Version::SortRand));
    }
}
