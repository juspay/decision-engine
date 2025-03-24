//! The success rate is calculated based on some predefined parameters. It counts the number of
//! successful payments and the number of failed payments for a given set of parameters. Then it
//! calculates the success rate based on the number of successful payments and the total number of
//! payments over a given window of time.
//!
//! The system, primarily uses the following formula to calculate the success rate:
//!
//! $$
//! weighted\_success\_rate
//!     = \sum_{i=0}^{n-1} (
//!         (\frac{success\_count[i]}
//!               {total\_count[i]}) *
//!         (\frac{i + 1}
//!               {\sum_{k=1}^{n} k}))
//!     * 100
//! $$

pub mod block;
pub mod configs;
pub mod error;
pub mod proto;
pub mod types;
pub mod utils;

use configs::SrCurrentBlockThreshold;
use error_stack::ResultExt;

use crate::{
    helpers, logger,
    success_rate::{
        block::Block,
        configs::{CalculateSrConfig, UpdateWindowConfig},
        error::SuccessRateError,
        proto::types::SuccessRateSpecificityLevel,
    },
    utils::ValueExt,
    DynamicRouting,
};

#[async_trait::async_trait]
impl DynamicRouting for types::SuccessRate {
    type UpdateWindowReport = Vec<(String, bool)>;
    type RoutingResponse = Vec<(f64, String)>;

    type Error = SuccessRateError;

    async fn perform_routing(
        &self,
        id: &str,
        params: &str,
        labels: Vec<String>,
        config: serde_json::Value,
        tenant_id: &Option<String>,
    ) -> error_stack::Result<Self::RoutingResponse, Self::Error> {
        let mut success_rates = Vec::with_capacity(labels.len());
        let config = config
            .parse_value::<CalculateSrConfig>("CalculateSrConfig")
            .change_context(SuccessRateError::DeserializationFailed)?;

        let specificity = config.specificity_level;

        for label in labels {
            let aggregates_key = helpers::redis_key_create_with_suffix(
                types::SUCCESS_RATE_PREFIX_IN_REDIS,
                tenant_id,
                &id,
                &params,
                &label,
                types::SUCCESS_RATE_AGGREGATES_SUFFIX_IN_REDIS,
            );
            match specificity {
                SuccessRateSpecificityLevel::Entity => {
                    perform_success_based_routing(
                        self,
                        &aggregates_key,
                        config.entity_min_aggregates_size,
                        config.entity_default_success_rate,
                        label,
                        &mut success_rates,
                    )
                    .await?;
                }
                SuccessRateSpecificityLevel::Global => {
                    perform_success_based_routing(
                        self,
                        &aggregates_key,
                        config.global_sr_config.min_aggregates_size,
                        config.global_sr_config.default_success_rate,
                        label.clone(),
                        &mut success_rates,
                    )
                    .await?;
                }
            }
        }

        success_rates = utils::sort_sr_by_score(success_rates);

        Ok(success_rates)
    }

    async fn update_window(
        &self,
        id: &str,
        params: &str,
        report: Self::UpdateWindowReport,
        config: serde_json::Value,
        tenant_id: &Option<String>,
    ) -> error_stack::Result<(), Self::Error> {
        let config = config
            .parse_value::<UpdateWindowConfig>("UpdateWindowConfig")
            .change_context(SuccessRateError::DeserializationFailed)?;

        let specificity = config.specificity_level;
        for (label, status) in report {
            let aggregates_key = helpers::redis_key_create_with_suffix(
                types::SUCCESS_RATE_PREFIX_IN_REDIS,
                tenant_id,
                &id,
                &params,
                &label,
                types::SUCCESS_RATE_AGGREGATES_SUFFIX_IN_REDIS,
            );

            let current_block_key = helpers::redis_key_create_with_suffix(
                types::SUCCESS_RATE_PREFIX_IN_REDIS,
                tenant_id,
                &id,
                &params,
                &label,
                types::SUCCESS_RATE_CURRENT_BLOCK_SUFFIX_IN_REDIS,
            );
            match specificity {
                SuccessRateSpecificityLevel::Entity => {
                    update_success_based_windows(
                        self,
                        &config.current_block_threshold,
                        config.max_aggregates_size,
                        &current_block_key,
                        &aggregates_key,
                        status,
                    )
                    .await?;
                }
                SuccessRateSpecificityLevel::Global => {
                    update_success_based_windows(
                        self,
                        &(config.global_sr_config.current_block_threshold.into()),
                        config.global_sr_config.max_aggregates_size,
                        &current_block_key,
                        &aggregates_key,
                        status,
                    )
                    .await?;
                }
            }
        }

        Ok(())
    }

