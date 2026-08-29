use clickhouse::Row;
use serde::Deserialize;

use crate::analytics::models::{PaymentAuditQuery, PaymentAuditSummary};
use crate::error::ApiError;

use super::super::common::{
    fetch_all, fetch_one, payment_audit_route_label, payment_audit_stage_label,
    payment_audit_summary_kind, DOMAIN_TABLE, PAYMENT_AUDIT_LOOKUP_SUMMARY_TABLE,
};
use super::super::filters::{payment_audit_summary_scope_filters, payment_audit_timeline_filters};
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
    call_count: u64,
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

/// Whether the summary table carries `call_count_state` yet.
///
/// The column arrives with `039_audit_call_counts.sh`, and ClickHouse scripts only run
/// automatically on a fresh volume — so an existing deployment can be running this code
/// against a table that predates it. Selecting a missing column would fail the whole
/// Decision Audit query, so the presence is probed and the fragment adapts: before the
/// migration the list reports event counts, after it, call counts. No deploy ordering to
/// get right, and nothing to re-run to keep the page working.
///
/// Only the *positive* answer is latched. A column is never dropped, so "present" is
/// permanent and worth caching forever; "absent" is temporary by nature — the normal
/// deploy order is app first, migration second — and a probe that failed on a ClickHouse
/// blip must not pin the feature off for the process lifetime. A negative is therefore
/// re-probed after `CALL_COUNT_PROBE_RETRY`, so the page starts reporting call counts on
/// its own once 039 lands, with no pod restart.
static CALL_COUNT_STATE_PRESENT: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
/// Unix-millis deadline before which a negative probe is not repeated. 0 = never probed.
static CALL_COUNT_PROBE_NOT_BEFORE: std::sync::atomic::AtomicI64 =
    std::sync::atomic::AtomicI64::new(0);
const CALL_COUNT_PROBE_RETRY: std::time::Duration = std::time::Duration::from_secs(300);

#[derive(Debug, Clone, Deserialize, Row)]
struct ColumnPresenceRow {
    present: u8,
}

async fn call_count_state_available(client: &clickhouse::Client) -> bool {
    use std::sync::atomic::Ordering;

    if CALL_COUNT_STATE_PRESENT.load(Ordering::Relaxed) {
        return true;
    }
    let now_ms = crate::analytics::now_ms();
    if now_ms < CALL_COUNT_PROBE_NOT_BEFORE.load(Ordering::Relaxed) {
        return false;
    }
    // Set the next window before probing, so concurrent requests during a slow or failing
    // probe do not each fire their own.
    CALL_COUNT_PROBE_NOT_BEFORE.store(
        now_ms.saturating_add(CALL_COUNT_PROBE_RETRY.as_millis() as i64),
        Ordering::Relaxed,
    );

    let query = client
        .query(
            "SELECT count() > 0 AS present FROM system.columns \
             WHERE database = currentDatabase() AND table = ? AND name = 'call_count_state'",
        )
        .bind(PAYMENT_AUDIT_LOOKUP_SUMMARY_TABLE);
    match query.fetch_one::<ColumnPresenceRow>().await {
        Ok(row) if row.present == 1 => {
            CALL_COUNT_STATE_PRESENT.store(true, Ordering::Relaxed);
            true
        }
        Ok(_) => false,
        Err(error) => {
            // Treat an unreadable system.columns as "absent": the fallback query is always
            // valid, where the richer one might not be.
            crate::logger::warn!(
                ?error,
                "could not probe call_count_state; audit list will report event counts until the next probe"
            );
            false
        }
    }
}

