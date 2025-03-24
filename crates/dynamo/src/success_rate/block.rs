use error_stack::ResultExt;
use fred::{error as fred_error, types as fred_types};

use crate::success_rate::{
    configs::SrCurrentBlockThreshold, error::SuccessRateError, types::BlockFields, utils,
};

#[derive(Debug, serde::Serialize, Default, serde::Deserialize, Clone)]
pub struct Block {
    pub success_count: u64,
    pub total_count: u64,
    pub created_at: u64,
}

impl Block {
    pub fn new(
        success_count: u64,
        total_count: u64,
    ) -> error_stack::Result<Self, SuccessRateError> {
        Ok(Self {
            success_count,
            total_count,
            created_at: utils::get_current_time_in_secs()?,
        })
    }

    pub(super) fn update_created_at(&mut self, created_at: u64) {
        self.created_at = created_at;
    }

    pub(super) async fn validate_threshold(
        &self,
        current_block_threshold: &SrCurrentBlockThreshold,
    ) -> error_stack::Result<bool, SuccessRateError> {
        let is_duration_threshold_breached =
            if let Some(threshold_duration) = current_block_threshold.duration_in_mins {
                let threshold_duration = threshold_duration.as_secs();
                let current_block_start_time = self.created_at;
                let current_time = utils::get_current_time_in_secs()?;

                current_time > (current_block_start_time + threshold_duration)
            } else {
                false
            };

        let is_size_threshold_breached =
            self.total_count >= current_block_threshold.max_total_count;

        Ok(is_duration_threshold_breached || is_size_threshold_breached)
    }

    pub(super) fn calculate_weighted_success_rate(aggregates: &[Self]) -> f64 {
        let sum_of_weights = (aggregates.len() * (aggregates.len() + 1)) / 2;
        let mut weighted_success_rate = 0.0;

        #[allow(clippy::as_conversions)]
        for (index, block) in aggregates.iter().enumerate() {
            let success_rate_of_block = block.success_count as f64 / block.total_count as f64;
            let weight_of_block = (index + 1) as f64 / sum_of_weights as f64;

            weighted_success_rate += (success_rate_of_block * weight_of_block) * 100.0;
        }

        (weighted_success_rate * 100.0).floor() / 100.0
    }
}

impl TryFrom<Block> for fred_types::RedisMap {
    type Error = error_stack::Report<SuccessRateError>;

    fn try_from(value: Block) -> Result<Self, Self::Error> {
        let block = vec![
            (BlockFields::SuccessCount.to_string(), value.success_count),
            (BlockFields::TotalCount.to_string(), value.total_count),
            (BlockFields::CreatedAt.to_string(), value.created_at),
        ];

        Self::try_from(block).change_context(SuccessRateError::RedisError(
            "Failed to convert from Block to RedisMap for Redis current_block insertion",
        ))
    }
}

#[derive(Debug, serde::Serialize, Clone, Default, serde::Deserialize)]
pub struct CurrentBlock(Option<Block>);

impl CurrentBlock {
    pub fn new() -> Self {
        Self(Some(Block::default()))
    }

    pub fn inner(self) -> Option<Block> {
        self.0
    }

    pub fn inner_mut(&mut self) -> Option<&mut Block> {
        self.0.as_mut()
    }

    pub fn from_block(block: Block) -> Self {
        Self(Some(block))
    }
}

impl fred_types::FromRedis for CurrentBlock {
    fn from_value(value: fred_types::RedisValue) -> Result<Self, fred_error::RedisError> {
        match value {
            fred_types::RedisValue::Map(map) => {
                if map.len() == 0 {
                    return Ok(Self(None));
                }

                let success_count = map
                    .get(&fred_types::RedisKey::from(
                        BlockFields::SuccessCount.to_string(),
                    ))
                    .and_then(|rv| rv.as_u64())
                    .ok_or(fred_error::RedisError::new(
                        fred_error::RedisErrorKind::Unknown,
                        "Unexpected error occurred when fetching success_count from hash in Redis",
                    ))?;
                let total_count = map
                    .get(&fred_types::RedisKey::from(
                        BlockFields::TotalCount.to_string(),
                    ))
                    .and_then(|rv| rv.as_u64())
                    .ok_or(fred_error::RedisError::new(
                        fred_error::RedisErrorKind::Unknown,
                        "Unexpected error occurred when fetching total_count from hash in Redis",
                    ))?;
                let created_at = map
                    .get(&fred_types::RedisKey::from(
                        BlockFields::CreatedAt.to_string(),
                    ))
                    .and_then(|rv| rv.as_u64())
                    .ok_or(fred_error::RedisError::new(
                        fred_error::RedisErrorKind::Unknown,
                        "Unexpected error occurred when fetching created_at from hash in Redis",
                    ))?;

                Ok(Self(Some(Block {
                    success_count,
                    total_count,
                    created_at,
                })))
            }
            _ => Err(fred_error::RedisError::new(
                fred_error::RedisErrorKind::Unknown,
                "Deserialization error: Unexpected Block type found in Redis",
            )),
        }
    }
}
