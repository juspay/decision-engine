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

// Original Haskell data type: SR_V3_BASED_FLOW_CUTOVER
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct SR_V3_BASED_FLOW_CUTOVER;

impl SC::ServiceConfigKey for SR_V3_BASED_FLOW_CUTOVER {
    fn get_key(&self) -> std::string::String {
        "sr_v3_based_flow_cutover".to_string()
    }
}

// Original Haskell data type: ENABLE_DEBUG_MODE_ON_SR_V3
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct ENABLE_DEBUG_MODE_ON_SR_V3;

// Original Haskell data type: SR_V3_INPUT_CONFIG_DEFAULT
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct SR_V3_INPUT_CONFIG_DEFAULT;

// Original Haskell data type: GW_REF_ID_ENABLED_MERCHANTS_SRV2_PRODUCER
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct GW_REF_ID_ENABLED_MERCHANTS_SRV2_PRODUCER;

impl SC::ServiceConfigKey for GW_REF_ID_ENABLED_MERCHANTS_SRV2_PRODUCER {
    fn get_key(&self) -> std::string::String {
        "gw_ref_id_enabled_merchants_SRv2_producer".to_string()
    }
}

// Original Haskell data type: AUTH_TYPE_SR_ROUTING_PRODUCER_ENABLED_MERCHANT
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct AUTH_TYPE_SR_ROUTING_PRODUCER_ENABLED_MERCHANT;

impl SC::ServiceConfigKey for AUTH_TYPE_SR_ROUTING_PRODUCER_ENABLED_MERCHANT {
    fn get_key(&self) -> std::string::String {
        "auth_type_sr_routing_producer_enabled_merchant".to_string()
    }
}

// Original Haskell data type: BANK_LEVEL_SR_ROUTING_PRODUCER_ENABLED_MERCHANT
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct BANK_LEVEL_SR_ROUTING_PRODUCER_ENABLED_MERCHANT;

impl SC::ServiceConfigKey for BANK_LEVEL_SR_ROUTING_PRODUCER_ENABLED_MERCHANT {
    fn get_key(&self) -> std::string::String {
        "bank_level_sr_routing_producer_enabled_merchant".to_string()
    }
}

// Original Haskell data type: PSP_APP_SR_ROUTING_PRODUCER_ENABLED_MERCHANT
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct PSP_APP_SR_ROUTING_PRODUCER_ENABLED_MERCHANT;

impl SC::ServiceConfigKey for PSP_APP_SR_ROUTING_PRODUCER_ENABLED_MERCHANT {
    fn get_key(&self) -> std::string::String {
        "psp_app_sr_routing_producer_enabled_merchant".to_string()
    }
}

// Original Haskell data type: PSP_PACKAGE_SR_ROUTING_PRODUCER_ENABLED_MERCHANT
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct PSP_PACKAGE_SR_ROUTING_PRODUCER_ENABLED_MERCHANT;

impl SC::ServiceConfigKey for PSP_PACKAGE_SR_ROUTING_PRODUCER_ENABLED_MERCHANT {
    fn get_key(&self) -> std::string::String {
        "psp_package_sr_routing_producer_enabled_merchant".to_string()
    }
}

// Original Haskell data type: SR_V3_INPUT_CONFIG
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct SR_V3_INPUT_CONFIG(pub String);

impl SC::ServiceConfigKey for SR_V3_INPUT_CONFIG {
    fn get_key(&self) -> String {
        format!("SR_V3_INPUT_CONFIG_{}", self.0)
    }
}

// Original Haskell data type: GLOBAL_GATEWAY_SCORING_ENABLED_MERCHANTS
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct GLOBAL_GATEWAY_SCORING_ENABLED_MERCHANTS;

impl SC::ServiceConfigKey for GLOBAL_GATEWAY_SCORING_ENABLED_MERCHANTS {
    fn get_key(&self) -> std::string::String {
        "global_gateway_scoring_enabled_merchants".to_string()
    }
}

// Original Haskell data type: GLOBAL_OUTAGE_GATEWAY_SCORING_ENABLED_MERCHANTS
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct GLOBAL_OUTAGE_GATEWAY_SCORING_ENABLED_MERCHANTS;

