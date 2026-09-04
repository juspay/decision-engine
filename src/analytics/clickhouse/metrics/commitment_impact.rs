//! Per-PSP totals over the cycle, split into what routing delivered, what the nudge moved *to* a
//! PSP, and what a PSP gave up *to* the nudge.

use clickhouse::Row;
use serde::Deserialize;
use std::collections::HashMap;

use crate::analytics::flow::FlowType;
use crate::analytics::models::{CommitmentAnalyticsQuery, CommitmentWindowTotals};
use crate::logger;

use super::super::common::{fetch_all, DOMAIN_TABLE};
use super::super::query::{BoundQueryBuilder, FilterClause};
use super::commitment_common::{
    amount_expr, base_filters, connector_filter, nan_to_zero, steered_pred,
};

#[derive(Debug, Deserialize, Row)]
struct WindowRow {
    gateway: Option<String>,
    payments: u64,
    volume: f64,
    steered_payments: u64,
    steered_volume: f64,
}

/// What a PSP lost to steering, keyed by the PSP routing had actually picked.
#[derive(Debug, Deserialize, Row)]
struct CededRow {
    sr_head: String,
    payments: u64,
    volume: f64,
}

pub async fn load(
    client: &clickhouse::Client,
    query: &CommitmentAnalyticsQuery,
) -> Vec<CommitmentWindowTotals> {
    let merchant_id = query.merchant_id.as_str();
    let steered = steered_pred(&query.steered_approach);
    let amount = amount_expr(query.amount_scale);

    // Connectors sharing a cycle share a pair of queries; a document mixing cycles runs a pair per
    // cycle, so each PSP's totals cover its own period and none of the one before it.
    let mut by_cycle: HashMap<(i64, i64), Vec<String>> = HashMap::new();
    for window in &query.windows {
        if window.cycle_end_ms <= window.cycle_start_ms {
            continue;
        }
        by_cycle
            .entry((window.cycle_start_ms, window.cycle_end_ms))
            .or_default()
            .push(window.connector.clone());
    }

    let mut by_connector: HashMap<String, CommitmentWindowTotals> = HashMap::new();

    for ((start_ms, end_ms), connectors) in by_cycle {
        let window_filters = |builder: &mut BoundQueryBuilder| {
            base_filters(builder, merchant_id, FlowType::DecideGatewayDecision);
            builder.add_filter(FilterClause::raw(format!("created_at_ms >= {start_ms}")));
            builder.add_filter(FilterClause::raw(format!("created_at_ms < {end_ms}")));
        };

        // What landed on each PSP, and how much of it the nudge put there.
        let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);
        builder.extend_selects([
            "gateway".to_string(),
            "toUInt64(count()) AS payments".to_string(),
            format!("sum({amount}) AS volume"),
            format!("toUInt64(countIf({steered})) AS steered_payments"),
            format!("sumIf({amount}, {steered}) AS steered_volume"),
        ]);
        window_filters(&mut builder);
        connector_filter(&mut builder, &connectors);
        builder.extend_group_bys(["gateway"]);
        match fetch_all::<WindowRow>(builder.build(client)).await {
            Ok(rows) => {
                for row in rows {
                    let Some(gateway) = row.gateway else { continue };
                    let entry = by_connector.entry(gateway.clone()).or_default();
                    entry.connector = gateway;
                    entry.payments = row.payments;
                    entry.volume = nan_to_zero(row.volume);
                    entry.steered_payments = row.steered_payments;
                    entry.steered_volume = nan_to_zero(row.steered_volume);
                }
            }
            Err(error) => {
                logger::error!(
                    tag = "volume_commitment",
                    merchant_id = merchant_id,
                    "could not read commitment window totals from clickhouse: {error:?}"
                );
                continue;
            }
        }

        // What each PSP gave up: steered decisions name the PSP routing had picked (`srHead`).
        let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);
        builder.extend_selects([
            "JSONExtractString(assumeNotNull(details), 'response', 'volume_steer_info', 'srHead') \
             AS sr_head"
                .to_string(),
            "toUInt64(count()) AS payments".to_string(),
            format!("sum({amount}) AS volume"),
        ]);
        window_filters(&mut builder);
        builder.add_filter(FilterClause::raw(steered.clone()));
        builder.extend_group_bys(["sr_head"]);
        match fetch_all::<CededRow>(builder.build(client)).await {
            Ok(rows) => {
                for row in rows {
                    // Only what this window's own connectors gave up — another cycle's PSP is
                    // counted against its own window, not this one.
                    if !connectors.contains(&row.sr_head) {
                        continue;
                    }
                    let entry = by_connector.entry(row.sr_head.clone()).or_default();
                    entry.connector = row.sr_head;
                    entry.ceded_payments = row.payments;
                    entry.ceded_volume = nan_to_zero(row.volume);
                }
            }
            Err(error) => logger::error!(
                tag = "volume_commitment",
                merchant_id = merchant_id,
                "could not read ceded volume from clickhouse: {error:?}"
            ),
        }
    }

    let mut totals: Vec<CommitmentWindowTotals> = by_connector.into_values().collect();
    totals.sort_by(|a, b| a.connector.cmp(&b.connector));
    totals
}
