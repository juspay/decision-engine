//! Merchant-facing API for per-connector blended fees.
//!
//! Surfaces, per connector, the model-derived blended cost (rolled up from the fitted snapshot) and
//! any manual override the merchant has set, and lets them upsert/clear that override. An override
//! replaces the learned model for every EV calculation on that connector — see
//! [`crate::cost_ingestion::serving::lookup`] and `overrides`.

use std::collections::{BTreeSet, HashMap};

use axum::extract::Path;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Iso8601;

use crate::config::ClickHouseAnalyticsConfig;
use crate::cost_ingestion::blended::{self, ConnectorBlend};
use crate::cost_ingestion::overrides::{self, FeeOverride};
use crate::cost_ingestion::{creds, serving};

/// One connector's fee picture for the dashboard.
#[derive(Debug, Serialize)]
pub struct ConnectorFee {
    pub connector: String,
    /// Connector-side account (first configured), when credentials are set.
    pub account: Option<String>,
    pub has_credentials: bool,
    /// Model-derived blended fee (volume-weighted over GOOD clusters), when a fit exists.
    pub model_pct_bps: Option<f64>,
    pub model_fixed: Option<f64>,
    pub good_gross: Option<f64>,
    /// Manual override, when set.
    pub override_pct_bps: Option<f64>,
    pub override_fixed: Option<f64>,
    pub override_updated_at: Option<String>,
    /// The fee actually used at decide time, and where it comes from.
    pub effective_pct_bps: Option<f64>,
    pub effective_fixed: Option<f64>,
    /// `"override"` | `"model"` | `"none"`.
    pub source: String,
}

pub(crate) fn clickhouse_config() -> Result<ClickHouseAnalyticsConfig, (StatusCode, String)> {
    Ok(crate::app::APP_STATE
        .get()
        .ok_or((
            StatusCode::INTERNAL_SERVER_ERROR,
            "app state not initialized".to_string(),
        ))?
        .global_config
        .analytics
        .clickhouse
        .clone())
}

/// `GET /merchant-account/:merchant_id/connector-fees`
pub async fn list_connector_fees(
    Path(merchant_id): Path<String>,
) -> Result<Json<Vec<ConnectorFee>>, (StatusCode, String)> {
    let cfg = clickhouse_config()?;

    // Three sources, each keyed by lowercase connector: the fitted-model blend, manual overrides,
    // and configured credentials (for the account label + a "configured but no data yet" state).
    // Surface a ClickHouse failure as a 500 with its message — swallowing it would look like a
    // merchant with no fitted models rather than a broken query.
    let model: HashMap<String, ConnectorBlend> = blended::by_connector(&cfg, &merchant_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:?}")))?;
    let override_map: HashMap<String, FeeOverride> = overrides::list(&merchant_id)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|(c, o)| (c.to_lowercase(), o))
        .collect();
    let mut accounts: HashMap<String, String> = HashMap::new();
    let mut credentialed: BTreeSet<String> = BTreeSet::new();
    if let Ok(sources) = creds::list_sources(&merchant_id).await {
        for s in sources {
            let connector = s.connector.to_lowercase();
            credentialed.insert(connector.clone());
            // Keep the first account seen per connector for the label.
            accounts.entry(connector).or_insert(s.account);
        }
    }
    // Fall back to the account the model was fit from for connectors that have data but no stored
    // credentials (e.g. ingested via manual upload) — otherwise the dashboard can't offer their
    // per-segment drill-down. Credentials win when both exist. `has_credentials` still reflects only
    // actual stored credentials (tracked separately above).
    for (connector, blend) in &model {
        if let Some(account) = &blend.account {
            accounts.entry(connector.clone()).or_insert_with(|| account.clone());
        }
    }

    // Union of every connector that appears anywhere, so the merchant sees credentials-only and
    // override-only connectors too, not just the ones with a fit.
    let connectors: BTreeSet<String> = model
        .keys()
        .chain(override_map.keys())
        .chain(accounts.keys())
        .cloned()
        .collect();

    let fees = connectors
        .into_iter()
        .map(|connector| {
            let m = model.get(&connector);
            let ov = override_map.get(&connector);
            let (effective_pct_bps, effective_fixed, source) = match (ov, m) {
                (Some(o), _) => (Some(o.pct_bps), Some(o.fixed), "override"),
                (None, Some(b)) => (Some(b.pct_bps), Some(b.fixed), "model"),
                (None, None) => (None, None, "none"),
            };
            ConnectorFee {
                account: accounts.get(&connector).cloned(),
                has_credentials: credentialed.contains(&connector),
                model_pct_bps: m.map(|b| b.pct_bps),
                model_fixed: m.map(|b| b.fixed),
                good_gross: m.map(|b| b.good_gross),
                override_pct_bps: ov.map(|o| o.pct_bps),
                override_fixed: ov.map(|o| o.fixed),
                override_updated_at: ov.map(|o| o.updated_at.clone()),
                effective_pct_bps,
                effective_fixed,
                source: source.to_string(),
                connector,
            }
        })
        .collect();

    Ok(Json(fees))
}

