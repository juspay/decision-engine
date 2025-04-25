use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct RoutingRule {
    pub name: String,
    pub description: String,
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

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ConfigVariant {
    #[serde(rename = "success_rate")]
    SuccessRate(SuccessRateData),

    #[serde(rename = "elimination")]
    Elimination(EliminationData),

    #[serde(rename = "debit_routing")]
    DebitRouting(DebitRoutingData),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuccessRateData {
    pub default_latency_threshold: u32,
    pub default_success_rate: f32,
    pub default_bucket_size: u32,
    pub default_hedging_percent: u8,
    pub sub_level_input_config: Vec<SubLevelInputConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubLevelInputConfig {
    pub payment_method_type: String,
    pub payment_method: String,
    pub bucket_size: u32,
    pub hedging_percent: u8,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EliminationData {
    pub threshold: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DebitRoutingData {
    pub merchant_category_code: String,
    pub acquirer_country: String,
}
