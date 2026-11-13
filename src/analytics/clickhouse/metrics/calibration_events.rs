use clickhouse::Row;
use serde::Deserialize;

use crate::analytics::models::{RoutingEventsQuery, AUTOPILOT_CALIBRATION_STAGE};
use crate::error::ApiError;

use super::super::common::{fetch_all, DOMAIN_TABLE};
use super::super::filters::base_window_filters;
use super::super::query::{BoundQueryBuilder, FilterClause, OrderClause};

/// One calibration retune emitted by the SR auto-calibrator. Numeric knobs ride the
/// typed domain-event columns the calibrator packed them into (see
/// `DomainAnalyticsEvent::autopilot_calibration`): `score_value`/`sigma_factor` carry the
/// new/previous hedging %, `transaction_count`/`average_latency` the new/previous bucket.
#[derive(Debug, Clone, Deserialize, Row)]
pub struct CalibrationEventRow {
    pub created_at_ms: i64,
    pub payment_method_type: Option<String>,
    pub payment_method: Option<String>,
    pub card_network: Option<String>,
    pub currency: Option<String>,
    pub country: Option<String>,
    pub auth_type: Option<String>,
    pub new_hedge: Option<f64>,
    pub previous_hedge: Option<f64>,
    pub new_bucket: Option<i64>,
    pub previous_bucket: Option<f64>,
}

/// Load the calibration retune events a merchant's autopilot emitted in `[start_ms, end_ms]`.
/// These are real domain-event rows (tagged `event_stage = autopilot_calibration`), replayed
/// as `calibration_applied` routing events — distinct from the score-derived leader/band
/// events, which are reconstructed from the score series.
pub async fn load(
    client: &clickhouse::Client,
    query: &RoutingEventsQuery,
    start_ms: i64,
    end_ms: i64,
) -> Result<Vec<CalibrationEventRow>, ApiError> {
    let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);
    builder.extend_selects([
        "created_at_ms",
        "payment_method_type",
        "payment_method",
        "card_network",
        "currency",
        "country",
        "auth_type",
        "score_value AS new_hedge",
        "sigma_factor AS previous_hedge",
        "transaction_count AS new_bucket",
        "average_latency AS previous_bucket",
    ]);
    builder.extend_filters(base_window_filters(start_ms, end_ms));
    builder.add_filter(FilterClause::eq("merchant_id", query.merchant_id.clone()));
    builder.add_filter(FilterClause::eq(
        "event_stage",
        AUTOPILOT_CALIBRATION_STAGE.to_string(),
    ));
    if let Some(value) = &query.payment_method_type {
        builder.add_filter(FilterClause::eq("payment_method_type", value.clone()));
    }
    if let Some(value) = &query.payment_method {
        builder.add_filter(FilterClause::eq("payment_method", value.clone()));
    }
    builder.add_order_by(OrderClause::asc("created_at_ms"));

    fetch_all::<CalibrationEventRow>(builder.build(client)).await
}
