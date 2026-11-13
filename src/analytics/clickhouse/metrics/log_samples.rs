use clickhouse::Row;
use serde::Deserialize;

use crate::analytics::models::{AnalyticsLogSample, AnalyticsQuery};
use crate::error::ApiError;

use super::super::common::{
    fetch_all, static_flow_type_in_sql, DOMAIN_TABLE, OVERVIEW_ERROR_FLOW_TYPES,
};
use super::super::filters::{analytics_dimension_filters, base_window_filters, merchant_filter};
use super::super::query::{BoundQueryBuilder, FilterClause, OrderClause};
use super::super::time::effective_window_bounds;

#[derive(Debug, Clone, Deserialize, Row)]
struct LogSampleRow {
    route: Option<String>,
    merchant_id: Option<String>,
    payment_id: Option<String>,
    request_id: Option<String>,
    global_request_id: Option<String>,
    trace_id: Option<String>,
    gateway: Option<String>,
    routing_approach: Option<String>,
    status: Option<String>,
    error_code: Option<String>,
    error_message: Option<String>,
    flow_type: String,
    created_at_ms: i64,
}

pub async fn load(
    client: &clickhouse::Client,
    query: &AnalyticsQuery,
) -> Result<Vec<AnalyticsLogSample>, ApiError> {
    let (start_ms, end_ms) = effective_window_bounds(query);
    let page = query.page;
    let page_size = query.page_size;
    let offset = (page - 1) * page_size;

    let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);
    builder.extend_selects([
        "route".to_string(),
        "merchant_id".to_string(),
        "payment_id".to_string(),
        "request_id".to_string(),
        "global_request_id".to_string(),
        "trace_id".to_string(),
        "gateway".to_string(),
        "routing_approach".to_string(),
        "status".to_string(),
        "error_code".to_string(),
        "error_message".to_string(),
        "flow_type".to_string(),
        "created_at_ms".to_string(),
    ]);
    builder.extend_filters(base_window_filters(start_ms, end_ms));
    builder.extend_filters(merchant_filter(&query.merchant_id));
    builder.extend_filters(analytics_dimension_filters(query));
    builder.add_filter(FilterClause::raw(format!(
        "flow_type IN {}",
        static_flow_type_in_sql(OVERVIEW_ERROR_FLOW_TYPES)
    )));
    builder.add_order_by(OrderClause::desc("created_at_ms"));
    builder.set_limit(Some(page_size as u64));
    builder.set_offset(Some(offset as u64));

    let rows = fetch_all::<LogSampleRow>(builder.build(client)).await?;
    Ok(rows
        .into_iter()
        .map(|row| AnalyticsLogSample {
            route: row.route,
            merchant_id: row.merchant_id,
            payment_id: row.payment_id,
            request_id: row.request_id,
            global_request_id: row.global_request_id,
            trace_id: row.trace_id,
            gateway: row.gateway,
            routing_approach: row.routing_approach,
            status: row.status,
            error_code: row.error_code,
            error_message: row.error_message,
            flow_type: Some(row.flow_type),
            created_at_ms: row.created_at_ms,
        })
        .collect())
}
