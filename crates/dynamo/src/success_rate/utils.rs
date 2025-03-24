use crate::success_rate::error::SuccessRateError;
use error_stack::ResultExt;
use std::time;

pub(crate) fn get_current_time_in_secs() -> error_stack::Result<u64, SuccessRateError> {
    Ok(time::SystemTime::now()
        .duration_since(time::UNIX_EPOCH)
        .change_context(SuccessRateError::FailedToGetCurrentTime)?
        .as_secs())
}

pub(crate) fn sort_sr_by_score<T>(mut vec: Vec<(f64, T)>) -> Vec<(f64, T)> {
    vec.sort_by(|a, b| b.0.total_cmp(&a.0));
    vec
}

pub(crate) fn get_success_and_total_count_based_on_status(status: bool) -> (u64, u64) {
    let success_count = status.then_some(1).unwrap_or_default();
    let total_count = 1;
    (success_count, total_count)
}
