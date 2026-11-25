use crate::euclid::types::ABTestData;

/// service_configuration key for the FeatureConf blob — same key the UI reads/writes.
const FEATURE_CONF_KEY: &str = "ab_test_real_payments_enabled";

pub struct AbTestConfig {
    pub experiment_id: String,
    pub data: ABTestData,
}

/// Whether live-traffic A/B testing is on for the merchant. Read through the service-config cache,
/// which every instance drops when the flag changes on any of them.
pub async fn is_enabled(merchant_id: &str) -> bool {
    crate::redis::feature::is_feature_enabled(
        FEATURE_CONF_KEY.to_string(),
        merchant_id.to_string(),
        crate::feedback::constants::kvRedis(),
    )
    .await
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
