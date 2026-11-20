use crate::custom_extractors::{Json, MerchantFromPath, ScopedJson};
use crate::types::service_configuration::MerchantConfig;
use error_stack::ResultExt;
use serde::{Deserialize, Serialize};

use crate::custom_extractors::TenantStateResolver;
use crate::decider::gatewaydecider::constants as C;
use crate::error;
use crate::logger;

#[derive(Debug, Deserialize, crate::auth::MerchantScoped)]
pub struct ResetGatewayScoresRequest {
    /// Merchant whose gateway (SR) scores should be flushed. Accepts both the
    /// snake_case and the camelCase the dashboard sends.
    #[merchant]
    #[serde(rename = "merchant_id", alias = "merchantId")]
    pub merchant_id: String,
}

#[derive(Debug, Serialize)]
pub struct ResetGatewayScoresResponse {
    pub merchant_id: String,
    pub deleted_keys: usize,
    pub removed_overrides: usize,
}

/// Flush all SR (gateway-selection) score/queue keys for a single merchant from Redis.
///
/// Scoped to the given merchant only — the SR v2/v3 keys embed the merchant id right
/// after their prefix (`<prefix>_<merchant_id>_...`), so a `<prefix>_<merchant_id>_*`
/// glob matches every cluster (payment method, card network, etc.) for that merchant
/// and nothing belonging to anyone else. Used by the simulator's "Hard refresh" so a
/// new run starts from fresh scores without touching other merchants' data.
pub async fn reset_gateway_scores(
    TenantStateResolver(state): TenantStateResolver,
    ScopedJson { merchant, .. }: ScopedJson<ResetGatewayScoresRequest>,
) -> Result<Json<ResetGatewayScoresResponse>, error::ContainerError<error::ApiError>> {
    let merchant_id = merchant.as_str().trim().to_string();
    if merchant_id.is_empty() {
        return Err(error::ApiError::MissingRequiredField("merchant_id").into());
    }

    // Both SR v2 and SR v3 (the multi-objective path) share the
    // `<prefix>_<merchant_id>_...` layout, with `_}score` / `_}queue` suffixes.
    let patterns = [
        format!(
            "{}_{}_*",
            C::GATEWAY_SELECTION_V3_ORDER_TYPE_KEY_PREFIX,
            merchant_id
        ),
        format!(
            "{}_{}_*",
            C::GATEWAY_SELECTION_ORDER_TYPE_KEY_PREFIX,
            merchant_id
        ),
    ];

    let mut deleted_keys = 0usize;
    for pattern in &patterns {
        deleted_keys += state
            .redis_conn
            .delete_keys_by_pattern(pattern)
            .await
            .change_context(error::ApiError::UnknownError)?;
    }

    // Also drop auto-calibrated sub-level overrides so the next run starts from a clean config;
    // human-authored overrides (no `source: autopilot` marker) are preserved.
    let removed_overrides = clear_autopilot_sublevel_overrides(&merchant_id).await;

    logger::info!(
        action = "RESET_GATEWAY_SCORES",
        tag = "RESET_GATEWAY_SCORES",
        "Flushed {} SR score keys and {} auto sub-level overrides for merchant {}",
        deleted_keys,
        removed_overrides,
        merchant_id
    );

    Ok(Json(ResetGatewayScoresResponse {
        merchant_id,
        deleted_keys,
        removed_overrides,
    }))
}

/// One SR v3 score window: the newest outcomes the router reads for one gateway in one segment.
#[derive(Debug, Serialize)]
pub struct GatewayScoreWindow {
    /// The score key between the merchant id and the `_}queue` suffix: the segment's order type,
    /// payment method type, payment method, dimensions and UDFs, then the gateway, joined by `_`.
    /// Gateway names can contain `_`, so the split between segment and gateway is left to callers
    /// that know the gateway names.
    pub series: String,
    pub success_count: usize,
    pub total_count: usize,
    /// `success_count / total_count`, as the router derives a gateway's score from its window.
    pub success_rate: f64,
}

#[derive(Debug, Serialize)]
pub struct GatewayScoresResponse {
    pub merchant_id: String,
    pub windows: Vec<GatewayScoreWindow>,
}

/// The merchant's live SR v3 score windows from Redis, one per segment and gateway. Each is the
/// list `/update-gateway-score` pushes outcomes onto (`1` success, `0` failure), trimmed to the
/// bucket size, which every payment routed or reported in that segment shares.
pub async fn list_gateway_scores(
    TenantStateResolver(state): TenantStateResolver,
    MerchantFromPath(merchant): MerchantFromPath,
) -> Result<Json<GatewayScoresResponse>, error::ContainerError<error::ApiError>> {
    let merchant_id = merchant.as_str().to_string();
    let key_start = format!(
        "{}_{}_",
        C::GATEWAY_SELECTION_V3_ORDER_TYPE_KEY_PREFIX,
        merchant_id
    );
    const QUEUE_SUFFIX: &str = "_}queue";

    let mut keys = state
        .redis_conn
        .keys_by_pattern(&format!("{key_start}*{QUEUE_SUFFIX}"))
        .await
        .change_context(error::ApiError::UnknownError)?;
    keys.sort();

    let mut windows = Vec::with_capacity(keys.len());
    for key in keys {
        let Some(series) = key
            .strip_prefix(&key_start)
            .and_then(|rest| rest.strip_suffix(QUEUE_SUFFIX))
        else {
            continue;
        };
        let outcomes = state
            .redis_conn
            .get_list_range(&key, 0, -1)
            .await
            .change_context(error::ApiError::UnknownError)?;
        let scores: Vec<f64> = outcomes.iter().filter_map(|v| v.parse().ok()).collect();
        if scores.is_empty() {
            continue;
        }
        // The same arithmetic as `get_score_from_redis`: the window's mean outcome.
        let success_sum: f64 = scores.iter().sum();
        windows.push(GatewayScoreWindow {
            series: series.to_string(),
            success_count: success_sum.round() as usize,
            total_count: scores.len(),
            success_rate: (success_sum / scores.len() as f64).clamp(0.0, 1.0),
        });
    }

    Ok(Json(GatewayScoresResponse {
        merchant_id,
        windows,
    }))
}

/// Remove auto-calibrated (`source: autopilot`) sub-level overrides from the merchant's SR config,
/// preserving human-authored entries. Best-effort: returns the count removed (0 on any miss/error).
async fn clear_autopilot_sublevel_overrides(merchant_id: &str) -> usize {
    use crate::types::service_configuration::{find_config_by_name, update_config};

    let name = crate::types::service_configuration::SrV3Input::name(merchant_id);
    let value = match find_config_by_name(name.clone()).await {
        Ok(Some(cfg)) => cfg.value,
        _ => None,
    };
    let Some(value) = value else { return 0 };
    let Ok(mut cfg) = serde_json::from_str::<serde_json::Value>(&value) else {
        return 0;
    };
    let Some(arr) = cfg
        .get("subLevelInputConfig")
        .and_then(|v| v.as_array())
        .cloned()
    else {
        return 0;
    };
    let before = arr.len();
    let kept: Vec<serde_json::Value> = arr
        .into_iter()
        .filter(|e| {
            e.get("source").and_then(|v| v.as_str())
                != Some(crate::sr_auto_calibration::AUTOPILOT_SOURCE)
        })
        .collect();
    let removed = before - kept.len();
    if removed == 0 {
        return 0;
    }
    cfg["subLevelInputConfig"] = serde_json::Value::Array(kept);
    if update_config(name, Some(cfg.to_string())).await.is_err() {
        return 0;
    }
    removed
}
