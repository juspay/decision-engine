use crate::contract_routing::{
    configs::{CalContractScoreConfig, TimeScale},
    errors::ContractRoutingError,
    types::ContractMap,
};
use error_stack::{ensure, report};

#[allow(
    unused_qualifications,
    clippy::use_self,
    clippy::unwrap_used,
    clippy::as_conversions
)]
pub mod proto_items {
    tonic::include_proto!("contract_routing");
}

pub use proto_items::{
    contract_score_calculator_server::{ContractScoreCalculator, ContractScoreCalculatorServer},
    invalidate_contract_response::InvalidationStatus,
    update_contract_response::UpdationStatus,
    CalContractScoreConfig as ProtoConfig, CalContractScoreRequest, CalContractScoreResponse,
    InvalidateContractRequest, InvalidateContractResponse, LabelInformation, ScoreData,
    UpdateContractRequest, UpdateContractResponse,
};

impl TryFrom<ProtoConfig> for CalContractScoreConfig {
    type Error = ContractRoutingError;
    fn try_from(value: ProtoConfig) -> Result<Self, ContractRoutingError> {
        let time_scale = value
            .time_scale
            .map(|scale| match scale.time_scale {
                0 => Ok(TimeScale::Day),
                1 => Ok(TimeScale::Month),
                _ => Err(ContractRoutingError::ConfigError(
                    "Invalid variant for time_scale",
                )),
            })
            .transpose()?;

        Ok(Self {
            constants: value.constants,
            time_scale,
        })
    }
}

impl From<LabelInformation> for ContractMap {
    fn from(value: LabelInformation) -> Self {
        Self {
            label: value.label,
            target_count: value.target_count,
            target_time: value.target_time,
            current_count: value.current_count,
        }
    }
}

impl CalContractScoreRequest {
    pub fn validate(&self) -> error_stack::Result<(), ContractRoutingError> {
        ensure!(
            !self.id.trim().is_empty(),
            ContractRoutingError::ConfigError("id field cannot be empty")
        );

        ensure!(
            !self.labels.is_empty() && !self.labels.iter().any(|item| item.trim().is_empty()),
            ContractRoutingError::ConfigError("labels field cannot be empty")
        );

        let config = self
            .config
            .as_ref()
            .ok_or_else(|| report!(ContractRoutingError::ConfigError("config not found")))?;

        ensure!(
            !config.constants.is_empty(),
            ContractRoutingError::ConfigError("Calculation constants cannot be empty")
        );

        Ok(())
    }
}

impl UpdateContractRequest {
    pub fn validate(&self) -> error_stack::Result<(), ContractRoutingError> {
        ensure!(
            !self.id.trim().is_empty(),
            ContractRoutingError::ConfigError("id field cannot be empty")
        );

        ensure!(
            !self.labels_information.is_empty()
                && !self
                    .labels_information
                    .iter()
                    .any(|item| item.label.trim().is_empty()
                        || item.target_count == 0
                        || item.target_time == 0),
            ContractRoutingError::ConfigError("incorrect label information provided")
        );

        Ok(())
    }
}

impl InvalidateContractRequest {
    pub fn validate(&self) -> error_stack::Result<(), ContractRoutingError> {
        ensure!(
            !self.id.trim().is_empty(),
            ContractRoutingError::ConfigError("id field cannot be empty")
        );

        Ok(())
    }
}
