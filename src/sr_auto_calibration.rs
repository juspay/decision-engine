//! Runtime auto-calibration of the SRv3 bucket size and hedging %.
//!
//! A periodic background job (gated per-merchant by the `sr_auto_calibration_enabled` feature
//! flag) derives both knobs purely from observed traffic — no merchant input. Volume and the
//! distinct-PSP count come from ClickHouse (off the decision hot path, zero new Redis keys);
//! the result is written back into the merchant's `SR_V3_INPUT_CONFIG_<merchant>` and applied
//! by the decider on the next decision. Bucket-size changes are non-destructive thanks to the
//! resize-in-place window (LPUSH + LTRIM), so re-tuning never wipes accumulated history.

use std::sync::Arc;
use std::time::Duration;

use futures::FutureExt;

use crate::analytics::events::DomainAnalyticsEvent;
use crate::analytics::flow::{AnalyticsFlowContext, AnalyticsRoute, ApiFlow, FlowType};
use crate::analytics::runtime::AnalyticsRuntime;
use crate::analytics::store::{AnalyticsReadStore, SegmentTraffic};
use crate::config::SrAutoCalibrationConfig;
use crate::euclid::types::SrDimensionConfig;
use crate::logger;
use crate::types::merchant_config::types::FeatureConf;
use crate::types::service_configuration::{find_config_by_name, insert_config, update_config};

/// Service-configuration key whose FeatureConf lists the merchants opted into auto-calibration
/// ("Self-tune routing settings…").
const FEATURE_CONF_KEY: &str = "sr_auto_calibration_enabled";
/// FeatureConf key for the Autopilot master toggle. Auto-calibration runs only when a merchant
/// has BOTH this and self-tuning enabled.
const AUTOPILOT_CONF_KEY: &str = "autopilot_enabled";
/// Default recalc cadence. 15 min tracks within-day traffic shifts while each tick still sees a
/// statistically meaningful volume delta and stays clear of analytics ingestion lag. Override
/// with `SR_AUTO_CALIBRATION_INTERVAL_SECS` (e.g. 60 for demos).
const DEFAULT_INTERVAL_SECS: u64 = 900;
/// Default trailing window the volume estimate is computed over (1 hour).
const DEFAULT_LOOKBACK_SECS: u64 = 3600;
/// Default minimum per-cluster volume before we trust the estimate (cold-start guard).
const DEFAULT_MIN_VOLUME: i64 = 100;
/// Only rewrite bucket size when it moves a full round-25 step (avoids churn).
const BUCKET_DEADBAND: i32 = 25;
/// Only rewrite hedging when it moves at least this many percentage points.
const HEDGE_DEADBAND_PCT: f64 = 1.0;
/// Provenance marker written on sub-level entries the calibrator creates/manages, so Hard
/// refresh can wipe only auto entries and the job can leave human-authored overrides alone.
pub const AUTOPILOT_SOURCE: &str = "autopilot";

// Tunable calibration bounds — defaults are production-safe; override per env in
// `[sr_auto_calibration]` (smaller bucket cap + short reaction horizon = fast demo reactions).
const DEFAULT_MIN_BUCKET: i32 = 100;
const DEFAULT_MAX_BUCKET: i32 = 2000;
const DEFAULT_MAX_HEDGING_PCT: f64 = 30.0;

/// Resolved calibration tuning, derived from config (with production defaults).
#[derive(Debug, Clone, Copy)]
struct CalibrationParams {
    min_bucket: i32,
    max_bucket: i32,
    max_hedging_pct: f64,
    /// Horizon hedging sizes window-refresh against. Shorter ⇒ more exploration ⇒ faster reaction.
    reaction_horizon_secs: f64,
    /// Trailing window (seconds) volume/dimensions are counted over in ClickHouse.
    lookback_secs: f64,
    /// Minimum per-cluster volume in the lookback before a cluster is calibrated.
    min_volume: i64,
}

