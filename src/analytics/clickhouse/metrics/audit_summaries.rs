use std::borrow::Cow;
use std::collections::HashMap;

use clickhouse::Row;
use serde::Deserialize;

use crate::analytics::models::{PaymentAuditQuery, PaymentAuditScope, PaymentAuditSummary};
use crate::error::ApiError;

use super::super::common::{
    fetch_all, fetch_one, payment_audit_route_label, payment_audit_stage_label, DOMAIN_TABLE,
    PAYMENT_AUDIT_SUMMARY_BUCKET_TABLE,
};
use super::super::filters::{
    payment_audit_needs_raw_rows, payment_audit_routing_kind_filters,
    payment_audit_summary_bucket_filters, payment_audit_summary_scope_filters,
    payment_audit_timeline_filters, raw_routing_families_sql, summary_routing_families_sql,
};
use super::super::query::{BindArg, BoundQueryBuilder, FilterClause, OrderClause, SqlFragment};
use super::super::time::effective_payment_audit_window_bounds;

#[derive(Debug, Clone, Deserialize, Row)]
struct AuditSummaryRow {
    #[serde(alias = "resolved_lookup_key")]
    lookup_key: String,
    payment_id: Option<String>,
    request_id: Option<String>,
    #[serde(alias = "resolved_merchant_id")]
    merchant_id: Option<String>,
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
    total_success: u64,
    total_failure: u64,
}

#[derive(Debug, Clone, Deserialize, Row)]
struct PageKeyRow {
    lookup_key: String,
}

/// One per-payment aggregate a summary query can compute.
///
/// ClickHouse holds every selected aggregate for every payment in the window while it groups, so
/// each query names only the aggregates it returns or filters on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Aggregate {
    PaymentId,
    RequestId,
    MerchantId,
    FirstSeen,
    LastSeen,
    EventCount,
    LatestStatus,
    LatestGateway,
    LatestStage,
    Gateways,
    Routes,
    Statuses,
    FlowTypes,
    /// The event families as a bitmask, which is all a routing-kind filter reads.
    RoutingFamilies,
    ErrorCodes,
}

/// What a returned summary row carries.
const ROW_AGGREGATES: &[Aggregate] = &[
    Aggregate::PaymentId,
    Aggregate::RequestId,
    Aggregate::MerchantId,
    Aggregate::FirstSeen,
    Aggregate::LastSeen,
    Aggregate::EventCount,
    Aggregate::LatestStatus,
    Aggregate::LatestGateway,
    Aggregate::LatestStage,
    Aggregate::Gateways,
    Aggregate::Routes,
];

/// What the count reads: the window overlap and the latest status it splits totals by.
const COUNT_AGGREGATES: &[Aggregate] = &[
    Aggregate::FirstSeen,
    Aggregate::LastSeen,
    Aggregate::LatestStatus,
];

/// What choosing a page reads: the window overlap and the page's sort keys.
const PAGE_KEY_AGGREGATES: &[Aggregate] = &[
    Aggregate::FirstSeen,
    Aggregate::LastSeen,
    Aggregate::EventCount,
];

