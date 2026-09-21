use clickhouse::Row;
use serde::Deserialize;

use crate::analytics::flow::FlowType;
use crate::analytics::models::{AnalyticsAuthRate, AnalyticsQuery};
use crate::error::ApiError;
use crate::feedback::gateway_scoring_service::{txn_failure_states, txn_success_states};
use crate::types::txn_details::types::TxnStatus;

use super::super::common::{
    all_decisions_filter, decision_shape, fetch_one, static_flow_type_in_sql, DOMAIN_TABLE,
};
use super::super::filters::{analytics_dimension_filters, base_window_filters, merchant_filter};
use super::super::query::{BoundQueryBuilder, FilterClause};
use super::super::time::effective_window_bounds;

/// Both carry the merchant's reported status. `_update` is an outcome the scorer consumed and
/// `_skipped` one it passed over. Dropping `_skipped` would lose most of the reported traffic.
const OUTCOME_FLOW_TYPES: &[FlowType] = &[
    FlowType::UpdateGatewayScoreUpdate,
    FlowType::UpdateGatewayScoreSkipped,
];

#[derive(Debug, Clone, Deserialize, Row)]
struct AuthRateRow {
    success_count: u64,
    failure_count: u64,
}

/// Authorization rate over the payments *this routing kind* decided.
///
/// Counted from reported outcomes, not the SR score store, which holds one merchant-wide
/// estimate both kinds write to, over the model's fixed window rather than the selected range.
/// Dimension filters sit on the decision subquery — outcome events carry only `gateway`.
pub async fn load(
    client: &clickhouse::Client,
    query: &AnalyticsQuery,
) -> Result<AnalyticsAuthRate, ApiError> {
    run(client, outcome_query(query)).await
}

/// The same rate over every payment the merchant routed, whichever kind decided it.
pub async fn load_all(
    client: &clickhouse::Client,
    query: &AnalyticsQuery,
) -> Result<AnalyticsAuthRate, ApiError> {
    run(client, all_outcome_query(query)).await
}

async fn run(
    client: &clickhouse::Client,
    builder: BoundQueryBuilder,
) -> Result<AnalyticsAuthRate, ApiError> {
    let row = fetch_one::<AuthRateRow>(builder.build(client)).await?;
    Ok(AnalyticsAuthRate {
        success_count: row.success_count as i64,
        failure_count: row.failure_count as i64,
    })
}

fn outcome_query(query: &AnalyticsQuery) -> BoundQueryBuilder {
    scoped_outcome_query(query, decision_shape(query.routing_kind).decision_filter())
}

fn all_outcome_query(query: &AnalyticsQuery) -> BoundQueryBuilder {
    scoped_outcome_query(query, all_decisions_filter())
}

fn scoped_outcome_query(
    query: &AnalyticsQuery,
    decision_filter: FilterClause,
) -> BoundQueryBuilder {
    let (start_ms, end_ms) = effective_window_bounds(query);

    let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);
    builder.extend_selects([
        format!(
            "countIf(status IN {}) AS success_count",
            status_in_sql(&txn_success_states())
        ),
        format!(
            "countIf(status IN {}) AS failure_count",
            status_in_sql(&txn_failure_states())
        ),
    ]);
    builder.extend_filters(base_window_filters(start_ms, end_ms));
    builder.extend_filters(merchant_filter(&query.merchant_id));
    builder.add_filter(FilterClause::raw(format!(
        "flow_type IN {}",
        static_flow_type_in_sql(OUTCOME_FLOW_TYPES)
    )));
    // Binds are emitted in the order filters are added, so the subquery's own binds follow the
    // window and merchant ones above.
    builder.add_filter(decided_payments_filter(
        query,
        start_ms,
        end_ms,
        decision_filter,
    ));

    builder
}

