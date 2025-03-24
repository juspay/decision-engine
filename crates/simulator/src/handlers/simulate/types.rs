use std::collections::HashMap;

use chrono::{DateTime, Utc};
use dynamo::{
    configs::GlobalSrConfig,
    success_rate::{
        configs::{CalculateSrConfig, SrCurrentBlockThreshold, UpdateWindowConfig},
        proto::types::SuccessRateSpecificityLevel,
    },
};
use serde::{Deserialize, Serialize};

#[derive(Clone, serde::Deserialize, Default, Debug)]
#[serde(rename_all = "snake_case")]
pub enum AlgorithmType {
    #[default]
    WindowBased,
}

#[derive(Clone, serde::Deserialize, Default, Debug)]
pub struct SimulateDataRequest {
    pub algo_type: AlgorithmType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulateDataResponse {
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CsvRecord {
    pub payment_intent_id: String,
    pub payment_attempt_id: String,
    pub amount: f64,
    pub payment_gateway: String,
    pub payment_status: bool,
    pub created_at: DateTime<Utc>,
    #[serde(flatten)]
    pub params: Params,
}

#[derive(Debug, Clone, Serialize, Deserialize)]

pub struct Payment {
    pub payment_intent_id: String,
    pub first_attempt_created_at: DateTime<Utc>,
    pub first_attempt_payment_status: bool,
    pub payment_attempts: Vec<CsvRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Params {
    pub payment_method_type: String,
    pub order_currency: String,
    pub card_network: String,
}

impl Params {
    pub fn get_concatenated(&self, suffix: String) -> String {
        format!(
            "{}:{}:{}:{}",
            self.payment_method_type, self.order_currency, self.card_network, suffix
        )
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct SrConfig {
    pub aggregates_threshold: AggregatesThreshold,
    pub current_block_threshold: CurrentBlockThreshold,
    pub default_success_rate: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct AggregatesThreshold {
    pub min_aggregates_size: usize,
    pub max_aggregates_size: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CurrentBlockThreshold {
    pub duration_in_mins: Option<std::time::Duration>,
    pub max_total_count: u64,
}

pub struct CalculateSrConfigWrapper(pub CalculateSrConfig);

impl From<(SrConfig, GlobalSrConfig)> for CalculateSrConfigWrapper {
    fn from(value: (SrConfig, GlobalSrConfig)) -> Self {
        Self(CalculateSrConfig {
            entity_min_aggregates_size: value.0.aggregates_threshold.min_aggregates_size,
            entity_default_success_rate: value.0.default_success_rate,
            specificity_level: SuccessRateSpecificityLevel::Entity,
            global_sr_config: value.1,
        })
    }
}

pub struct UpdateWindowConfigWrapper(pub UpdateWindowConfig);
impl From<(SrConfig, GlobalSrConfig)> for UpdateWindowConfigWrapper {
    fn from(value: (SrConfig, GlobalSrConfig)) -> Self {
        Self(UpdateWindowConfig {
            max_aggregates_size: value.0.aggregates_threshold.max_aggregates_size,
            current_block_threshold: SrCurrentBlockThreshold {
                duration_in_mins: value.0.current_block_threshold.duration_in_mins,
                max_total_count: value.0.current_block_threshold.max_total_count,
            },
            specificity_level: SuccessRateSpecificityLevel::Entity,
            global_sr_config: value.1,
        })
    }
}

#[derive(Debug, Clone)]
pub struct BaselineData {
    pub connectors: Vec<String>,
    pub total_attempts: usize,
    pub success_rate: f64,
    pub total_failed_payments: usize,
    pub total_revenue: f64,
    pub faar: f64,
    pub baseline_chunk_wise_sr: BaselineSrOfChunks,
    pub baseline_payments: Vec<Payment>,
}

#[derive(Debug, Clone)]
pub struct BaselineSrOfChunk {
    pub start: usize,
    pub end: usize,
    pub connectors_sr: HashMap<String, f64>,
}

#[derive(Debug, Clone, Default)]
pub struct BaselineSrOfChunks {
    pub chunk_wise_sr: Vec<BaselineSrOfChunk>,
}

impl BaselineSrOfChunks {
    pub fn push(&mut self, chunk: BaselineSrOfChunk) {
        self.chunk_wise_sr.push(chunk);
    }
}

#[derive(Debug)]
pub struct ModelOutcome {
    pub payment_gateway: String,
    pub status: bool,
    pub suggested_uplift: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationSummary {
    pub overall_success_rate: SuccessRate,
    pub total_failed_payments: FailedPayments,
    pub total_revenue: Revenue,
    pub faar: Faar,
    pub time_series_data: Vec<TimeSeriesData>,
    pub overall_success_rate_improvement: f64,
    pub total_payment_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuccessRate {
    pub baseline: f64,
    pub model: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Faar {
    pub baseline: f64,
    pub model: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailedPayments {
    pub baseline: usize,
    pub model: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Revenue {
    pub baseline: f64,
    pub model: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesSummary(Vec<TimeSeriesData>);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesData {
    pub time_stamp: String,
    pub success_rate: SuccessRate,
    pub revenue: Revenue,
    pub volume_distribution_as_per_sr: HashMap<String, VolumeDistributionAsPerSr>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeDistributionAsPerSr {
    pub success_rate: f64,
    pub baseline_volume: usize,
    pub model_volume: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationOutcomeOfEachTxn {
    pub txn_no: usize,
    #[serde(flatten)]
    pub baseline_record: CsvRecord,
    pub model_connector: String,
    #[serde(skip)]
    pub model_status: bool,
    pub suggested_uplift: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationReportResponse {
    pub total_payment_count: usize,
    pub simulation_outcome_of_each_txn: Vec<SimulationOutcomeOfEachTxn>,
}

impl SimulationReportResponse {
    pub fn new(
        total_payment_count: usize,
        simulation_outcome_of_each_txn: Vec<SimulationOutcomeOfEachTxn>,
    ) -> Self {
        Self {
            total_payment_count,
            simulation_outcome_of_each_txn,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SuccessBasedRoutingResponse {
    pub model_success_rate: f64,
    pub total_failed_payments: usize,
    pub total_revenue: f64,
    pub faar: f64,
    pub improvement: f64,
    pub simulation_outcome_of_each_txn: Vec<SimulationOutcomeOfEachTxn>,
    pub total_txn_count: usize,
}

#[derive(Debug, Clone, Default)]
pub struct ConnectorStats {
    pub success_count: u64,
    pub total_count: u64,
}

#[derive(Deserialize)]
pub struct SearchParams {
    pub offset: usize,
    pub limit: usize,
}
#[derive(Debug, Clone, Default)]
pub struct VolumeDistributionWithStats {
    pub connector_success_count: usize,
    pub baseline_volume: usize,
    pub model_volume: usize,
}
