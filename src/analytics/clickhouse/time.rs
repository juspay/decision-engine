use crate::analytics::models::{
    AnalyticsQuery, AnalyticsRange, PaymentAuditQuery, MAX_ANALYTICS_LOOKBACK_MS,
};
use crate::analytics::service::now_ms;

pub fn effective_window_bounds(query: &AnalyticsQuery) -> (i64, i64) {
    let now = now_ms();
    let end_ms = query.end_ms.unwrap_or(now).min(now);
    let min_start_ms = end_ms.saturating_sub(MAX_ANALYTICS_LOOKBACK_MS);
    let start_ms = query
        .start_ms
        .filter(|start_ms| *start_ms >= 0 && *start_ms < end_ms)
        .unwrap_or_else(|| end_ms.saturating_sub(query.range.window_ms()))
        .max(min_start_ms);
    (start_ms, end_ms)
}

pub fn effective_payment_audit_window_bounds(query: &PaymentAuditQuery) -> (i64, i64) {
    let now = now_ms();
    let end_ms = query.end_ms.unwrap_or(now).min(now);
    let min_start_ms = end_ms.saturating_sub(MAX_ANALYTICS_LOOKBACK_MS);
    let start_ms = query
        .start_ms
        .filter(|start_ms| *start_ms >= 0 && *start_ms < end_ms)
        .unwrap_or_else(|| end_ms.saturating_sub(query.range.window_ms()))
        .max(min_start_ms);
    (start_ms, end_ms)
}

pub fn query_bucket_size_ms(start_ms: i64, end_ms: i64) -> i64 {
    let window_ms = end_ms.saturating_sub(start_ms);
    match window_ms {
        0..=900_000 => 60 * 1000,
        900_001..=3_600_000 => 5 * 60 * 1000,
        3_600_001..=86_400_000 => 15 * 60 * 1000,
        86_400_001..=259_200_000 => 60 * 60 * 1000,
        259_200_001..=2_592_000_000 => 3 * 60 * 60 * 1000,
        2_592_000_001..=15_552_000_000 => 24 * 60 * 60 * 1000,
        _ => 7 * 24 * 60 * 60 * 1000,
    }
}

pub fn payment_audit_range(query: &PaymentAuditQuery) -> String {
    match query.range {
        AnalyticsRange::M15 => "15m".to_string(),
        AnalyticsRange::H1 => "1h".to_string(),
        AnalyticsRange::H24 => "24h".to_string(),
        AnalyticsRange::D30 => "30d".to_string(),
        AnalyticsRange::M18 => "18mo".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use crate::analytics::models::{AnalyticsQuery, AnalyticsRange, AnalyticsScope};

    use super::{effective_window_bounds, query_bucket_size_ms};

    fn query() -> AnalyticsQuery {
        AnalyticsQuery {
            merchant_id: None,
            scope: AnalyticsScope::Current,
            range: AnalyticsRange::H1,
            start_ms: None,
            end_ms: None,
            page: 1,
            page_size: 10,
            payment_method_type: None,
            payment_method: None,
            card_network: None,
            card_is_in: None,
            currency: None,
            country: None,
            auth_type: None,
            gateways: Vec::new(),
        }
    }

    #[test]
    fn effective_bounds_prefer_custom_window() {
        let mut query = query();
        query.start_ms = Some(100);
        query.end_ms = Some(200);
        assert_eq!(effective_window_bounds(&query), (100, 200));
    }

    #[test]
    fn bucket_size_matches_existing_thresholds() {
        assert_eq!(query_bucket_size_ms(0, 900_000), 60_000);
        assert_eq!(query_bucket_size_ms(0, 3_600_000), 300_000);
        assert_eq!(query_bucket_size_ms(0, 86_400_000), 900_000);
    }
}
