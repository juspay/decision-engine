use clickhouse::Row;
use serde::Deserialize;

use crate::analytics::clickhouse::common::{fetch_one, DOMAIN_TABLE};
use crate::analytics::clickhouse::query::{BoundQueryBuilder, FilterClause};
use crate::analytics::flow::FlowType;
use crate::analytics::models::{
    ExperimentTransaction, ExperimentTransactionsQuery, ExperimentTransactionsResponse,
};
use crate::error::ApiError;

/// One row per payment_id — uses argMax so the most recent event's fields win.
/// This ensures that the outcome event (emitted after the payment completes, status =
/// actual payment result) always overrides the routing event (status = routing succeeded).
#[derive(Debug, Clone, Deserialize, Row)]
struct TxnRow {
    payment_id: Option<String>,
    variant_arm: String,
    gateway: String,
    status: String,
    ts: i64,
}

#[derive(Debug, Clone, Deserialize, Row)]
struct CountRow {
    total: u64,
}

fn experiment_filter(query: &ExperimentTransactionsQuery) -> Vec<FilterClause> {
    let mut filters: Vec<FilterClause> = vec![
        FilterClause::raw(format!(
            "merchant_id = '{}'",
            query.merchant_id.replace('\'', "\\'")
        )),
        FilterClause::raw(format!(
            "flow_type = '{}'",
            FlowType::RoutingEvaluateAbTest.as_str()
        )),
        FilterClause::raw(format!(
            "JSONExtractString(assumeNotNull(details), 'experiment_id') = '{}'",
            query.experiment_id.replace('\'', "\\'")
        )),
        // Exclude Decision Explorer simulation events.
        // Real payment events are either routing intercepts (routing_source) or
        // outcome events emitted after score update (outcome_source). Include both.
        FilterClause::raw(
            "(JSONExtractString(assumeNotNull(details), 'routing_source') = 'real_payment_intercept' \
              OR JSONExtractString(assumeNotNull(details), 'outcome_source') = 'score_update')"
        ),
    ];
    if let Some(start) = query.start_ms {
        filters.push(FilterClause::raw(format!("created_at_ms >= {start}")));
    }
    filters
}

pub async fn load(
    client: &clickhouse::Client,
    query: &ExperimentTransactionsQuery,
) -> Result<ExperimentTransactionsResponse, ApiError> {
    // Count distinct payments (not events).
    let mut count_builder = BoundQueryBuilder::new(DOMAIN_TABLE);
    count_builder.add_select("uniq(payment_id) AS total");
    count_builder.extend_filters(experiment_filter(query));
    let total = fetch_one::<CountRow>(count_builder.build(client))
        .await
        .map(|r| r.total)
        .unwrap_or(0);

    // One row per payment using argMax — most recent event's fields win.
    // This ensures the outcome event (status = actual payment result) beats the
    // routing event (status = routing decision succeeded, always "success").
    let inner_filters: String = experiment_filter(query)
        .iter()
        .map(|f| f.predicate().to_string())
        .collect::<Vec<_>>()
        .join(" AND ");

    let page_size = query.page_size;
    let offset = query.page.saturating_sub(1) * page_size;

    let sql = format!(
        "SELECT
            payment_id,
            ifNull(argMax(JSONExtractString(assumeNotNull(details), 'variant_arm'), created_at_ms), '') AS variant_arm,
            ifNull(argMax(ifNull(gateway, ''), created_at_ms), '') AS gateway,
            ifNull(argMax(ifNull(status, ''), created_at_ms), '') AS status,
            max(created_at_ms) AS ts
         FROM {DOMAIN_TABLE}
         WHERE {inner_filters}
         GROUP BY payment_id
         ORDER BY ts DESC
         LIMIT {page_size} OFFSET {offset}",
    );

    let rows = client
        .query(&sql)
        .fetch_all::<TxnRow>()
        .await
        .map_err(|e| {
            crate::logger::error!(?e, "experiment_transactions fetch failed");
            ApiError::DatabaseError
        })?;

    Ok(ExperimentTransactionsResponse {
        experiment_id: query.experiment_id.clone(),
        total,
        transactions: rows
            .into_iter()
            .filter_map(|r| {
                Some(ExperimentTransaction {
                    payment_id: r.payment_id?,
                    variant_arm: r.variant_arm,
                    gateway: if r.gateway.is_empty() {
                        None
                    } else {
                        Some(r.gateway)
                    },
                    status: if r.status.is_empty() {
                        None
                    } else {
                        Some(r.status)
                    },
                    created_at_ms: r.ts,
                })
            })
            .collect(),
    })
}
