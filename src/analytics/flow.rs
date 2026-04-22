use axum::http::Method;
use serde::{Deserialize, Serialize};

use crate::euclid::types::StaticRoutingAlgorithm;
use crate::types::routing_configuration::{AlgorithmType, ConfigVariant};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ApiFlow {
    DynamicRouting,
    RuleBasedRouting,
    Analytics,
    MerchantAccount,
}

impl ApiFlow {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DynamicRouting => "dynamic_routing",
            Self::RuleBasedRouting => "rule_based_routing",
            Self::Analytics => "analytics",
            Self::MerchantAccount => "merchant_account",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FlowType {
    DecideGateway,
    UpdateGatewayScore,
    UpdateScoreLegacy,
    DecisionGatewayLegacy,
    RoutingHybrid,
    RoutingCreate,
    RoutingActivate,
    RoutingList,
    RoutingListActive,
    RoutingEvaluate,
    RuleConfigCreate,
    RuleConfigGet,
    RuleConfigUpdate,
    RuleConfigDelete,
    ConfigSrDimension,
    GetRoutingConfig,
    MerchantAccountCreate,
    MerchantAccountGet,
    MerchantAccountDelete,
    AnalyticsOverview,
    AnalyticsGatewayScores,
    AnalyticsDecisions,
    AnalyticsRoutingStats,
    AnalyticsLogSummaries,
    AnalyticsPaymentAudit,
    AnalyticsPreviewTrace,
    RoutingCreateSingle,
    RoutingCreatePriority,
    RoutingCreateVolumeSplit,
    RoutingCreateAdvanced,
    RoutingEvaluateSingle,
    RoutingEvaluatePriority,
    RoutingEvaluateVolumeSplit,
    RoutingEvaluateAdvanced,
    RuleConfigCreateSuccessRate,
    RuleConfigCreateElimination,
    RuleConfigCreateDebitRouting,
    RuleConfigGetSuccessRate,
    RuleConfigGetElimination,
    RuleConfigGetDebitRouting,
    RuleConfigUpdateSuccessRate,
    RuleConfigUpdateElimination,
    RuleConfigUpdateDebitRouting,
    RuleConfigDeleteSuccessRate,
    RuleConfigDeleteElimination,
    RuleConfigDeleteDebitRouting,
    DecideGatewayRequestHit,
    DecideGatewayDecision,
    DecideGatewayRuleHit,
    DecideGatewayError,
    UpdateGatewayScoreRequestHit,
    UpdateGatewayScoreUpdate,
    UpdateGatewayScoreScoreSnapshot,
    UpdateGatewayScoreError,
    UpdateScoreLegacyScoreSnapshot,
    UpdateScoreLegacyError,
    RoutingEvaluateRequestHit,
    RoutingEvaluatePreview,
    RoutingEvaluateError,
}

impl FlowType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DecideGateway => "decide_gateway",
            Self::UpdateGatewayScore => "update_gateway_score",
            Self::UpdateScoreLegacy => "update_score_legacy",
            Self::DecisionGatewayLegacy => "decision_gateway_legacy",
            Self::RoutingHybrid => "routing_hybrid",
            Self::RoutingCreate => "routing_create",
            Self::RoutingActivate => "routing_activate",
            Self::RoutingList => "routing_list",
            Self::RoutingListActive => "routing_list_active",
            Self::RoutingEvaluate => "routing_evaluate",
            Self::RuleConfigCreate => "rule_config_create",
            Self::RuleConfigGet => "rule_config_get",
            Self::RuleConfigUpdate => "rule_config_update",
            Self::RuleConfigDelete => "rule_config_delete",
            Self::ConfigSrDimension => "config_sr_dimension",
            Self::GetRoutingConfig => "get_routing_config",
            Self::MerchantAccountCreate => "merchant_account_create",
            Self::MerchantAccountGet => "merchant_account_get",
            Self::MerchantAccountDelete => "merchant_account_delete",
            Self::AnalyticsOverview => "analytics_overview",
            Self::AnalyticsGatewayScores => "analytics_gateway_scores",
            Self::AnalyticsDecisions => "analytics_decisions",
            Self::AnalyticsRoutingStats => "analytics_routing_stats",
            Self::AnalyticsLogSummaries => "analytics_log_summaries",
            Self::AnalyticsPaymentAudit => "analytics_payment_audit",
            Self::AnalyticsPreviewTrace => "analytics_preview_trace",
            Self::RoutingCreateSingle => "routing_create_single",
            Self::RoutingCreatePriority => "routing_create_priority",
            Self::RoutingCreateVolumeSplit => "routing_create_volume_split",
            Self::RoutingCreateAdvanced => "routing_create_advanced",
            Self::RoutingEvaluateSingle => "routing_evaluate_single",
            Self::RoutingEvaluatePriority => "routing_evaluate_priority",
            Self::RoutingEvaluateVolumeSplit => "routing_evaluate_volume_split",
            Self::RoutingEvaluateAdvanced => "routing_evaluate_advanced",
            Self::RuleConfigCreateSuccessRate => "rule_config_create_success_rate",
            Self::RuleConfigCreateElimination => "rule_config_create_elimination",
            Self::RuleConfigCreateDebitRouting => "rule_config_create_debit_routing",
            Self::RuleConfigGetSuccessRate => "rule_config_get_success_rate",
            Self::RuleConfigGetElimination => "rule_config_get_elimination",
            Self::RuleConfigGetDebitRouting => "rule_config_get_debit_routing",
            Self::RuleConfigUpdateSuccessRate => "rule_config_update_success_rate",
            Self::RuleConfigUpdateElimination => "rule_config_update_elimination",
            Self::RuleConfigUpdateDebitRouting => "rule_config_update_debit_routing",
            Self::RuleConfigDeleteSuccessRate => "rule_config_delete_success_rate",
            Self::RuleConfigDeleteElimination => "rule_config_delete_elimination",
            Self::RuleConfigDeleteDebitRouting => "rule_config_delete_debit_routing",
            Self::DecideGatewayRequestHit => "decide_gateway_request_hit",
            Self::DecideGatewayDecision => "decide_gateway_decision",
            Self::DecideGatewayRuleHit => "decide_gateway_rule_hit",
            Self::DecideGatewayError => "decide_gateway_error",
            Self::UpdateGatewayScoreRequestHit => "update_gateway_score_request_hit",
            Self::UpdateGatewayScoreUpdate => "update_gateway_score_update",
            Self::UpdateGatewayScoreScoreSnapshot => "update_gateway_score_score_snapshot",
            Self::UpdateGatewayScoreError => "update_gateway_score_error",
            Self::UpdateScoreLegacyScoreSnapshot => "update_score_legacy_score_snapshot",
            Self::UpdateScoreLegacyError => "update_score_legacy_error",
            Self::RoutingEvaluateRequestHit => "routing_evaluate_request_hit",
            Self::RoutingEvaluatePreview => "routing_evaluate_preview",
            Self::RoutingEvaluateError => "routing_evaluate_error",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct AnalyticsFlowContext {
    pub api_flow: ApiFlow,
    pub flow_type: FlowType,
}

impl AnalyticsFlowContext {
    pub const fn new(api_flow: ApiFlow, flow_type: FlowType) -> Self {
        Self {
            api_flow,
            flow_type,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnalyticsRoute {
    DecideGateway,
    UpdateGatewayScore,
    UpdateScore,
    RoutingEvaluate,
    RoutingCreate,
    RuleConfigCreate,
    RuleConfigGet,
    RuleConfigUpdate,
    RuleConfigDelete,
}

impl AnalyticsRoute {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DecideGateway => "decide_gateway",
            Self::UpdateGatewayScore => "update_gateway_score",
            Self::UpdateScore => "update_score",
            Self::RoutingEvaluate => "routing_evaluate",
            Self::RoutingCreate => "routing_create",
            Self::RuleConfigCreate => "rule_config_create",
            Self::RuleConfigGet => "rule_config_get",
            Self::RuleConfigUpdate => "rule_config_update",
            Self::RuleConfigDelete => "rule_config_delete",
        }
    }

    pub fn from_stored_value(value: &str) -> Option<Self> {
        match value {
            "decision_gateway" | "decide_gateway" => Some(Self::DecideGateway),
            "update_gateway_score" => Some(Self::UpdateGatewayScore),
            "update_score" => Some(Self::UpdateScore),
            "routing_evaluate" => Some(Self::RoutingEvaluate),
            "routing_create" => Some(Self::RoutingCreate),
            "rule_config_create" => Some(Self::RuleConfigCreate),
            "rule_config_get" => Some(Self::RuleConfigGet),
            "rule_config_update" => Some(Self::RuleConfigUpdate),
            "rule_config_delete" => Some(Self::RuleConfigDelete),
            _ => None,
        }
    }

    pub fn from_filter_value(value: &str) -> Option<Self> {
        let trimmed = value.trim();
        match trimmed {
            "Decide Gateway" => Some(Self::DecideGateway),
            "Update Gateway" => Some(Self::UpdateGatewayScore),
            "Update Score" => Some(Self::UpdateScore),
            "Rule Evaluate" => Some(Self::RoutingEvaluate),
            "Routing Create" => Some(Self::RoutingCreate),
            "Rule Config Create" => Some(Self::RuleConfigCreate),
            "Rule Config Get" => Some(Self::RuleConfigGet),
            "Rule Config Update" => Some(Self::RuleConfigUpdate),
            "Rule Config Delete" => Some(Self::RuleConfigDelete),
            _ => Self::from_stored_value(trimmed),
        }
    }

    pub const fn payment_audit_label(self) -> &'static str {
        match self {
            Self::DecideGateway => "Decide Gateway",
            Self::UpdateGatewayScore => "Update Gateway",
            Self::UpdateScore => "Update Score",
            Self::RoutingEvaluate => "Rule Evaluate",
            Self::RoutingCreate => "Routing Create",
            Self::RuleConfigCreate => "Rule Config Create",
            Self::RuleConfigGet => "Rule Config Get",
            Self::RuleConfigUpdate => "Rule Config Update",
            Self::RuleConfigDelete => "Rule Config Delete",
        }
    }

