use clickhouse::Row;
use serde::Deserialize;

use crate::analytics::flow::FlowType;
use crate::analytics::store::SegmentTraffic;
use crate::error::ApiError;

use super::super::common::{fetch_all, DOMAIN_TABLE};
use super::super::query::{BoundQueryBuilder, FilterClause};

/// Low-cardinality cluster dimensions the calibrator can group on (BIN is intentionally
/// excluded). Order here defines the SELECT/Row column order.
const DIMS: [&str; 4] = ["card_network", "currency", "country", "auth_type"];

#[derive(Debug, Clone, Deserialize, Row)]
struct SegmentTrafficRow {
    payment_method_type: Option<String>,
    payment_method: Option<String>,
    card_network: Option<String>,
    currency: Option<String>,
    country: Option<String>,
    auth_type: Option<String>,
    volume: u64,
    gateway_count: u64,
}

fn norm(v: Option<String>) -> Option<String> {
    v.filter(|s| !s.is_empty())
}

/// Per-cluster decision volume + distinct-PSP count for a merchant since `since_ms`, grouped by
/// (pmt, pm) plus the `active_dims` the merchant clusters on. Inactive dimensions are selected as
/// a typed NULL (kept in the same column position) so they don't fragment the grouping. Derived
/// entirely from analytics, so the decision hot path and Redis are untouched.
pub async fn load(
    client: &clickhouse::Client,
    merchant_id: &str,
    since_ms: i64,
    active_dims: &[&str],
) -> Result<Vec<SegmentTraffic>, ApiError> {
    let mut builder = BoundQueryBuilder::new(DOMAIN_TABLE);

    let mut selects: Vec<String> = vec![
        "payment_method_type".to_string(),
        "payment_method".to_string(),
    ];
    let mut group_bys: Vec<String> = vec![
        "payment_method_type".to_string(),
        "payment_method".to_string(),
    ];
    for col in DIMS {
        if active_dims.contains(&col) {
            selects.push(col.to_string());
            group_bys.push(col.to_string());
        } else {
            // Typed NULL keeps the column position stable for RowBinary decoding.
            selects.push(format!("CAST(NULL AS Nullable(String)) AS {col}"));
        }
    }
    selects.push("count() AS volume".to_string());
    selects.push("uniqExact(gateway) AS gateway_count".to_string());

    builder.extend_selects(selects);
    builder.add_filter(FilterClause::eq("merchant_id", merchant_id.to_string()));
    builder.add_filter(FilterClause::raw(format!("created_at_ms >= {since_ms}")));
    builder.add_filter(FilterClause::raw(format!(
        "flow_type = '{}'",
        FlowType::DecideGatewayDecision.as_str()
    )));
    builder.extend_group_bys(group_bys);

    let rows = fetch_all::<SegmentTrafficRow>(builder.build(client)).await?;
    Ok(rows
        .into_iter()
        .filter_map(|row| {
            Some(SegmentTraffic {
                payment_method_type: row.payment_method_type?,
                payment_method: row.payment_method?,
                card_network: norm(row.card_network),
                currency: norm(row.currency),
                country: norm(row.country),
                auth_type: norm(row.auth_type),
                volume: row.volume as i64,
                gateway_count: row.gateway_count as i64,
            })
        })
        .collect())
}
