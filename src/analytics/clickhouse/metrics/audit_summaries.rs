use clickhouse::Row;
use serde::Deserialize;

use crate::analytics::models::{PaymentAuditQuery, PaymentAuditSummary};
use crate::error::ApiError;

use super::super::common::{
    fetch_all, fetch_one, payment_audit_route_label, payment_audit_stage_label,
    PAYMENT_AUDIT_SUMMARY_BUCKET_TABLE,
};
use super::super::filters::payment_audit_summary_bucket_filters;
use super::super::query::{BoundQueryBuilder, FilterClause, OrderClause, SqlFragment};

#[derive(Debug, Clone, Deserialize, Row)]
struct AuditSummaryRow {
    lookup_key: String,
    payment_id: Option<String>,
    request_id: Option<String>,
    merchant_id: String,
    first_seen_ms: i64,
    last_seen_ms: i64,
    event_count: u64,
    latest_status: Option<String>,
    latest_gateway: Option<String>,
    latest_stage: Option<String>,
    gateways: Vec<String>,
    routes: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Row)]
struct CountRow {
    total_results: u64,
}

fn grouped_summary_fragment(query: &PaymentAuditQuery, preview_only: bool) -> SqlFragment {
    let mut builder = BoundQueryBuilder::new(PAYMENT_AUDIT_SUMMARY_BUCKET_TABLE);
    builder.extend_selects([
        "lookup_key".to_string(),
        "argMaxMerge(payment_id_state) AS payment_id".to_string(),
        "argMaxMerge(request_id_state) AS request_id".to_string(),
        "argMaxMerge(merchant_id_state) AS merchant_id".to_string(),
        "minMerge(first_seen_ms_state) AS first_seen_ms".to_string(),
        "maxMerge(last_seen_ms_state) AS last_seen_ms".to_string(),
        "sumMerge(event_count_state) AS event_count".to_string(),
        "argMaxMerge(latest_status_state) AS latest_status".to_string(),
        "argMaxMerge(latest_gateway_state) AS latest_gateway".to_string(),
        "argMaxMerge(latest_stage_state) AS latest_stage".to_string(),
        "arrayFilter(value -> value != '', groupUniqArrayMerge(gateways_state)) AS gateways"
            .to_string(),
        "arrayFilter(value -> value != '', groupUniqArrayMerge(routes_state)) AS routes"
            .to_string(),
        "arrayFilter(value -> value != '', groupUniqArrayMerge(statuses_state)) AS statuses"
            .to_string(),
        "arrayFilter(value -> value != '', groupUniqArrayMerge(flow_types_state)) AS flow_types"
            .to_string(),
        "arrayFilter(value -> value != '', groupUniqArrayMerge(error_codes_state)) AS error_codes"
            .to_string(),
    ]);
    builder.extend_filters(payment_audit_summary_bucket_filters(query, preview_only));
    builder.add_group_by("lookup_key");
    builder.into_fragment()
}

fn outer_summary_filters(query: &PaymentAuditQuery) -> Vec<FilterClause> {
    let mut filters = Vec::new();

    if let Some(lookup_key) = crate::analytics::derive_lookup_key(
        query.payment_id.as_deref(),
        query.request_id.as_deref(),
    ) {
        filters.push(FilterClause::eq("lookup_key", lookup_key));
    }
    if let Some(gateway) = &query.gateway {
        filters.push(FilterClause::new(
            "has(gateways, ?)",
            vec![gateway.clone().into()],
        ));
    }
    if let Some(route) = &query.route {
        filters.push(FilterClause::new(
            "has(routes, ?)",
            vec![route.clone().into()],
        ));
    }
    if let Some(status) = &query.status {
        filters.push(FilterClause::new(
            "has(statuses, ?)",
            vec![status.clone().into()],
        ));
    }
    if let Some(flow_type) = &query.flow_type {
        filters.push(FilterClause::new(
            "has(flow_types, ?)",
            vec![flow_type.clone().into()],
        ));
    }
    if let Some(error_code) = &query.error_code {
        filters.push(FilterClause::new(
            "has(error_codes, ?)",
            vec![error_code.clone().into()],
        ));
    }

    filters
}

fn results_builder(fragment: SqlFragment, query: &PaymentAuditQuery) -> BoundQueryBuilder {
    let mut builder = BoundQueryBuilder::from_fragment(SqlFragment::with_binds(
        format!("({})", fragment.sql()),
        fragment.binds().to_vec(),
    ));
    builder.extend_selects([
        "lookup_key".to_string(),
        "payment_id".to_string(),
        "request_id".to_string(),
        "merchant_id".to_string(),
        "first_seen_ms".to_string(),
        "last_seen_ms".to_string(),
        "event_count".to_string(),
        "latest_status".to_string(),
        "latest_gateway".to_string(),
        "latest_stage".to_string(),
        "gateways".to_string(),
        "routes".to_string(),
    ]);
    builder.extend_filters(outer_summary_filters(query));
    builder
}

