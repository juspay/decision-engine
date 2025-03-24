pub mod configs;
pub mod errors;
pub mod proto;
pub mod types;

use error_stack::ResultExt;

use crate::{
    contract_routing::{
        configs::CalContractScoreConfig, errors::ContractRoutingError, types as cr_types,
    },
    helpers, logger,
    utils::ValueExt,
    DynamicRouting,
};

#[async_trait::async_trait]
impl DynamicRouting for cr_types::ContractRouting {
    type UpdateWindowReport = Vec<cr_types::ContractMap>;
    type RoutingResponse = Vec<(f64, String, u64)>;

    type Error = ContractRoutingError;

    async fn perform_routing(
        &self,
        id: &str,
        params: &str,
        labels: Vec<String>,
        config: serde_json::Value,
        tenant_id: &Option<String>,
    ) -> error_stack::Result<Self::RoutingResponse, Self::Error> {
        let config = config
            .parse_value::<CalContractScoreConfig>("CalContractScoreConfig")
            .change_context(ContractRoutingError::DeserializationFailed)?;

        let mut ct_scores = Vec::with_capacity(labels.len());

        for label in labels {
            let redis_key = helpers::redis_key_create_with_suffix(
                types::CONTRACT_ROUTING_PREFIX_IN_REDIS,
                tenant_id,
                &id,
                &params,
                &label,
                types::CONTRACT_ROUTING_MAP_SUFFIX_IN_REDIS,
            );

            let contract_map = self
                .fetch_contract_map_from_redis::<cr_types::ContractMap>(&redis_key)
                .await?
                .ok_or(ContractRoutingError::ContractNotFound)?;

            #[allow(clippy::as_conversions)]
            let score = self.calculate_ct_score(
                contract_map.target_count as f64,
                contract_map.current_count as f64,
                contract_map.target_time as f64,
                config.constants.clone(),
                config.time_scale,
            )?;

            ct_scores.push((score, label, contract_map.current_count));
        }

        ct_scores.as_mut_slice().sort_by(|a, b| b.0.total_cmp(&a.0));

        Ok(ct_scores)
    }

    async fn update_window(
        &self,
        id: &str,
        params: &str,
        report: Self::UpdateWindowReport,
        _config: serde_json::Value,
        tenant_id: &Option<String>,
    ) -> error_stack::Result<(), Self::Error> {
        for label_info in report {
            let label = label_info.label.clone();
            let redis_key = helpers::redis_key_create_with_suffix(
                types::CONTRACT_ROUTING_PREFIX_IN_REDIS,
                tenant_id,
                &id,
                &params,
                &label,
                types::CONTRACT_ROUTING_MAP_SUFFIX_IN_REDIS,
            );

            // Check if data exists in redis
            let maybe_contract_map = self
                .fetch_contract_map_from_redis::<cr_types::ContractMap>(&redis_key)
                .await?;

            let final_cotract_map = match maybe_contract_map {
                // Should invalidate on contract completion?
                Some(mut existing_map) => {
                    if existing_map.current_count == existing_map.target_count {
                        return Ok(());
                    }
                    existing_map.current_count += label_info.current_count;
                    existing_map
                }
                None => label_info,
            };

            self.set_contract_map_in_redis(&redis_key, final_cotract_map)
                .await?
        }

        Ok(())
    }

