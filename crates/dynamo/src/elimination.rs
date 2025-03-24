pub mod configs;
pub mod error;
pub mod leaky_bucket;
pub mod proto;
pub mod types;

use error_stack::ResultExt;

use crate::{
    elimination::{configs::EliminationBucketSettings, error::EliminationError},
    helpers, logger,
    utils::ValueExt,
    DynamicRouting,
};

#[async_trait::async_trait]
impl DynamicRouting for types::Elimination {
    type UpdateWindowReport = Vec<(String, String)>;
    type RoutingResponse = Vec<(String, (bool, Vec<String>), (bool, Vec<String>))>;

    type Error = EliminationError;

    async fn perform_routing(
        &self,
        id: &str,
        params: &str,
        labels: Vec<String>,
        config: serde_json::Value,
        tenant_id: &Option<String>,
    ) -> error_stack::Result<Self::RoutingResponse, Self::Error> {
        let config = config
            .parse_value::<EliminationBucketSettings>("EliminationBucketSettings")
            .change_context(EliminationError::DeserializationFailed)?;

        let mut elimination_response = Vec::with_capacity(labels.len());

        for label in labels {
            let entity_elimination_key = helpers::redis_key_create_without_suffix(
                types::ELIMINATION_PREFIX_IN_REDIS,
                tenant_id,
                &id,
                &params,
                &label,
            );
            let global_elimination_key = helpers::redis_key_create_without_suffix(
                types::ELIMINATION_PREFIX_IN_REDIS,
                tenant_id,
                &(types::ELIMINATION_GLOBAL_ENTITY_ID.to_string()),
                &params,
                &label,
            );

            let (mut entity_buckets, mut global_buckets) = tokio::try_join!(
                self.fetch_buckets_from_redis::<Vec<_>>(&entity_elimination_key),
                self.fetch_buckets_from_redis::<Vec<_>>(&global_elimination_key)
            )?;

            let entity_elimination_status =
                leaky_bucket::get_elimination_status(&mut entity_buckets, &config.entity_bucket)?;
            let global_elimination_status =
                leaky_bucket::get_elimination_status(&mut global_buckets, &config.global_bucket)?;

            elimination_response.push((
                label,
                (
                    entity_elimination_status.should_eliminate,
                    entity_elimination_status.bucket_names,
                ),
                (
                    global_elimination_status.should_eliminate,
                    global_elimination_status.bucket_names,
                ),
            ));

            logger::debug!(entity_elimination_key=?entity_elimination_key, entity_bucket_status=?entity_buckets,
                           global_elimination_key=?global_elimination_key, global_bucket_status=?global_buckets);

            // Update buckets in redis if leaked
            let update_entity_buckets = async {
                if entity_elimination_status.should_update_leaks_in_redis {
                    self.set_buckets_in_redis(&entity_elimination_key, entity_buckets)
                        .await
                } else {
                    Ok(())
                }
            };
            let update_global_buckets = async {
                if global_elimination_status.should_update_leaks_in_redis {
                    self.set_buckets_in_redis(&global_elimination_key, global_buckets)
                        .await
                } else {
                    Ok(())
                }
            };
            tokio::try_join!(update_entity_buckets, update_global_buckets)?;
        }

        Ok(elimination_response)
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
            .parse_value::<EliminationBucketSettings>("EliminationBucketSettings")
            .change_context(EliminationError::DeserializationFailed)?;

        for (label, bucket_name) in report {
            let entity_elimination_key = helpers::redis_key_create_without_suffix(
                types::ELIMINATION_PREFIX_IN_REDIS,
                tenant_id,
                &id,
                &params,
                &label,
            );
            let global_elimination_key = helpers::redis_key_create_without_suffix(
                types::ELIMINATION_PREFIX_IN_REDIS,
                tenant_id,
                &(types::ELIMINATION_GLOBAL_ENTITY_ID.to_string()),
                &params,
                &label,
            );

            let (mut entity_buckets, mut global_buckets) = tokio::try_join!(
                self.fetch_buckets_from_redis::<Vec<_>>(&entity_elimination_key),
                self.fetch_buckets_from_redis::<Vec<_>>(&global_elimination_key)
            )?;

            // Fill the bucket with name matching bucket_name
            leaky_bucket::upsert_bucket(&mut entity_buckets, &bucket_name, &config.entity_bucket)?;
            leaky_bucket::upsert_bucket(&mut global_buckets, &bucket_name, &config.global_bucket)?;

            logger::debug!(entity_elimination_key=?entity_elimination_key, entity_buckets_after_updation=?entity_buckets,
                           global_elimination_key=?global_elimination_key, global_buckets_after_updation=?global_buckets);

            tokio::try_join!(
                self.set_buckets_in_redis(&entity_elimination_key, entity_buckets),
                self.set_buckets_in_redis(&global_elimination_key, global_buckets)
            )?;
        }

        Ok(())
    }

    async fn invalidate_metrics(
        &self,
        id: &str,
        tenant_id: &Option<String>,
    ) -> error_stack::Result<(), Self::Error> {
        let prefix = helpers::redis_key_create_for_metrics_invalidation(
            types::ELIMINATION_PREFIX_IN_REDIS,
            tenant_id,
            &id,
        );

        let keys_deleted = self.delete_keys_matching_prefix(&prefix).await?;

        logger::debug!("List of keys and its invalidation status: {keys_deleted:?}");

        Ok(())
    }
}
