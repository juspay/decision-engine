use clickhouse::Row;
use serde::Deserialize;

use crate::analytics::models::{
    RoutingEventsQuery, ROUTING_EVENTS_FAST_BUCKET_MS, ROUTING_EVENTS_SECOND_BUCKET_MS,
};
use crate::error::ApiError;

use super::super::common::{
    fetch_all, static_flow_type_in_sql, DOMAIN_TABLE, OVERVIEW_SCORE_FLOW_TYPES,
};
use super::super::filters::base_window_filters;
use super::super::query::{BoundQueryBuilder, FilterClause, OrderClause};

/// Latest score per gateway per fixed five-minute bucket. Buckets are
/// wall-clock aligned so downstream event IDs stay stable across polls.
#[derive(Debug, Clone, Deserialize, Row)]
pub struct ScoreBucketPoint {
    pub bucket_ms: i64,
    pub merchant_id: Option<String>,
    pub payment_method_type: Option<String>,
    pub payment_method: Option<String>,
    pub gateway: Option<String>,
    pub score_value: Option<f64>,
    pub transaction_count: Option<i64>,
}

pub async fn load(
    client: &clickhouse::Client,
    query: &RoutingEventsQuery,
    start_ms: i64,
    end_ms: i64,
) -> Result<Vec<ScoreBucketPoint>, ApiError> {
    let bucket_expr = if query.bucket_ms == ROUTING_EVENTS_SECOND_BUCKET_MS {
        "toUnixTimestamp(created_at) * 1000 AS bucket_ms"
    } else if query.bucket_ms == ROUTING_EVENTS_FAST_BUCKET_MS {
        "toUnixTimestamp(toStartOfMinute(created_at)) * 1000 AS bucket_ms"
    } else {
        "toUnixTimestamp(toStartOfFiveMinutes(created_at)) * 1000 AS bucket_ms"
    };
    let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);
    builder.extend_selects([
        bucket_expr,
        "merchant_id",
        "payment_method_type",
        "payment_method",
        "gateway",
        "argMax(score_value, created_at_ms) AS score_value",
        "argMax(transaction_count, created_at_ms) AS transaction_count",
    ]);
    builder.extend_filters(base_window_filters(start_ms, end_ms));
    builder.add_filter(FilterClause::eq("merchant_id", query.merchant_id.clone()));
    builder.add_filter(FilterClause::raw(format!(
        "flow_type IN {}",
        static_flow_type_in_sql(OVERVIEW_SCORE_FLOW_TYPES)
    )));
    if let Some(value) = &query.payment_method_type {
        builder.add_filter(FilterClause::eq("payment_method_type", value.clone()));
    }
    if let Some(value) = &query.payment_method {
        builder.add_filter(FilterClause::eq("payment_method", value.clone()));
    }
    builder.extend_group_bys([
        "bucket_ms",
        "merchant_id",
        "payment_method_type",
        "payment_method",
        "gateway",
    ]);
    builder.add_order_by(OrderClause::asc("bucket_ms"));
    builder.add_order_by(OrderClause::asc("gateway"));

    fetch_all::<ScoreBucketPoint>(builder.build(client)).await
}
