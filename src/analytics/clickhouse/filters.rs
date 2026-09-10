use crate::analytics::flow::{AnalyticsRoute, FlowType};
use crate::analytics::models::{
    AnalyticsQuery, PaymentAuditQuery, PaymentAuditRoutingKind, PaymentAuditScope,
};

use super::common::{
    payment_audit_flow_types, static_flow_type_array_sql, static_flow_type_in_sql,
    PAYMENT_AUDIT_HYBRID_FLOW_TYPES, PAYMENT_AUDIT_MULTI_OBJECTIVE_FLOW_TYPES,
    PAYMENT_AUDIT_PREVIEW_FLOW_TYPES,
};
use super::query::FilterClause;
use super::time::{effective_payment_audit_window_bounds, payment_audit_summary_bucket_bounds};

const DEBIT_ROUTING_APPROACH: &str = "NTW_BASED_ROUTING";
const DEBIT_ROUTING_DETAILS_MATCH: &str = r#"(positionCaseInsensitive(ifNull(details, ''), '"rankingAlgorithm":"NTW_BASED_ROUTING"') > 0 OR positionCaseInsensitive(ifNull(details, ''), '"routing_approach":"NTW_BASED_ROUTING"') > 0)"#;

pub fn base_window_filters(start_ms: i64, end_ms: i64) -> Vec<FilterClause> {
    vec![
        FilterClause::gte("created_at_ms", start_ms),
        FilterClause::lte("created_at_ms", end_ms),
    ]
}

pub fn merchant_filter(merchant_id: &str) -> Vec<FilterClause> {
    vec![FilterClause::eq("merchant_id", merchant_id)]
}

pub fn analytics_dimension_filters(query: &AnalyticsQuery) -> Vec<FilterClause> {
    let mut filters = Vec::new();

    if let Some(value) = &query.payment_method_type {
        filters.push(FilterClause::eq("payment_method_type", value.clone()));
    }
    if let Some(value) = &query.payment_method {
        filters.push(FilterClause::eq("payment_method", value.clone()));
    }
    if let Some(value) = &query.card_network {
        filters.push(FilterClause::eq("card_network", value.clone()));
    }
    if let Some(value) = &query.card_is_in {
        filters.push(FilterClause::eq("card_is_in", value.clone()));
    }
    if let Some(value) = &query.currency {
        filters.push(FilterClause::eq("currency", value.clone()));
    }
    if let Some(value) = &query.country {
        filters.push(FilterClause::eq("country", value.clone()));
    }
    if let Some(value) = &query.auth_type {
        filters.push(FilterClause::eq("auth_type", value.clone()));
    }
    if let Some(clause) = FilterClause::in_list("gateway", &query.gateways) {
        filters.push(clause);
    }

    filters
}

pub fn score_filters(query: &AnalyticsQuery, start_ms: i64, end_ms: i64) -> Vec<FilterClause> {
    let mut filters = base_window_filters(start_ms, end_ms);
    filters.extend(merchant_filter(&query.merchant_id));
    filters.extend(analytics_dimension_filters(query));
    filters
}

/// The flow-type category of a scope. The preview trail is pinned to the `routing_evaluate`
/// route, the dynamic trail is its flow-type list, and `All` is the union of both with no pin.
fn scope_flow_type_filters(scope: PaymentAuditScope) -> Vec<FilterClause> {
    let mut filters = Vec::new();
    if scope == PaymentAuditScope::Preview {
        filters.push(FilterClause::raw(format!(
            "route = '{}'",
            AnalyticsRoute::RoutingEvaluate.as_str()
        )));
    }
    filters.push(FilterClause::raw(format!(
        "flow_type IN {}",
        static_flow_type_in_sql(payment_audit_flow_types(scope))
    )));
    filters
}