    pub const fn overview_label(self) -> Option<&'static str> {
        match self {
            Self::DecideGateway => Some("/decide_gateway"),
            Self::UpdateGatewayScore => Some("/update_gateway"),
            Self::RoutingEvaluate => Some("/rule_evaluate"),
            _ => None,
        }
    }
}

pub fn classify_request(method: &Method, path: &str) -> Option<AnalyticsFlowContext> {
    let normalized_path = normalize_path(path);
    let segments = split_path(normalized_path);

    match (method.as_str(), segments.as_slice()) {
        ("POST", ["decide-gateway"]) => Some(AnalyticsFlowContext::new(
            ApiFlow::DynamicRouting,
            FlowType::DecideGateway,
        )),
        ("POST", ["update-gateway-score"]) => Some(AnalyticsFlowContext::new(
            ApiFlow::DynamicRouting,
            FlowType::UpdateGatewayScore,
        )),
        ("POST", ["update-score"]) => Some(AnalyticsFlowContext::new(
            ApiFlow::DynamicRouting,
            FlowType::UpdateScoreLegacy,
        )),
        ("POST", ["decision_gateway"]) => Some(AnalyticsFlowContext::new(
            ApiFlow::DynamicRouting,
            FlowType::DecisionGatewayLegacy,
        )),
        ("POST", ["routing", "hybrid"]) => Some(AnalyticsFlowContext::new(
            ApiFlow::DynamicRouting,
            FlowType::RoutingHybrid,
        )),
        ("POST", ["routing", "create"]) => Some(AnalyticsFlowContext::new(
            ApiFlow::RuleBasedRouting,
            FlowType::RoutingCreate,
        )),
        ("POST", ["routing", "activate"]) => Some(AnalyticsFlowContext::new(
            ApiFlow::RuleBasedRouting,
            FlowType::RoutingActivate,
        )),
        ("POST", ["routing", "list", "active", _]) => Some(AnalyticsFlowContext::new(
            ApiFlow::RuleBasedRouting,
            FlowType::RoutingListActive,
        )),
        ("POST", ["routing", "list", _]) => Some(AnalyticsFlowContext::new(
            ApiFlow::RuleBasedRouting,
            FlowType::RoutingList,
        )),
        ("POST", ["routing", "evaluate"]) => Some(AnalyticsFlowContext::new(
            ApiFlow::RuleBasedRouting,
            FlowType::RoutingEvaluate,
        )),
        ("POST", ["rule", "create"]) => Some(AnalyticsFlowContext::new(
            ApiFlow::RuleBasedRouting,
            FlowType::RuleConfigCreate,
        )),
        ("POST", ["rule", "get"]) => Some(AnalyticsFlowContext::new(
            ApiFlow::RuleBasedRouting,
            FlowType::RuleConfigGet,
        )),
        ("POST", ["rule", "update"]) => Some(AnalyticsFlowContext::new(
            ApiFlow::RuleBasedRouting,
            FlowType::RuleConfigUpdate,
        )),
        ("POST", ["rule", "delete"]) => Some(AnalyticsFlowContext::new(
            ApiFlow::RuleBasedRouting,
            FlowType::RuleConfigDelete,
        )),
        ("POST", ["config-sr-dimension"]) => Some(AnalyticsFlowContext::new(
            ApiFlow::RuleBasedRouting,
            FlowType::ConfigSrDimension,
        )),
        ("GET", ["config", "routing-keys"]) => Some(AnalyticsFlowContext::new(
            ApiFlow::RuleBasedRouting,
            FlowType::GetRoutingConfig,
        )),
        ("POST", ["merchant-account", "create"]) => Some(AnalyticsFlowContext::new(
            ApiFlow::MerchantAccount,
            FlowType::MerchantAccountCreate,
        )),
        ("GET", ["merchant-account", _]) => Some(AnalyticsFlowContext::new(
            ApiFlow::MerchantAccount,
            FlowType::MerchantAccountGet,
        )),
        ("DELETE", ["merchant-account", _]) => Some(AnalyticsFlowContext::new(
            ApiFlow::MerchantAccount,
            FlowType::MerchantAccountDelete,
        )),
        ("GET", ["analytics"]) | ("GET", ["analytics", "overview"]) => Some(
            AnalyticsFlowContext::new(ApiFlow::Analytics, FlowType::AnalyticsOverview),
        ),
        ("GET", ["analytics", "gateway-scores"]) => Some(AnalyticsFlowContext::new(
            ApiFlow::Analytics,
            FlowType::AnalyticsGatewayScores,
        )),
        ("GET", ["analytics", "decisions"]) => Some(AnalyticsFlowContext::new(
            ApiFlow::Analytics,
            FlowType::AnalyticsDecisions,
        )),
        ("GET", ["analytics", "routing-stats"]) => Some(AnalyticsFlowContext::new(
            ApiFlow::Analytics,
            FlowType::AnalyticsRoutingStats,
        )),
        ("GET", ["analytics", "log-summaries"]) => Some(AnalyticsFlowContext::new(
            ApiFlow::Analytics,
            FlowType::AnalyticsLogSummaries,
        )),
        ("GET", ["analytics", "payment-audit"]) => Some(AnalyticsFlowContext::new(
            ApiFlow::Analytics,
            FlowType::AnalyticsPaymentAudit,
        )),
        ("GET", ["analytics", "preview-trace"]) => Some(AnalyticsFlowContext::new(
            ApiFlow::Analytics,
            FlowType::AnalyticsPreviewTrace,
        )),
        _ => None,
    }
}

