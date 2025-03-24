use serde::{Deserialize, Serialize};

use super::proto::types::SuccessRateSpecificityLevel;
use crate::configs::GlobalSrConfig;
use std::time;

#[derive(Debug, Serialize, Deserialize)]
pub struct CalculateSrConfig {
    pub entity_min_aggregates_size: usize,
    pub entity_default_success_rate: f64,
    pub global_sr_config: GlobalSrConfig,
    pub specificity_level: SuccessRateSpecificityLevel,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct UpdateWindowConfig {
    pub max_aggregates_size: usize,
    pub current_block_threshold: SrCurrentBlockThreshold,
    pub global_sr_config: GlobalSrConfig,
    pub specificity_level: SuccessRateSpecificityLevel,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SrCurrentBlockThreshold {
    pub duration_in_mins: Option<time::Duration>,
    pub max_total_count: u64,
}
