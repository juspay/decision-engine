use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct CalContractScoreConfig {
    pub constants: Vec<f64>,
    pub time_scale: Option<TimeScale>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Deserialize, Serialize)]
pub enum TimeScale {
    Day,
    Month,
}
