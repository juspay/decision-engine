//! The commitment audit trail, reconstructed from stored events: forecasts and the eliminations
//! they recorded, plus the individual payments the nudge diverted.

use clickhouse::Row;
use serde::Deserialize;

use crate::analytics::flow::FlowType;
use crate::analytics::models::{
    CommitmentAnalyticsQuery, CommitmentAuditEvent, CommitmentAuditKind,
};
use crate::logger;

use super::super::common::{fetch_all, DOMAIN_TABLE};
use super::super::query::{BoundQueryBuilder, FilterClause, OrderClause};
use super::commitment_common::{amount_expr, base_filters, steered_pred};

#[derive(Debug, Deserialize, Row)]
struct ForecastEventRow {
    created_at_ms: i64,
    details: Option<String>,
}

#[derive(Debug, Deserialize, Row)]
struct SteerEventRow {
    created_at_ms: i64,
    gateway: Option<String>,
    amount: f64,
    reason: String,
    run_id: String,
}

pub async fn load(
    client: &clickhouse::Client,
    query: &CommitmentAnalyticsQuery,
) -> Vec<CommitmentAuditEvent> {
    let mut events = Vec::new();
    let merchant_id = query.merchant_id.as_str();

    // Forecast runs, which carry the eliminations — one event per controller run.
    let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);
    builder.extend_selects(["created_at_ms".to_string(), "details".to_string()]);
    base_filters(&mut builder, merchant_id, FlowType::VolumeCommitmentForecast);
    builder.add_order_by(OrderClause::desc("created_at_ms"));
    builder.set_limit(Some(query.audit_limit));
    match fetch_all::<ForecastEventRow>(builder.build(client)).await {
        Ok(rows) => {
            for row in rows {
                events.extend(forecast_row_to_events(&row));
            }
        }
        Err(error) => logger::error!(
            tag = "volume_commitment",
            merchant_id = merchant_id,
            "could not read forecast audit events from clickhouse: {error:?}"
        ),
    }

    // Steer chunks — the decide events the nudge diverted, with the reason it recorded.
    let steered = steered_pred(&query.steered_approach);
    let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);
    builder.extend_selects([
        "created_at_ms".to_string(),
        "gateway".to_string(),
        format!("{} AS amount", amount_expr(query.amount_scale)),
        "JSONExtractString(assumeNotNull(details), 'response', 'volume_steer_info', 'reason') \
         AS reason"
            .to_string(),
        "JSONExtractString(assumeNotNull(details), 'response', 'volume_steer_info', 'runId') \
         AS run_id"
            .to_string(),
    ]);
    base_filters(&mut builder, merchant_id, FlowType::DecideGatewayDecision);
    builder.add_filter(FilterClause::raw(steered));
    builder.add_order_by(OrderClause::desc("created_at_ms"));
    builder.set_limit(Some(query.audit_limit));
    match fetch_all::<SteerEventRow>(builder.build(client)).await {
        Ok(rows) => {
            for row in rows {
                events.push(CommitmentAuditEvent {
                    at_epoch_ms: row.created_at_ms,
                    kind: CommitmentAuditKind::Steered,
                    run_id: (!row.run_id.is_empty()).then(|| row.run_id.clone()),
                    connector: row.gateway.clone(),
                    message: if row.reason.is_empty() {
                        format!(
                            "Steered a payment of {:.0} to {}.",
                            row.amount,
                            row.gateway.as_deref().unwrap_or("?")
                        )
                    } else {
                        row.reason
                    },
                    amount: Some(row.amount),
                });
            }
        }
        Err(error) => logger::error!(
            tag = "volume_commitment",
            merchant_id = merchant_id,
            "could not read steer audit events from clickhouse: {error:?}"
        ),
    }

    events.sort_by_key(|e| std::cmp::Reverse(e.at_epoch_ms));
    events.truncate(usize::try_from(query.audit_limit).unwrap_or(usize::MAX));
    events
}

/// One stored forecast event into audit entries: the run itself, then one per elimination.
fn forecast_row_to_events(row: &ForecastEventRow) -> Vec<CommitmentAuditEvent> {
    let details: serde_json::Value = row
        .details
        .as_deref()
        .and_then(|d| serde_json::from_str(d).ok())
        .unwrap_or_default();
    let tracked = details["tracked"].as_u64().unwrap_or(0);
    let steering: Vec<&str> = details["steering"]
        .as_array()
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();

    let run_id = details["runId"].as_str().map(str::to_string);

    let mut events = vec![CommitmentAuditEvent {
        at_epoch_ms: row.created_at_ms,
        kind: CommitmentAuditKind::Forecast,
        run_id: run_id.clone(),
        connector: None,
        message: if steering.is_empty() {
            format!("Forecast: {tracked} commitment(s) tracked, all on pace — nothing to steer.")
        } else {
            format!(
                "Forecast: {tracked} commitment(s) tracked; {} behind pace and steering ({}).",
                steering.len(),
                steering.join(", ")
            )
        },
        amount: None,
    }];

    for dropped in details["dropped"].as_array().into_iter().flatten() {
        let connector = dropped["connector"].as_str().unwrap_or("?");
        events.push(CommitmentAuditEvent {
            at_epoch_ms: row.created_at_ms,
            kind: CommitmentAuditKind::Eliminated,
            run_id: run_id.clone(),
            connector: Some(connector.to_string()),
            message: format!(
                "{connector} eliminated: {}",
                dropped["reason"].as_str().unwrap_or("no reason recorded")
            ),
            amount: None,
        });
    }
    events
}
