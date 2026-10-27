use error_stack::Result;
use std::collections::HashMap;

use clickhouse::Row;
use serde::Deserialize;

use crate::analytics::flow::FlowType;
use crate::analytics::models::PaymentAuditExperiment;
use crate::error::ApiError;

use super::super::common::{fetch_all, DOMAIN_TABLE};
use super::super::filters::merchant_filter;
use super::super::query::{BoundQueryBuilder, FilterClause, OrderClause};

#[derive(Debug, Clone, Deserialize, Row)]
struct AssignmentRow {
    lookup_key: String,
    experiment_id: String,
    arm: String,
    endpoint: String,
    routed_at_ms: i64,
}

/// The A/B experiments that routed each payment behind `lookup_keys`, one entry per experiment,
/// arm and endpoint, from the routing events the experiments recorded. A key with no entry was
/// never routed under an experiment.
///
/// These events stay out of the audit timeline's flow-type families on purpose: a family drives
/// the payment's event counts and summaries, which an experiment assignment is not part of.
pub async fn load(
    client: &clickhouse::Client,
    merchant_id: &str,
    lookup_keys: &[String],
) -> Result<HashMap<String, Vec<PaymentAuditExperiment>>, ApiError> {
    let Some(keys) = FilterClause::in_list("lookup_key", lookup_keys) else {
        return Ok(HashMap::new());
    };
    let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);
    builder.extend_selects([
        "assumeNotNull(lookup_key) AS lookup_key",
        "JSONExtractString(assumeNotNull(details), 'experiment_id') AS experiment_id",
        "JSONExtractString(assumeNotNull(details), 'variant_arm') AS arm",
        "JSONExtractString(assumeNotNull(details), 'endpoint') AS endpoint",
        "min(created_at_ms) AS routed_at_ms",
    ]);
    builder.extend_filters(merchant_filter(merchant_id));
    builder.add_filter(keys);
    builder.add_filter(FilterClause::raw(format!(
        "flow_type = '{}'",
        FlowType::RoutingEvaluateAbTest.as_str()
    )));
    // Real payments only: a Decision Explorer preview carries no routing_source.
    builder.add_filter(FilterClause::raw(
        "JSONExtractString(assumeNotNull(details), 'routing_source') = 'real_payment_intercept'",
    ));
    builder.extend_group_bys(["lookup_key", "experiment_id", "arm", "endpoint"]);
    builder.add_order_by(OrderClause::asc("routed_at_ms"));

    let rows = fetch_all::<AssignmentRow>(builder.build(client)).await?;
    let mut assignments: HashMap<String, Vec<PaymentAuditExperiment>> = HashMap::new();
    for row in rows {
        assignments
            .entry(row.lookup_key)
            .or_default()
            .push(PaymentAuditExperiment {
                experiment_id: row.experiment_id,
                arm: row.arm,
                // Routing events from before endpoint tagging came from the decider intercept.
                endpoint: if row.endpoint.is_empty() {
                    "decide_gateway".to_string()
                } else {
                    row.endpoint
                },
                routed_at_ms: row.routed_at_ms,
            });
    }
    Ok(assignments)
}
