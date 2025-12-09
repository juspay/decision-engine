use crate::logger;
use crate::redis::types::DimensionConf;
use crate::redis::{cache::findByNameFromRedis, types::FeatureConf};
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Converted functions
// Original Haskell function: isFeatureEnabled
pub async fn is_feature_enabled(f_name: String, mid: String, redis_name: String) -> bool {
    is_feature_enabled_with_maybe_db_conf(None, f_name, mid, redis_name).await
}

// Original Haskell function: isFeatureEnabledWithMaybeDBConf
pub async fn is_feature_enabled_with_maybe_db_conf(
    maybe_db_conf: Option<String>,
    f_name: String,
    mid: String,
    _: String,
) -> bool {
    let maybe_conf = findByNameFromRedis::<FeatureConf>(f_name.clone()).await;
    check_merchant_enabled(maybe_conf, mid, f_name)
}

// Original Haskell function: isFeatureEnabledByDimension
pub async fn is_feature_enabled_by_dimension(f_name: String, dimension: String) -> bool {
    let maybe_conf = findByNameFromRedis::<DimensionConf>(f_name.clone()).await;
    check_dimension_enabled(maybe_conf, dimension, f_name)
}

// Original Haskell function: checkDimensionEnabled
pub fn check_dimension_enabled(
    dimension_conf: Option<DimensionConf>,
    dimension: String,
    key: String,
) -> bool {
    match dimension_conf {
        None => false,
        Some(conf) => {
            let is_enabled = if conf.enableAll {
                match conf.enableAllRollout {
                    Some(rollout) => roller(key.clone(), rollout),
                    None => true,
                }
            } else {
                match conf.dimensions {
                    Some(ref dimensions) => {
                        let dimension = dimensions.iter().find(|d_conf| {
                            d_conf.dimension.to_lowercase() == dimension.to_lowercase()
                        });
                        match dimension {
                            Some(dimension) => roller(key.clone(), dimension.rollout),
                            None => false,
                        }
                    }
                    None => false,
                }
            };
            is_dimension_config_enabled(conf, dimension, is_enabled, key)
        }
    }
}

pub fn is_dimension_config_enabled(
    dimension_conf: DimensionConf,
    dimension: String,
    is_enabled: bool,
    key: String,
) -> bool {
    match (is_enabled, dimension_conf.disableAny) {
        (true, Some(disabled)) => {
            let dimension = disabled
                .iter()
                .find(|d| d.dimension.to_lowercase() != dimension.to_lowercase());
            match dimension {
                Some(d) => roller(key, d.rollout),
                None => is_enabled,
            }
        }
        _ => is_enabled,
    }
}

// Original Haskell function: checkMerchantEnabled
pub fn check_merchant_enabled(conf: Option<FeatureConf>, mid: String, key: String) -> bool {
    match conf {
        None => false,
        Some(conf) => {
            if conf.enableAll {
                if let Some(disable_any) = conf.disableAny {
                    !disable_any.contains(&mid)
                } else {
                    match conf.enableAllRollout {
                        Some(rollout) => roller(key, rollout),
                        None => true,
                    }
                }
            } else {
                match conf.merchants {
                    Some(merchants) => {
                        let merchant = merchants
                            .iter()
                            .find(|m_conf| m_conf.merchantId.to_lowercase() == mid.to_lowercase());
                        match merchant {
                            Some(merchant) => roller(key, merchant.rollout),
                            None => false,
                        }
                    }
                    None => false,
                }
            }
        }
    }
}

