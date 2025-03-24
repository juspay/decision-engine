use std::time::{Duration, SystemTime};

use error_stack::ResultExt;

use crate::elimination::{
    configs::BucketSettings, error::EliminationError, types::EliminationStatus,
};

#[derive(Debug, serde::Deserialize, serde::Serialize)]
pub struct LeakyBucket {
    pub bucket_name: String,
    pub current_level: usize,
    last_leak: SystemTime,
}

pub struct BucketStatus {
    pub is_filled: bool,
    pub number_of_leaks: usize,
}

impl LeakyBucket {
    pub fn new(bucket_name: String) -> Self {
        Self {
            bucket_name,
            current_level: 0,
            last_leak: SystemTime::now(),
        }
    }

    fn leak(&mut self, leak_rate: Duration) -> error_stack::Result<usize, EliminationError> {
        let now = SystemTime::now();
        let elapsed = now
            .duration_since(self.last_leak)
            .change_context(EliminationError::FailedToGetElapsedTime)?;

        #[allow(clippy::as_conversions)]
        let current_leak = (elapsed.as_secs_f64() / leak_rate.as_secs_f64()).floor() as usize;

        if current_leak > 0 {
            self.last_leak = now;
            self.current_level = self.current_level.saturating_sub(current_leak);
        }

        Ok(current_leak)
    }

    pub fn fill(
        &mut self,
        amount: usize,
        config: &BucketSettings,
    ) -> error_stack::Result<bool, EliminationError> {
        self.leak(config.bucket_leak_interval_in_secs)?;

        let is_fill_successful = if self.current_level + amount <= config.bucket_size {
            self.current_level += amount;
            true
        } else {
            false
        };

        Ok(is_fill_successful)
    }

    pub fn check_if_full(
        &mut self,
        config: &BucketSettings,
    ) -> error_stack::Result<BucketStatus, EliminationError> {
        let number_of_leaks = self.leak(config.bucket_leak_interval_in_secs)?;

        Ok(BucketStatus {
            is_filled: self.current_level >= config.bucket_size,
            number_of_leaks,
        })
    }
}

pub fn get_elimination_status(
    buckets: &mut Vec<LeakyBucket>,
    config: &BucketSettings,
) -> error_stack::Result<EliminationStatus, EliminationError> {
    let mut should_eliminate = false;
    let mut should_update_leaks_in_redis = false;
    let mut bucket_names = Vec::new();

    for bucket in buckets {
        let BucketStatus {
            is_filled,
            number_of_leaks,
        } = bucket.check_if_full(config)?;

        if number_of_leaks > 0 {
            should_update_leaks_in_redis = true;
        }

        if is_filled {
            bucket_names.push(bucket.bucket_name.clone());
            should_eliminate = true;
        }
    }

    Ok(EliminationStatus {
        should_eliminate,
        should_update_leaks_in_redis,
        bucket_names,
    })
}

pub fn upsert_bucket(
    buckets: &mut Vec<LeakyBucket>,
    bucket_name_to_update: &str,
    config: &BucketSettings,
) -> error_stack::Result<(), EliminationError> {
    let bucket_to_update = buckets
        .iter_mut()
        .find(|bucket| bucket.bucket_name == bucket_name_to_update);

    // If bucket already exists, fill it by amount 1. Or else create a new bucket and add it to the existing list of buckets
    if let Some(bucket) = bucket_to_update {
        bucket.fill(1, config)?;
    } else {
        let mut new_bucket = LeakyBucket::new(bucket_name_to_update.to_string());
        new_bucket.fill(1, config)?;
        buckets.push(new_bucket);
    }

    Ok(())
}