    async fn invalidate_metrics(
        &self,
        id: &str,
        tenant_id: &Option<String>,
    ) -> error_stack::Result<(), Self::Error> {
        let prefix = helpers::redis_key_create_for_metrics_invalidation(
            types::CONTRACT_ROUTING_PREFIX_IN_REDIS,
            tenant_id,
            &id,
        );
        let keys_deleted = self.delete_keys_matching_prefix(&prefix).await?;

        logger::debug!(?keys_deleted, "List of keys invalidated from redis");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use crate::{
        configs::Config,
        contract_routing::{configs, types},
        success_rate::utils::get_current_time_in_secs,
        utils::Encode,
        DynamicRouting,
    };
    use redis_interface::{RedisConnectionPool, RedisSettings};
    use std::sync::Arc;

    async fn construct_ct_score_type() -> types::ContractRouting {
        let app_config = Config::new().expect("Failed to construct config");
        let redis_config = RedisSettings::default();
        let redis_conn = Arc::new(
            RedisConnectionPool::new(&redis_config)
                .await
                .expect("failed to create redis connection pool"),
        );

        let config = types::ContractRoutingConfig {
            keys_ttl: app_config.ttl_for_keys,
            is_multi_tenancy_enabled: app_config.multi_tenancy.enabled,
        };

        types::ContractRouting::new(config, redis_conn)
    }

    #[tokio::test]
    async fn update_contract() {
        let ct_type = construct_ct_score_type().await;
        let id = "merchant1".to_string();
        let params = "card:credit".to_string();
        let label = "stripe:mca1".to_string();
        let contract_map = types::ContractMap {
            label: label.clone(),
            target_count: 10000,
            target_time: get_current_time_in_secs().expect("failed to get current time"),
            current_count: 100,
        };

        ct_type
            .update_window(
                &id,
                &params,
                vec![contract_map],
                serde_json::Value::default(),
                &None,
            )
            .await
            .expect("Failed to update contracts");

        let invalidation_id = format!("{}:{}:{}", id, params, label);

        ct_type
            .invalidate_metrics(&invalidation_id, &None)
            .await
            .expect("Failed to invalidate contracts score");
    }

    #[tokio::test]
    async fn fetch_contract_score() {
        let ct_type = construct_ct_score_type().await;
        let id = "merchant2".to_string();
        let params = "card:credit".to_string();
        let label = "stripe:mca1".to_string();
        let config = configs::CalContractScoreConfig {
            constants: vec![0.7, 0.35],
            time_scale: Some(configs::TimeScale::Month),
        }
        .encode_to_value()
        .unwrap();

        let contract_map = types::ContractMap {
            label: label.clone(),
            target_count: 10000,
            target_time: get_current_time_in_secs().expect("failed to get current time"),
            current_count: 100,
        };

        ct_type
            .update_window(&id, &params, vec![contract_map], ().into(), &None)
            .await
            .expect("Failed to update contracts");

        let ct_scores = ct_type
            .perform_routing(&id, &params, vec![label.clone()], config, &None)
            .await
            .expect("Failed to fetch contract score");

        let first_connector = ct_scores
            .first()
            .expect("Empty array received for ct score");

        assert_eq!(label, first_connector.1);
        assert!(first_connector.0 > 0.0);

        let invalidation_id = format!("{}:{}:{}", id, params, label);

        ct_type
            .invalidate_metrics(&invalidation_id, &None)
            .await
            .expect("Failed to invalidate contracts score");
    }

    #[tokio::test]
    async fn invalidate_contract_score() {
        let ct_type = construct_ct_score_type().await;
        let id = "merchant3".to_string();
        let params = "card:credit".to_string();
        let label = "stripe:mca1".to_string();

        let invalidation_id = format!("{}:{}:{}", id, params, label);

        let config = configs::CalContractScoreConfig {
            constants: vec![0.7, 0.35],
            time_scale: Some(configs::TimeScale::Month),
        }
        .encode_to_value()
        .unwrap();

        let contract_map = types::ContractMap {
            label: label.clone(),
            target_count: 10000,
            target_time: get_current_time_in_secs().expect("failed to get current time"),
            current_count: 100,
        };

        ct_type
            .update_window(&id, &params, vec![contract_map], ().into(), &None)
            .await
            .expect("Failed to update contracts");

        ct_type
            .invalidate_metrics(&invalidation_id, &None)
            .await
            .expect("Failed to invalidate contracts score");

        let ct_score_result = ct_type
            .perform_routing(&id, &params, vec![label.clone()], config, &None)
            .await
            .ok();

        assert_eq!(ct_score_result, None);
    }
}