impl CalibrationParams {
    fn from_config(cfg: &SrAutoCalibrationConfig) -> Self {
        let lookback_secs = cfg
            .lookback_secs
            .filter(|&v| v > 0)
            .unwrap_or(DEFAULT_LOOKBACK_SECS) as f64;
        Self {
            min_bucket: cfg.min_bucket_size.filter(|&v| v > 0).unwrap_or(DEFAULT_MIN_BUCKET),
            max_bucket: cfg.max_bucket_size.filter(|&v| v > 0).unwrap_or(DEFAULT_MAX_BUCKET),
            max_hedging_pct: cfg
                .max_hedging_percent
                .filter(|&v| v > 0.0)
                .unwrap_or(DEFAULT_MAX_HEDGING_PCT),
            reaction_horizon_secs: cfg
                .reaction_horizon_secs
                .filter(|&v| v > 0)
                .map(|v| v as f64)
                .unwrap_or(lookback_secs),
            lookback_secs,
            min_volume: cfg.min_volume.filter(|&v| v > 0).unwrap_or(DEFAULT_MIN_VOLUME),
        }
    }
}

fn round25(x: f64) -> i32 {
    (((x / 25.0).round()) * 25.0) as i32
}

/// `B = round₂₅(5·√(V/(n−1)))`, clamped to `[min_bucket, max_bucket]`. A small `max_bucket`
/// gives a short, fast-reacting window. (Spread-reduction `1875/D²` refinement deferred.)
fn auto_bucket(volume: i64, gateways: i64, params: CalibrationParams) -> i32 {
    if gateways < 2 || volume <= 0 {
        return params.min_bucket;
    }
    let denom = (gateways - 1) as f64;
    round25(5.0 * (volume as f64 / denom).sqrt()).clamp(params.min_bucket, params.max_bucket)
}

/// Total hedging %: per-PSP share = `clamp(1%, min(refresh_share, cap))`, capped at
/// `max_hedging_pct`. `refresh_share` is what each non-top PSP needs to refresh a window of `B`
/// within the *reaction horizon* — a short horizon raises hedging so laggards recover fast.
fn auto_hedge_pct(bucket: i32, volume: i64, gateways: i64, params: CalibrationParams) -> f64 {
    if gateways < 2 || volume <= 0 {
        return 5.0;
    }
    let n_minus_1 = (gateways - 1) as f64;
    let per_pg_cap = (params.max_hedging_pct / 100.0) / n_minus_1;
    // Volume one PSP would see over the reaction horizon at its current share, scaled from the
    // lookback window. To refresh B within the horizon: share ≥ B / volume_in_horizon.
    let volume_in_horizon = (volume as f64) * (params.reaction_horizon_secs / params.lookback_secs);
    let refresh_share = if volume_in_horizon > 0.0 {
        bucket as f64 / volume_in_horizon
    } else {
        per_pg_cap
    };
    let per_pg = refresh_share.min(per_pg_cap).max(0.01);
    let total_pct = (per_pg * n_minus_1 * 100.0).min(params.max_hedging_pct);
    (total_pct * 100.0).round() / 100.0 // round to 2 dp
}

/// Spawn the recurring calibration loop. Call once at startup, after `APP_STATE` is set.
///
/// Cadence and calibration bounds come from `[sr_auto_calibration]` (with production defaults).
pub fn spawn(runtime: Arc<AnalyticsRuntime>, config: SrAutoCalibrationConfig) {
    let interval_secs = config
        .interval_secs
        .filter(|&v| v > 0)
        .unwrap_or(DEFAULT_INTERVAL_SECS);
    let params = CalibrationParams::from_config(&config);

    tokio::spawn(async move {
        logger::info!(
            tag = "sr_auto_calibration",
            action = "start",
            "SR auto-calibration job started; interval {}s, bucket [{}, {}], max_hedge {}%, reaction_horizon {}s",
            interval_secs,
            params.min_bucket,
            params.max_bucket,
            params.max_hedging_pct,
            params.reaction_horizon_secs
        );
        let mut ticker = tokio::time::interval(Duration::from_secs(interval_secs));
        loop {
            ticker.tick().await;
            // Isolate each cycle: a panic inside `run_once` is caught here so the supervisor
            // loop keeps ticking instead of the whole job dying. Per-merchant errors are already
            // handled inside `run_once` (logged, loop continues).
            let outcome = std::panic::AssertUnwindSafe(run_once(&runtime, params))
                .catch_unwind()
                .await;
            if let Err(panic) = outcome {
                let msg = panic
                    .downcast_ref::<&str>()
                    .map(|s| s.to_string())
                    .or_else(|| panic.downcast_ref::<String>().cloned())
                    .unwrap_or_else(|| "unknown panic".to_string());
                logger::error!(
                    tag = "sr_auto_calibration",
                    action = "panic",
                    "auto-calibration cycle panicked, continuing next cycle: {}",
                    msg
                );
            }
        }
    });
}