pub fn refine_routing_create_flow_type(algorithm: &StaticRoutingAlgorithm) -> FlowType {
    match algorithm {
        StaticRoutingAlgorithm::Single(_) => FlowType::RoutingCreateSingle,
        StaticRoutingAlgorithm::Priority(_) => FlowType::RoutingCreatePriority,
        StaticRoutingAlgorithm::VolumeSplit(_) => FlowType::RoutingCreateVolumeSplit,
        StaticRoutingAlgorithm::Advanced(_) => FlowType::RoutingCreateAdvanced,
    }
}

pub fn refine_routing_evaluate_flow_type(algorithm: &StaticRoutingAlgorithm) -> FlowType {
    match algorithm {
        StaticRoutingAlgorithm::Single(_) => FlowType::RoutingEvaluateSingle,
        StaticRoutingAlgorithm::Priority(_) => FlowType::RoutingEvaluatePriority,
        StaticRoutingAlgorithm::VolumeSplit(_) => FlowType::RoutingEvaluateVolumeSplit,
        StaticRoutingAlgorithm::Advanced(_) => FlowType::RoutingEvaluateAdvanced,
    }
}

pub fn refine_rule_config_create_flow_type(config: &ConfigVariant) -> FlowType {
    match config {
        ConfigVariant::SuccessRate(_) => FlowType::RuleConfigCreateSuccessRate,
        ConfigVariant::Elimination(_) => FlowType::RuleConfigCreateElimination,
        ConfigVariant::DebitRouting(_) => FlowType::RuleConfigCreateDebitRouting,
    }
}