    async fn invalidate_metrics(
        &self,
        id: &str,
        tenant_id: &Option<String>,
    ) -> error_stack::Result<(), Self::Error> {
        let prefix = helpers::redis_key_create_for_metrics_invalidation(
            types::SUCCESS_RATE_PREFIX_IN_REDIS,
            tenant_id,
            &id,
        );
        let keys_deleted = self
            .ephemeral_store
            .delete_keys_matching_prefix(&prefix)
            .await?;

        logger::debug!("List of keys invalidated from redis: {keys_deleted:?}");

        Ok(())
    }
}

async fn perform_success_based_routing(
    success_rate_type: &types::SuccessRate,
    key: &str,
    min_aggregates_size: usize,
    default_success_rate: f64,
    label: String,
    vec: &mut Vec<(f64, String)>,
) -> error_stack::Result<(), SuccessRateError> {
    let aggregates = success_rate_type
        .ephemeral_store
        .fetch_aggregates(key)
        .await?;

    let success_rate = if aggregates.len() < min_aggregates_size {
        default_success_rate
    } else {
        Block::calculate_weighted_success_rate(&aggregates)
    };

    vec.push((success_rate, label));

    Ok(())
}

async fn update_success_based_windows(
    success_rate_type: &types::SuccessRate,
    current_block_threshold: &SrCurrentBlockThreshold,
    max_aggregates_size: usize,
    current_block_key: &str,
    aggregates_key: &str,
    status: bool,
) -> error_stack::Result<(), SuccessRateError> {
    let current_block = success_rate_type
        .ephemeral_store
        .fetch_current_block(current_block_key)
        .await?;

    match current_block.inner() {
        Some(mut current_block) => {
            let is_threshold_breached_for_current_block = current_block
                .validate_threshold(current_block_threshold)
                .await?;

            if is_threshold_breached_for_current_block {
                current_block.update_created_at(utils::get_current_time_in_secs()?);

                success_rate_type
                    .move_current_block_to_aggregates_in_redis(
                        aggregates_key,
                        current_block,
                        max_aggregates_size,
                    )
                    .await?;

                let (new_success_count, new_total_count) =
                    utils::get_success_and_total_count_based_on_status(status);
                success_rate_type
                    .ephemeral_store
                    .initialize_current_block(current_block_key, new_success_count, new_total_count)
                    .await?;
            } else {
                let mut fields_to_increment = vec![(types::BlockFields::TotalCount, 1)];
                status.then(|| fields_to_increment.push((types::BlockFields::SuccessCount, 1)));
                success_rate_type
                    .ephemeral_store
                    .incr_current_block_fields(current_block_key, &fields_to_increment)
                    .await?;
            }
        }
        None => {
            let (success_count, total_count) =
                utils::get_success_and_total_count_based_on_status(status);
            success_rate_type
                .ephemeral_store
                .initialize_current_block(current_block_key, success_count, total_count)
                .await?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used, clippy::unwrap_used)]

    use std::sync::Arc;

    use crate::{
        configs::{Config, GlobalSrConfig},
        ephemeral_store::RedisEphemeralStore,
        helpers,
        success_rate::{
            block::Block,
            configs::{CalculateSrConfig, SrCurrentBlockThreshold, UpdateWindowConfig},
            proto::types::SuccessRateSpecificityLevel,
            types::{self, SuccessRate, SuccessRateConfig},
            utils,
        },
        utils::Encode,
        DynamicRouting,
    };
    use rand::{distributions::Alphanumeric, Rng};
    use redis_interface::{RedisConnectionPool, RedisSettings};

    struct TestSrConfig {
        aggregates_threshold: TestAggregatesThreshold,
        current_block_threshold: TestCurrentBlockThreshold,
        default_success_rate: f64,
    }

    struct TestAggregatesThreshold {
        min_aggregates_size: usize,
        max_aggregates_size: usize,
    }

    struct TestCurrentBlockThreshold {
        duration_in_mins: Option<std::time::Duration>,
        max_total_count: u64,
    }

    impl From<(TestSrConfig, GlobalSrConfig)> for CalculateSrConfig {
        fn from(value: (TestSrConfig, GlobalSrConfig)) -> Self {
            Self {
                entity_min_aggregates_size: value.0.aggregates_threshold.min_aggregates_size,
                entity_default_success_rate: value.0.default_success_rate,
                specificity_level: SuccessRateSpecificityLevel::Entity,
                global_sr_config: value.1,
            }
        }
    }

    impl From<(TestSrConfig, GlobalSrConfig)> for UpdateWindowConfig {
        fn from(value: (TestSrConfig, GlobalSrConfig)) -> Self {
            Self {
                max_aggregates_size: value.0.aggregates_threshold.max_aggregates_size,
                current_block_threshold: SrCurrentBlockThreshold {
                    duration_in_mins: value.0.current_block_threshold.duration_in_mins,
                    max_total_count: value.0.current_block_threshold.max_total_count,
                },
                specificity_level: SuccessRateSpecificityLevel::Entity,
                global_sr_config: value.1,
            }
        }
    }

    fn generate_random_id() -> String {
        let mut rng = rand::thread_rng();
        (0..7)
            .map(|_| rng.sample(Alphanumeric))
            .map(char::from)
            .collect()
    }

    async fn construct_sr() -> (SuccessRate, TestSrConfig) {
        let app_config = Config::new().expect("Failed to construct config");
        let redis_config = RedisSettings::default();
        let redis_conn = Arc::new(
            RedisConnectionPool::new(&redis_config)
                .await
                .expect("failed to create redis connection pool"),
        );

        let config = TestSrConfig {
            aggregates_threshold: TestAggregatesThreshold {
                min_aggregates_size: 2,
                max_aggregates_size: 3,
            },
            current_block_threshold: TestCurrentBlockThreshold {
                duration_in_mins: None,
                max_total_count: 2,
            },
            default_success_rate: 100.0,
        };

        (
            SuccessRate::new(
                SuccessRateConfig::new(
                    app_config.ttl_for_keys,
                    app_config.multi_tenancy.enabled,
                    app_config.global_routing_configs.success_rate,
                ),
                Box::new(RedisEphemeralStore::new(
                    Arc::clone(&redis_conn),
                    app_config.ttl_for_keys,
                )),
                [0; 32].into(),
                None,
            )
            .await,
            config,
        )
    }

    #[tokio::test]
    async fn test_initialize_current_block_in_redis() {
        let (sr, config) = construct_sr().await;

        let id = generate_random_id();
        let params = "card".to_string();
        let labels_with_status = vec![("stripe".into(), true), ("adyen".into(), false)];

        sr.update_window(
            &id,
            &params,
            labels_with_status.clone(),
            UpdateWindowConfig::from((config, sr.config.global_sr_config))
                .encode_to_value()
                .unwrap(),
            &None,
        )
        .await
        .expect("Failed while updating window");

        for (label, status) in labels_with_status {
            let current_block_key = helpers::redis_key_create_with_suffix(
                types::SUCCESS_RATE_PREFIX_IN_REDIS,
                &None,
                &id,
                &params,
                &label,
                types::SUCCESS_RATE_CURRENT_BLOCK_SUFFIX_IN_REDIS,
            );

            let global_current_block_key = helpers::redis_key_create_with_suffix(
                types::SUCCESS_RATE_PREFIX_IN_REDIS,
                &None,
                &(types::SUCCESS_RATE_GLOBAL_ENTITY_ID.to_string()),
                &params,
                &label,
                types::SUCCESS_RATE_CURRENT_BLOCK_SUFFIX_IN_REDIS,
            );

            let Block {
                success_count: actual_success_count,
                total_count: actual_total_count,
                ..
            } = sr
                .ephemeral_store
                .fetch_current_block(&current_block_key)
                .await
                .unwrap()
                .inner()
                .expect("current block not found in redis");

            let (expected_success_count, expected_total_count) =
                utils::get_success_and_total_count_based_on_status(status);

            assert_eq!(actual_success_count, expected_success_count);
            assert_eq!(actual_total_count, expected_total_count);

            sr.ephemeral_store
                .delete_key(&current_block_key)
                .await
                .expect("Failed to delete current_block in redis");

            sr.ephemeral_store
                .delete_key(&global_current_block_key)
                .await
                .expect("Failed to delete current_block in redis");
        }
    }

    #[tokio::test]
    async fn test_default_score_strategy() {
        let (sr, config) = construct_sr().await;

        let id = generate_random_id();
        let params = "card".to_string();
        let labels = vec!["stripe".into()];
        let default_score = config.default_success_rate;

        let success_rates = sr
            .perform_routing(
                &id,
                &params,
                labels,
                CalculateSrConfig::from((config, sr.config.global_sr_config))
                    .encode_to_value()
                    .unwrap(),
                &None,
            )
            .await
            .expect("Failed while calculating success rate");

        for (score, _) in success_rates {
            assert_eq!(score, default_score)
        }
    }

    #[tokio::test]
    async fn test_global_scores() {
        let (sr, config) = construct_sr().await;

        let id = generate_random_id();
        let params = "card".to_string();
        let labels = vec!["stripe".into()];
        let default_score = sr.config.global_sr_config.default_success_rate;

        let mut config: CalculateSrConfig = (config, sr.config.global_sr_config).into();
        config.specificity_level = SuccessRateSpecificityLevel::Global;

        let success_rates = sr
            .perform_routing(
                &id,
                &params,
                labels.clone(),
                config.encode_to_value().unwrap(),
                &None,
            )
            .await
            .expect("Failed while calculating success rate");

        for (score, _) in success_rates {
            assert_eq!(score, default_score)
        }

        for label in labels {
            let global_aggregates_key = helpers::redis_key_create_with_suffix(
                types::SUCCESS_RATE_PREFIX_IN_REDIS,
                &None,
                &(types::SUCCESS_RATE_GLOBAL_ENTITY_ID.to_string()),
                &params,
                &label,
                types::SUCCESS_RATE_AGGREGATES_SUFFIX_IN_REDIS,
            );
            let current_block_key = helpers::redis_key_create_with_suffix(
                types::SUCCESS_RATE_PREFIX_IN_REDIS,
                &None,
                &(types::SUCCESS_RATE_GLOBAL_ENTITY_ID.to_string()),
                &params,
                &label,
                types::SUCCESS_RATE_CURRENT_BLOCK_SUFFIX_IN_REDIS,
            );

            sr.ephemeral_store
                .delete_key(&global_aggregates_key)
                .await
                .expect("Failed to delete global key in redis");

            sr.ephemeral_store
                .delete_key(&current_block_key)
                .await
                .expect("Failed to delete global key in redis");
        }
    }

    #[tokio::test]
    async fn test_increment_current_block_fields() {
        let (sr, config) = construct_sr().await;

        let id = generate_random_id();
        let params = "card".to_string();
        let labels_with_status = vec![("stripe".into(), true), ("adyen".into(), false)];

        for (label, _) in &labels_with_status {
            let current_block_key = helpers::redis_key_create_with_suffix(
                types::SUCCESS_RATE_PREFIX_IN_REDIS,
                &None,
                &id,
                &params,
                &label,
                types::SUCCESS_RATE_CURRENT_BLOCK_SUFFIX_IN_REDIS,
            );

            sr.ephemeral_store
                .initialize_current_block(&current_block_key, 0, 0)
                .await
                .expect("Failed to initialize current_block in redis");
        }

        sr.update_window(
            &id,
            &params,
            labels_with_status.clone(),
            UpdateWindowConfig::from((config, sr.config.global_sr_config))
                .encode_to_value()
                .unwrap(),
            &None,
        )
        .await
        .expect("Failed while updating window");

        for (label, status) in labels_with_status {
            let current_block_key = helpers::redis_key_create_with_suffix(
                types::SUCCESS_RATE_PREFIX_IN_REDIS,
                &None,
                &id,
                &params,
                &label,
                types::SUCCESS_RATE_CURRENT_BLOCK_SUFFIX_IN_REDIS,
            );

            let global_current_block_key = helpers::redis_key_create_with_suffix(
                types::SUCCESS_RATE_PREFIX_IN_REDIS,
                &None,
                &(types::SUCCESS_RATE_GLOBAL_ENTITY_ID.to_string()),
                &params,
                &label,
                types::SUCCESS_RATE_CURRENT_BLOCK_SUFFIX_IN_REDIS,
            );

            let Block {
                success_count: actual_success_count,
                total_count: actual_total_count,
                ..
            } = sr
                .ephemeral_store
                .fetch_current_block(&current_block_key)
                .await
                .unwrap()
                .inner()
                .expect("current block not found in redis");

            let (expected_success_count, expected_total_count) =
                utils::get_success_and_total_count_based_on_status(status);

            assert_eq!(actual_success_count, expected_success_count);
            assert_eq!(actual_total_count, expected_total_count);

            sr.ephemeral_store
                .delete_key(&current_block_key)
                .await
                .expect("Failed to delete current_block in redis");

            sr.ephemeral_store
                .delete_key(&global_current_block_key)
                .await
                .expect("Failed to delete current_block in redis");
        }
    }

    #[tokio::test]
    async fn test_threshold_breach_strategy() {
        let (sr, config) = construct_sr().await;

        let id = generate_random_id();
        let params = "card".to_string();
        let label = "stripe";

        let aggregates_key = helpers::redis_key_create_with_suffix(
            types::SUCCESS_RATE_PREFIX_IN_REDIS,
            &None,
            &id,
            &params,
            &label,
            types::SUCCESS_RATE_AGGREGATES_SUFFIX_IN_REDIS,
        );
        let current_block_key = helpers::redis_key_create_with_suffix(
            types::SUCCESS_RATE_PREFIX_IN_REDIS,
            &None,
            &id,
            &params,
            &label,
            types::SUCCESS_RATE_CURRENT_BLOCK_SUFFIX_IN_REDIS,
        );

        let global_aggregates_key = helpers::redis_key_create_with_suffix(
            types::SUCCESS_RATE_PREFIX_IN_REDIS,
            &None,
            &(types::SUCCESS_RATE_GLOBAL_ENTITY_ID.to_string()),
            &params,
            &label,
            types::SUCCESS_RATE_AGGREGATES_SUFFIX_IN_REDIS,
        );
        let global_current_block_key = helpers::redis_key_create_with_suffix(
            types::SUCCESS_RATE_PREFIX_IN_REDIS,
            &None,
            &(types::SUCCESS_RATE_GLOBAL_ENTITY_ID.to_string()),
            &params,
            &label,
            types::SUCCESS_RATE_CURRENT_BLOCK_SUFFIX_IN_REDIS,
        );

        sr.ephemeral_store
            .initialize_current_block(&current_block_key, 2, 2)
            .await
            .expect("Failed to initialize current_block in redis");

        let aggregates = [
            Block {
                success_count: 1,
                total_count: 2,
                created_at: 1722533100,
            },
            Block {
                success_count: 2,
                total_count: 2,
                created_at: 1722533200,
            },
            Block {
                success_count: 2,
                total_count: 2,
                created_at: 1722533300,
            },
        ];

        sr.ephemeral_store
            .set_aggregates(&aggregates_key, aggregates.to_vec())
            .await
            .expect("Failed to set aggregates in redis");

        let labels_with_status = vec![("stripe".into(), true)];
        let max_aggregate_size = config.aggregates_threshold.max_aggregates_size;

        sr.update_window(
            &id,
            &params,
            labels_with_status,
            UpdateWindowConfig::from((config, sr.config.global_sr_config))
                .encode_to_value()
                .unwrap(),
            &None,
        )
        .await
        .expect("Failed to update window in redis");

        let fetched_aggregates = sr
            .ephemeral_store
            .fetch_aggregates(&aggregates_key)
            .await
            .expect("Failed to fetch aggregates from redis");

        assert_eq!(fetched_aggregates.len(), max_aggregate_size);

        // Validate whether old block is popped
        assert_eq!(
            fetched_aggregates
                .first()
                .expect("Aggregate not found")
                .created_at,
            1722533200
        );
        assert_eq!(
            fetched_aggregates
                .get(1)
                .expect("Aggregate not found")
                .created_at,
            1722533300
        );

        sr.ephemeral_store
            .delete_key(&aggregates_key)
            .await
            .expect("Failed to delete aggregates in redis");

        sr.ephemeral_store
            .delete_key(&current_block_key)
            .await
            .expect("Failed to delete current_block in redis");

        sr.ephemeral_store
            .delete_key(&global_aggregates_key)
            .await
            .expect("Failed to delete aggregates in redis");

        sr.ephemeral_store
            .delete_key(&global_current_block_key)
            .await
            .expect("Failed to delete current_block in redis");
    }

    #[tokio::test]
    async fn test_calculate_success_rate() {
        let (sr, config) = construct_sr().await;

        let id = generate_random_id();
        let params = "card".to_string();
        let aggregates = [
            (
                "stripe",
                [
                    Block {
                        success_count: 1,
                        total_count: 2,
                        created_at: 1722533100,
                    },
                    Block {
                        success_count: 2,
                        total_count: 2,
                        created_at: 1722533200,
                    },
                    Block {
                        success_count: 2,
                        total_count: 2,
                        created_at: 1722533300,
                    },
                ],
                91.66,
            ),
            (
                "adyen",
                [
                    Block {
                        success_count: 2,
                        total_count: 2,
                        created_at: 1722533100,
                    },
                    Block {
                        success_count: 1,
                        total_count: 2,
                        created_at: 1722533200,
                    },
                    Block {
                        success_count: 1,
                        total_count: 2,
                        created_at: 1722533300,
                    },
                ],
                58.33,
            ),
        ];

        for (label, blocks, _) in &aggregates {
            let aggregates_key = helpers::redis_key_create_with_suffix(
                types::SUCCESS_RATE_PREFIX_IN_REDIS,
                &None,
                &id,
                &params,
                &label,
                types::SUCCESS_RATE_AGGREGATES_SUFFIX_IN_REDIS,
            );

            sr.ephemeral_store
                .set_aggregates(&aggregates_key, blocks.to_vec())
                .await
                .expect("Failed to set aggregates in redis");
        }

        let success_rates = sr
            .perform_routing(
                &id,
                &params,
                aggregates
                    .iter()
                    .map(|block| block.0.to_string())
                    .collect::<Vec<_>>(),
                CalculateSrConfig::from((config, sr.config.global_sr_config))
                    .encode_to_value()
                    .unwrap(),
                &None,
            )
            .await
            .expect("Failed to calculate success rate");

        for ((actual_sr, actual_label), (expected_label, _, expected_sr)) in
            success_rates.into_iter().zip(aggregates)
        {
            assert_eq!(actual_label, expected_label);
            assert_eq!(actual_sr, expected_sr);

            let aggregates_key = helpers::redis_key_create_with_suffix(
                types::SUCCESS_RATE_PREFIX_IN_REDIS,
                &None,
                &id,
                &params,
                &expected_label,
                types::SUCCESS_RATE_AGGREGATES_SUFFIX_IN_REDIS,
            );
            sr.ephemeral_store
                .delete_key(&aggregates_key)
                .await
                .expect("Failed to delete aggregates in redis");
        }
    }

    #[tokio::test]
    async fn test_invalidate_metrics() {
        let (sr, _) = construct_sr().await;

        let id = generate_random_id();
        let params = "card".to_string();
        let label = "stripe";

        let aggregates_key = helpers::redis_key_create_with_suffix(
            types::SUCCESS_RATE_PREFIX_IN_REDIS,
            &None,
            &id,
            &params,
            &label,
            types::SUCCESS_RATE_AGGREGATES_SUFFIX_IN_REDIS,
        );
        let current_block_key = helpers::redis_key_create_with_suffix(
            types::SUCCESS_RATE_PREFIX_IN_REDIS,
            &None,
            &id,
            &params,
            &label,
            types::SUCCESS_RATE_CURRENT_BLOCK_SUFFIX_IN_REDIS,
        );

        sr.ephemeral_store
            .initialize_current_block(&current_block_key, 2, 2)
            .await
            .expect("Failed to initialize current_block in redis");

        let aggregates = [
            Block {
                success_count: 1,
                total_count: 2,
                created_at: 1722533100,
            },
            Block {
                success_count: 2,
                total_count: 2,
                created_at: 1722533200,
            },
            Block {
                success_count: 2,
                total_count: 2,
                created_at: 1722533300,
            },
        ];

        sr.ephemeral_store
            .set_aggregates(&aggregates_key, aggregates.to_vec())
            .await
            .expect("Failed to set aggregates in redis");

        sr.invalidate_metrics(&id, &None)
            .await
            .expect("Failed to keys matching pattern in redis");

        let current_block = sr
            .ephemeral_store
            .fetch_current_block(&current_block_key)
            .await
            .expect("Failed while fetching current block from redis");

        assert!(current_block.inner().is_none());

        let aggregates = sr
            .ephemeral_store
            .fetch_aggregates(&aggregates_key)
            .await
            .expect("Failed while fetching aggregates from redis");

        assert!(aggregates.is_empty())
    }
}
