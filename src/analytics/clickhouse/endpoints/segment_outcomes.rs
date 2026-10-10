//! Per-segment, per-gateway outcome counts for the SR v3 cold-start prior.
//!
//! Outcome events (`update_gateway_score_update`) carry only the gateway and the reported status;
//! the segment dimensions live on the decision event of the same payment. Each payment counts once
//! per gateway (its latest reported status), and only statuses in the success / failure sets are
//! counted, mirroring the auth-rate metric.

use clickhouse::Row;
use serde::Deserialize;

use crate::analytics::flow::FlowType;
use crate::analytics::store::GatewaySegmentOutcomes;
use crate::error::ApiError;
use crate::feedback::gateway_scoring_service::{txn_failure_states, txn_success_states};

use super::super::common::{fetch_all, DOMAIN_TABLE};
use super::super::metrics::auth_rate::status_in_sql;
use super::super::query::{BindArg, BoundQueryBuilder, SqlFragment};

/// Segment dimensions available on decision events.
pub const DIMS: [&str; 4] = ["card_network", "currency", "country", "auth_type"];

/// Decisions can precede the first counted outcome; look back this much further for them.
const DECISION_SLACK_MS: i64 = 3_600_000;

#[derive(Debug, Clone, Deserialize, Row)]
struct SegmentOutcomeRow {
    payment_method_type: Option<String>,
    payment_method: Option<String>,
    gateway: Option<String>,
    successes: u64,
    observations: u64,
}

pub async fn load(
    client: &clickhouse::Client,
    merchant_id: &str,
    since_ms: i64,
    active_dims: &[&str],
) -> Result<Vec<GatewaySegmentOutcomes>, ApiError> {
    let rows = fetch_all::<SegmentOutcomeRow>(
        build_query(merchant_id, since_ms, active_dims).build(client),
    )
    .await?;
    Ok(rows
        .into_iter()
        .filter_map(|row| {
            Some(GatewaySegmentOutcomes {
                payment_method_type: row.payment_method_type.filter(|s| !s.is_empty())?,
                payment_method: row.payment_method.filter(|s| !s.is_empty())?,
                gateway: row.gateway.filter(|s| !s.is_empty())?,
                successes: row.successes,
                observations: row.observations,
            })
        })
        .filter(|row| row.observations > 0)
        .collect())
}

fn build_query(merchant_id: &str, since_ms: i64, active_dims: &[&str]) -> BoundQueryBuilder {
    let dims: Vec<&str> = DIMS
        .iter()
        .copied()
        .filter(|d| active_dims.contains(d))
        .collect();
    let decision_dims = dims
        .iter()
        .map(|d| format!(", argMax({d}, created_at_ms) AS {d}"))
        .collect::<String>();
    let joined_dims = dims
        .iter()
        .map(|d| format!(", d.{d} AS {d}"))
        .collect::<String>();
    let decision_since_ms = since_ms.saturating_sub(DECISION_SLACK_MS);

    let source = format!(
        "(SELECT d.payment_method_type AS payment_method_type, d.payment_method AS payment_method, \
         o.gateway AS gateway, o.status AS status{joined_dims} \
         FROM (SELECT payment_id, gateway, argMax(status, created_at_ms) AS status \
               FROM {DOMAIN_TABLE} \
               WHERE merchant_id = ? AND flow_type = '{outcome}' \
                 AND created_at_ms >= ? AND created_at >= fromUnixTimestamp64Milli(toInt64(?)) \
                 AND payment_id IS NOT NULL AND gateway IS NOT NULL \
               GROUP BY payment_id, gateway) AS o \
         INNER JOIN (SELECT payment_id, \
                            argMax(payment_method_type, created_at_ms) AS payment_method_type, \
                            argMax(payment_method, created_at_ms) AS payment_method{decision_dims} \
               FROM {DOMAIN_TABLE} \
               WHERE merchant_id = ? AND flow_type = '{decision}' \
                 AND created_at_ms >= ? AND created_at >= fromUnixTimestamp64Milli(toInt64(?)) \
                 AND payment_id IS NOT NULL \
               GROUP BY payment_id) AS d \
         ON o.payment_id = d.payment_id)",
        outcome = FlowType::UpdateGatewayScoreUpdate.as_str(),
        decision = FlowType::DecideGatewayDecision.as_str(),
    );
    let binds: Vec<BindArg> = vec![
        merchant_id.to_string().into(),
        since_ms.into(),
        since_ms.into(),
        merchant_id.to_string().into(),
        decision_since_ms.into(),
        decision_since_ms.into(),
    ];

    let mut builder = BoundQueryBuilder::from_fragment(SqlFragment::with_binds(source, binds));
    let mut group_bys = vec![
        "payment_method_type".to_string(),
        "payment_method".to_string(),
        "gateway".to_string(),
    ];
    group_bys.extend(dims.iter().map(|d| d.to_string()));
    builder.extend_selects([
        "payment_method_type".to_string(),
        "payment_method".to_string(),
        "gateway".to_string(),
        format!(
            "countIf(status IN {}) AS successes",
            status_in_sql(&txn_success_states())
        ),
        format!(
            "countIf(status IN {} OR status IN {}) AS observations",
            status_in_sql(&txn_success_states()),
            status_in_sql(&txn_failure_states())
        ),
    ]);
    builder.extend_group_bys(group_bys);
    builder
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn query_is_scoped_to_the_merchant_in_both_subqueries() {
        let builder = build_query("m_1", 1_000_000_000, &[]);
        let sql = builder.sql();
        assert_eq!(sql.matches("merchant_id = ?").count(), 2);
        assert!(sql.contains("flow_type = 'update_gateway_score_update'"));
        assert!(sql.contains("flow_type = 'decide_gateway_decision'"));
        assert!(sql.contains("ON o.payment_id = d.payment_id"));
    }

    #[test]
    fn only_active_supported_dimensions_are_grouped() {
        let sql = build_query("m_1", 0, &["currency", "card_is_in", "udf1"]).sql();
        assert!(sql.contains("argMax(currency, created_at_ms) AS currency"));
        assert!(sql.ends_with("GROUP BY payment_method_type, payment_method, gateway, currency"));
        assert!(!sql.contains("card_is_in"));
        assert!(!sql.contains("udf1"));
        assert!(!sql.contains("card_network"));
    }

    #[test]
    fn outcomes_count_only_success_and_failure_statuses() {
        let sql = build_query("m_1", 0, &[]).sql();
        assert!(sql.contains("'CHARGED'"));
        assert!(sql.contains("'AUTHORIZATION_FAILED'"));
        assert!(!sql.contains("'PENDING_VBV'"));
    }
}