// Original Haskell function: roller
pub fn roller(key: String, num: i32) -> bool {
    logger::debug!("Roller key: {}, num: {}", key, num);
    let mut rng = rand::thread_rng();
    let random_int_v = rng.gen_range(1..=100);
    random_int_v <= num
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisCompressionConfig {
    pub compEnabled: bool,
    pub dictId: String,
    pub compLevel: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RedisDataStruct {
    STRING,
    HASHMAP,
    STREAM,
    STREAM_V2,
}

impl RedisDataStruct {
    pub fn as_str(&self) -> &'static str {
        match self {
            RedisDataStruct::STRING => "STRING",
            RedisDataStruct::HASHMAP => "HASHMAP",
            RedisDataStruct::STREAM => "STREAM",
            RedisDataStruct::STREAM_V2 => "STREAM_V2",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisCompEnabledMerchant {
    pub rc_merchant_id: String,
    pub rc_rollout: u32,
    pub enabled_dict_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisCompFeatureConf {
    pub rc_enable_all: bool,
    pub rc_enable_all_rollout: Option<u32>,
    pub global_dict_id: String,
    pub rc_disable_any: Option<Vec<String>>,
    pub explicit_enabled_merchants: Vec<RedisCompEnabledMerchant>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RedisCompressionCutover {
    MultiDataStructCutover(HashMap<RedisDataStruct, RedisCompFeatureConf>),
    StringDataStructCutover(RedisCompFeatureConf),
}

pub async fn check_redis_comp_merchant_flag(
    mid: String,
) -> Option<HashMap<String, RedisCompressionConfig>> {
    let mb_conf = findByNameFromRedis::<RedisCompressionCutover>(
        "REDIS_COMPRESSION_MERCHANT_CUTOVER".to_string(),
    )
    .await;

    match mb_conf {
        Some(RedisCompressionCutover::MultiDataStructCutover(ds_map)) => {
            let mut final_map = HashMap::new();

            for (data_struct_key, conf) in ds_map.iter() {
                if let Some(dict_id) =
                    check_merchant_enabled_for_compression(Some(conf.clone()), &mid).await
                {
                    let redis_comp_config = RedisCompressionConfig {
                        compEnabled: true,
                        dictId: dict_id,
                        compLevel: None,
                    };
                    final_map.insert(data_struct_key.as_str().to_string(), redis_comp_config);
                }
            }

            if !final_map.is_empty() {
                logger::info!(
                    "Redis compression config set for merchant {}: {:?}",
                    mid,
                    final_map
                );
                Some(final_map)
            } else {
                None
            }
        }
        Some(RedisCompressionCutover::StringDataStructCutover(conf)) => {
            if let Some(dict_id) =
                check_merchant_enabled_for_compression(Some(conf), &mid).await
            {
                let redis_comp_config = RedisCompressionConfig {
                    compEnabled: true,
                    dictId: dict_id,
                    compLevel: None,
                };
                let mut final_map = HashMap::new();
                final_map.insert("STRING".to_string(), redis_comp_config);

                logger::info!(
                    "Redis compression config set for merchant {}: {:?}",
                    mid,
                    final_map
                );
                Some(final_map)
            } else {
                None
            }
        }
        None => None,
    }
}

async fn check_merchant_enabled_for_compression(
    maybe_conf: Option<RedisCompFeatureConf>,
    mid: &str,
) -> Option<String> {
    match maybe_conf {
        Some(conf) => {
            // First check if merchant is explicitly enabled
            if let Some(dict_id_explicit) = is_merchant_enabled_explicitly(&conf, mid).await {
                return Some(dict_id_explicit);
            }

            // Check global enable with rollout
            let maybe_global_dict_id = if conf.rc_enable_all {
                match conf.rc_enable_all_rollout {
                    Some(rollout) => {
                        // Generate random number for rollout decision
                        let mut rng = rand::thread_rng();
                        let random_int_v: u32 = rng.gen_range(1..=100);

                        if random_int_v <= rollout {
                            Some(conf.global_dict_id.clone())
                        } else {
                            None
                        }
                    }
                    None => Some(conf.global_dict_id.clone()),
                }
            } else {
                None
            };

            is_merchant_enabled_after_disable_check(&conf, mid, maybe_global_dict_id)
        }
        None => None,
    }
}

fn is_merchant_enabled_after_disable_check(
    conf: &RedisCompFeatureConf,
    mid: &str,
    result: Option<String>,
) -> Option<String> {
    match (result, &conf.rc_disable_any) {
        (Some(dict_id), Some(disable_list)) => {
            // Check if merchant is NOT in the disable list
            let mid_lower = mid.to_lowercase();
            let is_disabled = disable_list
                .iter()
                .any(|disabled_mid| disabled_mid.to_lowercase() == mid_lower);

            if is_disabled {
                None
            } else {
                Some(dict_id)
            }
        }
        (res, _) => res,
    }
}

async fn is_merchant_enabled_explicitly(conf: &RedisCompFeatureConf, mid: &str) -> Option<String> {
    let list = &conf.explicit_enabled_merchants;
    let mid_lower = mid.to_lowercase();

    // Find the merchant configuration
    let opt_m_conf = list
        .iter()
        .find(|m_conf| m_conf.rc_merchant_id.to_lowercase() == mid_lower);

    match opt_m_conf {
        Some(m_conf) => {
            // Generate random number for rollout decision
            let mut rng = rand::thread_rng();
            let random_int_v: u32 = rng.gen_range(1..=100);

            if random_int_v <= m_conf.rc_rollout {
                // Return the enabled dict ID if present, otherwise use the global dict ID
                m_conf
                    .enabled_dict_id
                    .clone()
                    .or_else(|| Some(conf.global_dict_id.clone()))
            } else {
                None
            }
        }
        None => None,
    }
}
