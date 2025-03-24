use super::types as proto_types;
use crate::{
    contract_routing::{
        configs::CalContractScoreConfig, errors::ContractRoutingError, types as cr_types,
    },
    error::ResultExtGrpc,
    helpers, logger,
    utils::Encode,
    DynamicRouting,
};
use error_stack::ResultExt;
use tonic::{Request, Response, Status};

#[tonic::async_trait]
impl proto_types::ContractScoreCalculator for cr_types::ContractRouting {
    async fn fetch_contract_score(
        &self,
        request: Request<proto_types::CalContractScoreRequest>,
    ) -> Result<Response<proto_types::CalContractScoreResponse>, Status> {
        logger::debug!(?request, "fetch_contract_score request");
        let tenant_id =
            helpers::get_tenant_id_from_request(&request, self.config.is_multi_tenancy_enabled)
                .into_grpc_status()?;

        let request = request.into_inner();

        // validate req
        request.validate().into_grpc_status()?;

        let id = request.id;
        let params = request.params;
        let labels = request.labels;
        let config = CalContractScoreConfig::try_from(
            request
                .config
                .ok_or(Status::not_found("Config not found in request"))?,
        )
        .change_context(ContractRoutingError::ConfigError("Invalid config received"))
        .into_grpc_status()?;

        let config = config
            .encode_to_value()
            .change_context(ContractRoutingError::SerializationFailed)
            .into_grpc_status()?;

        let ct_scores = self
            .perform_routing(&id, &params, labels, config, &tenant_id)
            .await
            .into_grpc_status()?;

        let labels_with_score = ct_scores
            .into_iter()
            .map(|(score, label, current_count)| proto_types::ScoreData {
                score,
                label,
                current_count,
            })
            .collect::<Vec<_>>();

        let response = proto_types::CalContractScoreResponse { labels_with_score };
        logger::info!(?response, "fetch_contract_score response");

        Ok(Response::new(response))
    }

    async fn update_contract(
        &self,
        request: Request<proto_types::UpdateContractRequest>,
    ) -> Result<Response<proto_types::UpdateContractResponse>, Status> {
        logger::debug!(?request, "update_contract request");
        let tenant_id =
            helpers::get_tenant_id_from_request(&request, self.config.is_multi_tenancy_enabled)
                .into_grpc_status()?;

        let request = request.into_inner();

        // validate req
        request.validate().into_grpc_status()?;

        let id = request.id;
        let params = request.params;
        let labels_information = request.labels_information;
        let contract_maps = labels_information
            .into_iter()
            .map(cr_types::ContractMap::from)
            .collect::<Vec<_>>();

        self.update_window(
            &id,
            &params,
            contract_maps,
            serde_json::Value::default(),
            &tenant_id,
        )
        .await
        .into_grpc_status()?;

        let ideal_status = proto_types::UpdationStatus::ContractUpdationSucceeded;

        let response = proto_types::UpdateContractResponse {
            status: ideal_status.into(),
        };
        logger::info!(?response, "update_contract response");

        Ok(Response::new(response))
    }

    async fn invalidate_contract(
        &self,
        request: Request<proto_types::InvalidateContractRequest>,
    ) -> Result<Response<proto_types::InvalidateContractResponse>, Status> {
        logger::debug!(?request, "invalidate_contract request");

        let tenant_id =
            helpers::get_tenant_id_from_request(&request, self.config.is_multi_tenancy_enabled)
                .into_grpc_status()?;

        let request = request.into_inner();

        // validate req
        request.validate().into_grpc_status()?;

        let id = request.id.clone();

        self.invalidate_metrics(&id, &tenant_id)
            .await
            .into_grpc_status()?;

        let ideal_status = proto_types::InvalidationStatus::ContractInvalidationSucceeded;

        let response = proto_types::InvalidateContractResponse {
            status: ideal_status.into(),
        };
        logger::info!(?response, "invalidate_contract response");

        Ok(Response::new(response))
    }
}
