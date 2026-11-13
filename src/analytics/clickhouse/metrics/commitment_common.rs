//! Shared pieces of the volume-commitment metrics: the filters every one of them starts from, and
//! the predicates that separate volume the nudge moved from volume routing delivered unaided.

use crate::analytics::flow::FlowType;

use super::super::query::{BoundQueryBuilder, FilterClause};

/// The payment amount, on the contract's own scale. Traffic reaches `/decide-gateway` in major
/// currency units and contract goals are stored in minor ones, so a commitment query cannot sum
/// the raw amount and compare it to a goal; `CommitmentAnalyticsQuery::amount_scale` carries the
/// factor between them, and is `1.0` for a contract measured in transaction counts.
pub fn amount_expr(scale: f64) -> String {
    super::super::common::payment_amount_expr(scale)
}

/// One merchant, one flow type — where every commitment query starts.
pub fn base_filters(builder: &mut BoundQueryBuilder, merchant_id: &str, flow: FlowType) {
    builder.add_filter(FilterClause::eq("merchant_id", merchant_id.to_string()));
    builder.add_filter(FilterClause::raw(format!(
        "flow_type = '{}'",
        flow.as_str()
    )));
}

/// Decisions the nudge diverted. `steered_approach` is supplied by the caller rather than read
/// from the decider's enums, which keeps this layer free of that dependency.
pub fn steered_pred(steered_approach: &str) -> String {
    format!("routing_approach = '{steered_approach}'")
}

/// Restrict to a set of connectors; a no-op on an empty list.
pub fn connector_filter(builder: &mut BoundQueryBuilder, connectors: &[String]) {
    if let Some(filter) = FilterClause::in_list("gateway", connectors) {
        builder.add_filter(filter);
    }
}

/// Float aggregates are non-nullable and can come back NaN/inf; every consumer wants zero.
pub fn nan_to_zero(value: f64) -> f64 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}
