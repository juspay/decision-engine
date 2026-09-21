use crate::euclid::types::ABTestData;
use crate::types::service_configuration;

/// service_configuration key for the FeatureConf blob — same key the UI reads/writes.
const FEATURE_CONF_KEY: &str = "ab_test_real_payments_enabled";

pub struct AbTestConfig {
    pub experiment_id: String,
    pub data: ABTestData,
}

pub async fn is_enabled(merchant_id: &str) -> bool {
    let conf = service_configuration::find_config_by_name(FEATURE_CONF_KEY.to_string())
        .await
        .ok()
        .flatten()
        .and_then(|c| c.value)
        .and_then(|v| serde_json::from_str::<crate::redis::types::FeatureConf>(&v).ok());

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
