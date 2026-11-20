//! Admin-only API for how long a merchant's scoring contexts stay in Redis.

use crate::auth::AuthorizedMerchant;
use crate::custom_extractors::{Json, MerchantFromPath};
use crate::error::RouteError;
use crate::generics::MeshError;
use crate::types::service_configuration::{self, ScoringDataTtl};
use serde::{Deserialize, Serialize};

/// Body of the PUT: the merchant's new TTL in seconds.
#[derive(Debug, Deserialize)]
pub struct SetScoringDataTtlRequest {
    pub ttl_secs: i64,
}

/// The merchant's TTL in effect, with what it was resolved from.
#[derive(Debug, Serialize)]
pub struct ScoringDataTtlResponse {
    pub merchant_id: String,
    /// The TTL new scoring contexts get, in seconds.
    pub ttl_secs: i64,
    /// The merchant's own TTL, if it has one. One above `max_ttl_secs` is cut to that.
    pub merchant_ttl_secs: Option<i64>,
    /// `[scoring_data] default_ttl_secs`: the TTL of a merchant without its own.
    pub default_ttl_secs: i64,
    /// `[scoring_data] max_ttl_secs`: no merchant's contexts stay longer.
    pub max_ttl_secs: i64,
}

// Builds the response from the merchant's stored TTL and the deployment config.
async fn response(merchant_id: &str, merchant_ttl_secs: Option<i64>) -> ScoringDataTtlResponse {
    let config = crate::app::get_tenant_app_state()
        .await
        .config
        .scoring_data
        .clone();
    ScoringDataTtlResponse {
        merchant_id: merchant_id.to_string(),
        ttl_secs: config.ttl_secs(merchant_ttl_secs),
        merchant_ttl_secs,
        default_ttl_secs: config.default_ttl_secs,
        max_ttl_secs: config.max_ttl_secs,
    }
}

// A TTL is kept by merchant id whether or not the merchant exists, so an unknown one is a 404.
async fn ensure_merchant_exists(merchant: &AuthorizedMerchant) -> Result<(), RouteError> {
    crate::types::merchant::merchant_account::load_authorized(merchant)
        .await
        .map(|_| ())
        .ok_or_else(|| RouteError::not_found("MerchantId not found"))
}

/// `GET /admin/merchant-account/:merchant_id/scoring-data-ttl`
pub async fn get_scoring_data_ttl(
    MerchantFromPath(merchant): MerchantFromPath,
) -> Result<Json<ScoringDataTtlResponse>, RouteError> {
    ensure_merchant_exists(&merchant).await?;
    let stored = service_configuration::get::<ScoringDataTtl>(&merchant)
        .await
        .map_err(RouteError::internal)?;
    Ok(Json(response(merchant.as_str(), stored).await))
}

/// `PUT /admin/merchant-account/:merchant_id/scoring-data-ttl`: above the max applies as the max.
pub async fn set_scoring_data_ttl(
    MerchantFromPath(merchant): MerchantFromPath,
    Json(body): Json<SetScoringDataTtlRequest>,
) -> Result<Json<ScoringDataTtlResponse>, RouteError> {
    ensure_merchant_exists(&merchant).await?;
    if body.ttl_secs < 1 {
        return Err(RouteError::bad_request("ttl_secs must be at least 1"));
    }
    service_configuration::upsert::<ScoringDataTtl>(&merchant, &body.ttl_secs)
        .await
        .map_err(RouteError::internal)?;
    Ok(Json(response(merchant.as_str(), Some(body.ttl_secs)).await))
}

/// `DELETE /admin/merchant-account/:merchant_id/scoring-data-ttl`: back to the default TTL.
pub async fn delete_scoring_data_ttl(
    MerchantFromPath(merchant): MerchantFromPath,
) -> Result<Json<ScoringDataTtlResponse>, RouteError> {
    ensure_merchant_exists(&merchant).await?;
    // Nothing stored means the merchant is already on the default, so a repeat DELETE succeeds.
    if let Err(error) = service_configuration::delete::<ScoringDataTtl>(&merchant).await {
        if !matches!(
            error.downcast_ref::<MeshError>(),
            Some(MeshError::NoRowstoDelete)
        ) {
            return Err(RouteError::internal(error));
        }
    }
    Ok(Json(response(merchant.as_str(), None).await))
}
