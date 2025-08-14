use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct RoutingRule {
    pub merchant_id: String,
    pub config: ConfigVariant,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FetchRoutingRule {
    pub merchant_id: String,
    pub algorithm: AlgorithmType,
}

#[derive(Debug, Serialize, Deserialize)]
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
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SRSubLevelInputConfig {
    pub payment_method_type: Option<String>,
    pub payment_method: Option<String>,
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