fn finalized_summary_fragment(
    query: &PaymentAuditQuery,
    preview_only: bool,
    has_call_count_state: bool,
) -> SqlFragment {
    let mut builder = BoundQueryBuilder::new(format!("{PAYMENT_AUDIT_LOOKUP_SUMMARY_TABLE} FINAL"));
    builder.extend_selects([
        "lookup_key".to_string(),
        "finalizeAggregation(payment_id_state) AS payment_id".to_string(),
        "finalizeAggregation(request_id_state) AS request_id".to_string(),
        "finalizeAggregation(merchant_id_state) AS merchant_id".to_string(),
        "finalizeAggregation(first_seen_ms_state) AS first_seen_ms".to_string(),
        "finalizeAggregation(last_seen_ms_state) AS last_seen_ms".to_string(),
        "finalizeAggregation(event_count_state) AS event_count".to_string(),
        // Zero makes the caller fall back to the event count, which is what a
        // pre-migration table can honestly report.
        if has_call_count_state {
            "finalizeAggregation(call_count_state) AS call_count".to_string()
        } else {
            "toUInt64(0) AS call_count".to_string()
        },
        "finalizeAggregation(latest_status_state) AS latest_status".to_string(),
        "finalizeAggregation(latest_gateway_state) AS latest_gateway".to_string(),
        "finalizeAggregation(latest_stage_state) AS latest_stage".to_string(),
        "arrayFilter(value -> value != '', finalizeAggregation(gateways_state)) AS gateways"
            .to_string(),
        "arrayFilter(value -> value != '', finalizeAggregation(routes_state)) AS routes"
            .to_string(),
        "arrayFilter(value -> value != '', finalizeAggregation(statuses_state)) AS statuses"
            .to_string(),
        "arrayFilter(value -> value != '', finalizeAggregation(flow_types_state)) AS flow_types"
            .to_string(),
        "arrayFilter(value -> value != '', finalizeAggregation(error_codes_state)) AS error_codes"
            .to_string(),
    ]);
    builder.extend_filters([
        FilterClause::eq("merchant_id", query.merchant_id.clone()),
        FilterClause::eq("summary_kind", payment_audit_summary_kind(preview_only)),
    ]);
    builder.into_fragment()
}

fn raw_summary_fragment(query: &PaymentAuditQuery, preview_only: bool) -> SqlFragment {
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
        "flow_type".to_string(),
        "error_code".to_string(),
    ]);
    source.extend_filters(payment_audit_summary_scope_filters(query, preview_only));
    source.add_filter(FilterClause::raw("lookup_key IS NOT NULL"));
    source.add_filter(FilterClause::raw("lookup_key != ''"));

    let source = source.into_fragment();
    let mut builder = BoundQueryBuilder::from_fragment(SqlFragment::with_binds(
        format!("({})", source.sql()),
        source.binds().to_vec(),
    ));
    builder.extend_selects([
        "assumeNotNull(lookup_key) AS lookup_key".to_string(),
        "argMax(payment_id, created_at_ms) AS payment_id".to_string(),
        "argMax(request_id, created_at_ms) AS request_id".to_string(),
        "any(merchant_id) AS merchant_id".to_string(),
        "min(created_at_ms) AS first_seen_ms".to_string(),
        "max(created_at_ms) AS last_seen_ms".to_string(),
        "count() AS event_count".to_string(),
        "uniqExact(request_id) AS call_count".to_string(),
        "argMax(status, created_at_ms) AS latest_status".to_string(),
        "argMax(gateway, created_at_ms) AS latest_gateway".to_string(),
        "argMax(event_stage, created_at_ms) AS latest_stage".to_string(),
        "arrayFilter(value -> value != '', groupUniqArray(ifNull(gateway, ''))) AS gateways"
            .to_string(),
        "arrayFilter(value -> value != '', groupUniqArray(ifNull(route, ''))) AS routes"
            .to_string(),
        "arrayFilter(value -> value != '', groupUniqArray(ifNull(status, ''))) AS statuses"
            .to_string(),
        "arrayFilter(value -> value != '', groupUniqArray(flow_type)) AS flow_types".to_string(),
        "arrayFilter(value -> value != '', groupUniqArray(ifNull(error_code, ''))) AS error_codes"
            .to_string(),
    ]);
    builder.add_group_by("lookup_key");
    builder.into_fragment()
}

