//! Cost-model coverage endpoint backing the dashboard health card.

use crate::custom_extractors::MerchantFromPath;

use axum::Json;

use crate::cost_ingestion::coverage::{self, CoverageSummary};
use crate::error::RouteError;

/// `GET /merchant-account/:merchant_id/cost-coverage`
pub async fn get_cost_coverage(
    MerchantFromPath(merchant): MerchantFromPath,
) -> Result<Json<CoverageSummary>, RouteError> {
    // ClickHouse config lives on the global config (not the per-tenant config).
    let clickhouse = &super::global_config()?.analytics.clickhouse;

    let summary = coverage::for_merchant(clickhouse, merchant.as_str())
        .await
        .map_err(RouteError::internal)?;
    Ok(Json(summary))
}
