use clickhouse::Row;
use serde::Deserialize;

use crate::analytics::models::{AnalyticsQuery, RoutingFilterDimension, RoutingFilterOptions};
use crate::error::ApiError;

use super::super::common::{
    fetch_all, static_flow_type_in_sql, DOMAIN_TABLE, OVERVIEW_SCORE_FLOW_TYPES,
};
use super::super::filters::{base_window_filters, merchant_filter};
use super::super::query::{BoundQueryBuilder, FilterClause};
use super::super::time::effective_window_bounds;

#[derive(Debug, Clone, Deserialize, Row)]
struct DistinctDimensionRow {
    payment_method_type: Option<String>,
    payment_method: Option<String>,
    card_network: Option<String>,
    card_is_in: Option<String>,
    currency: Option<String>,
    country: Option<String>,
    auth_type: Option<String>,
    gateway: Option<String>,
}

pub async fn load(
    client: &clickhouse::Client,
    query: &AnalyticsQuery,
) -> Result<RoutingFilterOptions, ApiError> {
    let (start_ms, end_ms) = effective_window_bounds(query);
    let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);
    builder.extend_selects([
        "DISTINCT payment_method_type",
        "payment_method",
        "card_network",
        "card_is_in",
        "country",
        "currency",
        "auth_type",
        "gateway",
    ]);
    builder.extend_filters(base_window_filters(start_ms, end_ms));
    builder.extend_filters(merchant_filter(&query.merchant_id));
    builder.add_filter(FilterClause::raw(format!(
        "flow_type IN {}",
        static_flow_type_in_sql(OVERVIEW_SCORE_FLOW_TYPES)
    )));

    let rows = fetch_all::<DistinctDimensionRow>(builder.build(client)).await?;

    let gateways = rows
        .iter()
        .filter_map(|row| row.gateway.clone())
        .filter(|value| !value.is_empty())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();

    let mut dimensions = Vec::new();
    for (key, label) in [
        ("payment_method_type", "Payment Method Type"),
        ("payment_method", "Payment Method"),
        ("card_network", "Card Network"),
        ("card_is_in", "Card ISIN"),
        ("currency", "Currency"),
        ("country", "Country"),
        ("auth_type", "Auth Type"),
    ] {
        let values = rows
            .iter()
            .filter_map(|row| match key {
                "payment_method_type" => row.payment_method_type.clone(),
                "payment_method" => row.payment_method.clone(),
                "card_network" => row.card_network.clone(),
                "card_is_in" => row.card_is_in.clone(),
                "currency" => row.currency.clone(),
                "country" => row.country.clone(),
                "auth_type" => row.auth_type.clone(),
                _ => None,
            })
            .filter(|value| !value.is_empty())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        if !values.is_empty() {
            dimensions.push(RoutingFilterDimension {
                key: key.to_string(),
                label: label.to_string(),
                values,
            });
        }
    }

    Ok(RoutingFilterOptions {
        dimensions,
        missing_dimensions: Vec::new(),
        gateways,
    })
}