pub fn refine_rule_config_get_flow_type(config: &ConfigVariant) -> FlowType {
    match config {
        ConfigVariant::SuccessRate(_) => FlowType::RuleConfigGetSuccessRate,
        ConfigVariant::Elimination(_) => FlowType::RuleConfigGetElimination,
        ConfigVariant::DebitRouting(_) => FlowType::RuleConfigGetDebitRouting,
    }
}

pub fn refine_rule_config_update_flow_type(config: &ConfigVariant) -> FlowType {
    match config {
        ConfigVariant::SuccessRate(_) => FlowType::RuleConfigUpdateSuccessRate,
        ConfigVariant::Elimination(_) => FlowType::RuleConfigUpdateElimination,
        ConfigVariant::DebitRouting(_) => FlowType::RuleConfigUpdateDebitRouting,
    }
}

pub fn refine_rule_config_delete_flow_type(config: &ConfigVariant) -> FlowType {
    match config {
        ConfigVariant::SuccessRate(_) => FlowType::RuleConfigDeleteSuccessRate,
        ConfigVariant::Elimination(_) => FlowType::RuleConfigDeleteElimination,
        ConfigVariant::DebitRouting(_) => FlowType::RuleConfigDeleteDebitRouting,
    }
}

pub fn flow_type_for_rule_config_algorithm_get(algorithm: &AlgorithmType) -> FlowType {
    match algorithm {
        AlgorithmType::SuccessRate => FlowType::RuleConfigGetSuccessRate,
        AlgorithmType::Elimination => FlowType::RuleConfigGetElimination,
        AlgorithmType::DebitRouting => FlowType::RuleConfigGetDebitRouting,
    }
}

