use crate::app::get_tenant_app_state;
use crate::euclid::types::{ABTestData, RoutingAlgorithm, RoutingAlgorithmMapper, StaticRoutingAlgorithm};
use crate::types::service_configuration;
use crate::generics::generic_find_one;
use diesel::associations::HasTable;
use diesel::prelude::*;

#[cfg(feature = "mysql")]
use crate::storage::schema::routing_algorithm_mapper::dsl as mapper_dsl;
#[cfg(feature = "mysql")]
use crate::storage::schema::routing_algorithm::dsl as algo_dsl;

#[cfg(feature = "postgres")]
use crate::storage::schema_pg::routing_algorithm_mapper::dsl as mapper_dsl;
#[cfg(feature = "postgres")]
use crate::storage::schema_pg::routing_algorithm::dsl as algo_dsl;

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

pub async fn load_active_ab_test(merchant_id: &str) -> Option<AbTestConfig> {
    let state = get_tenant_app_state().await;

    // Load the active routing algorithm mapper for this merchant
    let mapper = generic_find_one::<
        <RoutingAlgorithmMapper as HasTable>::Table,
        _,
        RoutingAlgorithmMapper,
    >(&state.db, mapper_dsl::created_by.eq(merchant_id.to_string()))
    .await
    .ok()?;

    let experiment_id = mapper.routing_algorithm_id.clone();

    // Load the routing algorithm record
    let algorithm = generic_find_one::<
        <RoutingAlgorithm as HasTable>::Table,
        _,
        RoutingAlgorithm,
    >(&state.db, algo_dsl::id.eq(experiment_id.clone()))
    .await
    .ok()?;

    // Parse and check it's an AB test
    let parsed: StaticRoutingAlgorithm = serde_json::from_str(&algorithm.algorithm_data).ok()?;

    match parsed {
        StaticRoutingAlgorithm::AbTest(data) => Some(AbTestConfig { experiment_id, data }),
        _ => None,
    }
}