async fn run_once(runtime: &AnalyticsRuntime, params: CalibrationParams) {
    let merchants = enrolled_merchants().await;
    if merchants.is_empty() {
        return;
    }
    let store = runtime.read_store();
    let since_ms = crate::analytics::now_ms() - (params.lookback_secs as i64) * 1000;
    for merchant_id in merchants {
        // NOTE: single-replica assumption for now — a Redis SET NX lock per merchant should be
        // added before running multiple replicas, to avoid concurrent writes of the same config.
        if let Err(reason) = calibrate_merchant(store.as_ref(), &merchant_id, since_ms, params).await {
            logger::warn!(
                tag = "sr_auto_calibration",
                action = "skip",
                "auto-calibration for {} skipped: {}",
                merchant_id,
                reason
            );
        }
    }
}

async fn enrolled_merchants() -> Vec<String> {
    // Calibrate only merchants who enabled BOTH self-tuning (this job's flag) AND Autopilot.
    let self_tuning = merchants_for_flag(FEATURE_CONF_KEY).await;
    if self_tuning.is_empty() {
        return Vec::new();
    }
    let autopilot: std::collections::HashSet<String> = merchants_for_flag(AUTOPILOT_CONF_KEY)
        .await
        .into_iter()
        .map(|m| m.to_lowercase())
        .collect();
    self_tuning
        .into_iter()
        .filter(|m| autopilot.contains(&m.to_lowercase()))
        .collect()
}

/// Merchant IDs listed in a feature flag's FeatureConf `merchants` array.
async fn merchants_for_flag(key: &str) -> Vec<String> {
    let value = match find_config_by_name(key.to_string()).await {
        Ok(Some(cfg)) => cfg.value,
        _ => None,
    };
    let Some(value) = value else {
        return Vec::new();
    };
    match serde_json::from_str::<FeatureConf>(&value) {
        Ok(conf) => conf
            .merchants
            .unwrap_or_default()
            .into_iter()
            .map(|m| m.merchantId)
            .collect(),
        Err(_) => Vec::new(),
    }
}

