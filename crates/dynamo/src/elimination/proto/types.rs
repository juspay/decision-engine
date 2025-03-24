#[allow(
    unused_qualifications,
    clippy::use_self,
    clippy::unwrap_used,
    clippy::as_conversions
)]
pub mod proto_items {
    tonic::include_proto!("elimination");
}

use std::time::Duration;

use error_stack::{ensure, report, Report, ResultExt};
pub use proto_items::{
    elimination_analyser_server::{EliminationAnalyser, EliminationAnalyserServer},
    invalidate_bucket_response::InvalidationStatus,
    update_elimination_bucket_response::UpdationStatus,
    BucketInformation, EliminationBucketConfig, EliminationInformation, EliminationRequest,
    EliminationResponse, InvalidateBucketRequest, InvalidateBucketResponse, LabelWithStatus,
    UpdateEliminationBucketRequest, UpdateEliminationBucketResponse,
};

use crate::elimination::{
    configs::{BucketSettings, EliminationBucketSettings},
    error::EliminationError,
};

impl TryFrom<(EliminationBucketConfig, BucketSettings)> for EliminationBucketSettings {
    type Error = Report<EliminationError>;
    fn try_from(
        (entity_config, global_config): (EliminationBucketConfig, BucketSettings),
    ) -> Result<Self, Self::Error> {
        let entity_settings = BucketSettings {
            bucket_size: usize::try_from(entity_config.bucket_size).change_context(
                EliminationError::TypeConversionError {
                    field: "bucket_size",
                    from: "u32",
                    to: "usize",
                },
            )?,
            bucket_leak_interval_in_secs: Duration::from_secs(
                entity_config.bucket_leak_interval_in_secs,
            ),
        };

        let global_settings = BucketSettings {
            bucket_size: global_config.bucket_size,
            bucket_leak_interval_in_secs: global_config.bucket_leak_interval_in_secs,
        };

        Ok(Self {
            entity_bucket: entity_settings,
            global_bucket: global_settings,
        })
    }
}

impl EliminationRequest {
    pub fn validate(&self) -> error_stack::Result<(), EliminationError> {
        ensure!(
            !self.id.trim().is_empty(),
            EliminationError::ConfigError("id field cannot be empty")
        );

        ensure!(
            !self.params.trim().is_empty(),
            EliminationError::ConfigError("params field cannot be empty")
        );

        ensure!(
            !self.labels.is_empty() && !self.labels.iter().any(|label| label.is_empty()),
            EliminationError::ConfigError("labels field cannot be empty")
        );

        let config = self
            .config
            .as_ref()
            .ok_or_else(|| report!(EliminationError::ConfigError("config not found")))?;

        ensure!(
            config.bucket_size > 0,
            EliminationError::ConfigError("invalid bucket_size")
        );

        ensure!(
            config.bucket_leak_interval_in_secs > 0,
            EliminationError::ConfigError("invalid bucket_leak_interval_in_secs")
        );

        Ok(())
    }
}

impl UpdateEliminationBucketRequest {
    pub fn validate(&self) -> error_stack::Result<(), EliminationError> {
        ensure!(
            !self.id.trim().is_empty(),
            EliminationError::ConfigError("id field cannot be empty")
        );

        ensure!(
            !self.params.trim().is_empty(),
            EliminationError::ConfigError("params field cannot be empty")
        );

        ensure!(
            !self.labels_with_bucket_name.is_empty()
                && !self
                    .labels_with_bucket_name
                    .iter()
                    .any(|item| item.label.trim().is_empty() || item.bucket_name.trim().is_empty()),
            EliminationError::ConfigError("labels field cannot be empty")
        );

        let config = self
            .config
            .as_ref()
            .ok_or_else(|| report!(EliminationError::ConfigError("config not found")))?;

        ensure!(
            config.bucket_size > 0,
            EliminationError::ConfigError("invalid bucket_size")
        );

        ensure!(
            config.bucket_leak_interval_in_secs > 0,
            EliminationError::ConfigError("invalid bucket_leak_interval_in_secs")
        );

        Ok(())
    }
}

impl InvalidateBucketRequest {
    pub fn validate(&self) -> error_stack::Result<(), EliminationError> {
        ensure!(
            !self.id.trim().is_empty(),
            EliminationError::ConfigError("id field cannot be empty")
        );

        Ok(())
    }
}