fn map_rows(rows: Vec<AuditSummaryRow>) -> Vec<PaymentAuditSummary> {
    rows.into_iter()
        .map(|row| PaymentAuditSummary {
            lookup_key: row.lookup_key,
            payment_id: row.payment_id,
            request_id: row.request_id,
            merchant_id: Some(row.merchant_id),
            first_seen_ms: row.first_seen_ms,
            last_seen_ms: row.last_seen_ms,
            event_count: row.event_count as usize,
            latest_status: row.latest_status,
            latest_gateway: row.latest_gateway,
            latest_stage: row.latest_stage.map(payment_audit_stage_label),
            gateways: row.gateways,
            routes: row
                .routes
                .into_iter()
                .map(payment_audit_route_label)
                .collect(),
        })
        .collect()
}

pub async fn count(
    client: &clickhouse::Client,
    query: &PaymentAuditQuery,
    preview_only: bool,
) -> Result<usize, ApiError> {
    let grouped = grouped_summary_fragment(query, preview_only);
    let mut builder = BoundQueryBuilder::from_fragment(SqlFragment::with_binds(
        format!("({})", grouped.sql()),
        grouped.binds().to_vec(),
    ));
    builder.add_select("count() AS total_results");
    builder.extend_filters(outer_summary_filters(query));
    let row = fetch_one::<CountRow>(builder.build(client)).await?;
    Ok(row.total_results as usize)
}

pub async fn load_page(
    client: &clickhouse::Client,
    query: &PaymentAuditQuery,
    preview_only: bool,
) -> Result<Vec<PaymentAuditSummary>, ApiError> {
    let grouped = grouped_summary_fragment(query, preview_only);
    let mut builder = results_builder(grouped, query);
    builder.add_order_by(OrderClause::desc("last_seen_ms"));
    builder.add_order_by(OrderClause::desc("event_count"));
    builder.set_limit(Some(query.page_size as u64));
    builder.set_offset(Some(((query.page - 1) * query.page_size) as u64));
    let rows = fetch_all::<AuditSummaryRow>(builder.build(client)).await?;
    Ok(map_rows(rows))
}

pub async fn load_exact(
    client: &clickhouse::Client,
    query: &PaymentAuditQuery,
    preview_only: bool,
    lookup_key: &str,
) -> Result<Vec<PaymentAuditSummary>, ApiError> {
    let mut exact_query = query.clone();
    exact_query.payment_id = Some(lookup_key.to_string());
    exact_query.request_id = None;

    let grouped = grouped_summary_fragment(&exact_query, preview_only);
    let mut builder = results_builder(grouped, &exact_query);
    builder.add_order_by(OrderClause::desc("last_seen_ms"));
    builder.add_order_by(OrderClause::desc("event_count"));
    builder.set_limit(Some(1));
    let rows = fetch_all::<AuditSummaryRow>(builder.build(client)).await?;
    Ok(map_rows(rows))
}

#[cfg(test)]
mod tests {
    use crate::analytics::clickhouse::common::PAYMENT_AUDIT_SUMMARY_BUCKET_TABLE;
    use crate::analytics::models::{AnalyticsRange, PaymentAuditQuery};

    use super::{grouped_summary_fragment, outer_summary_filters};

    fn payment_audit_query() -> PaymentAuditQuery {
        PaymentAuditQuery {
            merchant_id: "m_123".to_string(),
            range: AnalyticsRange::H1,
            start_ms: Some(100),
            end_ms: Some(200),
            page: 1,
            page_size: 10,
            payment_id: None,
            request_id: None,
            gateway: None,
            route: None,
            status: None,
            flow_type: None,
            error_code: None,
        }
    }

    #[test]
    fn grouped_summary_fragment_uses_summary_bucket_table() {
        let fragment = grouped_summary_fragment(&payment_audit_query(), false);
        assert!(fragment.sql().contains(PAYMENT_AUDIT_SUMMARY_BUCKET_TABLE));
        assert!(fragment.sql().contains("GROUP BY lookup_key"));
        assert!(!fragment.sql().contains("analytics_domain_events"));
    }

    #[test]
    fn outer_filters_use_lookup_key_for_exact_request_filters() {
        let mut query = payment_audit_query();
        query.request_id = Some("req_123".to_string());
        let predicates = outer_summary_filters(&query)
            .into_iter()
            .map(|filter| filter.predicate().to_string())
            .collect::<Vec<_>>();
        assert!(predicates
            .iter()
            .any(|predicate| predicate == "lookup_key = ?"));
        assert!(!predicates
            .iter()
            .any(|predicate| predicate == "request_id = ?"));
    }
}
