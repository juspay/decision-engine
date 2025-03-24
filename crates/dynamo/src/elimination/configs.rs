use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EliminationBucketSettings {
    pub entity_bucket: BucketSettings,
    pub global_bucket: BucketSettings,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
pub struct BucketSettings {
    pub bucket_size: usize,
    #[serde(deserialize_with = "deserialize_duration_from_secs")]
    pub bucket_leak_interval_in_secs: Duration,
}

fn deserialize_duration_from_secs<'de, D>(deserializer: D) -> Result<Duration, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let secs = u64::deserialize(deserializer)?;
    Ok(Duration::from_secs(secs))
}
