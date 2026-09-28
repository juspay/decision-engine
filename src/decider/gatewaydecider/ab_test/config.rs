use crate::euclid::types::ABTestData;
use crate::types::service_configuration;

/// service_configuration key for the FeatureConf blob — same key the UI reads/writes.
const FEATURE_CONF_KEY: &str = "ab_test_real_payments_enabled";

pub struct AbTestConfig {
    pub experiment_id: String,
    pub data: ABTestData,
}

/// How long the parsed `FeatureConf` blob is reused before Postgres is read again. The decider
/// asks on every payment; the window is short so a change made in the UI still lands promptly.
const FEATURE_CONF_CACHE_TTL_MS: u64 = 5_000;

/// Parsed `FeatureConf` for [`FEATURE_CONF_KEY`], or `None` when the row is absent or unparsable.
/// `None` is cached too — a missing row is the common case, and it costs a query to discover.
static FEATURE_CONF_CACHE: once_cell::sync::Lazy<
    crate::redis::mem_cache::TypedCache<Option<crate::redis::types::FeatureConf>>,
> = once_cell::sync::Lazy::new(|| {
    crate::redis::mem_cache::TypedCache::new(FEATURE_CONF_CACHE_TTL_MS, 1)
});

pub async fn is_enabled(merchant_id: &str) -> bool {
    let conf = match FEATURE_CONF_CACHE.get(FEATURE_CONF_KEY) {
        Some(cached) => cached,
        None => {
            let fetched = service_configuration::find_config_by_name(FEATURE_CONF_KEY.to_string())
                .await
                .ok()
                .flatten()
                .and_then(|c| c.value)
                .and_then(|v| serde_json::from_str::<crate::redis::types::FeatureConf>(&v).ok());
            FEATURE_CONF_CACHE.store(FEATURE_CONF_KEY.to_string(), fetched.clone());
            fetched
        }
    };

    if let Some(conf) = conf {
        return crate::redis::feature::check_merchant_enabled(
            Some(conf),
            merchant_id.to_string(),
            FEATURE_CONF_KEY.to_string(),
        );
    }
    false
}

/// The merchant's running payment experiment, which sits beside the active rule in its own slot
/// (see the experiment slot in `euclid::handlers::routing_rules`).
pub async fn load_active_ab_test(merchant_id: &str) -> Option<AbTestConfig> {
    let (experiment_id, data) =
        crate::euclid::handlers::routing_rules::active_payment_experiment(merchant_id).await?;
    Some(AbTestConfig {
        experiment_id,
        data,
    })
}
