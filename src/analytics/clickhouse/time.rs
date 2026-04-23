use crate::analytics::models::{
    AnalyticsQuery, AnalyticsRange, PaymentAuditQuery, MAX_ANALYTICS_LOOKBACK_MS,
};
use crate::analytics::service::now_ms;

const MINUTE_MS: i64 = 60 * 1000;
const FIFTEEN_MINUTES_MS: i64 = 15 * MINUTE_MS;
const HOUR_MS: i64 = 60 * MINUTE_MS;
const TWELVE_HOURS_MS: i64 = 12 * HOUR_MS;
const DAY_MS: i64 = 24 * HOUR_MS;
const WEEK_MS: i64 = 7 * DAY_MS;
const OVER_FIFTEEN_MINUTES_MS: i64 = FIFTEEN_MINUTES_MS + 1;
const OVER_HOUR_MS: i64 = HOUR_MS + 1;
const OVER_TWELVE_HOURS_MS: i64 = TWELVE_HOURS_MS + 1;
const OVER_DAY_MS: i64 = DAY_MS + 1;

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

fn bucket_start_expr(query: &AnalyticsQuery, start_ms: i64, end_ms: i64) -> &'static str {
    if query.start_ms.is_some() && query.end_ms.is_some() {
        return match end_ms.saturating_sub(start_ms) {
            0..=FIFTEEN_MINUTES_MS => "toStartOfInterval(created_at, INTERVAL 1 MINUTE)",
            OVER_FIFTEEN_MINUTES_MS..=HOUR_MS => "toStartOfInterval(created_at, INTERVAL 5 MINUTE)",
            OVER_HOUR_MS..=TWELVE_HOURS_MS => "toStartOfInterval(created_at, INTERVAL 1 HOUR)",
            OVER_TWELVE_HOURS_MS..=DAY_MS => "toStartOfInterval(created_at, INTERVAL 1 HOUR)",
            OVER_DAY_MS..=WEEK_MS => "toStartOfInterval(created_at, INTERVAL 1 DAY)",
            _ => "toStartOfInterval(created_at, INTERVAL 7 DAY)",
        };
    }

    match query.range {
        AnalyticsRange::M15 => "toStartOfMinute(created_at)",
        AnalyticsRange::H1 => "toStartOfFiveMinutes(created_at)",
        AnalyticsRange::H12 => "toStartOfHour(created_at)",
        AnalyticsRange::D1 => "toStartOfHour(created_at)",
        AnalyticsRange::W1 => "toStartOfDay(created_at)",
    }
}

pub fn query_bucket_select_expr(query: &AnalyticsQuery, start_ms: i64, end_ms: i64) -> String {
    format!(
        "toUnixTimestamp({}) * 1000 AS bucket_ms",
        bucket_start_expr(query, start_ms, end_ms)
    )
}

pub fn payment_audit_range(query: &PaymentAuditQuery) -> String {
    match query.range {
        AnalyticsRange::M15 => "15m".to_string(),
        AnalyticsRange::H1 => "1h".to_string(),
        AnalyticsRange::H12 => "12h".to_string(),
        AnalyticsRange::D1 => "1d".to_string(),
        AnalyticsRange::W1 => "1w".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use crate::analytics::models::{AnalyticsQuery, AnalyticsRange};

    use super::{effective_window_bounds, query_bucket_select_expr, HOUR_MS, MINUTE_MS};

    fn query() -> AnalyticsQuery {
        AnalyticsQuery {
            merchant_id: "m_123".to_string(),
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
    fn preset_bucket_uses_fixed_helper() {
        let query = query();
        assert_eq!(
            query_bucket_select_expr(&query, 0, HOUR_MS),
            "toUnixTimestamp(toStartOfFiveMinutes(created_at)) * 1000 AS bucket_ms"
        );
    }

    #[test]
    fn custom_bucket_uses_interval_helper() {
        let mut query = query();
        query.start_ms = Some(0);
        query.end_ms = Some(15 * MINUTE_MS);
        assert_eq!(
            query_bucket_select_expr(&query, 0, 15 * MINUTE_MS),
            "toUnixTimestamp(toStartOfInterval(created_at, INTERVAL 1 MINUTE)) * 1000 AS bucket_ms"
        );
    }
}
