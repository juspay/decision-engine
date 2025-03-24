use crate::configs::{GlobalSrConfig, GlobalSrCurrentBlockThreshold};
use crate::success_rate::{
    configs::{CalculateSrConfig, SrCurrentBlockThreshold, UpdateWindowConfig},
    error::SuccessRateError,
};
use error_stack::{ensure, report, Report, ResultExt};
pub use proto_items::{
    invalidate_windows_response::InvalidationStatus,
    success_rate_calculator_server::{SuccessRateCalculator, SuccessRateCalculatorServer},
    update_success_rate_window_response::UpdationStatus,
    CalGlobalSuccessRateConfig, CalGlobalSuccessRateRequest, CalGlobalSuccessRateResponse,
    CalSuccessRateConfig, CalSuccessRateRequest, CalSuccessRateResponse, CurrentBlockThreshold,
    InvalidateWindowsRequest, InvalidateWindowsResponse, LabelWithScore,
    SuccessRateSpecificityLevel, UpdateSuccessRateWindowConfig, UpdateSuccessRateWindowRequest,
    UpdateSuccessRateWindowResponse,
};
use std::time::Duration;

#[allow(
    unused_qualifications,
    clippy::use_self,
    clippy::unwrap_used,
    clippy::as_conversions
)]
pub mod proto_items {
    tonic::include_proto!("success_rate");
}

impl TryFrom<(CalSuccessRateConfig, GlobalSrConfig)> for CalculateSrConfig {
    type Error = Report<SuccessRateError>;
    fn try_from(
        (entity_config, global_config): (CalSuccessRateConfig, GlobalSrConfig),
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            entity_min_aggregates_size: usize::try_from(entity_config.min_aggregates_size)
                .change_context(SuccessRateError::TypeConversionError {
                    field: "min_aggregates_size",
                    from: "u32",
                    to: "usize",
                })?,
            entity_default_success_rate: entity_config.default_success_rate,
            specificity_level: entity_config
                .specificity_level
                .map(|level| match level {
                    0 => Ok(SuccessRateSpecificityLevel::Entity),
                    1 => Ok(SuccessRateSpecificityLevel::Global),
                    _ => Err(SuccessRateError::ConfigError(
                        "Invalid variant received for SuccessRateSpecificityLevel",
                    )),
                })
                .transpose()?
                .unwrap_or(SuccessRateSpecificityLevel::Entity),
            global_sr_config: global_config,
        })
    }
}

impl TryFrom<(CalGlobalSuccessRateConfig, GlobalSrConfig)> for CalculateSrConfig {
    type Error = Report<SuccessRateError>;
    fn try_from(
        (entity_config, global_config): (CalGlobalSuccessRateConfig, GlobalSrConfig),
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            entity_min_aggregates_size: usize::try_from(entity_config.entity_min_aggregates_size)
                .change_context(SuccessRateError::TypeConversionError {
                field: "entity_min_aggregates_size",
                from: "u32",
                to: "usize",
            })?,
            entity_default_success_rate: entity_config.entity_default_success_rate,
            specificity_level: SuccessRateSpecificityLevel::Entity,
            global_sr_config: global_config,
        })
    }
}

impl TryFrom<(UpdateSuccessRateWindowConfig, GlobalSrConfig)> for UpdateWindowConfig {
    type Error = Report<SuccessRateError>;
    fn try_from(
        (entity_config, global_config): (UpdateSuccessRateWindowConfig, GlobalSrConfig),
    ) -> Result<Self, Self::Error> {
        let (duration_in_mins, max_total_count) = entity_config
            .current_block_threshold
            .map(|val| (val.duration_in_mins, val.max_total_count))
            .ok_or(SuccessRateError::ConfigError(
                "Current block threshold config not found in request",
            ))?;

        Ok(Self {
            max_aggregates_size: usize::try_from(entity_config.max_aggregates_size)
                .change_context(SuccessRateError::TypeConversionError {
                    field: "max_aggregates_size",
                    from: "u32",
                    to: "usize",
                })?,
            current_block_threshold: SrCurrentBlockThreshold {
                duration_in_mins: duration_in_mins.map(|dur| Duration::from_secs(dur * 60)),
                max_total_count,
            },
            global_sr_config: global_config,
            specificity_level: SuccessRateSpecificityLevel::Entity,
        })
    }
}

