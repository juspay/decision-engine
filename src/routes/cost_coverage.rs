//! Cost-model coverage endpoint backing the dashboard health card.

use crate::custom_extractors::MerchantFromPath;

use axum::http::StatusCode;
use axum::Json;

use crate::cost_ingestion::coverage::{self, CoverageSummary};
use crate::error::TextError;

/// `GET /merchant-account/:merchant_id/cost-coverage`
pub async fn get_cost_coverage(
    MerchantFromPath(merchant): MerchantFromPath,
) -> Result<Json<CoverageSummary>, TextError> {
    let merchant_id = merchant.as_str().to_string();
    // ClickHouse config lives on the global config (not the per-tenant config).
    let clickhouse = crate::app::APP_STATE
        .get()
        .ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            "app state not initialized".to_string(),
        ))?
        .global_config
        .analytics
        .clickhouse
        .clone();

    let summary = coverage::for_merchant(&clickhouse, &merchant_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:?}")))?;
    Ok(Json(summary))
}
