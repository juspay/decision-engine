use error_stack::ResultExt;
use tonic::{Request, Response, Status};

use super::types as sr_proto_types;
use crate::success_rate::proto::types::{CalSuccessRateConfig, UpdateSuccessRateWindowConfig};
use crate::{
    authentication::authentication_proxy::Authenticate, success_rate::error::SuccessRateError,
};
use crate::{
    error::ResultExtGrpc,
    helpers, logger,
    success_rate::{
        configs::{CalculateSrConfig, UpdateWindowConfig},
        proto::types::SuccessRateSpecificityLevel,
        types::{SuccessRate, SUCCESS_RATE_GLOBAL_ENTITY_ID},
    },
    DynamicRouting,
};
use crate::{metrics, utils::Encode};

#[tonic::async_trait]
impl sr_proto_types::SuccessRateCalculator for SuccessRate {
    async fn fetch_success_rate(
        &self,
        request: Request<sr_proto_types::CalSuccessRateRequest>,
    ) -> Result<Response<sr_proto_types::CalSuccessRateResponse>, Status> {
        logger::debug!(?request, "fetch_success_rate request");

        // Authenticate the request
        let auth_info = self
            .authenticate(request.metadata())
            .await
            .into_grpc_status()?;

        let config: CalSuccessRateConfig = CalSuccessRateConfig::from(
            self.fetch_configs(
                request.metadata(),
                &auth_info.tenant_id,
                &auth_info.merchant_id,
            )
            .await
            .into_grpc_status()?,
        );

        metrics::SUCCESS_BASED_ROUTING_REQUEST.inc();
        let start = std::time::Instant::now();
        let elapsed = start.elapsed();

        let tenant_id =
            helpers::get_tenant_id_from_request(&request, self.config.is_multi_tenancy_enabled)
                .into_grpc_status()?;

        let request = request.into_inner();

        request.validate().into_grpc_status()?;

        let id = request.id;
        let params = request.params;
        let labels = request.labels;
        let configs = CalculateSrConfig::try_from((config, self.config.global_sr_config))
            .into_grpc_status()?
            .encode_to_value()
            .change_context(SuccessRateError::SerializationFailed)
            .into_grpc_status()?;

        let success_rates = self
            .perform_routing(&id, &params, labels, configs, &tenant_id)
            .await
            .into_grpc_status()?;

        let labels_with_score = success_rates
            .into_iter()
            .map(|(score, label)| sr_proto_types::LabelWithScore { score, label })
            .collect::<Vec<_>>();

        let response = sr_proto_types::CalSuccessRateResponse { labels_with_score };
        metrics::SUCCESS_BASED_ROUTING_DECISION_REQUEST_TIME.observe(elapsed.as_secs_f64());
        logger::info!(?response, "fetch_success_rate response");
        metrics::SUCCESS_BASED_ROUTING_SUCCESSFUL_RESPONSE_COUNT.inc();

        Ok(Response::new(response))
    }

    async fn update_success_rate_window(
        &self,
        request: Request<sr_proto_types::UpdateSuccessRateWindowRequest>,
    ) -> Result<Response<sr_proto_types::UpdateSuccessRateWindowResponse>, Status> {
        logger::debug!(?request, "update_success_rate_window request");

        // Authenticate the request
        let auth_info = self
            .authenticate(request.metadata())
            .await
            .into_grpc_status()?;

        let config: UpdateSuccessRateWindowConfig = UpdateSuccessRateWindowConfig::from(
            self.fetch_configs(
                request.metadata(),
                &auth_info.tenant_id,
                &auth_info.merchant_id,
            )
            .await
            .into_grpc_status()?,
        );

        let start = std::time::Instant::now();
        let elapsed = start.elapsed();

        let tenant_id =
            helpers::get_tenant_id_from_request(&request, self.config.is_multi_tenancy_enabled)
                .into_grpc_status()?;

        let request = request.into_inner();
        request.validate().into_grpc_status()?;

        let entity_id = request.id;
        let entity_params = request.params;
        let (entity_labels, global_labels) = (
            request
                .labels_with_status
                .into_iter()
                .map(|item| (item.label, item.status))
                .collect::<Vec<_>>(),
            request
                .global_labels_with_status
                .into_iter()
                .map(|item| (item.label, item.status))
                .collect::<Vec<_>>(),
        );
        let mut configs = UpdateWindowConfig::try_from((config, self.config.global_sr_config))
            .into_grpc_status()?;

        let entity_configs = configs
            .encode_to_value()
            .change_context(SuccessRateError::SerializationFailed)
            .into_grpc_status()?;

        let mut status = sr_proto_types::UpdationStatus::WindowUpdationSucceeded;

        self.update_window(
            &entity_id,
            &entity_params,
            entity_labels,
            entity_configs,
            &tenant_id,
        )
        .await
        .into_grpc_status()
        .inspect_err(|_| status = sr_proto_types::UpdationStatus::WindowUpdationFailed)?;

        configs.specificity_level = SuccessRateSpecificityLevel::Global;

        let global_configs = configs
            .encode_to_value()
            .change_context(SuccessRateError::SerializationFailed)
            .into_grpc_status()?;

        self.update_window(
            SUCCESS_RATE_GLOBAL_ENTITY_ID,
            &entity_params,
            global_labels,
            global_configs,
            &tenant_id,
        )
        .await
        .into_grpc_status()
        .inspect_err(|_| status = sr_proto_types::UpdationStatus::WindowUpdationFailed)?;

        let response = sr_proto_types::UpdateSuccessRateWindowResponse {
            status: status.into(),
        };
        metrics::SUCCESS_BASED_ROUTING_UPDATE_WINDOW_DECISION_REQUEST_TIME
            .observe(elapsed.as_secs_f64());
        metrics::SUCCESS_BASED_ROUTING_UPDATE_WINDOW_COUNT.inc();
        logger::info!(?response, "update_success_rate_window response");

        Ok(Response::new(response))
    }

