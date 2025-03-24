use error_stack::ResultExt;
use tonic::{Request, Response, Status};

use super::types as elimination_proto_types;
use crate::{
    elimination::{
        configs::EliminationBucketSettings, error::EliminationError, types::Elimination,
    },
    error::ResultExtGrpc,
    helpers, logger,
    utils::Encode,
    DynamicRouting,
};

#[tonic::async_trait]
impl elimination_proto_types::EliminationAnalyser for Elimination {
    async fn get_elimination_status(
        &self,
        request: Request<elimination_proto_types::EliminationRequest>,
    ) -> Result<Response<elimination_proto_types::EliminationResponse>, Status> {
        logger::debug!(?request, "get_elimination_status request");
        let tenant_id =
            helpers::get_tenant_id_from_request(&request, self.config.is_multi_tenancy_enabled)
                .into_grpc_status()?;

        let request = request.into_inner();

        request.validate().into_grpc_status()?;

        let id = request.id;
        let params = request.params;
        let labels = request.labels;
        let configs = EliminationBucketSettings::try_from((
            request
                .config
                .ok_or(Status::not_found("Config not found in request"))?,
            self.config.global_er_config,
        ))
        .into_grpc_status()?;

        let configs = configs
            .encode_to_value()
            .change_context(EliminationError::SerializationFailed)
            .into_grpc_status()?;

        let elimination_response = self
            .perform_routing(&id, &params, labels, configs, &tenant_id)
            .await
            .into_grpc_status()?;

        let labels_with_status = elimination_response
            .into_iter()
            .map(
                |(
                    label,
                    (entity_eliminated, entity_buckets),
                    (global_eliminated, global_buckets),
                )| elimination_proto_types::LabelWithStatus {
                    label,
                    elimination_information: Some(
                        elimination_proto_types::EliminationInformation {
                            entity: Some(elimination_proto_types::BucketInformation {
                                is_eliminated: entity_eliminated,
                                bucket_name: entity_buckets,
                            }),
                            global: Some(elimination_proto_types::BucketInformation {
                                is_eliminated: global_eliminated,
                                bucket_name: global_buckets,
                            }),
                        },
                    ),
                },
            )
            .collect::<Vec<_>>();

        let response = elimination_proto_types::EliminationResponse { labels_with_status };

        logger::info!(?response, "get_elimination_status response");
        Ok(Response::new(response))
    }

    async fn update_elimination_bucket(
        &self,
        request: Request<elimination_proto_types::UpdateEliminationBucketRequest>,
    ) -> Result<Response<elimination_proto_types::UpdateEliminationBucketResponse>, Status> {
        logger::debug!(?request, "update_elimination_bucket request");
        let tenant_id =
            helpers::get_tenant_id_from_request(&request, self.config.is_multi_tenancy_enabled)
                .into_grpc_status()?;

        let request = request.into_inner();
        request.validate().into_grpc_status()?;

        let id = request.id;
        let params = request.params;
        let labels = request
            .labels_with_bucket_name
            .into_iter()
            .map(|item| (item.label, item.bucket_name))
            .collect::<Vec<_>>();
        let configs = EliminationBucketSettings::try_from((
            request
                .config
                .ok_or(Status::not_found("Config not found in request"))?,
            self.config.global_er_config,
        ))
        .into_grpc_status()?;

        let configs = configs
            .encode_to_value()
            .change_context(EliminationError::SerializationFailed)
            .into_grpc_status()?;

        let mut status = elimination_proto_types::UpdationStatus::BucketUpdationSucceeded;
        self.update_window(&id, &params, labels, configs, &tenant_id)
            .await
            .into_grpc_status()
            .inspect_err(|_| {
                status = elimination_proto_types::UpdationStatus::BucketUpdationFailed
            })?;

        let response = elimination_proto_types::UpdateEliminationBucketResponse {
            status: status.into(),
        };
        logger::info!(?response, "update_elimination_bucket response");

        Ok(Response::new(response))
    }

    async fn invalidate_bucket(
        &self,
        request: Request<elimination_proto_types::InvalidateBucketRequest>,
    ) -> Result<Response<elimination_proto_types::InvalidateBucketResponse>, Status> {
        logger::debug!(?request, "invalidate_bucket request");
        let tenant_id =
            helpers::get_tenant_id_from_request(&request, self.config.is_multi_tenancy_enabled)
                .into_grpc_status()?;

        let request = request.into_inner();
        request.validate().into_grpc_status()?;

        let id = request.id;

        let mut status = elimination_proto_types::InvalidationStatus::BucketInvalidationSucceeded;
        self.invalidate_metrics(&id, &tenant_id)
            .await
            .into_grpc_status()
            .inspect_err(|_| {
                status = elimination_proto_types::InvalidationStatus::BucketInvalidationFailed
            })?;

        let response = elimination_proto_types::InvalidateBucketResponse {
            status: status.into(),
        };
        logger::info!(?response, "invalidate_bucket response");

        Ok(Response::new(response))
    }
}