impl From<GlobalSrCurrentBlockThreshold> for SrCurrentBlockThreshold {
    fn from(value: GlobalSrCurrentBlockThreshold) -> Self {
        Self {
            duration_in_mins: value
                .duration_in_mins
                .map(|dur| Duration::from_secs(dur * 60)),
            max_total_count: value.max_total_count,
        }
    }
}

impl CalSuccessRateRequest {
    #[track_caller]
    pub fn validate(&self) -> error_stack::Result<(), SuccessRateError> {
        let is_id_required = self
            .config
            .and_then(|conf| conf.specificity_level.map(|level| level == 0))
            .unwrap_or(false);

        if is_id_required {
            ensure!(
                !self.id.trim().is_empty(),
                SuccessRateError::ConfigError("id field cannot be empty")
            );
        }

        ensure!(
            !self.params.trim().is_empty(),
            SuccessRateError::ConfigError("params field cannot be empty")
        );

        ensure!(
            !self.labels.is_empty() && !self.labels.iter().any(|label| label.is_empty()),
            SuccessRateError::ConfigError("labels field cannot be empty")
        );
        Ok(())
    }
}

impl UpdateSuccessRateWindowRequest {
    pub fn validate(&self) -> error_stack::Result<(), SuccessRateError> {
        ensure!(
            !self.id.trim().is_empty(),
            SuccessRateError::ConfigError("id field cannot be empty")
        );

        ensure!(
            !self.params.trim().is_empty(),
            SuccessRateError::ConfigError("params field cannot be empty")
        );

        ensure!(
            !self.labels_with_status.is_empty()
                && !self
                    .labels_with_status
                    .iter()
                    .any(|item| item.label.is_empty()),
            SuccessRateError::ConfigError("labels field cannot be empty")
        );

        let config = self
            .config
            .as_ref()
            .ok_or_else(|| report!(SuccessRateError::ConfigError("config not found")))?;

        ensure!(
            config.max_aggregates_size > 0,
            SuccessRateError::ConfigError("invalid max_aggregates_size")
        );

        let current_block_threshold = config.current_block_threshold.as_ref().ok_or_else(|| {
            report!(SuccessRateError::ConfigError(
                "current_block_threshold not found"
            ))
        })?;

        ensure!(
            current_block_threshold.max_total_count > 0,
            SuccessRateError::ConfigError("invalid max_total_count")
        );

        Ok(())
    }
}

impl InvalidateWindowsRequest {
    pub fn validate(&self) -> error_stack::Result<(), SuccessRateError> {
        ensure!(
            !self.id.trim().is_empty(),
            SuccessRateError::ConfigError("id field cannot be empty")
        );

        Ok(())
    }
}

impl CalGlobalSuccessRateRequest {
    #[track_caller]
    pub fn validate(&self) -> error_stack::Result<(), SuccessRateError> {
        ensure!(
            !self.entity_id.trim().is_empty(),
            SuccessRateError::ConfigError("entity_id field cannot be empty")
        );

        ensure!(
            !self.entity_params.trim().is_empty(),
            SuccessRateError::ConfigError("entity_params field cannot be empty")
        );

        ensure!(
            !self.entity_labels.is_empty()
                && !self.entity_labels.iter().any(|label| label.is_empty()),
            SuccessRateError::ConfigError("entity_labels field cannot be empty")
        );

        ensure!(
            !self.global_labels.is_empty()
                && !self.global_labels.iter().any(|label| label.is_empty()),
            SuccessRateError::ConfigError("global_labels field cannot be empty")
        );

        let config = self
            .config
            .as_ref()
            .ok_or_else(|| report!(SuccessRateError::ConfigError("config not found")))?;

        ensure!(
            config.entity_min_aggregates_size > 0,
            SuccessRateError::ConfigError("invalid entity_min_aggregates_size")
        );

        ensure!(
            config.entity_default_success_rate > 0.0,
            SuccessRateError::ConfigError("invalid entity_default_success_rate")
        );
        Ok(())
    }
}
