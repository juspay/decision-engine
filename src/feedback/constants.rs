// Automatically converted from Haskell to Rust
// Generated on 2025-03-23 10:19:40

// Converted imports
// use eulerhs::prelude::*;
// use utils::config::service_configuration as SC;
use crate::redis::types as SC;
use serde::{Deserialize, Serialize};

// Converted data types
// Original Haskell data type: Database
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Database {
    #[serde(rename = "ECRDB")]
    ECDB,

    #[serde(rename = "EulerDB")]
    EulerDB,

    #[serde(rename = "ProcessTrackerDB")]
    ProcessTrackerDB,

    #[serde(rename = "UnknownDB")]
    UnknownDB(String),
}

// Original Haskell data type: SrV3BasedFlowCutover
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct SrV3BasedFlowCutover;

impl SC::ServiceConfigKey for SrV3BasedFlowCutover {
    fn get_key(&self) -> std::string::String {
        "sr_v3_based_flow_cutover".to_string()
    }
}

// Original Haskell data type: EnableDebugModeOnSrV3
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct EnableDebugModeOnSrV3;

// Original Haskell data type: SrV3InputConfigDefault
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct SrV3InputConfigDefault;

// Original Haskell data type: GwRefIdEnabledMerchantsSrv2Producer
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct GwRefIdEnabledMerchantsSrv2Producer;

impl SC::ServiceConfigKey for GwRefIdEnabledMerchantsSrv2Producer {
    fn get_key(&self) -> std::string::String {
        "gw_ref_id_enabled_merchants_SRv2_producer".to_string()
    }
}

// Original Haskell data type: AuthTypeSrRoutingProducerEnabledMerchant
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct AuthTypeSrRoutingProducerEnabledMerchant;

impl SC::ServiceConfigKey for AuthTypeSrRoutingProducerEnabledMerchant {
    fn get_key(&self) -> std::string::String {
        "auth_type_sr_routing_producer_enabled_merchant".to_string()
    }
}

// Original Haskell data type: BankLevelSrRoutingProducerEnabledMerchant
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct BankLevelSrRoutingProducerEnabledMerchant;

impl SC::ServiceConfigKey for BankLevelSrRoutingProducerEnabledMerchant {
    fn get_key(&self) -> std::string::String {
        "bank_level_sr_routing_producer_enabled_merchant".to_string()
    }
}

// Original Haskell data type: PspAppSrRoutingProducerEnabledMerchant
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct PspAppSrRoutingProducerEnabledMerchant;

impl SC::ServiceConfigKey for PspAppSrRoutingProducerEnabledMerchant {
    fn get_key(&self) -> std::string::String {
        "psp_app_sr_routing_producer_enabled_merchant".to_string()
    }
}

// Original Haskell data type: PspPackageSrRoutingProducerEnabledMerchant
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct PspPackageSrRoutingProducerEnabledMerchant;

impl SC::ServiceConfigKey for PspPackageSrRoutingProducerEnabledMerchant {
    fn get_key(&self) -> std::string::String {
        "psp_package_sr_routing_producer_enabled_merchant".to_string()
    }
}

// Original Haskell data type: SrV3InputConfig
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct SrV3InputConfig(pub String);

impl SC::ServiceConfigKey for SrV3InputConfig {
    fn get_key(&self) -> String {
        format!("SR_V3_INPUT_CONFIG_{}", self.0)
    }
}

// Original Haskell data type: GlobalGatewayScoringEnabledMerchants
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct GlobalGatewayScoringEnabledMerchants;

impl SC::ServiceConfigKey for GlobalGatewayScoringEnabledMerchants {
    fn get_key(&self) -> std::string::String {
        "global_gateway_scoring_enabled_merchants".to_string()
    }
}

// Original Haskell data type: GlobalOutageGatewayScoringEnabledMerchants
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct GlobalOutageGatewayScoringEnabledMerchants;