pub fn payment_audit_raw_filters(
    query: &PaymentAuditQuery,
    scope: PaymentAuditScope,
) -> Vec<FilterClause> {
    let (start_ms, end_ms) = effective_payment_audit_window_bounds(query);
    let mut filters = base_window_filters(start_ms, end_ms);

    filters.extend(merchant_filter(&query.merchant_id));
    filters.extend(scope_flow_type_filters(scope));
    if scope != PaymentAuditScope::Preview {
        if let Some(route) = &query.route {
            filters.push(FilterClause::eq("route", route.clone()));
        }
    }

    if let Some(gateway) = &query.gateway {
        filters.push(FilterClause::eq("gateway", gateway.clone()));
    }
    if let Some(status) = &query.status {
        filters.push(FilterClause::eq("status", status.clone()));
    }
    if let Some(flow_type) = &query.flow_type {
        filters.push(FilterClause::eq("flow_type", flow_type.clone()));
    }
    if let Some(routing_approach) = &query.routing_approach {
        filters.push(routing_approach_match_filter(routing_approach));
    }
    if let Some(routing_approach) = &query.exclude_routing_approach {
        filters.push(routing_approach_exclusion_filter(routing_approach));
    }
    if let Some(error_code) = &query.error_code {
        filters.push(FilterClause::eq("error_code", error_code.clone()));
    }

    filters
}

/// Filters for a single selected transaction's event timeline.
///
/// Unlike [`payment_audit_raw_filters`], this deliberately omits the per-event dimension filters
/// (status, gateway, route, routing_approach/exclusion, error_code, specific flow_type). Those exist
/// to *find* matching transactions in the summary list; applied to a single transaction's trace they
/// fragment it — e.g. a `status=failure` filter drops the `decide_gateway` events (whose status is
/// `received`/`success`) and leaves only the `update_gateway_score` failure event. Once a transaction
/// is selected (via `lookup_key`, added by the caller), its full trace should be returned. Only the
/// scope needed to identify that trace is kept: time window, merchant, and the preview/live flow-type
/// category (plus the preview route for preview traces).
pub fn payment_audit_timeline_filters(
    query: &PaymentAuditQuery,
    scope: PaymentAuditScope,
) -> Vec<FilterClause> {
    let (start_ms, end_ms) = effective_payment_audit_window_bounds(query);
    let mut filters = base_window_filters(start_ms, end_ms);

    filters.extend(merchant_filter(&query.merchant_id));
    filters.extend(scope_flow_type_filters(scope));

    filters
}

/// Whether the summary must be aggregated from raw `analytics_domain_events` rows instead of
/// the pre-aggregated lookup summaries: any filter on `routing_approach`, which the summary
/// table does not carry. The Multi-objective and Debit routing kinds are such filters (they
/// exclude / require `NTW_BASED_ROUTING`, exactly what the old tabs sent).
pub fn payment_audit_needs_raw_rows(query: &PaymentAuditQuery) -> bool {
    query.routing_approach.is_some()
        || query.exclude_routing_approach.is_some()
        || matches!(
            query.routing_kind,
            Some(PaymentAuditRoutingKind::MultiObjective | PaymentAuditRoutingKind::DebitRouting)
        )
}

fn has_any_flow_type(flow_types: &[FlowType]) -> FilterClause {
    FilterClause::raw(format!(
        "hasAny(flow_types, {})",
        static_flow_type_array_sql(flow_types)
    ))
}

fn has_no_flow_type(flow_types: &[FlowType]) -> FilterClause {
    FilterClause::raw(format!(
        "NOT hasAny(flow_types, {})",
        static_flow_type_array_sql(flow_types)
    ))
}

/// The routing-kind filter over a payment's aggregated `flow_types`. A payment is listed under
/// exactly one kind, by precedence: `hybrid` when it went through `/routing/hybrid`; otherwise
/// `multi_objective` when an SR decision (or its score feedback) exists; otherwise `rule_based`
/// when only rule evaluations exist. So a payment id that carries both a rule evaluation and an
/// SR decision shows up under Multi-objective only, with its whole trail. Debit routing is
/// decided by `routing_approach` on the raw rows instead (see
/// `payment_audit_summary_scope_filters`).
pub fn payment_audit_routing_kind_filters(kind: PaymentAuditRoutingKind) -> Vec<FilterClause> {
    match kind {
        PaymentAuditRoutingKind::MultiObjective => vec![
            has_any_flow_type(PAYMENT_AUDIT_MULTI_OBJECTIVE_FLOW_TYPES),
            has_no_flow_type(PAYMENT_AUDIT_HYBRID_FLOW_TYPES),
        ],
        // Rule based = the rule evaluations recorded by direct `/routing/evaluate` calls, i.e.
        // the same flow types the preview scope reads.
        PaymentAuditRoutingKind::RuleBased => vec![
            has_any_flow_type(PAYMENT_AUDIT_PREVIEW_FLOW_TYPES),
            has_no_flow_type(PAYMENT_AUDIT_HYBRID_FLOW_TYPES),
            has_no_flow_type(PAYMENT_AUDIT_MULTI_OBJECTIVE_FLOW_TYPES),
        ],
        PaymentAuditRoutingKind::Hybrid => vec![has_any_flow_type(PAYMENT_AUDIT_HYBRID_FLOW_TYPES)],
        PaymentAuditRoutingKind::DebitRouting => Vec::new(),
    }
}

