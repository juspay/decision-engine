use std::borrow::Cow;
use std::collections::HashMap;

use clickhouse::Row;
use serde::Deserialize;

use crate::analytics::flow::FlowType;
use crate::analytics::models::{
    PaymentAuditOutcome, PaymentAuditQuery, PaymentAuditScope, PaymentAuditSummary,
};
use crate::error::ApiError;
use crate::feedback::gateway_scoring_service::{txn_failure_states, txn_success_states};
use crate::types::txn_details::types::TxnStatus;

use super::super::common::{
    fetch_all, fetch_one, payment_audit_route_label, payment_audit_stage_label,
    static_flow_type_in_sql, ALL_DECISION_FLOW_TYPES, DOMAIN_TABLE,
    PAYMENT_AUDIT_SUMMARY_BUCKET_TABLE,
};
use super::super::filters::{
    payment_audit_needs_raw_rows, payment_audit_routing_kind_filters,
    payment_audit_summary_bucket_filters, payment_audit_summary_scope_filters,
    payment_audit_timeline_filters, payment_field_match, payment_request_events_filter,
    raw_routing_families_sql, summary_routing_families_sql,
};
use super::super::query::{BindArg, BoundQueryBuilder, FilterClause, OrderClause, SqlFragment};
use super::super::time::effective_payment_audit_window_bounds;

#[derive(Debug, Clone, Deserialize, Row)]
struct AuditSummaryRow {
    #[serde(alias = "resolved_lookup_key")]
    lookup_key: String,
    payment_key: u64,
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
    outcome: String,
}

#[derive(Debug, Clone, Deserialize, Row)]
struct CountRow {
    total_results: u64,
    total_success: u64,
    total_failure: u64,
}

#[derive(Debug, Clone, Deserialize, Row)]
struct PageKeyRow {
    payment_key: u64,
    total_results: u64,
    total_success: u64,
    total_failure: u64,
}

/// One per-payment aggregate a summary query can compute.
///
/// ClickHouse holds every selected aggregate for every payment in the window while it groups, so
/// the count and the page's keys hold only fixed-size numbers per payment (see `Grouping`); the
/// lists (`Gateways`, `Routes`) are built for the page's payments alone. Filters on what a
/// payment carried select payments per event instead (`payment_selection_filters`).
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
    /// Whether any attempt succeeded (bit 1) or failed terminally (bit 2): all the outcome reads.
    Outcome,
    /// The event families as a bitmask, which is all a routing-kind filter reads.
    RoutingFamilies,
}

/// How a summary groups events into payments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Grouping {
    /// By a 64-bit hash of the payment's lookup key: 8 bytes per payment instead of the key's
    /// text, for the count and the page's keys, which group every payment in the window.
    PaymentKey,
    /// By the lookup key itself, for the page's rows, which carry it.
    LookupKey,
}

/// The payment key: `cityHash64` of the lookup key. Two payments in one merchant's window share
/// a key with a probability around n²/2⁶⁵ (5·10⁻¹⁰ for 140k payments).
const PAYMENT_KEY_SQL: &str = "cityHash64(assumeNotNull(lookup_key))";

/// A routing decision's payment amount: the decider request's on `/decide-gateway`, either half's
/// on `/routing/hybrid`. 0 on events that carry none.
const EVENT_AMOUNT_SQL: &str = "greatest(\
    JSONExtractFloat(assumeNotNull(details), 'request', 'paymentInfo', 'amount'), \
    JSONExtractFloat(assumeNotNull(details), 'request', 'dynamic_routing_request', 'paymentInfo', 'amount'), \
    JSONExtractFloat(assumeNotNull(details), 'request', 'static_routing_request', 'parameters', 'amount', 'value'))";

/// The `[...]` literal for a set of transaction statuses, as score updates store them.
fn status_array_sql(statuses: &[TxnStatus]) -> String {
    format!(
        "[{}]",
        statuses
            .iter()
            .map(|status| format!("'{}'", status.to_text()))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// An event's (or a summary bucket's) outcome flags from its statuses: bit 1 when one is a
/// success, bit 2 when one is a terminal failure. Routing events' own `success`/`failure`
/// statuses (the API call's result) are not payment statuses and never match.
fn outcome_flags_of(statuses: &str) -> String {
    format!(
        "(hasAny({statuses}, {}) + 2 * hasAny({statuses}, {}))",
        status_array_sql(&txn_success_states()),
        status_array_sql(&txn_failure_states()),
    )
}

/// The raw events' outcome flags, combined per payment.
fn raw_outcome_flags_sql() -> String {
    format!("groupBitOr({})", outcome_flags_of("[ifNull(status, '')]"))
}

/// A payment's outcome from its `outcome_flags`: success if any attempt succeeded, failure if one
/// failed terminally and none succeeded, pending otherwise.
pub(crate) fn outcome_sql() -> String {
    outcome_of("outcome_flags")
}

/// [`outcome_sql`] over any expression yielding the payment's outcome flags.
fn outcome_of(flags: &str) -> String {
    format!(
        "multiIf(bitAnd({flags}, 1) != 0, 'success', bitAnd({flags}, 2) != 0, 'failure', 'pending')"
    )
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
    Aggregate::Outcome,
];

/// What the count reads: the outcome it splits totals by.
const COUNT_AGGREGATES: &[Aggregate] = &[Aggregate::Outcome];

/// What choosing a page reads: the page's sort keys, and the outcome its totals split by.
const PAGE_KEY_AGGREGATES: &[Aggregate] = &[
    Aggregate::LastSeen,
    Aggregate::EventCount,
    Aggregate::Outcome,
];

/// The list's totals: matching payments, and those that succeeded and failed.
type Totals = (usize, usize, usize);

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
            Self::Outcome => "statuses_state",
            Self::RoutingFamilies => "summary_kind",
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
            // Each bucket's statuses are reduced to their flags before grouping, so no
            // payment's set of statuses is ever merged.
            Self::Outcome => {
                return Cow::Owned(format!(
                    "groupBitOr({}) AS outcome_flags",
                    outcome_flags_of("finalizeAggregation(statuses_state)")
                ))
            }
            Self::RoutingFamilies => return Cow::Owned(summary_routing_families_sql()),
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
            Self::LatestStatus | Self::Outcome => Some("status"),
            Self::LatestGateway | Self::Gateways => Some("gateway"),
            Self::LatestStage => Some("event_stage"),
            Self::Routes => Some("route"),
            Self::RoutingFamilies => Some("flow_type"),
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
            Self::Outcome => {
                return Cow::Owned(format!("{} AS outcome_flags", raw_outcome_flags_sql()))
            }
            Self::RoutingFamilies => return Cow::Owned(raw_routing_families_sql()),
        })
    }
}