impl Aggregate {
    /// The state column this aggregate merges from `analytics_payment_audit_summary_buckets`.
    const fn state_column(self) -> &'static str {
        match self {
            Self::PaymentId => "payment_id_state",
            Self::RequestId => "request_id_state",
            Self::MerchantId => "merchant_id",
            Self::FirstSeen => "first_seen_ms_state",
            Self::LastSeen => "last_seen_ms_state",
            Self::EventCount => "event_count_state",
            Self::LatestStatus => "latest_status_state",
            Self::LatestGateway => "latest_gateway_state",
            Self::LatestStage => "latest_stage_state",
            Self::Gateways => "gateways_state",
            Self::Routes => "routes_state",
            Self::Statuses => "statuses_state",
            Self::FlowTypes => "flow_types_state",
            Self::RoutingFamilies => "summary_kind",
            Self::ErrorCodes => "error_codes_state",
        }
    }

    fn merged_sql(self) -> Cow<'static, str> {
        Cow::Borrowed(match self {
            Self::PaymentId => "argMaxMerge(payment_id_state) AS payment_id",
            Self::RequestId => "argMaxMerge(request_id_state) AS request_id",
            Self::MerchantId => "toNullable(any(merchant_id)) AS merchant_id",
            Self::FirstSeen => "minMerge(first_seen_ms_state) AS first_seen_ms",
            Self::LastSeen => "maxMerge(last_seen_ms_state) AS last_seen_ms",
            Self::EventCount => "sumMerge(event_count_state) AS event_count",
            Self::LatestStatus => "argMaxMerge(latest_status_state) AS latest_status",
            Self::LatestGateway => "argMaxMerge(latest_gateway_state) AS latest_gateway",
            Self::LatestStage => "argMaxMerge(latest_stage_state) AS latest_stage",
            Self::Gateways => {
                "arrayFilter(value -> value != '', groupUniqArrayMerge(gateways_state)) AS gateways"
            }
            Self::Routes => {
                "arrayFilter(value -> value != '', groupUniqArrayMerge(routes_state)) AS routes"
            }
            Self::Statuses => {
                "arrayFilter(value -> value != '', groupUniqArrayMerge(statuses_state)) AS statuses"
            }
            Self::FlowTypes => {
                "arrayFilter(value -> value != '', groupUniqArrayMerge(flow_types_state)) AS flow_types"
            }
            Self::RoutingFamilies => return Cow::Owned(summary_routing_families_sql()),
            Self::ErrorCodes => {
                "arrayFilter(value -> value != '', groupUniqArrayMerge(error_codes_state)) AS error_codes"
            }
        })
    }

    /// The raw `analytics_domain_events` column this aggregate reads, beyond `lookup_key` and
    /// `created_at_ms`, which every raw summary selects.
    const fn raw_column(self) -> Option<&'static str> {
        match self {
            Self::PaymentId => Some("payment_id"),
            Self::RequestId => Some("request_id"),
            Self::MerchantId => Some("merchant_id"),
            Self::FirstSeen | Self::LastSeen | Self::EventCount => None,
            Self::LatestStatus | Self::Statuses => Some("status"),
            Self::LatestGateway | Self::Gateways => Some("gateway"),
            Self::LatestStage => Some("event_stage"),
            Self::Routes => Some("route"),
            Self::FlowTypes | Self::RoutingFamilies => Some("flow_type"),
            Self::ErrorCodes => Some("error_code"),
        }
    }

    fn raw_sql(self) -> Cow<'static, str> {
        Cow::Borrowed(match self {
            Self::PaymentId => "argMax(payment_id, created_at_ms) AS payment_id",
            Self::RequestId => "argMax(request_id, created_at_ms) AS request_id",
            Self::MerchantId => "any(merchant_id) AS merchant_id",
            Self::FirstSeen => "min(created_at_ms) AS first_seen_ms",
            Self::LastSeen => "max(created_at_ms) AS last_seen_ms",
            Self::EventCount => "count() AS event_count",
            Self::LatestStatus => "argMax(status, created_at_ms) AS latest_status",
            Self::LatestGateway => "argMax(gateway, created_at_ms) AS latest_gateway",
            Self::LatestStage => "argMax(event_stage, created_at_ms) AS latest_stage",
            Self::Gateways => {
                "arrayFilter(value -> value != '', groupUniqArray(ifNull(gateway, ''))) AS gateways"
            }
            Self::Routes => {
                "arrayFilter(value -> value != '', groupUniqArray(ifNull(route, ''))) AS routes"
            }
            Self::Statuses => {
                "arrayFilter(value -> value != '', groupUniqArray(ifNull(status, ''))) AS statuses"
            }
            Self::FlowTypes => {
                "arrayFilter(value -> value != '', groupUniqArray(flow_type)) AS flow_types"
            }
            Self::RoutingFamilies => return Cow::Owned(raw_routing_families_sql()),
            Self::ErrorCodes => {
                "arrayFilter(value -> value != '', groupUniqArray(ifNull(error_code, ''))) AS error_codes"
            }
        })
    }
}