async fn calibrate_merchant(
    store: &dyn AnalyticsReadStore,
    merchant_id: &str,
    since_ms: i64,
    params: CalibrationParams,
) -> Result<(), String> {
    // Group at the merchant's real cluster grain: (pmt, pm) plus whatever low-cardinality
    // dimensions it clusters on (card scheme / currency / country / auth type — BIN excluded).
    let dims = active_low_card_dims(merchant_id).await;
    let dim_refs: Vec<&str> = dims.iter().map(|s| s.as_str()).collect();
    let segments = store
        .merchant_segment_traffic(merchant_id, since_ms, &dim_refs)
        .await
        .map_err(|e| format!("clickhouse query failed: {e:?}"))?;

    // v3: calibrate every cluster with enough traffic and write each as a dimension-scoped
    // sub-level override, so the values surface under "Sub-level overrides" in Manual config and
    // the decider applies them at that granularity (defaults stay as the manual fallback).
    let qualifying: Vec<_> = segments
        .into_iter()
        .filter(|s| s.volume >= params.min_volume && s.gateway_count >= 2)
        .collect();
    if qualifying.is_empty() {
        return Err("no segment with enough volume / PSPs".into());
    }

    let name = format!("SR_V3_INPUT_CONFIG_{merchant_id}");
    let existing = find_config_by_name(name.clone()).await.ok().flatten();
    let exists = existing.is_some();
    // Edit as raw JSON so fields we don't manage (defaults, margin, dimension-scoped sub-level
    // entries…) are preserved verbatim.
    let mut cfg: serde_json::Value = existing
        .and_then(|c| c.value)
        .and_then(|v| serde_json::from_str(&v).ok())
        .unwrap_or_else(|| serde_json::json!({}));

    let mut entries = cfg
        .get("subLevelInputConfig")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut changed = 0usize;
    for seg in &qualifying {
        let new_bucket = auto_bucket(seg.volume, seg.gateway_count, params);
        let new_hedge = auto_hedge_pct(new_bucket, seg.volume, seg.gateway_count, params);

        if let Some(i) = entries.iter().position(|e| segment_entry_matches(e, seg)) {
            // Respect human-authored overrides — the calibrator only manages its own entries.
            if entries[i].get("source").and_then(|v| v.as_str()) != Some(AUTOPILOT_SOURCE) {
                continue;
            }
            let cur_bucket = entries[i].get("bucketSize").and_then(|v| v.as_i64());
            let cur_hedge = entries[i].get("hedgingPercent").and_then(|v| v.as_f64());
            let bucket_changed =
                cur_bucket.map_or(true, |c| (c as i32 - new_bucket).abs() >= BUCKET_DEADBAND);
            let hedge_changed =
                cur_hedge.map_or(true, |c| (c - new_hedge).abs() >= HEDGE_DEADBAND_PCT);
            if bucket_changed || hedge_changed {
                entries[i]["bucketSize"] = serde_json::json!(new_bucket);
                entries[i]["hedgingPercent"] = serde_json::json!(new_hedge);
                entries[i]["source"] = serde_json::json!(AUTOPILOT_SOURCE);
                changed += 1;
                let prev_bucket = cur_bucket.map(|c| c as i32);
                log_segment(merchant_id, seg, prev_bucket, new_bucket, cur_hedge, new_hedge);
                emit_calibration_event(merchant_id, seg, prev_bucket, new_bucket, cur_hedge, new_hedge);
            }
        } else {
            entries.push(new_segment_entry(seg, new_bucket, new_hedge));
            changed += 1;
            log_segment(merchant_id, seg, None, new_bucket, None, new_hedge);
            emit_calibration_event(merchant_id, seg, None, new_bucket, None, new_hedge);
        }
    }

    if changed == 0 {
        return Ok(()); // every segment within deadband — skip the write to avoid churn
    }

    cfg["subLevelInputConfig"] = serde_json::Value::Array(entries);
    let serialized = cfg.to_string();

    if exists {
        update_config(name, Some(serialized))
            .await
            .map_err(|e| format!("config update failed: {e:?}"))?;
    } else {
        insert_config(name, Some(serialized))
            .await
            .map_err(|e| format!("config insert failed: {e:?}"))?;
    }

    Ok(())
}

/// Low-cardinality dimensions the calibrator will cluster on (BIN/`card_is_in` excluded — it
/// would explode into thousands of mostly sub-threshold clusters).
const LOW_CARD_DIMS: [&str; 4] = ["card_network", "currency", "country", "auth_type"];

