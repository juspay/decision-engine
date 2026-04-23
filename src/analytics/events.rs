use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::analytics::flow::{AnalyticsFlowContext, AnalyticsRoute};
use crate::analytics::flow::{ApiFlow, FlowType};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainAnalyticsEvent {
    pub event_id: String,
    pub api_flow: ApiFlow,
    pub flow_type: FlowType,
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

impl DomainAnalyticsEvent {
    fn base(flow: AnalyticsFlowContext, route: AnalyticsRoute, created_at_ms: i64) -> Self {
        Self {
            event_id: next_event_id(),
            api_flow: flow.api_flow,
            flow_type: flow.flow_type,
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
            created_at_ms,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn decision(
        flow: AnalyticsFlowContext,
        route: AnalyticsRoute,
        merchant_id: Option<String>,
        routing_approach: Option<String>,
        gateway: Option<String>,
        status: Option<String>,
        rule_name: Option<String>,
        details: Option<String>,
        payment_id: Option<String>,
        request_id: Option<String>,
        global_request_id: Option<String>,
        trace_id: Option<String>,
        event_stage: Option<String>,
        payment_method_type: Option<String>,
        payment_method: Option<String>,
        auth_type: Option<String>,
        created_at_ms: i64,
    ) -> Self {
        let lookup_key = derive_lookup_key(payment_id.as_deref(), request_id.as_deref());
        Self {
            merchant_id,
            payment_id,
            request_id,
            lookup_key,
            global_request_id,
            trace_id,
            payment_method_type,
            payment_method,
            auth_type,
            gateway,
            event_stage,
            routing_approach,
            rule_name,
            status,
            details,
            ..Self::base(flow, route, created_at_ms)
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn score_snapshot(
        flow: AnalyticsFlowContext,
        route: AnalyticsRoute,
        merchant_id: Option<String>,
        payment_method_type: Option<String>,
        payment_method: Option<String>,
        card_network: Option<String>,
        card_is_in: Option<String>,
        currency: Option<String>,
        country: Option<String>,
        auth_type: Option<String>,
        gateway: Option<String>,
        score_value: Option<f64>,
        sigma_factor: Option<f64>,
        average_latency: Option<f64>,
        tp99_latency: Option<f64>,
        transaction_count: Option<i64>,
        details: Option<String>,
        payment_id: Option<String>,
        request_id: Option<String>,
        global_request_id: Option<String>,
        trace_id: Option<String>,
        event_stage: Option<String>,
        created_at_ms: i64,
    ) -> Self {
        let lookup_key = derive_lookup_key(payment_id.as_deref(), request_id.as_deref());
        Self {
            merchant_id,
            payment_id,
            request_id,
            lookup_key,
            global_request_id,
            trace_id,
            payment_method_type,
            payment_method,
            card_network,
            card_is_in,
            currency,
            country,
            auth_type,
            gateway,
            event_stage,
            status: Some("snapshot".to_string()),
            score_value,
            sigma_factor,
            average_latency,
            tp99_latency,
            transaction_count,
            details,
            ..Self::base(flow, route, created_at_ms)
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn gateway_update(
        flow: AnalyticsFlowContext,
        route: AnalyticsRoute,
        merchant_id: Option<String>,
        gateway: Option<String>,
        status: Option<String>,
        details: Option<String>,
        payment_id: Option<String>,
        request_id: Option<String>,
        global_request_id: Option<String>,
        trace_id: Option<String>,
        event_stage: Option<String>,
        created_at_ms: i64,
    ) -> Self {
        let lookup_key = derive_lookup_key(payment_id.as_deref(), request_id.as_deref());
        Self {
            merchant_id,
            payment_id,
            request_id,
            lookup_key,
            global_request_id,
            trace_id,
            gateway,
            event_stage,
            status,
            details,
            ..Self::base(flow, route, created_at_ms)
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn rule_hit(
        flow: AnalyticsFlowContext,
        route: AnalyticsRoute,
        merchant_id: Option<String>,
        rule_name: String,
        gateway: Option<String>,
        routing_approach: Option<String>,
        details: Option<String>,
        payment_id: Option<String>,
        request_id: Option<String>,
        global_request_id: Option<String>,
        trace_id: Option<String>,
        event_stage: Option<String>,
        created_at_ms: i64,
    ) -> Self {
        let lookup_key = derive_lookup_key(payment_id.as_deref(), request_id.as_deref());
        Self {
            merchant_id,
            payment_id,
            request_id,
            lookup_key,
            global_request_id,
            trace_id,
            gateway,
            event_stage,
            routing_approach,
            rule_name: Some(rule_name),
            status: Some("hit".to_string()),
            details,
            ..Self::base(flow, route, created_at_ms)
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn rule_evaluation_preview(
        flow: AnalyticsFlowContext,
        merchant_id: Option<String>,
        payment_id: Option<String>,
        gateway: Option<String>,
        rule_name: Option<String>,
        status: Option<String>,
        details: Option<String>,
        request_id: Option<String>,
        global_request_id: Option<String>,
        trace_id: Option<String>,
        created_at_ms: i64,
    ) -> Self {
        let lookup_key = derive_lookup_key(payment_id.as_deref(), request_id.as_deref());
        Self {
            merchant_id,
            payment_id,
            request_id,
            lookup_key,
            global_request_id,
            trace_id,
            gateway,
            event_stage: Some("preview_evaluated".to_string()),
            routing_approach: Some("RULE_EVALUATE_PREVIEW".to_string()),
            rule_name,
            status,
            details,
            ..Self::base(flow, AnalyticsRoute::RoutingEvaluate, created_at_ms)
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn error(
        flow: AnalyticsFlowContext,
        route: AnalyticsRoute,
        merchant_id: Option<String>,
        payment_id: Option<String>,
        request_id: Option<String>,
        global_request_id: Option<String>,
        trace_id: Option<String>,
        gateway: Option<String>,
        routing_approach: Option<String>,
        error_code: String,
        error_message: String,
        details: Option<String>,
        event_stage: Option<String>,
        auth_type: Option<String>,
        created_at_ms: i64,
    ) -> Self {
        let lookup_key = derive_lookup_key(payment_id.as_deref(), request_id.as_deref());
        Self {
            merchant_id,
            payment_id,
            request_id,
            lookup_key,
            global_request_id,
            trace_id,
            auth_type,
            gateway,
            event_stage,
            routing_approach,
            status: Some("failure".to_string()),
            error_code: Some(error_code),
            error_message: Some(error_message),
            details,
            ..Self::base(flow, route, created_at_ms)
        }
    }

    pub fn request_hit(
        flow: AnalyticsFlowContext,
        route: AnalyticsRoute,
        merchant_id: Option<String>,
        payment_id: Option<String>,
        request_id: Option<String>,
        global_request_id: Option<String>,
        trace_id: Option<String>,
        auth_type: Option<String>,
        created_at_ms: i64,
    ) -> Self {
        Self {
            merchant_id,
            payment_id,
            request_id,
            global_request_id,
            trace_id,
            auth_type,
            event_stage: Some("request_received".to_string()),
            status: Some("received".to_string()),
            ..Self::base(flow, route, created_at_ms)
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn operation(
        flow: AnalyticsFlowContext,
        route: AnalyticsRoute,
        merchant_id: Option<String>,
        payment_id: Option<String>,
        request_id: Option<String>,
        global_request_id: Option<String>,
        trace_id: Option<String>,
        status: Option<String>,
        details: Option<String>,
        event_stage: Option<String>,
        created_at_ms: i64,
    ) -> Self {
        let lookup_key = derive_lookup_key(payment_id.as_deref(), request_id.as_deref());
        Self {
            merchant_id,
            payment_id,
            request_id,
            lookup_key,
            global_request_id,
            trace_id,
            event_stage,
            status,
            details,
            ..Self::base(flow, route, created_at_ms)
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
    pub auth_type: Option<String>,
    pub request: String,
    pub user_agent: Option<String>,
    pub ip_addr: Option<String>,
    pub url_path: String,
    pub response: Option<String>,
    pub error: Option<serde_json::Value>,
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

#[cfg(test)]
mod tests {
    use uuid::{Uuid, Version};

    use super::{derive_lookup_key, next_event_id};

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
    fn next_event_id_uses_uuid_v7() {
        let event_id = next_event_id();
        let parsed = Uuid::parse_str(&event_id).expect("event id should be a valid uuid");

        assert_eq!(parsed.get_version(), Some(Version::SortRand));
    }
}