#[derive(Debug, Deserialize)]
pub struct SetFeeOverrideRequest {
    pub pct_bps: f64,
    pub fixed: f64,
}

#[derive(Debug, Serialize)]
pub struct FeeOverrideResponse {
    pub merchant_id: String,
    pub connector: String,
    pub pct_bps: f64,
    pub fixed: f64,
    pub updated_at: String,
}

/// `PUT /merchant-account/:merchant_id/connectors/:connector/fee-override`
pub async fn set_fee_override(
    Path((merchant_id, connector)): Path<(String, String)>,
    Json(body): Json<SetFeeOverrideRequest>,
) -> Result<Json<FeeOverrideResponse>, (StatusCode, String)> {
    // A fee can't be negative; guard so a typo can't flip EV ranking into nonsense.
    if !body.pct_bps.is_finite() || !body.fixed.is_finite() || body.pct_bps < 0.0 || body.fixed < 0.0
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "pct_bps and fixed must be finite and non-negative".to_string(),
        ));
    }
    let connector = connector.to_lowercase();
    let updated_at = time::OffsetDateTime::now_utc()
        .format(&Iso8601::DEFAULT)
        .unwrap_or_default();
    let ov = FeeOverride {
        pct_bps: body.pct_bps,
        fixed: body.fixed,
        updated_at: updated_at.clone(),
    };
    overrides::put(&merchant_id, &connector, &ov)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:?}")))?;

    // Push the change onto the hot path now, so the next decide uses it without waiting for the
    // periodic refresh. Best-effort: the override is already persisted and will be picked up on the
    // next tick even if this inline refresh fails.
    refresh_serving(&merchant_id).await;

    Ok(Json(FeeOverrideResponse {
        merchant_id,
        connector,
        pct_bps: body.pct_bps,
        fixed: body.fixed,
        updated_at,
    }))
}

/// `DELETE /merchant-account/:merchant_id/connectors/:connector/fee-override`
pub async fn delete_fee_override(
    Path((merchant_id, connector)): Path<(String, String)>,
) -> Result<StatusCode, (StatusCode, String)> {
    let connector = connector.to_lowercase();
    overrides::delete(&merchant_id, &connector)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:?}")))?;
    refresh_serving(&merchant_id).await;
    Ok(StatusCode::NO_CONTENT)
}

/// Reload one merchant's served models (so an override edit applies immediately). Logged, not
/// surfaced — the write already succeeded and the periodic refresh is the backstop.
pub(crate) async fn refresh_serving(merchant_id: &str) {
    let Ok(cfg) = clickhouse_config() else { return };
    if let Err(e) = serving::refresh_merchant(&cfg, merchant_id).await {
        crate::logger::warn!(
            tag = "cost_serving",
            "inline refresh after override change failed for {}: {}",
            merchant_id,
            e
        );
    }
}