/// `base` plus the aggregates `outer_summary_filters` tests for this query, without repeats.
fn with_filter_aggregates(query: &PaymentAuditQuery, base: &[Aggregate]) -> Vec<Aggregate> {
    let routing_kind_filters = query
        .routing_kind
        .is_some_and(|kind| !payment_audit_routing_kind_filters(kind).is_empty());
    let filtered = [
        (routing_kind_filters, Aggregate::RoutingFamilies),
        (
            !query.payment_filters.outcome.is_empty(),
            Aggregate::Outcome,
        ),
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

/// A payment-level filter: retain its whole summary and trail, and apply before pagination.
/// The existing summaries don't distinguish decided gateways from reported update gateways,
/// so read those raw rows only when this filter is enabled.
fn connector_mismatch_filter(
    query: &PaymentAuditQuery,
    scope: PaymentAuditScope,
) -> Option<FilterClause> {
    if !query.payment_filters.connector_mismatch {
        return None;
    }
    let decisions = static_flow_type_in_sql(ALL_DECISION_FLOW_TYPES);
    let updates = static_flow_type_in_sql(&[
        FlowType::UpdateGatewayScoreUpdate,
        FlowType::UpdateGatewayScoreSkipped,
        FlowType::UpdateGatewayScoreError,
    ]);
    let mut payments = BoundQueryBuilder::new(DOMAIN_TABLE);
    payments.extend_selects([
        "assumeNotNull(lookup_key) AS lookup_key".to_string(),
        format!("argMaxIf(ifNull(gateway, ''), tuple(created_at_ms, event_id), flow_type IN {decisions}) AS decided_gateway"),
        format!("groupUniqArrayIf(ifNull(gateway, ''), flow_type IN {updates}) AS update_gateways"),
    ]);
    payments.extend_filters(payment_audit_timeline_filters(query, scope));
    payments.add_filter(FilterClause::raw("notEmpty(ifNull(payment_id, ''))"));
    payments.add_filter(FilterClause::raw("notEmpty(ifNull(lookup_key, ''))"));
    payments.add_filter(FilterClause::raw("notEmpty(ifNull(gateway, ''))"));
    payments.add_filter(FilterClause::raw(format!(
        "(flow_type IN {decisions} OR flow_type IN {updates})"
    )));
    payments.add_group_by("lookup_key");
    let mut mismatches = subquery(payments.into_fragment());
    mismatches.add_select(PAYMENT_KEY_SQL);
    mismatches.add_filter(FilterClause::raw("notEmpty(decided_gateway)"));
    // Hybrid can record `gateway:mca_id` while feedback records just `gateway`.
    // Provider names also arrive in mixed case; account suffixes alone are not mismatches.
    mismatches.add_filter(FilterClause::raw(
        "arrayExists(gateway -> lowerUTF8(splitByChar(':', gateway)[1]) != lowerUTF8(splitByChar(':', decided_gateway)[1]), update_gateways)",
    ));
    Some(payment_key_in(mismatches.into_fragment()))
}

/// `payment's key IN (<keys the fragment selects>)`. A set of 8-byte keys, however many payments
/// match.
fn payment_key_in(keys: SqlFragment) -> FilterClause {
    FilterClause::new(
        format!("{PAYMENT_KEY_SQL} IN ({})", keys.sql()),
        keys.binds().to_vec(),
    )
}

/// The payments with an event in scope matching `predicate`.
fn payments_with(
    query: &PaymentAuditQuery,
    scope: PaymentAuditScope,
    predicate: FilterClause,
) -> FilterClause {
    let mut payments = BoundQueryBuilder::new(DOMAIN_TABLE);
    payments.add_select(PAYMENT_KEY_SQL);
    payments.extend_filters(payment_audit_summary_scope_filters(query, scope));
    payments.add_filter(FilterClause::raw("notEmpty(ifNull(lookup_key, ''))"));
    payments.add_filter(predicate);
    payment_key_in(payments.into_fragment())
}

/// `column IN (values)`, or with `lowercase`, the same ignoring case.
fn any_of(column: &str, values: &[String], lowercase: bool) -> Option<FilterClause> {
    if values.is_empty() {
        return None;
    }
    let (column, placeholder) = if lowercase {
        (format!("lowerUTF8(ifNull({column}, ''))"), "lowerUTF8(?)")
    } else {
        (column.to_string(), "?")
    };
    Some(FilterClause::new(
        format!(
            "{column} IN ({})",
            vec![placeholder; values.len()].join(", ")
        ),
        values.iter().cloned().map(BindArg::from).collect(),
    ))
}

/// Filters that select payments by what their events carried: a payment matches when any of its
/// events in scope does, and keeps its whole trail. Each is a payment-key set over the window's
/// raw events, so no payment's values are collected to test them. A payment must pass every
/// filter; several values of one filter match a payment carrying any of them.
fn payment_selection_filters(
    query: &PaymentAuditQuery,
    scope: PaymentAuditScope,
) -> Vec<FilterClause> {
    let payment = &query.payment_filters;
    let mut predicates = vec![
        any_of("gateway", &query.gateway, false),
        any_of("route", &query.route, false),
        any_of("error_code", &query.error_code, false),
        query
            .status
            .as_ref()
            .map(|status| FilterClause::eq("status", status.clone())),
        query
            .flow_type
            .as_ref()
            .map(|flow_type| FilterClause::eq("flow_type", flow_type.clone())),
        // Attributes match ignoring case.
        any_of("currency", &payment.currency, true),
        any_of("payment_method", &payment.payment_method, true),
        any_of("payment_method_type", &payment.payment_method_type, true),
        any_of("card_network", &payment.card_network, true),
        any_of("auth_type", &payment.auth_type, true),
    ];
    // The bounds are inclusive and read the routing decision's amount; events without one
    // (score updates, rule evaluations without an amount) never match.
    if payment.min_amount.is_some() || payment.max_amount.is_some() {
        let mut bounds = format!("{EVENT_AMOUNT_SQL} > 0");
        let mut binds = Vec::new();
        for (bound, operator) in [(payment.min_amount, ">="), (payment.max_amount, "<=")] {
            if let Some(bound) = bound {
                bounds.push_str(&format!(" AND {EVENT_AMOUNT_SQL} {operator} ?"));
                binds.push(bound.into());
            }
        }
        predicates.push(Some(FilterClause::new(bounds, binds)));
    }

    connector_mismatch_filter(query, scope)
        .into_iter()
        .chain(
            predicates
                .into_iter()
                .flatten()
                .map(|predicate| payments_with(query, scope, predicate)),
        )
        .chain(payment_fields_selection(query, scope))
        .collect()
}

/// The payments carrying every payment field's value, from one scan: each request-storing event
/// in scope is parsed once and tested against every field, and a payment matches when each field
/// matched on one of its events. A subquery per field would parse every request once per field.
fn payment_fields_selection(
    query: &PaymentAuditQuery,
    scope: PaymentAuditScope,
) -> Option<FilterClause> {
    let fields = &query.payment_filters.fields;
    if fields.is_empty() {
        return None;
    }
    let matches: Vec<FilterClause> = fields.iter().map(payment_field_match).collect();
    let mut scope_filters = payment_audit_summary_scope_filters(query, scope);
    scope_filters.push(FilterClause::raw("notEmpty(ifNull(lookup_key, ''))"));
    scope_filters.push(payment_request_events_filter());

    // Each event's match per field, as columns `field_0`, `field_1`, …
    let columns = matches
        .iter()
        .enumerate()
        .map(|(index, matched)| format!("{} AS field_{index}", matched.predicate()))
        .collect::<Vec<_>>()
        .join(", ");
    let conditions = scope_filters
        .iter()
        .map(FilterClause::predicate)
        .collect::<Vec<_>>()
        .join(" AND ");
    let binds = matches
        .iter()
        .chain(&scope_filters)
        .flat_map(|clause| clause.binds().iter().cloned())
        .collect();
    let events = SqlFragment::with_binds(
        format!(
            "(SELECT {PAYMENT_KEY_SQL} AS payment_key, {columns} FROM {DOMAIN_TABLE} WHERE {conditions})"
        ),
        binds,
    );

    let field_names = (0..fields.len()).map(|index| format!("field_{index}"));
    let mut payments = BoundQueryBuilder::from_fragment(events);
    payments.add_select("payment_key");
    // Named apart from the event columns, which the WHERE below reads.
    payments.extend_selects(
        field_names
            .clone()
            .map(|name| format!("max({name}) AS has_{name}")),
    );
    payments.add_filter(FilterClause::raw(
        field_names.clone().collect::<Vec<_>>().join(" OR "),
    ));
    payments.add_group_by("payment_key");

    let mut matched = subquery(payments.into_fragment());
    matched.add_select("payment_key");
    matched.add_filter(FilterClause::raw(
        field_names
            .map(|name| format!("has_{name}"))
            .collect::<Vec<_>>()
            .join(" AND "),
    ));
    Some(payment_key_in(matched.into_fragment()))
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
    page: Option<&[u64]>,
) -> SqlFragment {
    let mut source = BoundQueryBuilder::new(PAYMENT_AUDIT_SUMMARY_BUCKET_TABLE);
    source.add_select("lookup_key");
    source.extend_selects(aggregates.iter().map(|aggregate| aggregate.state_column()));
    source.extend_filters(payment_audit_summary_bucket_filters(query, scope));
    // A bucket is 15 minutes, so the first and last can reach past the window; a payment is in
    // the window when one of its buckets has events inside it.
    let (start_ms, end_ms) = effective_payment_audit_window_bounds(query);
    source.add_filter(FilterClause::lte(
        "finalizeAggregation(first_seen_ms_state)",
        end_ms,
    ));
    source.add_filter(FilterClause::gte(
        "finalizeAggregation(last_seen_ms_state)",
        start_ms,
    ));
    source.extend_filters(page_filter(query, scope, page));
    group(
        source.into_fragment(),
        aggregates,
        page,
        Aggregate::merged_sql,
    )
}

/// The selection filters, or for a page's rows, those payments alone.
fn page_filter(
    query: &PaymentAuditQuery,
    scope: PaymentAuditScope,
    page: Option<&[u64]>,
) -> Vec<FilterClause> {
    match page {
        None => payment_selection_filters(query, scope),
        Some(keys) => vec![FilterClause::new(
            format!(
                "{PAYMENT_KEY_SQL} IN ({})",
                vec!["?"; keys.len()].join(", ")
            ),
            keys.iter().copied().map(BindArg::U64).collect(),
        )],
    }
}

/// Groups `source` rows into payments: by payment key when choosing the page (`page` is
/// `None`), by lookup key for the page's rows.
fn group(
    source: SqlFragment,
    aggregates: &[Aggregate],
    page: Option<&[u64]>,
    aggregate_sql: fn(Aggregate) -> Cow<'static, str>,
) -> SqlFragment {
    let grouping = match page {
        None => Grouping::PaymentKey,
        Some(_) => Grouping::LookupKey,
    };
    let mut builder = subquery(source);
    match grouping {
        Grouping::PaymentKey => {
            builder.add_select(format!("{PAYMENT_KEY_SQL} AS payment_key"));
            builder.add_group_by("payment_key");
        }
        Grouping::LookupKey => {
            builder.add_select("assumeNotNull(lookup_key) AS lookup_key");
            builder.add_select(format!("{PAYMENT_KEY_SQL} AS payment_key"));
            builder.add_group_by("lookup_key");
        }
    }
    builder.extend_selects(
        aggregates
            .iter()
            .map(|aggregate| aggregate_sql(*aggregate).into_owned()),
    );
    builder.into_fragment()
}

fn raw_summary_fragment(
    query: &PaymentAuditQuery,
    scope: PaymentAuditScope,
    aggregates: &[Aggregate],
    page: Option<&[u64]>,
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

    // Every row is an event inside the window, so each payment here is in it.
    let mut source = BoundQueryBuilder::new(DOMAIN_TABLE);
    source.extend_selects(columns);
    source.extend_filters(payment_audit_summary_scope_filters(query, scope));
    source.add_filter(FilterClause::raw("lookup_key IS NOT NULL"));
    source.add_filter(FilterClause::raw("lookup_key != ''"));
    source.extend_filters(page_filter(query, scope, page));
    group(source.into_fragment(), aggregates, page, Aggregate::raw_sql)
}

fn summary_fragment(
    query: &PaymentAuditQuery,
    scope: PaymentAuditScope,
    aggregates: &[Aggregate],
    page: Option<&[u64]>,
) -> SqlFragment {
    if payment_audit_needs_raw_rows(query) {
        return raw_summary_fragment(query, scope, aggregates, page);
    }

    merged_summary_fragment(query, scope, aggregates, page)
}

fn exact_lookup_filter(lookup_key: &str) -> FilterClause {
    FilterClause::new(
        "(lookup_key = ? OR payment_id = ? OR request_id = ? OR global_request_id = ? OR event_id = ?)",
        std::iter::repeat_n(BindArg::from(lookup_key), 5).collect(),
    )
}

/// Filters on a payment's grouped flags. Everything else that narrows the list selects payments
/// per event (`payment_selection_filters`), and the window is checked per bucket or event.
fn outer_summary_filters(query: &PaymentAuditQuery) -> Vec<FilterClause> {
    let mut filters = Vec::new();
    if let Some(kind) = query.routing_kind {
        filters.extend(payment_audit_routing_kind_filters(kind));
    }
    let outcomes = &query.payment_filters.outcome;
    if !outcomes.is_empty() {
        // Enum labels, not user input, so they are safe to inline.
        let outcomes = outcomes
            .iter()
            .map(|outcome| format!("'{}'", outcome.as_str()))
            .collect::<Vec<_>>()
            .join(", ");
        filters.push(FilterClause::raw(format!(
            "{} IN ({outcomes})",
            outcome_sql()
        )));
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
                outcome: PaymentAuditOutcome::from_query(Some(&row.outcome))
                    .unwrap_or(PaymentAuditOutcome::Pending),
                latest_gateway: row.latest_gateway,
                latest_stage: row.latest_stage.map(payment_audit_stage_label),
                gateways: row.gateways,
                routes: row
                    .routes
                    .into_iter()
                    .map(payment_audit_route_label)
                    .collect(),
                // Filled in by the payment audit endpoint, which looks them up per page.
                experiments: None,
            })
        })
        .collect()
}