/// `base` plus the aggregates `outer_summary_filters` tests for this query, without repeats.
fn with_filter_aggregates(query: &PaymentAuditQuery, base: &[Aggregate]) -> Vec<Aggregate> {
    let routing_kind_filters = query
        .routing_kind
        .is_some_and(|kind| !payment_audit_routing_kind_filters(kind).is_empty());
    let filtered = [
        (query.gateway.is_some(), Aggregate::Gateways),
        (query.route.is_some(), Aggregate::Routes),
        (query.status.is_some(), Aggregate::Statuses),
        (query.flow_type.is_some(), Aggregate::FlowTypes),
        (routing_kind_filters, Aggregate::RoutingFamilies),
        (query.error_code.is_some(), Aggregate::ErrorCodes),
    ];

    let mut aggregates = base.to_vec();
    for (needed, aggregate) in filtered {
        if needed && !aggregates.contains(&aggregate) {
            aggregates.push(aggregate);
        }
    }
    aggregates
}

fn subquery(fragment: SqlFragment) -> BoundQueryBuilder {
    BoundQueryBuilder::from_fragment(SqlFragment::with_binds(
        format!("({})", fragment.sql()),
        fragment.binds().to_vec(),
    ))
}

/// The pre-aggregated path: one row per payment, merged across every `summary_kind` the payment
/// produced (rule preview, hybrid, dynamic), so a hybrid payment's rule-evaluate, decide-gateway
/// and score-update events read as one trail. It reads the 15-minute summary buckets that overlap
/// the query window, so its cost follows the window rather than the merchant's whole history, and
/// the aggregates describe the payment's events inside that window. A narrow scope pins its
/// `summary_kind`s. Same two-level shape as `raw_summary_fragment`, so the merchant filter stays
/// on the source rows and never collides with an aggregate alias.
fn merged_summary_fragment(
    query: &PaymentAuditQuery,
    scope: PaymentAuditScope,
    aggregates: &[Aggregate],
    lookup_keys: Option<&[String]>,
) -> SqlFragment {
    let mut source = BoundQueryBuilder::new(PAYMENT_AUDIT_SUMMARY_BUCKET_TABLE);
    source.add_select("lookup_key");
    source.extend_selects(aggregates.iter().map(|aggregate| aggregate.state_column()));
    source.extend_filters(payment_audit_summary_bucket_filters(query, scope));
    source.extend_filters(lookup_keys.and_then(|keys| FilterClause::in_list("lookup_key", keys)));

    let mut builder = subquery(source.into_fragment());
    builder.add_select("lookup_key");
    builder.extend_selects(
        aggregates
            .iter()
            .map(|aggregate| aggregate.merged_sql().into_owned()),
    );
    builder.add_group_by("lookup_key");
    builder.into_fragment()
}

fn raw_summary_fragment(
    query: &PaymentAuditQuery,
    scope: PaymentAuditScope,
    aggregates: &[Aggregate],
    lookup_keys: Option<&[String]>,
) -> SqlFragment {
    let mut columns = vec!["lookup_key", "created_at_ms"];
    for column in aggregates
        .iter()
        .filter_map(|aggregate| aggregate.raw_column())
    {
        if !columns.contains(&column) {
            columns.push(column);
        }
    }

    let mut source = BoundQueryBuilder::new(DOMAIN_TABLE);
    source.extend_selects(columns);
    source.extend_filters(payment_audit_summary_scope_filters(query, scope));
    source.add_filter(FilterClause::raw("lookup_key IS NOT NULL"));
    source.add_filter(FilterClause::raw("lookup_key != ''"));
    source.extend_filters(lookup_keys.and_then(|keys| FilterClause::in_list("lookup_key", keys)));

    let mut builder = subquery(source.into_fragment());
    builder.add_select("assumeNotNull(lookup_key) AS lookup_key");
    builder.extend_selects(
        aggregates
            .iter()
            .map(|aggregate| aggregate.raw_sql().into_owned()),
    );
    builder.add_group_by("lookup_key");
    builder.into_fragment()
}

