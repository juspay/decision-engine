use crate::analytics::flow::AnalyticsRoute;
use crate::analytics::models::{AnalyticsQuery, PaymentAuditQuery};

use super::common::{
    payment_audit_summary_kind, static_flow_type_in_sql, PAYMENT_AUDIT_DYNAMIC_FLOW_TYPES,
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

pub fn payment_audit_raw_filters(
    query: &PaymentAuditQuery,
    preview_only: bool,
) -> Vec<FilterClause> {
    let (start_ms, end_ms) = effective_payment_audit_window_bounds(query);
    let mut filters = base_window_filters(start_ms, end_ms);

    filters.extend(merchant_filter(&query.merchant_id));

    if preview_only {
        filters.push(FilterClause::raw(format!(
            "route = '{}'",
            AnalyticsRoute::RoutingEvaluate.as_str()
        )));
        filters.push(FilterClause::raw(format!(
            "flow_type IN {}",
            static_flow_type_in_sql(PAYMENT_AUDIT_PREVIEW_FLOW_TYPES)
        )));
    } else {
        filters.push(FilterClause::raw(format!(
            "flow_type IN {}",
            static_flow_type_in_sql(PAYMENT_AUDIT_DYNAMIC_FLOW_TYPES)
        )));
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

pub fn payment_audit_summary_bucket_filters(
    query: &PaymentAuditQuery,
    preview_only: bool,
) -> Vec<FilterClause> {
    let (start_ms, end_ms) = payment_audit_summary_bucket_bounds(query);
    vec![
        FilterClause::eq("merchant_id", query.merchant_id.clone()),
        FilterClause::new(
            "bucket_start >= fromUnixTimestamp64Milli(?)",
            vec![start_ms.into()],
        ),
        FilterClause::new(
            "bucket_start <= fromUnixTimestamp64Milli(?)",
            vec![end_ms.into()],
        ),
        FilterClause::eq("summary_kind", payment_audit_summary_kind(preview_only)),
    ]
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
    use crate::analytics::models::{AnalyticsQuery, AnalyticsRange, PaymentAuditQuery};

    use super::{
        analytics_dimension_filters, merchant_filter, payment_audit_raw_filters,
        payment_audit_summary_bucket_filters,
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
        }
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
        let filters = payment_audit_raw_filters(&payment_audit_query(), true);
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
        let filters = payment_audit_summary_bucket_filters(&payment_audit_query(), true);
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
            .any(|predicate| predicate == "summary_kind = ?"));
    }

    #[test]
    fn payment_audit_filters_include_debit_rows_without_explicit_routing_approach() {
        let mut query = payment_audit_query();
        query.routing_approach = Some("NTW_BASED_ROUTING".to_string());

        let predicates = payment_audit_raw_filters(&query, false)
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
    fn payment_audit_filters_exclude_debit_rows_without_explicit_routing_approach() {
        let mut query = payment_audit_query();
        query.exclude_routing_approach = Some("NTW_BASED_ROUTING".to_string());

        let predicates = payment_audit_raw_filters(&query, false)
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
}