pub fn flow_type_for_rule_config_algorithm_delete(algorithm: &AlgorithmType) -> FlowType {
    match algorithm {
        AlgorithmType::SuccessRate => FlowType::RuleConfigDeleteSuccessRate,
        AlgorithmType::Elimination => FlowType::RuleConfigDeleteElimination,
        AlgorithmType::DebitRouting => FlowType::RuleConfigDeleteDebitRouting,
    }
}

fn normalize_path(path: &str) -> &str {
    let trimmed = path.trim_end_matches('/');
    if trimmed.is_empty() {
        "/"
    } else {
        trimmed
    }
}

fn split_path(path: &str) -> Vec<&str> {
    path.trim_start_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;

    #[test]
    fn serializes_enum_names_as_snake_case_strings() {
        assert_eq!(
            serde_json::to_string(&ApiFlow::DynamicRouting).unwrap(),
            "\"dynamic_routing\""
        );
        assert_eq!(
            serde_json::to_string(&FlowType::RoutingEvaluateVolumeSplit).unwrap(),
            "\"routing_evaluate_volume_split\""
        );
    }

    #[test]
    fn classifies_dynamic_routing_routes() {
        assert_eq!(
            classify_request(&Method::POST, "/decide-gateway"),
            Some(AnalyticsFlowContext::new(
                ApiFlow::DynamicRouting,
                FlowType::DecideGateway,
            ))
        );
        assert_eq!(
            classify_request(&Method::POST, "/update-score"),
            Some(AnalyticsFlowContext::new(
                ApiFlow::DynamicRouting,
                FlowType::UpdateScoreLegacy,
            ))
        );
        assert_eq!(
            classify_request(&Method::POST, "/decision_gateway"),
            Some(AnalyticsFlowContext::new(
                ApiFlow::DynamicRouting,
                FlowType::DecisionGatewayLegacy,
            ))
        );
    }

    #[test]
    fn classifies_merchant_account_routes_by_method() {
        assert_eq!(
            classify_request(&Method::POST, "/merchant-account/create"),
            Some(AnalyticsFlowContext::new(
                ApiFlow::MerchantAccount,
                FlowType::MerchantAccountCreate,
            ))
        );
        assert_eq!(
            classify_request(&Method::GET, "/merchant-account/mid_123"),
            Some(AnalyticsFlowContext::new(
                ApiFlow::MerchantAccount,
                FlowType::MerchantAccountGet,
            ))
        );
        assert_eq!(
            classify_request(&Method::DELETE, "/merchant-account/mid_123"),
            Some(AnalyticsFlowContext::new(
                ApiFlow::MerchantAccount,
                FlowType::MerchantAccountDelete,
            ))
        );
    }

    #[test]
    fn classifies_analytics_routes() {
        assert_eq!(
            classify_request(&Method::GET, "/analytics"),
            Some(AnalyticsFlowContext::new(
                ApiFlow::Analytics,
                FlowType::AnalyticsOverview,
            ))
        );
        assert_eq!(
            classify_request(&Method::GET, "/analytics/preview-trace"),
            Some(AnalyticsFlowContext::new(
                ApiFlow::Analytics,
                FlowType::AnalyticsPreviewTrace,
            ))
        );
    }
}