fn summary_fragment(
    query: &PaymentAuditQuery,
    scope: PaymentAuditScope,
    aggregates: &[Aggregate],
    lookup_keys: Option<&[String]>,
) -> SqlFragment {
    if payment_audit_needs_raw_rows(query) {
        return raw_summary_fragment(query, scope, aggregates, lookup_keys);
    }

    merged_summary_fragment(query, scope, aggregates, lookup_keys)
}

fn exact_lookup_filter(lookup_key: &str) -> FilterClause {
    FilterClause::new(
        "(lookup_key = ? OR payment_id = ? OR request_id = ? OR global_request_id = ? OR event_id = ?)",
        std::iter::repeat_n(BindArg::from(lookup_key), 5).collect(),
    )
}

fn outer_summary_filters(query: &PaymentAuditQuery) -> Vec<FilterClause> {
    let (start_ms, end_ms) = effective_payment_audit_window_bounds(query);
    let mut filters = vec![
        FilterClause::lte("first_seen_ms", end_ms),
        FilterClause::gte("last_seen_ms", start_ms),
    ];

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
    if let Some(kind) = query.routing_kind {
        filters.extend(payment_audit_routing_kind_filters(kind));
    }
    // The lookup summary table intentionally does not carry routing_approach state.
    // When routing_approach is included or excluded, callers use raw_summary_fragment instead.
    if let Some(error_code) = &query.error_code {
        filters.push(FilterClause::new(
            "has(error_codes, ?)",
            vec![error_code.clone().into()],
        ));
    }

    filters
}

