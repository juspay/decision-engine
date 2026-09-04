//! Delivered volume per PSP, bucketed by position in that PSP's billing cycle.
//!
//! The same shape as `gateway_share`, with one difference that is the whole reason it is its own
//! metric: buckets are numbered from the instant the cycle opened, not from the wall clock, and a
//! "day" is whatever `day_secs` says it is.

use clickhouse::Row;
use serde::Deserialize;
use std::collections::HashMap;

use crate::analytics::flow::FlowType;
use crate::analytics::models::{CommitmentAnalyticsQuery, CommitmentDayVolume};
use crate::logger;

use super::super::common::{fetch_all, DOMAIN_TABLE};
use super::super::query::{BoundQueryBuilder, FilterClause};
use super::super::time::origin_bucket_select_expr;
use super::commitment_common::{
    amount_expr, base_filters, connector_filter, nan_to_zero, steered_pred,
};

#[derive(Debug, Deserialize, Row)]
struct DayVolumeRow {
    gateway: Option<String>,
    bucket_index: i64,
    total: f64,
    steered: f64,
    payments: u64,
    steered_payments: u64,
}

pub async fn load(
    client: &clickhouse::Client,
    query: &CommitmentAnalyticsQuery,
) -> Vec<CommitmentDayVolume> {
    let mut series = Vec::new();
    let per_day = i64::from(query.per_day.max(1));
    let steered = steered_pred(&query.steered_approach);
    let amount = amount_expr(query.amount_scale);

    // Connectors sharing a cycle share a query; a document mixing cycles runs one per cycle.
    let mut by_cycle: HashMap<(i64, i64, u64), Vec<String>> = HashMap::new();
    for window in &query.windows {
        by_cycle
            .entry((window.cycle_start_ms, window.cycle_end_ms, window.day_secs))
            .or_default()
            .push(window.connector.clone());
    }

    for ((cycle_start_ms, cycle_end_ms, day_secs), connectors) in by_cycle {
        let day_ms = (day_secs.max(1) as i64).saturating_mul(1000);
        // A bucket is a contract day, or a slice of one; the index is turned back into whole days
        // and a fractional position below.
        let bucket_ms = (day_ms / per_day).max(1);

        let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);
        builder.extend_selects([
            "gateway".to_string(),
            origin_bucket_select_expr(cycle_start_ms, bucket_ms),
            format!("sum({amount}) AS total"),
            format!("sumIf({amount}, {steered}) AS steered"),
            "toUInt64(count()) AS payments".to_string(),
            format!("toUInt64(countIf({steered})) AS steered_payments"),
        ]);
        base_filters(&mut builder, &query.merchant_id, FlowType::DecideGatewayDecision);
        builder.add_filter(FilterClause::raw(format!(
            "created_at_ms >= {cycle_start_ms}"
        )));
        // Bounded above too, or a later cycle's traffic lands in this one's last bucket.
        builder.add_filter(FilterClause::raw(format!("created_at_ms < {cycle_end_ms}")));
        connector_filter(&mut builder, &connectors);
        builder.extend_group_bys(["gateway", "bucket_index"]);

        match fetch_all::<DayVolumeRow>(builder.build(client)).await {
            Ok(rows) => {
                for row in rows {
                    let Some(gateway) = row.gateway else { continue };
                    let Ok(day_index) = u32::try_from(row.bucket_index / per_day) else {
                        continue;
                    };
                    series.push(CommitmentDayVolume {
                        connector: gateway,
                        day_index,
                        day: row.bucket_index as f64 / per_day as f64,
                        total: nan_to_zero(row.total),
                        steered: nan_to_zero(row.steered),
                        payments: row.payments,
                        steered_payments: row.steered_payments,
                    });
                }
            }
            Err(error) => logger::error!(
                tag = "volume_commitment",
                merchant_id = query.merchant_id.as_str(),
                "could not read the commitment volume series from clickhouse: {error:?}"
            ),
        }
    }

    series.sort_by(|a, b| a.day.total_cmp(&b.day));
    series
}