/// The merchant's active SR dimensions, filtered to the low-cardinality set we calibrate on.
/// Empty when the merchant has no dimension config (then clustering is just (pmt, pm)).
async fn active_low_card_dims(merchant_id: &str) -> Vec<String> {
    let fields = match find_config_by_name(format!("SR_DIMENSION_CONFIG_{merchant_id}")).await {
        Ok(Some(cfg)) => cfg
            .value
            .and_then(|v| serde_json::from_str::<SrDimensionConfig>(&v).ok())
            .and_then(|c| c.paymentInfo.fields)
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    fields
        .into_iter()
        .filter(|f| LOW_CARD_DIMS.contains(&f.as_str()))
        .collect()
}

/// Build a dimension-scoped sub-level entry for a cluster, setting only the dimensions present.
fn new_segment_entry(seg: &SegmentTraffic, bucket: i32, hedge: f64) -> serde_json::Value {
    let mut entry = serde_json::json!({
        "paymentMethodType": seg.payment_method_type,
        "paymentMethod": seg.payment_method,
        "bucketSize": bucket,
        "hedgingPercent": hedge,
        "source": AUTOPILOT_SOURCE,
    });
    if let Some(v) = &seg.card_network {
        entry["cardNetwork"] = serde_json::json!(v);
    }
    if let Some(v) = &seg.currency {
        entry["currency"] = serde_json::json!(v);
    }
    if let Some(v) = &seg.country {
        entry["country"] = serde_json::json!(v);
    }
    if let Some(v) = &seg.auth_type {
        entry["authType"] = serde_json::json!(v);
    }
    entry
}

/// True when `entry` is exactly this cluster: same (pmt, pm) and every dimension matches
/// (case-insensitive), with absent/empty == "not set". `cardIsIn` must be unset since the
/// calibrator never manages BIN-scoped entries — so it never clobbers them.
fn segment_entry_matches(entry: &serde_json::Value, seg: &SegmentTraffic) -> bool {
    dim_matches(entry, "paymentMethodType", Some(&seg.payment_method_type))
        && dim_matches(entry, "paymentMethod", Some(&seg.payment_method))
        && dim_matches(entry, "cardNetwork", seg.card_network.as_deref())
        && dim_matches(entry, "cardIsIn", None)
        && dim_matches(entry, "currency", seg.currency.as_deref())
        && dim_matches(entry, "country", seg.country.as_deref())
        && dim_matches(entry, "authType", seg.auth_type.as_deref())
}

fn dim_matches(entry: &serde_json::Value, key: &str, expected: Option<&str>) -> bool {
    let stored = entry
        .get(key)
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty());
    match (stored, expected) {
        (Some(s), Some(e)) => s.eq_ignore_ascii_case(e),
        (None, None) => true,
        _ => false,
    }
}

/// Surface a calibration retune in the routing-events feed (as a `calibration_applied`
/// event) so it shows up in the multi-objective simulation UI's Autopilot Actions panel.
/// Fire-and-forget: a no-op when the analytics write path is disabled, and it never blocks
/// or fails the calibration write.
fn emit_calibration_event(
    merchant_id: &str,
    seg: &SegmentTraffic,
    prev_bucket: Option<i32>,
    new_bucket: i32,
    prev_hedge: Option<f64>,
    new_hedge: f64,
) {
    DomainAnalyticsEvent::record_autopilot_calibration(
        AnalyticsFlowContext::new(ApiFlow::DynamicRouting, FlowType::AutopilotCalibration),
        AnalyticsRoute::UpdateGatewayScore,
        Some(merchant_id.to_string()),
        Some(seg.payment_method_type.clone()),
        Some(seg.payment_method.clone()),
        seg.card_network.clone(),
        seg.currency.clone(),
        seg.country.clone(),
        seg.auth_type.clone(),
        new_bucket,
        prev_bucket,
        new_hedge,
        prev_hedge,
    );
}

fn log_segment(
    merchant_id: &str,
    seg: &crate::analytics::store::SegmentTraffic,
    cur_bucket: Option<i32>,
    new_bucket: i32,
    cur_hedge: Option<f64>,
    new_hedge: f64,
) {
    logger::info!(
        tag = "sr_auto_calibration",
        action = "applied",
        "auto-calibrated {} [{}/{}] V={} n={}: bucket {:?}->{}, hedging {:?}->{:.2}%",
        merchant_id,
        seg.payment_method_type,
        seg.payment_method,
        seg.volume,
        seg.gateway_count,
        cur_bucket,
        new_bucket,
        cur_hedge,
        new_hedge
    );
}
