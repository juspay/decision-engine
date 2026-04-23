use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};

use crate::analytics::flow::{ApiFlow, FlowType};

static EVENT_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainAnalyticsEvent {
    pub event_id: u64,
    pub shard_key: String,
    pub api_flow: ApiFlow,
    pub flow_type: FlowType,
    pub merchant_id: Option<String>,
    pub payment_id: Option<String>,
    pub request_id: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiEvent {
    pub event_id: u64,
    pub shard_key: String,
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

pub fn next_event_id(now_ms: i64) -> u64 {
    let offset = EVENT_COUNTER.fetch_add(1, Ordering::Relaxed) % 1000;
    (now_ms.max(0) as u64)
        .saturating_mul(1000)
        .saturating_add(offset)
}