/// The totals alone: for a page past the last, whose query returns no row to carry them.
async fn count(
    client: &clickhouse::Client,
    query: &PaymentAuditQuery,
    scope: PaymentAuditScope,
) -> Result<Totals, ApiError> {
    let aggregates = with_filter_aggregates(query, COUNT_AGGREGATES);
    let mut builder = subquery(summary_fragment(query, scope, &aggregates, None));
    builder.add_select("count() AS total_results");
    builder.add_select(format!(
        "countIf({} = 'success') AS total_success",
        outcome_sql()
    ));
    builder.add_select(format!(
        "countIf({} = 'failure') AS total_failure",
        outcome_sql()
    ));
    builder.extend_filters(outer_summary_filters(query));
    let row = fetch_one::<CountRow>(builder.build(client)).await?;
    Ok((
        row.total_results as usize,
        row.total_success as usize,
        row.total_failure as usize,
    ))
}

/// The payment keys on the requested page, in page order, and the totals over every matching
/// payment, from one grouping: the totals are window functions over the matches, computed before
/// the page is cut. `None` totals when the page is past the last.
///
/// The filters, a payment field's request parsing above all, are evaluated once for both.
async fn load_page_keys(
    client: &clickhouse::Client,
    query: &PaymentAuditQuery,
    scope: PaymentAuditScope,
) -> Result<(Vec<u64>, Option<Totals>), ApiError> {
    let rows = fetch_all::<PageKeyRow>(page_keys_query(query, scope).build(client)).await?;
    let totals = rows.first().map(|row| {
        (
            row.total_results as usize,
            row.total_success as usize,
            row.total_failure as usize,
        )
    });
    Ok((
        rows.into_iter().map(|row| row.payment_key).collect(),
        totals,
    ))
}

