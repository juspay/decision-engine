use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingRule {
    pub merchant_id: String,
    pub config: ConfigVariant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchRoutingRule {
    pub merchant_id: String,
    pub algorithm: AlgorithmType,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AlgorithmType {
    SuccessRate,
    Elimination,
    DebitRouting,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
#[serde(rename_all = "camelCase")]
pub enum ConfigVariant {
    SuccessRate(SuccessRateData),
    Elimination(EliminationData),
    DebitRouting(DebitRoutingData),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuccessRateData {
    pub default_latency_threshold: Option<f64>,
    pub default_bucket_size: Option<i32>,
    pub default_hedging_percent: Option<f64>,
    pub default_lower_reset_factor: Option<f64>,
    pub default_upper_reset_factor: Option<f64>,
    pub default_gateway_extra_score: Option<Vec<GatewayWiseExtraScore>>,
    pub sub_level_input_config: Option<Vec<SRSubLevelInputConfig>>,
    /// Merchant margin (fraction of ticket, e.g. 0.20). Used in the multi-objective
    /// expected-value ranking `EV = auth·(margin − cost/10_000)`; there is no auth
    /// band or admission gate. Defaults to [`crate::decider::gatewaydecider::
    /// multi_objective::DEFAULT_MARGIN`] when unset.
    pub margin: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SRSubLevelInputConfig {
    pub payment_method_type: Option<String>,
    pub payment_method: Option<String>,
    // Optional cluster dimensions (serialize as cardNetwork/cardIsIn/currency/country/authType).
    // Required here so dimension-scoped sub-level overrides round-trip through /rule/get and
    // /rule/update instead of being silently dropped by serde.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card_network: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card_is_in: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_type: Option<String>,
    /// Provenance: "autopilot" for entries the auto-calibrator created/manages, absent for
    /// human-authored ones. Lets Hard refresh wipe only auto entries and lets the job avoid
    /// overwriting manual overrides.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub latency_threshold: Option<f64>,
    pub bucket_size: Option<i32>,
    pub hedging_percent: Option<f64>,
    pub lower_reset_factor: Option<f64>,
    pub upper_reset_factor: Option<f64>,
    pub gateway_extra_score: Option<Vec<GatewayWiseExtraScore>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayWiseExtraScore {
    pub gateway_name: String,
    pub gateway_sigma_factor: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EliminationData {
    pub threshold: f64,
    pub txnLatency: Option<TransactionLatencyThreshold>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebitRoutingData {
    pub merchant_category_code: String,
    pub acquirer_country: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionLatencyThreshold {
    /// To have a hard threshold for latency in millis, which is used to filter out gateways that exceed this threshold.
    pub gatewayLatency: Option<f64>,
}