async fn summary_fragment(
    client: &clickhouse::Client,
    query: &PaymentAuditQuery,
    preview_only: bool,
) -> SqlFragment {
    if query.routing_approach.is_some() || query.exclude_routing_approach.is_some() {
        // The raw fragment reads request_id straight off the events table, so it needs
        // no migration to report call counts.
        return raw_summary_fragment(query, preview_only);
    }

    finalized_summary_fragment(query, preview_only, call_count_state_available(client).await)
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
        "call_count".to_string(),
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
                // Passed through as-is: 0 means "not known here" — the summary row predates
                // the call_count column, or every request_id was NULL. Substituting the event
                // count would report those events as if each were its own call, overstating
                // the calls made, so the distinction is left for the caller to render.
                call_count: row.call_count as usize,
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
    preview_only: bool,
) -> Result<(usize, usize, usize), ApiError> {
    let finalized = summary_fragment(client, query, preview_only).await;
    let mut builder = BoundQueryBuilder::from_fragment(SqlFragment::with_binds(
        format!("({})", finalized.sql()),
        finalized.binds().to_vec(),
    ));
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

pub async fn load_page(
    client: &clickhouse::Client,
    query: &PaymentAuditQuery,
    preview_only: bool,
) -> Result<Vec<PaymentAuditSummary>, ApiError> {
    let finalized = summary_fragment(client, query, preview_only).await;
    let mut builder = results_builder(finalized, query);
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
    source.extend_filters(payment_audit_timeline_filters(&exact_query, preview_only));
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
        "uniqExact(request_id) AS call_count".to_string(),
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
    use crate::analytics::clickhouse::common::PAYMENT_AUDIT_LOOKUP_SUMMARY_TABLE;
    use crate::analytics::models::{AnalyticsRange, PaymentAuditQuery};

    use super::{
        exact_lookup_filter, finalized_summary_fragment, outer_summary_filters,
        raw_summary_fragment,
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
        }
    }

    #[test]
    fn finalized_summary_fragment_uses_lookup_summary_table() {
        let fragment = finalized_summary_fragment(&payment_audit_query(), false, true);
        assert!(fragment.sql().contains(PAYMENT_AUDIT_LOOKUP_SUMMARY_TABLE));
        assert!(fragment.sql().contains("FINAL"));
        assert!(!fragment.sql().contains("GROUP BY lookup_key"));
    }

    #[test]
    fn raw_summary_fragment_exposes_outer_summary_column_names() {
        let fragment = raw_summary_fragment(&payment_audit_query(), false);
        assert!(fragment.sql().contains("AS lookup_key"));
        assert!(fragment.sql().contains("AS merchant_id"));
        assert!(!fragment.sql().contains("resolved_lookup_key"));
        assert!(!fragment.sql().contains("resolved_merchant_id"));
        assert!(fragment.sql().contains("uniqExact(request_id) AS call_count"));
    }

    #[test]
    fn both_summary_fragments_expose_call_count() {
        // load_page selects `call_count` from whichever fragment answers, so each must
        // produce the column.
        let finalized = finalized_summary_fragment(&payment_audit_query(), false, true);
        assert!(finalized
            .sql()
            .contains("finalizeAggregation(call_count_state) AS call_count"));
        let raw = raw_summary_fragment(&payment_audit_query(), false);
        assert!(raw.sql().contains("AS call_count"));
    }

    #[test]
    fn finalized_summary_fragment_omits_call_count_state_before_the_migration() {
        // A database that has not run 039 yet has no call_count_state column; selecting it
        // would fail the whole audit query, so the fragment must substitute a literal the
        // reader interprets as "unknown" (and falls back to the event count).
        let fragment = finalized_summary_fragment(&payment_audit_query(), false, false);
        assert!(!fragment.sql().contains("call_count_state"));
        assert!(fragment.sql().contains("toUInt64(0) AS call_count"));
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
