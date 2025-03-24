use crate::redis::types::DimensionConf;
use rand::Rng;
use crate::redis::{cache::findByNameFromRedis, types::FeatureConf};

// Converted functions
// Original Haskell function: isFeatureEnabled
pub async fn isFeatureEnabled(f_name: String, mid: String, redis_name: String) -> bool {
    isFeatureEnabledWithMaybeDBConf(None, f_name, mid, redis_name).await
}

// Original Haskell function: isFeatureEnabledWithMaybeDBConf
pub async fn isFeatureEnabledWithMaybeDBConf(
    maybe_db_conf: Option<String>,
    f_name: String,
    mid: String,
    _: String,
) -> bool {
    let maybe_conf = findByNameFromRedis::<FeatureConf>(f_name.clone()).await;
    checkMerchantEnabled(maybe_conf, mid, f_name)
}

// Original Haskell function: isFeatureEnabledByDimension
pub async fn isFeatureEnabledByDimension(f_name: String, dimension: String) -> bool {
    let maybe_conf = findByNameFromRedis::<DimensionConf>(f_name.clone()).await;
    checkDimensionEnabled(maybe_conf, dimension, f_name)
}

// Original Haskell function: checkDimensionEnabled
pub fn checkDimensionEnabled(
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
            isDimensionConfigEnabled(conf, dimension, is_enabled, key)
        }
    }
}

pub fn isDimensionConfigEnabled(
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
pub fn checkMerchantEnabled(conf: Option<FeatureConf>, mid: String, key: String) -> bool {
    match conf {
        None => false,
        Some(conf) => {
            if conf.enableAll {
                if let Some(disable_any) = conf.disableAny {
                    !disable_any.contains(&mid)
                }
                else {
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
    println!("Roller key: {}, num: {}", key, num);
    let mut rng = rand::thread_rng();
    let random_int_v = rng.gen_range(1..=100);
    random_int_v <= num
}