/// `payment_id IN (…)` over the decisions `decision_filter` selects in the window.
fn decided_payments_filter(
    query: &AnalyticsQuery,
    start_ms: i64,
    end_ms: i64,
    decision_filter: FilterClause,
) -> FilterClause {
    let mut filters = base_window_filters(start_ms, end_ms);
    filters.extend(merchant_filter(&query.merchant_id));
    filters.push(decision_filter);
    filters.extend(analytics_dimension_filters(query));

    let predicate = filters
        .iter()
        .map(FilterClause::predicate)
        .collect::<Vec<_>>()
        .join(" AND ");
    let binds = filters
        .iter()
        .flat_map(|filter| filter.binds().iter().cloned())
        .collect();

    FilterClause::new(
        format!("payment_id IN (SELECT payment_id FROM {DOMAIN_TABLE} WHERE {predicate})"),
        binds,
    )
}

/// The `IN (…)` list for a set of transaction statuses, as they are stored on the event.
fn status_in_sql(statuses: &[TxnStatus]) -> String {
    format!(
        "({})",
        statuses
            .iter()
            .map(|status| format!("'{}'", status.to_text()))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

#[cfg(test)]
mod tests {
    use crate::analytics::models::{AnalyticsRange, AnalyticsRoutingKind};

    use super::*;

    fn analytics_query(routing_kind: AnalyticsRoutingKind) -> AnalyticsQuery {
        AnalyticsQuery {
            merchant_id: "m_123".to_string(),
            range: AnalyticsRange::H1,
            start_ms: Some(100),
            end_ms: Some(200),
            page: 1,
            page_size: 20,
            payment_method_type: None,
            payment_method: None,
            card_network: None,
            card_is_in: None,
            currency: None,
            country: None,
            auth_type: None,
            gateways: Vec::new(),
            routing_kind,
        }
    }

    #[test]
    fn each_routing_kind_counts_only_the_payments_it_decided() {
        let hybrid = outcome_query(&analytics_query(AnalyticsRoutingKind::Hybrid)).sql();
        assert!(hybrid.contains("flow_type = 'routing_hybrid_decision'"));
        assert!(!hybrid.contains("flow_type = 'decide_gateway_decision'"));

        let multi_objective =
            outcome_query(&analytics_query(AnalyticsRoutingKind::MultiObjective)).sql();
        assert!(multi_objective.contains("flow_type = 'decide_gateway_decision'"));
        assert!(!multi_objective.contains("flow_type = 'routing_hybrid_decision'"));
    }

    #[test]
    fn the_merchant_wide_rate_counts_payments_either_kind_decided() {
        let sql = all_outcome_query(&analytics_query(AnalyticsRoutingKind::MultiObjective)).sql();

        assert!(sql.contains("'decide_gateway_decision'"));
        assert!(sql.contains("'routing_hybrid_decision'"));
    }

    #[test]
    fn outcomes_come_from_both_scored_and_skipped_feedback() {
        let sql = outcome_query(&analytics_query(AnalyticsRoutingKind::Hybrid)).sql();
        assert!(sql.contains("'update_gateway_score_update'"));
        assert!(sql.contains("'update_gateway_score_skipped'"));
    }

    #[test]
    fn statuses_outside_the_success_and_failure_states_stay_out_of_the_rate() {
        let sql = outcome_query(&analytics_query(AnalyticsRoutingKind::Hybrid)).sql();
        assert!(sql.contains("'CHARGED'"));
        assert!(sql.contains("'AUTHORIZED'"));
        assert!(sql.contains("'AUTHORIZATION_FAILED'"));
        // `PENDING` is in neither state list, so it lands in neither count.
        assert!(!sql.contains("'PENDING'"));
    }

    #[test]
    fn dimension_filters_scope_the_decision_subquery_only() {
        let mut query = analytics_query(AnalyticsRoutingKind::Hybrid);
        query.payment_method = Some("credit".to_string());
        let sql = outcome_query(&query).sql();

        let (outer, subquery) = sql
            .split_once("payment_id IN (SELECT")
            .expect("the decided-payments subquery");
        assert!(!outer.contains("payment_method = ?"));
        assert!(subquery.contains("payment_method = ?"));
    }
}