/// Filters for the raw (non-materialized) summary aggregation.
///
/// Mirrors the pre-aggregated `finalized_summary_fragment` path: the per-transaction aggregates
/// (`event_count`, `gateways`, `latest_status`, …) must reflect the transaction's full curated
/// trace, not just the events matching the list's dimension filters. Applying `status`/`gateway`/etc
/// inside the aggregation shrinks the count (e.g. a two-event trace shows `1 event` under
/// `status=failure`). Which transactions appear is instead decided by the outer `has(...)` filters
/// in `outer_summary_filters`, exactly as the materialized path does. Only scope (window, merchant,
/// flow-type category) and the routing_approach include/exclude — the reason this raw path is used
/// at all, since the summary table carries no routing_approach — are applied here.
pub fn payment_audit_summary_scope_filters(
    query: &PaymentAuditQuery,
    scope: PaymentAuditScope,
) -> Vec<FilterClause> {
    let mut filters = payment_audit_timeline_filters(query, scope);

    if let Some(routing_approach) = &query.routing_approach {
        filters.push(routing_approach_match_filter(routing_approach));
    }
    if let Some(routing_approach) = &query.exclude_routing_approach {
        filters.push(routing_approach_exclusion_filter(routing_approach));
    }
    // The routing-kind filter's routing_approach half; skipped when the explicit param already
    // carries the same predicate.
    match query.routing_kind {
        Some(PaymentAuditRoutingKind::DebitRouting)
            if query.routing_approach.as_deref() != Some(DEBIT_ROUTING_APPROACH) =>
        {
            filters.push(routing_approach_match_filter(DEBIT_ROUTING_APPROACH));
        }
        Some(PaymentAuditRoutingKind::MultiObjective)
            if query.exclude_routing_approach.as_deref() != Some(DEBIT_ROUTING_APPROACH) =>
        {
            filters.push(routing_approach_exclusion_filter(DEBIT_ROUTING_APPROACH));
        }
        _ => {}
    }

    filters
}

pub fn payment_audit_summary_bucket_filters(
    query: &PaymentAuditQuery,
    scope: PaymentAuditScope,
) -> Vec<FilterClause> {
    let (start_ms, end_ms) = payment_audit_summary_bucket_bounds(query);
    let mut filters = vec![
        FilterClause::eq("merchant_id", query.merchant_id.clone()),
        FilterClause::new(
            "bucket_start >= fromUnixTimestamp64Milli(?)",
            vec![start_ms.into()],
        ),
        FilterClause::new(
            "bucket_start <= fromUnixTimestamp64Milli(?)",
            vec![end_ms.into()],
        ),
    ];
    if let Some(kinds) = scope.summary_kinds() {
        let kinds = kinds
            .iter()
            .map(|kind| kind.to_string())
            .collect::<Vec<_>>();
        if let Some(filter) = FilterClause::in_list("summary_kind", &kinds) {
            filters.push(filter);
        }
    }
    filters
}

fn routing_approach_match_filter(routing_approach: &str) -> FilterClause {
    if routing_approach == DEBIT_ROUTING_APPROACH {
        FilterClause::new(
            format!("(routing_approach = ? OR {DEBIT_ROUTING_DETAILS_MATCH})"),
            vec![routing_approach.to_string().into()],
        )
    } else {
        FilterClause::eq("routing_approach", routing_approach.to_string())
    }
}