fn page_keys_query(query: &PaymentAuditQuery, scope: PaymentAuditScope) -> BoundQueryBuilder {
    let aggregates = with_filter_aggregates(query, PAGE_KEY_AGGREGATES);
    let mut builder = subquery(summary_fragment(query, scope, &aggregates, None));
    builder.add_select("payment_key");
    builder.add_select("count() OVER () AS total_results");
    builder.add_select(format!(
        "countIf({} = 'success') OVER () AS total_success",
        outcome_sql()
    ));
    builder.add_select(format!(
        "countIf({} = 'failure') OVER () AS total_failure",
        outcome_sql()
    ));
    builder.extend_filters(outer_summary_filters(query));
    builder.add_order_by(OrderClause::desc("last_seen_ms"));
    builder.add_order_by(OrderClause::desc("event_count"));
    builder.add_order_by(OrderClause::asc("payment_key"));
    builder.set_limit(Some(query.page_size as u64));
    builder.set_offset(Some(((query.page - 1) * query.page_size) as u64));
    builder
}

/// One page of the list and its totals: the page's keys and the totals first, then the full row
/// aggregates for those keys alone.
pub async fn load_list(
    client: &clickhouse::Client,
    query: &PaymentAuditQuery,
    scope: PaymentAuditScope,
) -> Result<(Totals, Vec<PaymentAuditSummary>), ApiError> {
    let (keys, totals) = load_page_keys(client, query, scope).await?;
    let totals = match totals {
        Some(totals) => totals,
        None if query.page <= 1 => (0, 0, 0),
        None => count(client, query, scope).await?,
    };
    if keys.is_empty() {
        return Ok((totals, Vec::new()));
    }

    let mut builder = subquery(summary_fragment(query, scope, ROW_AGGREGATES, Some(&keys)));
    builder.extend_selects([
        "lookup_key",
        "payment_key",
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
    builder.add_select(format!("{} AS outcome", outcome_sql()));
    let mut rows = fetch_all::<AuditSummaryRow>(builder.build(client)).await?;

    let position: HashMap<u64, usize> = keys
        .iter()
        .enumerate()
        .map(|(index, key)| (*key, index))
        .collect();
    rows.sort_by_key(|row| {
        position
            .get(&row.payment_key)
            .copied()
            .unwrap_or(usize::MAX)
    });
    Ok((totals, map_rows(rows)))
}

pub async fn load_exact(
    client: &clickhouse::Client,
    query: &PaymentAuditQuery,
    scope: PaymentAuditScope,
    lookup_key: &str,
) -> Result<Vec<PaymentAuditSummary>, ApiError> {
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
    source.extend_filters(payment_audit_timeline_filters(query, scope));
    source.add_filter(exact_lookup_filter(lookup_key));

    let source = source.into_fragment();
    let mut builder = BoundQueryBuilder::from_fragment(SqlFragment::with_binds(
        format!("({})", source.sql()),
        source.binds().to_vec(),
    ));
    builder.extend_selects([
        "assumeNotNull(any(lookup_key)) AS lookup_key".to_string(),
        // `lookup_key` is the alias above, already aggregated.
        format!("{PAYMENT_KEY_SQL} AS payment_key"),
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
        // Computed in place: an extra `outcome_flags` column would misalign the row.
        format!("{} AS outcome", outcome_of(&raw_outcome_flags_sql())),
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
        summary_fragment, with_filter_aggregates, Aggregate, COUNT_AGGREGATES, PAGE_KEY_AGGREGATES,
        ROW_AGGREGATES,
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
            gateway: Vec::new(),
            route: Vec::new(),
            status: None,
            flow_type: None,
            routing_approach: None,
            exclude_routing_approach: None,
            error_code: Vec::new(),
            scope: PaymentAuditScope::All,
            routing_kind: None,
            payment_filters: Default::default(),
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
        // Every kind's buckets of a payment merge into one row per payment key.
        assert!(sql.contains("GROUP BY payment_key"));
        assert!(!sql.contains("summary_kind"));
        assert_eq!(
            fragment.binds().len(),
            5,
            "merchant + bucket window bounds + the window per bucket"
        );
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
        // Totals split by outcome, which reads every reported status, not just the latest.
        assert!(count.sql().contains("statuses_state"));
        assert!(!count.sql().contains("latest_status_state"));
        assert!(!count.sql().contains("gateways_state"));
        assert!(!count.sql().contains("payment_id_state"));

        let raw = raw_summary_fragment(&query, PaymentAuditScope::All, COUNT_AGGREGATES, None);
        assert!(raw
            .sql()
            .contains("SELECT lookup_key, created_at_ms, status FROM"));
        assert!(!raw.sql().contains("ifNull(gateway"));
    }

    #[test]
    fn outcome_reads_payment_statuses_not_the_routing_result() {
        use crate::feedback::gateway_scoring_service::{txn_failure_states, txn_success_states};

        let success = super::status_array_sql(&txn_success_states());
        let failure = super::status_array_sql(&txn_failure_states());
        assert!(success.contains("'CHARGED'"));
        assert!(failure.contains("'AUTHORIZATION_FAILED'"));
        // A routing event's `success` / `failure` is the API call's result, not the payment's,
        // and statuses match case-sensitively.
        assert!(!success.contains("'success'"));
        assert!(!failure.contains("'failure'"));
        let flags = super::outcome_flags_of("statuses");
        assert!(flags.contains(&success) && flags.contains(&failure));
        // A payment's outcome reads only its flags: success wins over a failed attempt.
        assert_eq!(
            super::outcome_sql(),
            "multiIf(bitAnd(outcome_flags, 1) != 0, 'success', bitAnd(outcome_flags, 2) != 0, 'failure', 'pending')"
        );
    }

    #[test]
    fn outcome_filter_keeps_the_pre_aggregated_path() {
        let mut query = payment_audit_query();
        query.payment_filters.outcome = vec![
            crate::analytics::models::PaymentAuditOutcome::Failure,
            crate::analytics::models::PaymentAuditOutcome::Pending,
        ];
        assert!(with_filter_aggregates(&query, &[]).contains(&Aggregate::Outcome));
        assert!(summary_fragment(&query, PaymentAuditScope::All, &[], None)
            .sql()
            .contains(PAYMENT_AUDIT_SUMMARY_BUCKET_TABLE));
        assert!(outer_summary_filters(&query)
            .iter()
            .any(|f| f.predicate().ends_with("IN ('failure', 'pending')")));
    }

    #[test]
    fn payment_attribute_filters_select_payments_per_event_on_the_summary_path() {
        let mut query = payment_audit_query();
        query.payment_filters.currency = vec!["USD".to_string(), "eur".to_string()];
        query.payment_filters.min_amount = Some(50.0);
        query.payment_filters.max_amount = Some(150.0);

        // No per-payment lists: the count still groups only the outcome flags.
        assert_eq!(
            with_filter_aggregates(&query, COUNT_AGGREGATES),
            COUNT_AGGREGATES
        );
        assert!(!super::payment_audit_needs_raw_rows(&query));
        let fragment = summary_fragment(&query, PaymentAuditScope::All, COUNT_AGGREGATES, None);
        assert!(fragment.sql().contains(PAYMENT_AUDIT_SUMMARY_BUCKET_TABLE));
        assert!(!fragment.sql().contains("AS currencies"));

        let filters = super::payment_selection_filters(&query, PaymentAuditScope::All);
        let currency = filters
            .iter()
            .find(|f| f.predicate().contains("lowerUTF8(ifNull(currency, ''))"))
            .expect("currency filter");
        // A payment-key set over events carrying either currency, ignoring case.
        assert!(currency
            .predicate()
            .starts_with("cityHash64(assumeNotNull(lookup_key)) IN (SELECT cityHash64("));
        assert!(currency
            .predicate()
            .contains("lowerUTF8(ifNull(currency, '')) IN (lowerUTF8(?), lowerUTF8(?))"));
        let amount = filters
            .iter()
            .find(|f| f.predicate().contains("greatest("))
            .expect("amount filter");
        assert!(amount.predicate().contains("> 0 AND greatest("));
        assert_eq!(
            amount.predicate().matches('?').count(),
            amount.binds().len()
        );
    }

    #[test]
    fn filtered_aggregates_join_the_base_set_once() {
        let mut query = payment_audit_query();
        assert_eq!(
            with_filter_aggregates(&query, COUNT_AGGREGATES),
            COUNT_AGGREGATES
        );

        // Gateway and status select payments per event, adding nothing to the grouping.
        query.gateway = vec!["adyen".to_string()];
        query.status = Some("FAILURE".to_string());
        query.routing_kind = Some(PaymentAuditRoutingKind::RuleBased);
        query.payment_filters.outcome =
            vec![crate::analytics::models::PaymentAuditOutcome::Failure];
        let aggregates = with_filter_aggregates(&query, ROW_AGGREGATES);
        assert_eq!(
            aggregates
                .iter()
                .filter(|aggregate| **aggregate == Aggregate::Outcome)
                .count(),
            1
        );
        assert!(aggregates.contains(&Aggregate::RoutingFamilies));
        assert_eq!(
            with_filter_aggregates(&query, PAGE_KEY_AGGREGATES),
            [
                Aggregate::LastSeen,
                Aggregate::EventCount,
                Aggregate::Outcome,
                Aggregate::RoutingFamilies
            ]
        );

        // Debit routing filters on raw routing_approach, not on the aggregated families.
        query.routing_kind = Some(PaymentAuditRoutingKind::DebitRouting);
        assert!(
            !with_filter_aggregates(&query, COUNT_AGGREGATES).contains(&Aggregate::RoutingFamilies)
        );
    }

    #[test]
    fn page_rows_are_read_for_the_page_keys_only() {
        let keys = [11_u64, 22];
        let merged = merged_summary_fragment(
            &payment_audit_query(),
            PaymentAuditScope::All,
            ROW_AGGREGATES,
            Some(&keys),
        );
        assert!(merged
            .sql()
            .contains("cityHash64(assumeNotNull(lookup_key)) IN (?, ?)"));
        // The page's rows carry the lookup key itself.
        assert!(merged.sql().contains("GROUP BY lookup_key"));
        assert_eq!(merged.sql().matches('?').count(), merged.binds().len());

        let raw = raw_summary_fragment(
            &payment_audit_query(),
            PaymentAuditScope::All,
            ROW_AGGREGATES,
            Some(&keys),
        );
        assert!(raw
            .sql()
            .contains("cityHash64(assumeNotNull(lookup_key)) IN (?, ?)"));
    }

    /// Each selection is a set of payment keys compared with the payment's key: a set of lookup
    /// keys would compare text with a number and match nothing.
    #[test]
    fn every_payment_selection_compares_payment_keys() {
        use crate::analytics::models::{PaymentField, PaymentFieldFilter};
        let mut query = payment_audit_query();
        query.payment_filters.connector_mismatch = true;
        query.gateway = vec!["adyen".to_string()];
        query.payment_filters.fields = vec![PaymentFieldFilter {
            field: PaymentField::RoutingKey("card_bin".into()),
            values: vec!["424242".into()],
        }];
        let filters = super::payment_selection_filters(&query, PaymentAuditScope::All);
        assert_eq!(filters.len(), 3);
        for filter in filters {
            let predicate = filter.predicate();
            // The selected column is the payment key, computed directly or as `payment_key`.
            let computed = predicate
                .starts_with("cityHash64(assumeNotNull(lookup_key)) IN (SELECT cityHash64(");
            let named = predicate
                .starts_with("cityHash64(assumeNotNull(lookup_key)) IN (SELECT payment_key FROM")
                && predicate.contains("cityHash64(assumeNotNull(lookup_key)) AS payment_key");
            assert!(computed || named, "{predicate}");
            assert_eq!(predicate.matches('?').count(), filter.binds().len());
        }
    }

    /// The page's keys and the totals come from one grouping, so each filter, a payment field's
    /// request parsing above all, is evaluated once per search rather than once for the count
    /// and again for the page.
    #[test]
    fn one_query_chooses_the_page_and_counts_the_matches() {
        use crate::analytics::models::{PaymentField, PaymentFieldFilter};
        let mut query = payment_audit_query();
        query.payment_filters.fields = vec![PaymentFieldFilter {
            field: PaymentField::Metadata("segment".into()),
            values: vec!["premium".into()],
        }];
        let sql = super::page_keys_query(&query, PaymentAuditScope::All)
            .into_fragment()
            .sql()
            .to_string();
        assert!(sql.contains("count() OVER () AS total_results"), "{sql}");
        assert!(sql.contains("OVER () AS total_success"));
        assert!(sql.contains("OVER () AS total_failure"));
        // The field's subquery reads both paymentInfo paths once.
        assert_eq!(sql.matches("'paymentInfo', 'metadata'").count(), 2);
        assert!(sql.contains("LIMIT"));
    }

    /// The count and the page's keys group every payment in the window, so each holds only
    /// fixed-size values per payment: an 8-byte key, flags and numbers, never a list or the
    /// lookup key's text.
    #[test]
    fn choosing_the_page_holds_fixed_size_values_per_payment() {
        let mut query = payment_audit_query();
        query.gateway = vec!["adyen".to_string()];
        query.payment_filters.currency = vec!["USD".to_string()];
        for (aggregates, path) in [
            (with_filter_aggregates(&query, COUNT_AGGREGATES), "merged"),
            (
                with_filter_aggregates(&query, PAGE_KEY_AGGREGATES),
                "merged",
            ),
        ] {
            let sql = summary_fragment(&query, PaymentAuditScope::All, &aggregates, None)
                .sql()
                .to_string();
            assert!(sql.contains("GROUP BY payment_key"), "{path}: {sql}");
            assert!(!sql.contains("groupUniqArray"), "{path}: {sql}");
        }
        query.routing_kind = Some(PaymentAuditRoutingKind::AuthRate);
        let raw = summary_fragment(&query, PaymentAuditScope::All, COUNT_AGGREGATES, None)
            .sql()
            .to_string();
        assert!(raw.contains(DOMAIN_TABLE));
        assert!(raw.contains("GROUP BY payment_key"));
        assert!(!raw.contains("groupUniqArray"));
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
        assert_eq!(
            fragment.binds().len(),
            7,
            "merchant + window + two kinds + the window per bucket"
        );
        let preview = merged_summary_fragment(
            &payment_audit_query(),
            PaymentAuditScope::Preview,
            COUNT_AGGREGATES,
            None,
        );
        assert!(preview.sql().contains("summary_kind IN (?)"));
        assert_eq!(preview.binds().len(), 6);
    }

    #[test]
    fn payment_fields_select_payments_on_both_summary_paths_without_reading_raw_rows() {
        use crate::analytics::models::{PaymentField, PaymentFieldFilter};
        let mut query = payment_audit_query();
        query.payment_filters.fields = vec![
            PaymentFieldFilter {
                field: PaymentField::RoutingKey("card_bin".into()),
                values: vec!["424242".into()],
            },
            PaymentFieldFilter {
                field: PaymentField::Metadata("segment".into()),
                values: vec!["premium".into()],
            },
        ];
        assert!(!super::payment_audit_needs_raw_rows(&query));
        let merged = summary_fragment(&query, PaymentAuditScope::All, COUNT_AGGREGATES, None);
        // One scan for both fields: each event is tested against both, and a payment must
        // have matched each on one of its events.
        let sql = merged.sql();
        assert_eq!(
            sql.matches("cityHash64(assumeNotNull(lookup_key)) IN (SELECT payment_key")
                .count(),
            1
        );
        assert!(sql.contains("AS field_0") && sql.contains("AS field_1"));
        assert!(sql.contains("WHERE field_0 OR field_1 GROUP BY payment_key"));
        assert!(sql.contains("WHERE has_field_0 AND has_field_1"));
        assert_eq!(merged.sql().matches('?').count(), merged.binds().len());

        query.routing_kind = Some(PaymentAuditRoutingKind::AuthRate);
        let raw = summary_fragment(&query, PaymentAuditScope::All, COUNT_AGGREGATES, None);
        assert!(raw.sql().contains(DOMAIN_TABLE));
        assert_eq!(
            raw.sql()
                .matches("cityHash64(assumeNotNull(lookup_key)) IN (SELECT payment_key")
                .count(),
            1
        );
        assert_eq!(raw.sql().matches('?').count(), raw.binds().len());
    }

    /// What bounds a payment field's cost: it parses stored requests only for the window's
    /// request-carrying events in the window's monthly partitions. Each of these predicates must
    /// stay in its subquery; the benchmark in `scripts/bench/payment-audit-filters.py` measures
    /// what they save.
    #[test]
    fn a_payment_field_parses_requests_only_inside_the_window() {
        use crate::analytics::models::{PaymentField, PaymentFieldFilter};
        let mut query = payment_audit_query();
        query.payment_filters.fields = vec![PaymentFieldFilter {
            field: PaymentField::RoutingKey("card_bin".into()),
            values: vec!["424242".into()],
        }];
        let filters = super::payment_selection_filters(&query, PaymentAuditScope::All);
        let [field] = filters.as_slice() else {
            panic!("one filter per payment field, got {}", filters.len());
        };
        let subquery = field.predicate();
        for bound in [
            "created_at_ms >= ?",
            "created_at_ms <= ?",
            // The partition column, so months outside the window are skipped.
            "created_at >= fromUnixTimestamp64Milli",
            "created_at <= fromUnixTimestamp64Milli",
            "merchant_id = ?",
            "flow_type IN ('decide_gateway_decision'",
        ] {
            assert!(subquery.contains(bound), "{bound} missing from {subquery}");
        }
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

        query.routing_kind = Some(PaymentAuditRoutingKind::MultiObjective);
        assert!(sql(&query).contains(PAYMENT_AUDIT_SUMMARY_BUCKET_TABLE));

        query.routing_kind = Some(PaymentAuditRoutingKind::DebitRouting);
        let debit = sql(&query);
        assert!(debit.contains(DOMAIN_TABLE));
        assert!(!debit.contains(PAYMENT_AUDIT_SUMMARY_BUCKET_TABLE));

        query.routing_kind = Some(PaymentAuditRoutingKind::AuthRate);
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
            Some(&[7]),
        );
        assert!(fragment.sql().contains("AS lookup_key"));
        assert!(fragment.sql().contains("AS payment_key"));
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

    /// A payment is in the window when one of its buckets has events inside it, checked on each
    /// bucket, so no payment carries a first-seen time just for the check. The grouped query
    /// filters only on the payment's flags.
    #[test]
    fn the_window_is_checked_per_bucket_and_the_grouping_filters_only_flags() {
        let mut query = payment_audit_query();
        query.gateway = vec!["adyen".to_string()];
        query.payment_filters.currency = vec!["USD".to_string()];
        assert!(outer_summary_filters(&query).is_empty());

        let sql = merged_summary_fragment(&query, PaymentAuditScope::All, COUNT_AGGREGATES, None)
            .sql()
            .to_string();
        assert!(sql.contains("finalizeAggregation(first_seen_ms_state) <= ?"));
        assert!(sql.contains("finalizeAggregation(last_seen_ms_state) >= ?"));
        assert!(!sql.contains("minMerge(first_seen_ms_state)"));
    }
}