fn map_rows(rows: Vec<AuditSummaryRow>) -> Vec<PaymentAuditSummary> {
    rows.into_iter()
        .filter_map(|row| {
            if row.event_count == 0 {
                return None;
            }

            Some(PaymentAuditSummary {
                lookup_key: row.lookup_key,
                payment_id: row.payment_id,
                request_id: row.request_id,
                merchant_id: row.merchant_id,
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
        })
        .collect()
}

pub async fn count(
    client: &clickhouse::Client,
    query: &PaymentAuditQuery,
    scope: PaymentAuditScope,
) -> Result<(usize, usize, usize), ApiError> {
    let aggregates = with_filter_aggregates(query, COUNT_AGGREGATES);
    let mut builder = subquery(summary_fragment(query, scope, &aggregates, None));
    builder.add_select("count() AS total_results");
    builder.add_select(
        "countIf(upper(latest_status) IN ('SUCCESS', 'CHARGED', 'AUTHORIZED')) AS total_success",
    );
    builder.add_select(
        "countIf(upper(latest_status) = 'FAILURE' OR upper(latest_status) LIKE '%FAILED%' OR upper(latest_status) LIKE '%DECLINED%') AS total_failure",
    );
    builder.extend_filters(outer_summary_filters(query));
    let row = fetch_one::<CountRow>(builder.build(client)).await?;
    Ok((
        row.total_results as usize,
        row.total_success as usize,
        row.total_failure as usize,
    ))
}

/// The lookup keys on the requested page, in page order. Carries only the sort keys and the
/// filtered aggregates, so the grouping holds a few numbers per payment.
async fn load_page_keys(
    client: &clickhouse::Client,
    query: &PaymentAuditQuery,
    scope: PaymentAuditScope,
) -> Result<Vec<String>, ApiError> {
    let aggregates = with_filter_aggregates(query, PAGE_KEY_AGGREGATES);
    let mut builder = subquery(summary_fragment(query, scope, &aggregates, None));
    builder.add_select("lookup_key");
    builder.extend_filters(outer_summary_filters(query));
    builder.add_order_by(OrderClause::desc("last_seen_ms"));
    builder.add_order_by(OrderClause::desc("event_count"));
    builder.add_order_by(OrderClause::asc("lookup_key"));
    builder.set_limit(Some(query.page_size as u64));
    builder.set_offset(Some(((query.page - 1) * query.page_size) as u64));
    let rows = fetch_all::<PageKeyRow>(builder.build(client)).await?;
    Ok(rows.into_iter().map(|row| row.lookup_key).collect())
}

/// One page of summaries: the page's keys first, then the full row aggregates for those keys
/// alone.
pub async fn load_page(
    client: &clickhouse::Client,
    query: &PaymentAuditQuery,
    scope: PaymentAuditScope,
) -> Result<Vec<PaymentAuditSummary>, ApiError> {
    let keys = load_page_keys(client, query, scope).await?;
    if keys.is_empty() {
        return Ok(Vec::new());
    }

    let mut builder = subquery(summary_fragment(query, scope, ROW_AGGREGATES, Some(&keys)));
    builder.extend_selects([
        "lookup_key",
        "payment_id",
        "request_id",
        "merchant_id",
        "first_seen_ms",
        "last_seen_ms",
        "event_count",
        "latest_status",
        "latest_gateway",
        "latest_stage",
        "gateways",
        "routes",
    ]);
    let mut rows = fetch_all::<AuditSummaryRow>(builder.build(client)).await?;

    let position: HashMap<&str, usize> = keys
        .iter()
        .enumerate()
        .map(|(index, key)| (key.as_str(), index))
        .collect();
    rows.sort_by_key(|row| {
        position
            .get(row.lookup_key.as_str())
            .copied()
            .unwrap_or(usize::MAX)
    });
    Ok(map_rows(rows))
}

pub async fn load_exact(
    client: &clickhouse::Client,
    query: &PaymentAuditQuery,
    scope: PaymentAuditScope,
    lookup_key: &str,
) -> Result<Vec<PaymentAuditSummary>, ApiError> {
    let mut exact_query = query.clone();
    exact_query.payment_id = None;
    exact_query.request_id = None;

    let mut source = BoundQueryBuilder::new(DOMAIN_TABLE);
    source.extend_selects([
        "lookup_key".to_string(),
        "payment_id".to_string(),
        "request_id".to_string(),
        "merchant_id".to_string(),
        "created_at_ms".to_string(),
        "status".to_string(),
        "gateway".to_string(),
        "event_stage".to_string(),
        "route".to_string(),
    ]);
    // A specific transaction's summary should report its full curated trace (like the timeline),
    // not just events matching the list's dimension filters.
    source.extend_filters(payment_audit_timeline_filters(&exact_query, scope));
    source.add_filter(exact_lookup_filter(lookup_key));

    let source = source.into_fragment();
    let mut builder = BoundQueryBuilder::from_fragment(SqlFragment::with_binds(
        format!("({})", source.sql()),
        source.binds().to_vec(),
    ));
    builder.extend_selects([
        "assumeNotNull(any(lookup_key)) AS lookup_key".to_string(),
        "argMax(payment_id, created_at_ms) AS payment_id".to_string(),
        "argMax(request_id, created_at_ms) AS request_id".to_string(),
        "any(merchant_id) AS merchant_id".to_string(),
        "min(created_at_ms) AS first_seen_ms".to_string(),
        "max(created_at_ms) AS last_seen_ms".to_string(),
        "count() AS event_count".to_string(),
        "argMax(status, created_at_ms) AS latest_status".to_string(),
        "argMax(gateway, created_at_ms) AS latest_gateway".to_string(),
        "argMax(event_stage, created_at_ms) AS latest_stage".to_string(),
        "arrayFilter(value -> value != '', groupUniqArray(ifNull(gateway, ''))) AS gateways"
            .to_string(),
        "arrayFilter(value -> value != '', groupUniqArray(ifNull(route, ''))) AS routes"
            .to_string(),
    ]);
    let rows = fetch_all::<AuditSummaryRow>(builder.build(client)).await?;
    Ok(map_rows(rows))
}

#[cfg(test)]
mod tests {
    use crate::analytics::clickhouse::common::{DOMAIN_TABLE, PAYMENT_AUDIT_SUMMARY_BUCKET_TABLE};
    use crate::analytics::models::{
        AnalyticsRange, PaymentAuditQuery, PaymentAuditRoutingKind, PaymentAuditScope,
    };

    use super::{
        exact_lookup_filter, merged_summary_fragment, outer_summary_filters, raw_summary_fragment,
        summary_fragment, with_filter_aggregates, Aggregate, COUNT_AGGREGATES, ROW_AGGREGATES,
    };

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
            routing_approach: None,
            exclude_routing_approach: None,
            error_code: None,
            scope: PaymentAuditScope::All,
            routing_kind: None,
        }
    }

    #[test]
    fn merged_summary_fragment_merges_every_summary_kind_per_payment() {
        let fragment = merged_summary_fragment(
            &payment_audit_query(),
            PaymentAuditScope::All,
            ROW_AGGREGATES,
            None,
        );
        let sql = fragment.sql();
        assert!(sql.contains(PAYMENT_AUDIT_SUMMARY_BUCKET_TABLE));
        assert!(!sql.contains("FINAL"));
        assert!(sql.contains("sumMerge(event_count_state) AS event_count"));
        assert!(sql.contains("groupUniqArrayMerge(gateways_state)) AS gateways"));
        assert!(sql.contains("GROUP BY lookup_key"));
        assert!(!sql.contains("summary_kind"));
        assert_eq!(fragment.binds().len(), 3, "merchant + bucket window bounds");
    }

    #[test]
    fn merged_summary_fragment_reads_only_buckets_inside_the_window() {
        let sql = merged_summary_fragment(
            &payment_audit_query(),
            PaymentAuditScope::All,
            COUNT_AGGREGATES,
            None,
        )
        .sql()
        .to_string();
        assert!(sql.contains("bucket_start >= fromUnixTimestamp64Milli(toInt64(?))"));
        assert!(sql.contains("bucket_start <= fromUnixTimestamp64Milli(toInt64(?))"));
    }

    #[test]
    fn summary_queries_select_only_the_aggregates_they_use() {
        let query = payment_audit_query();
        let count = merged_summary_fragment(&query, PaymentAuditScope::All, COUNT_AGGREGATES, None);
        assert!(count.sql().contains("latest_status_state"));
        assert!(!count.sql().contains("gateways_state"));
        assert!(!count.sql().contains("payment_id_state"));

        let raw = raw_summary_fragment(&query, PaymentAuditScope::All, COUNT_AGGREGATES, None);
        assert!(raw
            .sql()
            .contains("SELECT lookup_key, created_at_ms, status FROM"));
        assert!(!raw.sql().contains("groupUniqArray"));
    }

    #[test]
    fn filtered_aggregates_join_the_base_set_once() {
        let mut query = payment_audit_query();
        assert_eq!(
            with_filter_aggregates(&query, COUNT_AGGREGATES),
            COUNT_AGGREGATES
        );

        query.gateway = Some("adyen".to_string());
        query.status = Some("FAILURE".to_string());
        query.routing_kind = Some(PaymentAuditRoutingKind::RuleBased);
        let aggregates = with_filter_aggregates(&query, ROW_AGGREGATES);
        assert_eq!(
            aggregates
                .iter()
                .filter(|aggregate| **aggregate == Aggregate::Gateways)
                .count(),
            1
        );
        assert!(aggregates.contains(&Aggregate::Statuses));
        assert!(aggregates.contains(&Aggregate::RoutingFamilies));
        assert!(!aggregates.contains(&Aggregate::FlowTypes));

        // Debit routing filters on raw routing_approach, not on the aggregated families.
        query.routing_kind = Some(PaymentAuditRoutingKind::DebitRouting);
        assert!(
            !with_filter_aggregates(&query, COUNT_AGGREGATES).contains(&Aggregate::RoutingFamilies)
        );
    }

    #[test]
    fn page_rows_are_read_for_the_page_keys_only() {
        let keys = vec!["pay_1".to_string(), "pay_2".to_string()];
        let merged = merged_summary_fragment(
            &payment_audit_query(),
            PaymentAuditScope::All,
            ROW_AGGREGATES,
            Some(&keys),
        );
        assert!(merged.sql().contains("lookup_key IN (?, ?)"));
        assert_eq!(merged.binds().len(), 5);

        let raw = raw_summary_fragment(
            &payment_audit_query(),
            PaymentAuditScope::All,
            ROW_AGGREGATES,
            Some(&keys),
        );
        assert!(raw.sql().contains("lookup_key IN (?, ?)"));
    }

    #[test]
    fn narrow_scopes_pin_their_summary_kinds() {
        // dynamic = live decision path: SR rows and hybrid rows, matching its timeline list
        let fragment = merged_summary_fragment(
            &payment_audit_query(),
            PaymentAuditScope::Dynamic,
            COUNT_AGGREGATES,
            None,
        );
        assert!(fragment.sql().contains("summary_kind IN (?, ?)"));
        assert_eq!(fragment.binds().len(), 5, "merchant + window + two kinds");
        let preview = merged_summary_fragment(
            &payment_audit_query(),
            PaymentAuditScope::Preview,
            COUNT_AGGREGATES,
            None,
        );
        assert!(preview.sql().contains("summary_kind IN (?)"));
        assert_eq!(preview.binds().len(), 4);
    }

    #[test]
    fn summary_fragment_reads_raw_rows_only_for_routing_approach_kinds() {
        let mut query = payment_audit_query();
        let sql = |query: &PaymentAuditQuery| {
            summary_fragment(query, PaymentAuditScope::All, COUNT_AGGREGATES, None)
                .sql()
                .to_string()
        };
        assert!(sql(&query).contains(PAYMENT_AUDIT_SUMMARY_BUCKET_TABLE));

        query.routing_kind = Some(PaymentAuditRoutingKind::Hybrid);
        assert!(sql(&query).contains(PAYMENT_AUDIT_SUMMARY_BUCKET_TABLE));

        query.routing_kind = Some(PaymentAuditRoutingKind::DebitRouting);
        let debit = sql(&query);
        assert!(debit.contains(DOMAIN_TABLE));
        assert!(!debit.contains(PAYMENT_AUDIT_SUMMARY_BUCKET_TABLE));

        query.routing_kind = Some(PaymentAuditRoutingKind::MultiObjective);
        assert!(sql(&query).contains(DOMAIN_TABLE));
    }

    #[test]
    fn outer_filters_read_routing_families_for_family_backed_kinds() {
        let mut query = payment_audit_query();
        query.routing_kind = Some(PaymentAuditRoutingKind::RuleBased);
        let predicates = outer_summary_filters(&query)
            .into_iter()
            .map(|filter| filter.predicate().to_string())
            .collect::<Vec<_>>();
        assert!(predicates
            .iter()
            .any(|predicate| predicate == "bitAnd(routing_families, 1) != 0"));

        query.routing_kind = Some(PaymentAuditRoutingKind::DebitRouting);
        let predicates = outer_summary_filters(&query)
            .into_iter()
            .map(|filter| filter.predicate().to_string())
            .collect::<Vec<_>>();
        assert!(!predicates
            .iter()
            .any(|predicate| predicate.contains("routing_families")));
    }

    #[test]
    fn raw_summary_fragment_exposes_outer_summary_column_names() {
        let fragment = raw_summary_fragment(
            &payment_audit_query(),
            PaymentAuditScope::All,
            ROW_AGGREGATES,
            None,
        );
        assert!(fragment.sql().contains("AS lookup_key"));
        assert!(fragment.sql().contains("AS merchant_id"));
        assert!(!fragment.sql().contains("resolved_lookup_key"));
        assert!(!fragment.sql().contains("resolved_merchant_id"));
    }

    #[test]
    fn exact_lookup_filter_matches_every_visible_identifier() {
        let filter = exact_lookup_filter("id_123");
        assert_eq!(
            filter.predicate(),
            "(lookup_key = ? OR payment_id = ? OR request_id = ? OR global_request_id = ? OR event_id = ?)"
        );
        assert_eq!(filter.binds().len(), 5);
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
        assert!(predicates
            .iter()
            .any(|predicate| predicate == "first_seen_ms <= ?"));
        assert!(predicates
            .iter()
            .any(|predicate| predicate == "last_seen_ms >= ?"));
    }
}