    async fn invalidate_windows(
        &self,
        request: Request<sr_proto_types::InvalidateWindowsRequest>,
    ) -> Result<Response<sr_proto_types::InvalidateWindowsResponse>, Status> {
        logger::debug!(?request, "invalidate_windows request");

        // Authenticate the request
        self.authenticate(request.metadata())
            .await
            .into_grpc_status()?;

        let tenant_id =
            helpers::get_tenant_id_from_request(&request, self.config.is_multi_tenancy_enabled)
                .into_grpc_status()?;

        let request = request.into_inner();
        request.validate().into_grpc_status()?;

        let id = request.id;

        let mut status = sr_proto_types::InvalidationStatus::WindowInvalidationSucceeded;
        self.invalidate_metrics(&id, &tenant_id)
            .await
            .into_grpc_status()
            .inspect_err(|_| {
                status = sr_proto_types::InvalidationStatus::WindowInvalidationFailed
            })?;

        let response = sr_proto_types::InvalidateWindowsResponse {
            status: status.into(),
        };
        logger::info!(?response, "invalidate_windows response");

        Ok(Response::new(response))
    }

    async fn fetch_entity_and_global_success_rate(
        &self,
        request: Request<sr_proto_types::CalGlobalSuccessRateRequest>,
    ) -> Result<Response<sr_proto_types::CalGlobalSuccessRateResponse>, Status> {
        logger::debug!(?request, "fetch_entity_and_global_success_rate request");

        // Authenticate the request
        self.authenticate(request.metadata())
            .await
            .into_grpc_status()?;

        metrics::SUCCESS_BASED_ROUTING_METRICS_REQUEST.inc();
        let start = std::time::Instant::now();
        let elapsed = start.elapsed();

        let tenant_id =
            helpers::get_tenant_id_from_request(&request, self.config.is_multi_tenancy_enabled)
                .into_grpc_status()?;

        let request = request.into_inner();

        request.validate().into_grpc_status()?;

        let entity_id = request.entity_id;
        let entity_params = request.entity_params;
        let (entity_labels, global_labels) = (request.entity_labels, request.global_labels);
        let entity_configs = CalculateSrConfig::try_from((
            request
                .config
                .ok_or(Status::not_found("Config not found in request"))?,
            self.config.global_sr_config,
        ))
        .into_grpc_status()?;

        let entity_configs = entity_configs
            .encode_to_value()
            .change_context(SuccessRateError::SerializationFailed)
            .into_grpc_status()?;

        let mut global_configs = CalculateSrConfig::try_from((
            request
                .config
                .ok_or(Status::not_found("Config not found in request"))?,
            self.config.global_sr_config,
        ))
        .into_grpc_status()?;

        global_configs.specificity_level = SuccessRateSpecificityLevel::Global;

        let global_configs = global_configs
            .encode_to_value()
            .change_context(SuccessRateError::SerializationFailed)
            .into_grpc_status()?;

        let (entity_success_rates, global_success_rates) = tokio::try_join!(
            self.perform_routing(
                &entity_id,
                &entity_params,
                entity_labels,
                entity_configs,
                &tenant_id,
            ),
            self.perform_routing(
                SUCCESS_RATE_GLOBAL_ENTITY_ID,
                &entity_params,
                global_labels,
                global_configs,
                &tenant_id
            )
        )
        .into_grpc_status()?;

        let entity_scores_with_labels = entity_success_rates
            .into_iter()
            .map(|(score, label)| sr_proto_types::LabelWithScore { score, label })
            .collect::<Vec<_>>();

        let global_scores_with_labels = global_success_rates
            .into_iter()
            .map(|(score, label)| sr_proto_types::LabelWithScore { score, label })
            .collect::<Vec<_>>();

        let response = sr_proto_types::CalGlobalSuccessRateResponse {
            entity_scores_with_labels,
            global_scores_with_labels,
        };

        metrics::SUCCESS_BASED_ROUTING_METRICS_DECISION_REQUEST_TIME.observe(elapsed.as_secs_f64());
        logger::info!(?response, "fetch_entity_and_global_success_rate response");
        metrics::SUCCESS_BASED_ROUTING__METRICS_SUCCESSFUL_RESPONSE_COUNT.inc();
        Ok(Response::new(response))
    }
}