fn routing_approach_exclusion_filter(routing_approach: &str) -> FilterClause {
    if routing_approach == DEBIT_ROUTING_APPROACH {
        FilterClause::new(
            format!(
                "((routing_approach IS NULL OR routing_approach != ?) AND NOT {DEBIT_ROUTING_DETAILS_MATCH})"
            ),
            vec![routing_approach.to_string().into()],
        )
    } else {
        FilterClause::new(
            "(routing_approach IS NULL OR routing_approach != ?)",
            vec![routing_approach.to_string().into()],
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::analytics::models::{
        AnalyticsQuery, AnalyticsRange, PaymentAuditQuery, PaymentAuditRoutingKind,
        PaymentAuditScope,
    };

    use super::{
        analytics_dimension_filters, merchant_filter, payment_audit_needs_raw_rows,
        payment_audit_raw_filters, payment_audit_routing_kind_filters,
        payment_audit_summary_bucket_filters, payment_audit_summary_scope_filters,
        payment_audit_timeline_filters,
    };

    fn analytics_query() -> AnalyticsQuery {
        AnalyticsQuery {
            merchant_id: "m_123".to_string(),
            range: AnalyticsRange::H1,
            start_ms: Some(100),
            end_ms: Some(200),
            page: 1,
            page_size: 20,
            payment_method_type: Some("card".to_string()),
            payment_method: Some("credit".to_string()),
            card_network: None,
            card_is_in: None,
            currency: Some("USD".to_string()),
            country: None,
            auth_type: None,
            gateways: vec!["adyen".to_string()],
        }
    }

    fn payment_audit_query() -> PaymentAuditQuery {
        PaymentAuditQuery {
            merchant_id: "m_123".to_string(),
            range: AnalyticsRange::H1,
            start_ms: Some(100),
            end_ms: Some(200),
            page: 1,
            page_size: 20,
            payment_id: None,
            request_id: Some("req_1".to_string()),
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

    fn predicates(filters: &[super::FilterClause]) -> Vec<String> {
        filters
            .iter()
            .map(|filter| filter.predicate().to_string())
            .collect()
    }

    #[test]
    fn merchant_filter_always_applies_merchant_scope() {
        let filters = merchant_filter("m_123");
        let predicates = filters
            .iter()
            .map(|filter| filter.predicate().to_string())
            .collect::<Vec<_>>();
        assert_eq!(predicates, vec!["merchant_id = ?".to_string()]);
    }

    #[test]
    fn analytics_dimension_filters_include_requested_fields() {
        let filters = analytics_dimension_filters(&analytics_query());
        let predicates = filters
            .iter()
            .map(|filter| filter.predicate().to_string())
            .collect::<Vec<_>>();
        assert!(predicates
            .iter()
            .any(|predicate| predicate == "payment_method_type = ?"));
        assert!(predicates
            .iter()
            .any(|predicate| predicate == "payment_method = ?"));
        assert!(predicates
            .iter()
            .any(|predicate| predicate == "currency = ?"));
        assert!(predicates
            .iter()
            .any(|predicate| predicate == "gateway IN (?)"));
    }

    #[test]
    fn payment_audit_filters_switch_preview_flow_types() {
        let filters = payment_audit_raw_filters(&payment_audit_query(), PaymentAuditScope::Preview);
        let predicates = filters
            .iter()
            .map(|filter| filter.predicate().to_string())
            .collect::<Vec<_>>();
        assert!(predicates
            .iter()
            .any(|predicate| predicate.contains("route = 'routing_evaluate'")));
        assert!(predicates.iter().any(|predicate| {
            predicate.contains("flow_type IN")
                && predicate.contains("routing_evaluate_advanced")
                && predicate.contains("routing_evaluate_preview")
        }));
    }

    #[test]
    fn payment_audit_summary_bucket_filters_use_bucket_time_and_kind() {
        let filters = payment_audit_summary_bucket_filters(
            &payment_audit_query(),
            PaymentAuditScope::Preview,
        );
        let predicates = filters
            .iter()
            .map(|filter| filter.predicate().to_string())
            .collect::<Vec<_>>();
        assert!(predicates
            .iter()
            .any(|predicate| predicate == "merchant_id = ?"));
        assert!(predicates
            .iter()
            .any(|predicate| predicate == "bucket_start >= fromUnixTimestamp64Milli(?)"));
        assert!(predicates
            .iter()
            .any(|predicate| predicate == "bucket_start <= fromUnixTimestamp64Milli(?)"));
        assert!(predicates
            .iter()
            .any(|predicate| predicate == "summary_kind IN (?)"));
    }

    #[test]
    fn payment_audit_filters_include_debit_rows_without_explicit_routing_approach() {
        let mut query = payment_audit_query();
        query.routing_approach = Some("NTW_BASED_ROUTING".to_string());

        let predicates = payment_audit_raw_filters(&query, PaymentAuditScope::Dynamic)
            .iter()
            .map(|filter| filter.predicate().to_string())
            .collect::<Vec<_>>();

        assert!(predicates.iter().any(|predicate| {
            predicate.contains("routing_approach = ?")
                && predicate.contains("rankingAlgorithm")
                && predicate.contains("NTW_BASED_ROUTING")
        }));
    }

    #[test]
    fn payment_audit_timeline_filters_omit_per_event_dimension_filters() {
        // A selected transaction's timeline must show its full trace (decide_gateway +
        // update_gateway_score), so the list-level dimension filters must NOT be applied per event.
        let mut query = payment_audit_query();
        query.status = Some("FAILURE".to_string());
        query.exclude_routing_approach = Some("NTW_BASED_ROUTING".to_string());
        query.gateway = Some("adyen".to_string());
        query.error_code = Some("DECLINED".to_string());

        let predicates = payment_audit_timeline_filters(&query, PaymentAuditScope::Dynamic)
            .iter()
            .map(|filter| filter.predicate().to_string())
            .collect::<Vec<_>>();

        // Scope filters are kept.
        assert!(predicates.iter().any(|p| p == "merchant_id = ?"));
        assert!(predicates.iter().any(|p| p.contains("flow_type IN")));
        // Per-event dimension filters are dropped. (Check exact predicates, not substrings — the
        // flow_type IN clause contains gateway/route flow-type names like "decide_gateway_decision".)
        assert!(!predicates.iter().any(|p| p == "status = ?"));
        assert!(!predicates.iter().any(|p| p == "gateway = ?"));
        assert!(!predicates.iter().any(|p| p == "error_code = ?"));
        assert!(!predicates.iter().any(|p| p.contains("routing_approach")));
    }

    #[test]
    fn payment_audit_filters_exclude_debit_rows_without_explicit_routing_approach() {
        let mut query = payment_audit_query();
        query.exclude_routing_approach = Some("NTW_BASED_ROUTING".to_string());

        let predicates = payment_audit_raw_filters(&query, PaymentAuditScope::Dynamic)
            .iter()
            .map(|filter| filter.predicate().to_string())
            .collect::<Vec<_>>();

        assert!(predicates.iter().any(|predicate| {
            predicate.contains("routing_approach IS NULL")
                && predicate.contains("AND NOT")
                && predicate.contains("rankingAlgorithm")
                && predicate.contains("NTW_BASED_ROUTING")
        }));
    }

    #[test]
    fn timeline_filters_for_all_scope_union_both_trails_without_a_route_pin() {
        let predicates = predicates(&payment_audit_timeline_filters(
            &payment_audit_query(),
            PaymentAuditScope::All,
        ));
        let flow_clause = predicates
            .iter()
            .find(|predicate| predicate.contains("flow_type IN"))
            .expect("flow_type clause");
        assert!(flow_clause.contains("decide_gateway_decision"));
        assert!(flow_clause.contains("routing_evaluate_advanced"));
        assert!(flow_clause.contains("routing_hybrid_decision"));
        assert!(flow_clause.contains("update_gateway_score_update"));
        assert!(!predicates
            .iter()
            .any(|p| p.contains("route = 'routing_evaluate'")));
    }

    #[test]
    fn narrow_scopes_keep_their_own_trails() {
        let dynamic = predicates(&payment_audit_timeline_filters(
            &payment_audit_query(),
            PaymentAuditScope::Dynamic,
        ));
        assert!(dynamic
            .iter()
            .any(|p| p.contains("routing_hybrid_decision")));
        assert!(!dynamic
            .iter()
            .any(|p| p.contains("routing_evaluate_advanced")));

        let preview = predicates(&payment_audit_timeline_filters(
            &payment_audit_query(),
            PaymentAuditScope::Preview,
        ));
        assert!(preview.iter().any(|p| p == "route = 'routing_evaluate'"));
        assert!(preview
            .iter()
            .any(|p| p.contains("routing_evaluate_advanced")));
        assert!(!preview
            .iter()
            .any(|p| p.contains("routing_hybrid_decision")));
    }

    #[test]
    fn routing_kind_decides_which_summary_path_is_needed() {
        let mut query = payment_audit_query();
        assert!(!payment_audit_needs_raw_rows(&query));
        query.routing_kind = Some(PaymentAuditRoutingKind::RuleBased);
        assert!(!payment_audit_needs_raw_rows(&query));
        query.routing_kind = Some(PaymentAuditRoutingKind::Hybrid);
        assert!(!payment_audit_needs_raw_rows(&query));
        query.routing_kind = Some(PaymentAuditRoutingKind::MultiObjective);
        assert!(payment_audit_needs_raw_rows(&query));
        query.routing_kind = Some(PaymentAuditRoutingKind::DebitRouting);
        assert!(payment_audit_needs_raw_rows(&query));
        query.routing_kind = None;
        query.exclude_routing_approach = Some("NTW_BASED_ROUTING".to_string());
        assert!(payment_audit_needs_raw_rows(&query));
    }

    #[test]
    fn routing_kinds_are_exclusive_over_the_aggregated_flow_types() {
        let predicates = |kind| {
            payment_audit_routing_kind_filters(kind)
                .into_iter()
                .map(|filter| filter.predicate().to_string())
                .collect::<Vec<_>>()
        };

        let hybrid = predicates(PaymentAuditRoutingKind::Hybrid);
        assert_eq!(hybrid.len(), 1);
        assert!(hybrid[0].starts_with("hasAny(flow_types, ["));
        assert!(hybrid[0].contains("routing_hybrid_decision"));
        assert!(!hybrid[0].contains("update_gateway_score_update"));

        let rule_based = predicates(PaymentAuditRoutingKind::RuleBased);
        assert_eq!(rule_based.len(), 3);
        assert!(rule_based[0].contains("routing_evaluate_advanced"));
        assert!(!rule_based[0].contains("routing_hybrid_decision"));
        assert!(rule_based[1].starts_with("NOT hasAny(flow_types, ["));
        assert!(rule_based[1].contains("routing_hybrid_decision"));
        // an SR decision on the same payment id takes precedence: rule based excludes it
        assert!(rule_based[2].starts_with("NOT hasAny(flow_types, ["));
        assert!(rule_based[2].contains("decide_gateway_decision"));
        assert!(rule_based[2].contains("update_gateway_score_update"));

        let multi = predicates(PaymentAuditRoutingKind::MultiObjective);
        assert_eq!(multi.len(), 2);
        assert!(multi[0].contains("decide_gateway_decision"));
        assert!(multi[0].contains("update_gateway_score_update"));
        assert!(!multi[0].contains("routing_hybrid_decision"));
        assert!(multi[1].starts_with("NOT hasAny(flow_types, ["));

        assert!(predicates(PaymentAuditRoutingKind::DebitRouting).is_empty());
    }

    #[test]
    fn debit_and_multi_objective_kinds_add_the_routing_approach_scope_the_tabs_sent() {
        let mut query = payment_audit_query();
        query.routing_kind = Some(PaymentAuditRoutingKind::DebitRouting);
        let debit = predicates(&payment_audit_summary_scope_filters(
            &query,
            PaymentAuditScope::All,
        ));
        assert!(debit
            .iter()
            .any(|p| p.contains("routing_approach = ?") && p.contains("rankingAlgorithm")));

        query.routing_kind = Some(PaymentAuditRoutingKind::MultiObjective);
        let multi = predicates(&payment_audit_summary_scope_filters(
            &query,
            PaymentAuditScope::All,
        ));
        assert!(multi
            .iter()
            .any(|p| p.contains("routing_approach IS NULL") && p.contains("AND NOT")));

        // An explicit param carrying the same predicate is not duplicated.
        query.exclude_routing_approach = Some("NTW_BASED_ROUTING".to_string());
        let deduped = predicates(&payment_audit_summary_scope_filters(
            &query,
            PaymentAuditScope::All,
        ));
        assert_eq!(
            deduped
                .iter()
                .filter(|p| p.contains("routing_approach IS NULL"))
                .count(),
            1
        );

        query.routing_kind = Some(PaymentAuditRoutingKind::RuleBased);
        query.exclude_routing_approach = None;
        let rule = predicates(&payment_audit_summary_scope_filters(
            &query,
            PaymentAuditScope::All,
        ));
        assert!(!rule.iter().any(|p| p.contains("routing_approach")));
    }
}