impl SC::ServiceConfigKey for GLOBAL_OUTAGE_GATEWAY_SCORING_ENABLED_MERCHANTS {
    fn get_key(&self) -> std::string::String {
        "global_outage_gateway_scoring_enabled_merchants".to_string()
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct UPDATE_SCORE_LOCK_FEATURE_ENABLED_MERCHANT;

impl SC::ServiceConfigKey for UPDATE_SCORE_LOCK_FEATURE_ENABLED_MERCHANT {
    fn get_key(&self) -> std::string::String {
        "update_score_lock_feature_enabled_merchant".to_string()
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct UPDATE_GATEWAY_SCORE_LOCK_FLAG_TTL;

impl SC::ServiceConfigKey for UPDATE_GATEWAY_SCORE_LOCK_FLAG_TTL {
    fn get_key(&self) -> std::string::String {
        "update_gateway_score_lock_flag_ttl".to_string()
    }
}

// Original Haskell data type: MINIMUM_GATEWAY_SCORE
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct MINIMUM_GATEWAY_SCORE;

impl SC::ServiceConfigKey for MINIMUM_GATEWAY_SCORE {
    fn get_key(&self) -> std::string::String {
        "minimum_gateway_score".to_string()
    }
}

// Original Haskell data type: GATEWAY_SCORE_LATENCY_CHECK_IN_MINS
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct GATEWAY_SCORE_LATENCY_CHECK_IN_MINS;

impl SC::ServiceConfigKey for GATEWAY_SCORE_LATENCY_CHECK_IN_MINS {
    fn get_key(&self) -> std::string::String {
        "gateway_score_latency_check_in_mins".to_string()
    }
}

// Original Haskell data type: GATEWAY_SCORE_LATENCY_CHECK_EXEMPT_GATEWAYS
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct GATEWAY_SCORE_LATENCY_CHECK_EXEMPT_GATEWAYS;

impl SC::ServiceConfigKey for GATEWAY_SCORE_LATENCY_CHECK_EXEMPT_GATEWAYS {
    fn get_key(&self) -> std::string::String {
        "gateway_score_latency_check_exempt_gateways".to_string()
    }
}

// Original Haskell data type: DEFAULT_GW_SCORE_LATENCY_THRESHOLD
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct DEFAULT_GW_SCORE_LATENCY_THRESHOLD {
    #[serde(rename = "default_gw_score_latency_threshold")]
    pub default_gw_score_latency_threshold: Option<String>,
}

// Original Haskell data type: GATEWAY_PENALTY_FACTOR
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct GATEWAY_PENALTY_FACTOR;

impl SC::ServiceConfigKey for GATEWAY_PENALTY_FACTOR {
    fn get_key(&self) -> std::string::String {
        "gateway_penalty_factor".to_string()
    }
}

// Original Haskell data type: GATEWAY_REWARD_FACTOR
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct GATEWAY_REWARD_FACTOR;

impl SC::ServiceConfigKey for GATEWAY_REWARD_FACTOR {
    fn get_key(&self) -> std::string::String {
        "gateway_reward_factor".to_string()
    }
}

// Original Haskell data type: OUTAGE_PENALTY_FACTOR
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct OUTAGE_PENALTY_FACTOR;

impl SC::ServiceConfigKey for OUTAGE_PENALTY_FACTOR {
    fn get_key(&self) -> std::string::String {
        "outage_penalty_factor".to_string()
    }
}

// Original Haskell data type: OUTAGE_REWARD_FACTOR
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct OUTAGE_REWARD_FACTOR;

impl SC::ServiceConfigKey for OUTAGE_REWARD_FACTOR {
    fn get_key(&self) -> std::string::String {
        "outage_reward_factor".to_string()
    }
}

// Original Haskell data type: GATEWAY_SCORE_OUTAGE_TTL
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct GATEWAY_SCORE_OUTAGE_TTL;

impl SC::ServiceConfigKey for GATEWAY_SCORE_OUTAGE_TTL {
    fn get_key(&self) -> std::string::String {
        "gateway_score_outage_ttl".to_string()
    }
}

// Original Haskell data type: GATEWAY_SCORE_GLOBAL_TTL
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct GATEWAY_SCORE_GLOBAL_TTL;

impl SC::ServiceConfigKey for GATEWAY_SCORE_GLOBAL_TTL {
    fn get_key(&self) -> std::string::String {
        "gateway_score_global_ttl".to_string()
    }
}

// Original Haskell data type: GATEWAY_SCORE_GLOBAL_OUTAGE_TTL
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct GATEWAY_SCORE_GLOBAL_OUTAGE_TTL;

impl SC::ServiceConfigKey for GATEWAY_SCORE_GLOBAL_OUTAGE_TTL {
    fn get_key(&self) -> std::string::String {
        "gateway_score_global_outage_ttl".to_string()
    }
}

// Original Haskell data type: GATEWAY_SCORE_MERCHANT_ARR_MAX_LENGTH
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct GATEWAY_SCORE_MERCHANT_ARR_MAX_LENGTH;

impl SC::ServiceConfigKey for GATEWAY_SCORE_MERCHANT_ARR_MAX_LENGTH {
    fn get_key(&self) -> std::string::String {
        "gateway_score_merchant_arr_max_length".to_string()
    }
}

// Original Haskell data type: GATEWAY_SCORE_THIRD_DIMENSION_TTL
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct GATEWAY_SCORE_THIRD_DIMENSION_TTL;

impl SC::ServiceConfigKey for GATEWAY_SCORE_THIRD_DIMENSION_TTL {
    fn get_key(&self) -> std::string::String {
        "gateway_score_third_dimension_ttl".to_string()
    }
}

// Original Haskell data type: ENFORCE_GW_SCORE_KV_REDIS
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct ENFORCE_GW_SCORE_KV_REDIS;

impl SC::ServiceConfigKey for ENFORCE_GW_SCORE_KV_REDIS {
    fn get_key(&self) -> std::string::String {
        "enforce_gw_score_kv_redis".to_string()
    }
}

// Original Haskell data type: SR_SCORE_REDIS_FALLBACK_LOOKUP_DISABLE
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct SR_SCORE_REDIS_FALLBACK_LOOKUP_DISABLE;

impl SC::ServiceConfigKey for SR_SCORE_REDIS_FALLBACK_LOOKUP_DISABLE {
    fn get_key(&self) -> std::string::String {
        "sr_score_redis_fallback_lookup_disable".to_string()
    }
}

// Original Haskell data type: SR_V3_PRODUCER_ISOLATION
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct SR_V3_PRODUCER_ISOLATION;

impl SC::ServiceConfigKey for SR_V3_PRODUCER_ISOLATION {
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

// Original Haskell function: pendingTxnsKeyPrefix
pub const pendingTxnsKeyPrefix: &str = "PENDING_TXNS_";

// Original Haskell function: defaultSrV3BasedBucketSize
pub const defaultSrV3BasedBucketSize: i32 = 125;

// Original Haskell function: gatewaySelectionV3OrderTypeKeyPrefix
pub const gatewaySelectionV3OrderTypeKeyPrefix: &str = "{gw_sr_v3_score";

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

// Original Haskell function: gatewayScoringData
pub const gatewayScoringData: &str = "gateway_scoring_data_";

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