impl SC::ServiceConfigKey for GlobalOutageGatewayScoringEnabledMerchants {
    fn get_key(&self) -> std::string::String {
        "global_outage_gateway_scoring_enabled_merchants".to_string()
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct UpdateScoreLockFeatureEnabledMerchant;

impl SC::ServiceConfigKey for UpdateScoreLockFeatureEnabledMerchant {
    fn get_key(&self) -> std::string::String {
        "update_score_lock_feature_enabled_merchant".to_string()
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct UpdateGatewayScoreLockFlagTtl;

impl SC::ServiceConfigKey for UpdateGatewayScoreLockFlagTtl {
    fn get_key(&self) -> std::string::String {
        "update_gateway_score_lock_flag_ttl".to_string()
    }
}

// Original Haskell data type: MinimumGatewayScore
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct MinimumGatewayScore;

impl SC::ServiceConfigKey for MinimumGatewayScore {
    fn get_key(&self) -> std::string::String {
        "minimum_gateway_score".to_string()
    }
}

// Original Haskell data type: GatewayScoreLatencyCheckInMins
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct GatewayScoreLatencyCheckInMins;

impl SC::ServiceConfigKey for GatewayScoreLatencyCheckInMins {
    fn get_key(&self) -> std::string::String {
        "gateway_score_latency_check_in_mins".to_string()
    }
}

// Original Haskell data type: GatewayScoreLatencyCheckExemptGateways
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct GatewayScoreLatencyCheckExemptGateways;

impl SC::ServiceConfigKey for GatewayScoreLatencyCheckExemptGateways {
    fn get_key(&self) -> std::string::String {
        "gateway_score_latency_check_exempt_gateways".to_string()
    }
}

// Original Haskell data type: DefaultGwScoreLatencyThreshold
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct DefaultGwScoreLatencyThreshold {
    #[serde(rename = "default_gw_score_latency_threshold")]
    pub default_gw_score_latency_threshold: Option<String>,
}

// Original Haskell data type: GatewayPenaltyFactor
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct GatewayPenaltyFactor;

impl SC::ServiceConfigKey for GatewayPenaltyFactor {
    fn get_key(&self) -> std::string::String {
        "gateway_penalty_factor".to_string()
    }
}

// Original Haskell data type: GatewayRewardFactor
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct GatewayRewardFactor;

impl SC::ServiceConfigKey for GatewayRewardFactor {
    fn get_key(&self) -> std::string::String {
        "gateway_reward_factor".to_string()
    }
}

// Original Haskell data type: OutagePenaltyFactor
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct OutagePenaltyFactor;

impl SC::ServiceConfigKey for OutagePenaltyFactor {
    fn get_key(&self) -> std::string::String {
        "outage_penalty_factor".to_string()
    }
}

// Original Haskell data type: OutageRewardFactor
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct OutageRewardFactor;

impl SC::ServiceConfigKey for OutageRewardFactor {
    fn get_key(&self) -> std::string::String {
        "outage_reward_factor".to_string()
    }
}

// Original Haskell data type: GatewayScoreOutageTtl
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct GatewayScoreOutageTtl;

impl SC::ServiceConfigKey for GatewayScoreOutageTtl {
    fn get_key(&self) -> std::string::String {
        "gateway_score_outage_ttl".to_string()
    }
}

// Original Haskell data type: GatewayScoreGlobalTtl
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct GatewayScoreGlobalTtl;

impl SC::ServiceConfigKey for GatewayScoreGlobalTtl {
    fn get_key(&self) -> std::string::String {
        "gateway_score_global_ttl".to_string()
    }
}

// Original Haskell data type: GatewayScoreGlobalOutageTtl
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct GatewayScoreGlobalOutageTtl;

impl SC::ServiceConfigKey for GatewayScoreGlobalOutageTtl {
    fn get_key(&self) -> std::string::String {
        "gateway_score_global_outage_ttl".to_string()
    }
}

// Original Haskell data type: GatewayScoreMerchantArrMaxLength
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct GatewayScoreMerchantArrMaxLength;

impl SC::ServiceConfigKey for GatewayScoreMerchantArrMaxLength {
    fn get_key(&self) -> std::string::String {
        "gateway_score_merchant_arr_max_length".to_string()
    }
}

// Original Haskell data type: GatewayScoreThirdDimensionTtl
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct GatewayScoreThirdDimensionTtl;

impl SC::ServiceConfigKey for GatewayScoreThirdDimensionTtl {
    fn get_key(&self) -> std::string::String {
        "gateway_score_third_dimension_ttl".to_string()
    }
}

// Original Haskell data type: EnforceGwScoreKvRedis
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct EnforceGwScoreKvRedis;

impl SC::ServiceConfigKey for EnforceGwScoreKvRedis {
    fn get_key(&self) -> std::string::String {
        "enforce_gw_score_kv_redis".to_string()
    }
}

// Original Haskell data type: SrScoreRedisFallbackLookupDisable
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct SrScoreRedisFallbackLookupDisable;

impl SC::ServiceConfigKey for SrScoreRedisFallbackLookupDisable {
    fn get_key(&self) -> std::string::String {
        "sr_score_redis_fallback_lookup_disable".to_string()
    }
}

// Original Haskell data type: SrV3ProducerIsolation
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct SrV3ProducerIsolation;

impl SC::ServiceConfigKey for SrV3ProducerIsolation {
    fn get_key(&self) -> std::string::String {
        "sr_v3_producer_isolation".to_string()
    }
}

// Converted functions
// Original Haskell function: ecDB
pub fn ecDB() -> Database {
    Database::ECDB
}

// Original Haskell function: kvRedis
pub fn kvRedis() -> String {
    "kv_redis".into()
}

// Original Haskell function: PENDING_TXNS_KEY_PREFIX
pub const PENDING_TXNS_KEY_PREFIX: &str = "PENDING_TXNS_";

// Original Haskell function: DEFAULT_SR_V3_BASED_BUCKET_SIZE
pub const DEFAULT_SR_V3_BASED_BUCKET_SIZE: i32 = 125;

// Original Haskell function: GATEWAY_SELECTION_V3_ORDER_TYPE_KEY_PREFIX
pub const GATEWAY_SELECTION_V3_ORDER_TYPE_KEY_PREFIX: &str = "{gw_sr_v3_score";

// Original Haskell function: ecRedis
pub fn ecRedis() -> String {
    "ECRRedis".into()
}

// Original Haskell function: ecRedis2
pub fn ecRedis2() -> String {
    "ECRRedis2".to_string()
}

// Original Haskell function: kvRedis2
pub fn kvRedis2() -> String {
    "KVRedis2".into()
}

// Original Haskell function: defaultGWScoringRewardFactor
pub fn defaultGWScoringRewardFactor() -> f64 {
    5.26
}

// Original Haskell function: defaultGWScoringPenaltyFactor
pub fn defaultGWScoringPenaltyFactor() -> f64 {
    5.0
}

// Original Haskell function: defaultScoreKeysTTL
pub fn defaultScoreKeysTTL() -> u128 {
    9000000
}

// Original Haskell function: defaultScoreGlobalKeysTTL
pub fn defaultScoreGlobalKeysTTL() -> u128 {
    1800000
}

// Original Haskell function: defaultGatewayScoreLatencyCheckInMins
pub fn defaultGatewayScoreLatencyCheckInMins() -> u128 {
    15
}

// Original Haskell function: defaultMinimumGatewayScore
pub fn defaultMinimumGatewayScore() -> f64 {
    0.0
}

// Original Haskell function: GATEWAY_SCORING_DATA
pub const GATEWAY_SCORING_DATA: &str = "gateway_scoring_data_";

// Original Haskell function: defaultMerchantArrMaxLength
pub fn defaultMerchantArrMaxLength() -> i32 {
    40
}

pub fn defaultUpdateGatewayScoreLockFlagTtl() -> i32 {
    300
}

pub fn defaultSrV3LatencyThresholdInSecs() -> f64 {
    300 as f64
}
