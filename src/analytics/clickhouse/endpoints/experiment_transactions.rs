use clickhouse::Row;
use serde::Deserialize;

use super::experiment_results::{experiment_payments, ExperimentScope};
use crate::analytics::clickhouse::common::{fetch_all, fetch_one};
use crate::analytics::clickhouse::query::{BoundQueryBuilder, OrderClause};
use crate::analytics::models::{
    ExperimentTransaction, ExperimentTransactionsQuery, ExperimentTransactionsResponse,
};
use crate::error::ApiError;

/// One row per payment, with its outcome joined from the score updates reported for it.
#[derive(Debug, Clone, Deserialize, Row)]
struct TxnRow {
    payment_id: String,
    variant_arm: String,
    gateway: String,
    /// `success` or `failure` once a terminal status was reported, else the routing status.
    status: String,
    ts: i64,
}

#[derive(Debug, Clone, Deserialize, Row)]
struct CountRow {
    total: u64,
}

fn scope(query: &ExperimentTransactionsQuery) -> ExperimentScope<'_> {
    ExperimentScope {
        merchant_id: &query.merchant_id,
        experiment_id: &query.experiment_id,
        endpoint: query.endpoint,
        start_ms: query.start_ms,
        end_ms: None,
    }
}

pub async fn load(
    client: &clickhouse::Client,
    query: &ExperimentTransactionsQuery,
) -> Result<ExperimentTransactionsResponse, ApiError> {
    // Count distinct payments (not events).
    let mut count_builder = BoundQueryBuilder::from_fragment(experiment_payments(&scope(query)));
    count_builder.add_select("uniqExact(payment_id) AS total");
    let total = fetch_one::<CountRow>(count_builder.build(client))
        .await
        .map(|r| r.total)
        .unwrap_or(0);

    // A payment routed more than once shows its latest arm and gateway.
    let mut builder = BoundQueryBuilder::from_fragment(experiment_payments(&scope(query)));
    builder.extend_selects([
        "payment_id",
        "argMax(arm, last_event_ms) AS variant_arm",
        "argMax(gateway, last_event_ms) AS gateway",
        "multiIf(max(succeeded) = 1, 'success', max(resolved) = 1, 'failure', 'ab_test_decision') AS status",
        "max(last_event_ms) AS ts",
    ]);
    builder.add_group_by("payment_id");
    builder.add_order_by(OrderClause::desc("ts"));
    builder.set_limit(Some(query.page_size));
    builder.set_offset(Some(query.page.saturating_sub(1) * query.page_size));

    let rows = fetch_all::<TxnRow>(builder.build(client)).await?;

    Ok(ExperimentTransactionsResponse {
        experiment_id: query.experiment_id.clone(),
        total,
        transactions: rows
            .into_iter()
            .map(|r| ExperimentTransaction {
                payment_id: r.payment_id,
                variant_arm: r.variant_arm,
                gateway: (!r.gateway.is_empty()).then_some(r.gateway),
                status: (!r.status.is_empty()).then_some(r.status),
                created_at_ms: r.ts,
            })
            .collect(),
    })
}
