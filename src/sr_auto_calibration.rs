//! Runtime auto-calibration of the SRv3 bucket size and hedging %.
//!
//! A periodic background job (gated per-merchant by the `autopilot_enabled` feature
//! flag) derives both knobs purely from observed traffic — no merchant input. Volume and the
//! distinct-PSP count come from ClickHouse (off the decision hot path, zero new Redis keys);
//! the result is written back into the merchant's `SR_V3_INPUT_CONFIG_<merchant>` and applied
//! by the decider on the next decision. Bucket-size changes are non-destructive thanks to the
//! resize-in-place window (LPUSH + LTRIM), so re-tuning never wipes accumulated history.
//!
//! The same loop refreshes the SR v3 cold-start prior for merchants listed under
//! `ENABLE_SR_V3_COLD_START_PRIOR` (independently of `autopilot_enabled`): per-gateway parent
//! success rates and, for merchants also under `ENABLE_SR_V3_LEARNED_PRIOR_STRENGTH`, an
//! empirical-Bayes prior strength, written to the job-owned `SR_V3_PRIOR_<merchant>` config. The
//! merchant's `SR_V3_INPUT_CONFIG_*` (including a manual `defaultPriorStrength`) is never touched
//! by this step. See `decider::gatewaydecider::sr_prior`.

use std::sync::Arc;
use std::time::Duration;

use futures::FutureExt;

use crate::analytics::clickhouse::endpoints::segment_outcomes;
use crate::analytics::events::DomainAnalyticsEvent;
use crate::analytics::flow::{AnalyticsFlowContext, AnalyticsRoute, ApiFlow, FlowType};
use crate::analytics::runtime::AnalyticsRuntime;
use crate::analytics::store::{AnalyticsReadStore, GatewaySegmentOutcomes, SegmentTraffic};
use crate::app::get_tenant_app_state;
use crate::config::SrAutoCalibrationConfig;
use crate::decider::gatewaydecider::constants::{
    ENABLE_SR_V3_COLD_START_PRIOR, ENABLE_SR_V3_LEARNED_PRIOR_STRENGTH,
};
use crate::decider::gatewaydecider::sr_prior::{
    self, GatewaySegmentOutcome, PriorBuildConfig, PriorFitConfig, SegmentOutcome, SrPriorSnapshot,
};
use crate::euclid::types::SrDimensionConfig;
use crate::logger;
use crate::redis::feature::is_feature_enabled;
use crate::redis::types::ServiceConfigKey;
use crate::types::merchant_config::types::FeatureConf;
use crate::types::service_configuration::{find_config_by_name, insert_config, update_config};

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
            min_bucket: cfg
                .min_bucket_size
                .filter(|&v| v > 0)
                .unwrap_or(DEFAULT_MIN_BUCKET),
            max_bucket: cfg
                .max_bucket_size
                .filter(|&v| v > 0)
                .unwrap_or(DEFAULT_MAX_BUCKET),
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
            min_volume: cfg
                .min_volume
                .filter(|&v| v > 0)
                .unwrap_or(DEFAULT_MIN_VOLUME),
        }
    }
}

// Cold-start prior refresh defaults (see `SrAutoCalibrationConfig::prior_*`).
const DEFAULT_PRIOR_PARENT_LOOKBACK_SECS: u64 = 3_600;
const DEFAULT_PRIOR_FIT_LOOKBACK_SECS: u64 = 86_400;
const DEFAULT_PRIOR_MAX_AGE_SECS: u64 = 21_600;

/// Resolved cold-start prior settings, derived from config (with production defaults).
#[derive(Debug, Clone, Copy)]
struct PriorParams {
    /// Short window for parent rates: gateway health changes quickly.
    parent_lookback_secs: u64,
    /// Long window for the prior-strength fit: heterogeneity changes slowly.
    fit_lookback_secs: u64,
    max_age_secs: u64,
    build: PriorBuildConfig,
}

impl PriorParams {
    fn from_config(cfg: &SrAutoCalibrationConfig) -> Self {
        let defaults = PriorBuildConfig::default();
        Self {
            parent_lookback_secs: cfg
                .prior_parent_lookback_secs
                .filter(|&v| v > 0)
                .unwrap_or(DEFAULT_PRIOR_PARENT_LOOKBACK_SECS),
            fit_lookback_secs: cfg
                .prior_fit_lookback_secs
                .filter(|&v| v > 0)
                .unwrap_or(DEFAULT_PRIOR_FIT_LOOKBACK_SECS),
            // Never longer than the ceiling the decider enforces anyway.
            max_age_secs: cfg
                .prior_max_age_secs
                .filter(|&v| v > 0)
                .unwrap_or(DEFAULT_PRIOR_MAX_AGE_SECS)
                .min((sr_prior::MAX_PRIOR_SNAPSHOT_AGE_MS / 1_000) as u64),
            build: PriorBuildConfig {
                fit: PriorFitConfig {
                    min_segments: cfg
                        .prior_min_segments
                        .filter(|&v| v > 0)
                        .unwrap_or(defaults.fit.min_segments),
                    min_segment_observations: cfg
                        .prior_min_segment_observations
                        .filter(|&v| v > 0)
                        .unwrap_or(defaults.fit.min_segment_observations),
                },
                min_parent_observations: cfg
                    .prior_min_parent_observations
                    .filter(|&v| v > 0)
                    .unwrap_or(defaults.min_parent_observations),
                learn_strength: false,
            },
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
    let prior_params = PriorParams::from_config(&config);

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
            let outcome = std::panic::AssertUnwindSafe(run_once(
                &runtime,
                params,
                prior_params,
                interval_secs,
            ))
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

async fn run_once(
    runtime: &AnalyticsRuntime,
    params: CalibrationParams,
    prior_params: PriorParams,
    interval_secs: u64,
) {
    calibrate_enrolled_merchants(runtime, params, interval_secs).await;
    refresh_cold_start_priors(runtime, prior_params, interval_secs).await;
}

async fn calibrate_enrolled_merchants(
    runtime: &AnalyticsRuntime,
    params: CalibrationParams,
    interval_secs: u64,
) {
    let merchants = enrolled_merchants().await;
    if merchants.is_empty() {
        return;
    }
    let store = runtime.read_store();
    let since_ms = crate::analytics::now_ms() - (params.lookback_secs as i64) * 1000;
    // Interval bucket for the per-merchant lock key. Keying by bucket (rather than relying on
    // TTL expiry timing) means each new interval always gets a fresh key, so the lock dedupes
    // concurrent replicas *within* an interval without ever throttling the per-cycle cadence.
    let interval_bucket = crate::analytics::now_ms() / 1000 / (interval_secs.max(1) as i64);
    for merchant_id in merchants {
        // Multi-replica safety: acquire a per-merchant, per-interval Redis SET-NX lock so only
        // one replica calibrates a given merchant per interval — avoids concurrent rewrites of
        // the same SR_V3_INPUT_CONFIG_* and duplicate `calibration_applied` events. The lock is
        // best-effort (advisory): we let it expire rather than release it, and we fail *open* on
        // a Redis error so a single replica keeps calibrating even if Redis is unavailable.
        let lock_key = format!(
            "sr_auto_calibration_lock_{}_{}",
            merchant_id, interval_bucket
        );
        let app_state = get_tenant_app_state().await;
        match app_state
            .redis_conn
            .set_key_if_not_exists(&lock_key, "1", interval_secs.saturating_mul(2) as i64)
            .await
        {
            Ok(true) => {} // acquired — this replica owns the merchant this interval.
            Ok(false) => {
                logger::debug!(
                    tag = "sr_auto_calibration",
                    action = "skip_locked",
                    "another replica is calibrating {} this interval; skipping",
                    merchant_id
                );
                continue;
            }
            Err(err) => {
                logger::warn!(
                    tag = "sr_auto_calibration",
                    action = "lock_error",
                    "lock acquisition for {} failed ({:?}); proceeding without lock",
                    merchant_id,
                    err
                );
            }
        }
        if let Err(reason) =
            calibrate_merchant(store.as_ref(), &merchant_id, since_ms, params).await
        {
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
    merchants_for_flag(AUTOPILOT_CONF_KEY).await
}

/// Refresh `SR_V3_PRIOR_<merchant>` for every merchant explicitly listed under
/// `ENABLE_SR_V3_COLD_START_PRIOR`. Merchants enabled only via `enableAll` cannot be enumerated;
/// they get no snapshot and keep legacy scoring.
async fn refresh_cold_start_priors(
    runtime: &AnalyticsRuntime,
    params: PriorParams,
    interval_secs: u64,
) {
    let merchants = merchants_for_flag(&ENABLE_SR_V3_COLD_START_PRIOR.get_key()).await;
    if merchants.is_empty() {
        return;
    }
    let store = runtime.read_store();
    let now_ms = crate::analytics::now_ms();
    let interval_bucket = now_ms / 1000 / (interval_secs.max(1) as i64);
    for merchant_id in merchants {
        // Same advisory, fail-open, per-interval lock as calibration, under its own key.
        let lock_key = format!("sr_v3_prior_lock_{}_{}", merchant_id, interval_bucket);
        let app_state = get_tenant_app_state().await;
        match app_state
            .redis_conn
            .set_key_if_not_exists(&lock_key, "1", interval_secs.saturating_mul(2) as i64)
            .await
        {
            Ok(true) => {}
            Ok(false) => continue,
            Err(err) => {
                logger::warn!(
                    tag = "sr_v3_prior",
                    action = "lock_error",
                    "prior lock for {} failed ({:?}); proceeding without lock",
                    merchant_id,
                    err
                );
            }
        }
        let learn = is_feature_enabled(
            ENABLE_SR_V3_LEARNED_PRIOR_STRENGTH.get_key(),
            merchant_id.clone(),
            "kv_redis".to_string(),
        )
        .await;
        if let Err(reason) =
            refresh_merchant_prior(store.as_ref(), &merchant_id, now_ms, learn, params).await
        {
            logger::warn!(
                tag = "sr_v3_prior",
                action = "skip",
                "cold-start prior refresh for {} skipped: {}",
                merchant_id,
                reason
            );
        }
    }
}

async fn refresh_merchant_prior(
    store: &dyn AnalyticsReadStore,
    merchant_id: &str,
    now_ms: i64,
    learn: bool,
    params: PriorParams,
) -> Result<(), String> {
    let (dims, granularity_supported) = cold_start_fit_dims(merchant_id).await;
    // Parent rates only need (pmt, pm, gateway) totals over the short window.
    let parent_since_ms = now_ms - (params.parent_lookback_secs as i64) * 1000;
    let parent_rows = store
        .merchant_gateway_segment_outcomes(merchant_id, parent_since_ms, &[])
        .await
        .map_err(|e| format!("clickhouse parent query failed: {e:?}"))?;
    // The fit needs per-segment rows over the long window; skipped when nothing is learned.
    let fit_rows = if learn && granularity_supported {
        let dim_refs: Vec<&str> = dims.iter().map(|s| s.as_str()).collect();
        let fit_since_ms = now_ms - (params.fit_lookback_secs as i64) * 1000;
        store
            .merchant_gateway_segment_outcomes(merchant_id, fit_since_ms, &dim_refs)
            .await
            .map_err(|e| format!("clickhouse fit query failed: {e:?}"))?
    } else {
        Vec::new()
    };
    let snapshot = build_prior_snapshot(
        merchant_id,
        &parent_rows,
        &fit_rows,
        now_ms,
        learn,
        granularity_supported,
        params,
    );
    logger::info!(
        tag = "sr_v3_prior",
        action = "refreshed",
        "cold-start prior for {}: {} gateway entries ({} with learned strength), {} pooled, from {} parent / {} fit rows",
        merchant_id,
        snapshot.entries.len(),
        snapshot
            .entries
            .iter()
            .filter(|e| e.learned_prior_strength.is_some())
            .count(),
        snapshot.pooled_entries.len(),
        parent_rows.len(),
        fit_rows.len()
    );
    let serialized =
        serde_json::to_string(&snapshot).map_err(|e| format!("serialize failed: {e}"))?;
    let name = sr_prior::prior_snapshot_config_name(merchant_id);
    // Written even when empty, so entries of a gateway that stopped receiving traffic expire.
    let exists = find_config_by_name(name.clone())
        .await
        .map_err(|e| format!("config read failed: {e:?}"))?
        .is_some();
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

/// The merchant's SR key dimensions that decision events carry, and whether those are *all* of
/// the key's dimensions. BIN (`card_is_in`) and UDFs split the SR key but are not on decision
/// events, so a fit at the coarser granularity would understate between-segment heterogeneity
/// and over-shrink; in that case the prior strength is not learned.
async fn cold_start_fit_dims(merchant_id: &str) -> (Vec<String>, bool) {
    let info = match find_config_by_name(format!("SR_DIMENSION_CONFIG_{merchant_id}")).await {
        Ok(Some(cfg)) => cfg
            .value
            .and_then(|v| serde_json::from_str::<SrDimensionConfig>(&v).ok())
            .map(|c| c.paymentInfo)
            .unwrap_or_default(),
        _ => Default::default(),
    };
    let fields = info.fields.unwrap_or_default();
    let supported = fields
        .iter()
        .all(|f| segment_outcomes::DIMS.contains(&f.as_str()))
        && info.udfs.is_empty();
    let dims = fields
        .into_iter()
        .filter(|f| segment_outcomes::DIMS.contains(&f.as_str()))
        .collect();
    (dims, supported)
}

fn to_segment_outcomes(rows: &[GatewaySegmentOutcomes]) -> Vec<GatewaySegmentOutcome> {
    rows.iter()
        .map(|r| GatewaySegmentOutcome {
            payment_method_type: r.payment_method_type.clone(),
            payment_method: r.payment_method.clone(),
            gateway: r.gateway.clone(),
            outcome: SegmentOutcome {
                successes: r.successes,
                observations: r.observations,
            },
        })
        .collect()
}

/// Pure: the snapshot to write for a merchant. Rows are assumed to be the merchant's own (both
/// queries filter on `merchant_id`); the snapshot is stamped with it.
fn build_prior_snapshot(
    merchant_id: &str,
    parent_rows: &[GatewaySegmentOutcomes],
    fit_rows: &[GatewaySegmentOutcomes],
    now_ms: i64,
    learn: bool,
    granularity_supported: bool,
    params: PriorParams,
) -> SrPriorSnapshot {
    let learn_strength = learn && granularity_supported;
    let build = PriorBuildConfig {
        learn_strength,
        ..params.build
    };
    let mut built = sr_prior::build_prior_entries(
        &to_segment_outcomes(parent_rows),
        &to_segment_outcomes(fit_rows),
        &build,
    );
    if learn && !granularity_supported {
        for entry in &mut built.entries {
            entry.fit_error =
                Some("SR key splits by card_is_in / udfs, which analytics cannot reproduce".into());
        }
    }
    SrPriorSnapshot {
        version: sr_prior::PRIOR_SNAPSHOT_VERSION,
        merchant_id: merchant_id.to_string(),
        computed_at_ms: now_ms,
        valid_until_ms: now_ms.saturating_add((params.max_age_secs as i64).saturating_mul(1000)),
        lookback_secs: params.parent_lookback_secs,
        fit_lookback_secs: learn_strength.then_some(params.fit_lookback_secs),
        entries: built.entries,
        pooled_entries: built.pooled_entries,
    }
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
                cur_bucket.is_none_or(|c| (c as i32 - new_bucket).abs() >= BUCKET_DEADBAND);
            let hedge_changed =
                cur_hedge.is_none_or(|c| (c - new_hedge).abs() >= HEDGE_DEADBAND_PCT);
            if bucket_changed || hedge_changed {
                entries[i]["bucketSize"] = serde_json::json!(new_bucket);
                entries[i]["hedgingPercent"] = serde_json::json!(new_hedge);
                entries[i]["source"] = serde_json::json!(AUTOPILOT_SOURCE);
                changed += 1;
                let prev_bucket = cur_bucket.map(|c| c as i32);
                log_segment(
                    merchant_id,
                    seg,
                    prev_bucket,
                    new_bucket,
                    cur_hedge,
                    new_hedge,
                );
                emit_calibration_event(
                    merchant_id,
                    seg,
                    prev_bucket,
                    new_bucket,
                    cur_hedge,
                    new_hedge,
                );
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

#[cfg(test)]
mod tests {
    use super::*;

    fn params() -> PriorParams {
        PriorParams::from_config(&SrAutoCalibrationConfig::default())
    }

    fn rows(gateway: &str, segments: &[(u64, u64)]) -> Vec<GatewaySegmentOutcomes> {
        segments
            .iter()
            .map(|&(successes, observations)| GatewaySegmentOutcomes {
                payment_method_type: "CARD".into(),
                payment_method: "VISA".into(),
                gateway: gateway.into(),
                successes,
                observations,
            })
            .collect()
    }

    fn heterogeneous(gateway: &str) -> Vec<GatewaySegmentOutcomes> {
        let segs: Vec<(u64, u64)> = (0..30).map(|i| (10 + (i % 10), 20)).collect();
        rows(gateway, &segs)
    }

    #[test]
    fn prior_params_default_to_production_values() {
        let p = params();
        assert_eq!(p.parent_lookback_secs, DEFAULT_PRIOR_PARENT_LOOKBACK_SECS);
        assert_eq!(p.fit_lookback_secs, DEFAULT_PRIOR_FIT_LOOKBACK_SECS);
        assert_eq!(p.max_age_secs, DEFAULT_PRIOR_MAX_AGE_SECS);
        assert_eq!(p.build, PriorBuildConfig::default());
        let cfg = SrAutoCalibrationConfig {
            prior_parent_lookback_secs: Some(0),
            prior_min_segments: Some(25),
            prior_max_age_secs: Some(u64::MAX),
            ..SrAutoCalibrationConfig::default()
        };
        let p = PriorParams::from_config(&cfg);
        assert_eq!(
            p.max_age_secs as i64 * 1_000,
            sr_prior::MAX_PRIOR_SNAPSHOT_AGE_MS
        );
        assert_eq!(p.parent_lookback_secs, DEFAULT_PRIOR_PARENT_LOOKBACK_SECS);
        assert_eq!(p.build.fit.min_segments, 25);
        assert!(!p.build.learn_strength);
    }

    #[test]
    fn snapshot_is_stamped_with_the_merchant_and_validity_window() {
        let snap = build_prior_snapshot(
            "m_1",
            &heterogeneous("adyen"),
            &[],
            1_000,
            false,
            true,
            params(),
        );
        assert_eq!(snap.merchant_id, "m_1");
        assert_eq!(snap.version, sr_prior::PRIOR_SNAPSHOT_VERSION);
        assert_eq!(snap.computed_at_ms, 1_000);
        assert_eq!(
            snap.valid_until_ms,
            1_000 + DEFAULT_PRIOR_MAX_AGE_SECS as i64 * 1000
        );
        assert!(snap.usable_for("m_1", 2_000));
        assert!(!snap.usable_for("m_2", 2_000));
        assert_eq!(snap.entries.len(), 1);
        assert_eq!(snap.entries[0].learned_prior_strength, None);
    }

    #[test]
    fn strength_is_learned_only_with_the_flag_and_supported_granularity() {
        let learned = build_prior_snapshot(
            "m_1",
            &heterogeneous("adyen"),
            &heterogeneous("adyen"),
            0,
            true,
            true,
            params(),
        );
        assert!(learned.entries[0].learned_prior_strength.is_some());
        assert_eq!(learned.entries[0].fit_error, None);

        let unsupported = build_prior_snapshot(
            "m_1",
            &heterogeneous("adyen"),
            &[],
            0,
            true,
            false,
            params(),
        );
        assert_eq!(unsupported.entries[0].learned_prior_strength, None);
        assert!(unsupported.entries[0]
            .fit_error
            .as_deref()
            .is_some_and(|e| e.contains("card_is_in")));
        // The parent rate is still published: shrinkage keeps working with the configured strength.
        assert_eq!(
            unsupported.entries[0].parent_success_rate,
            learned.entries[0].parent_success_rate
        );
    }

    #[test]
    fn thin_gateways_get_only_the_pooled_prior_and_windows_are_recorded() {
        let mut parent = rows("adyen", &[(80, 100)]);
        parent.extend(rows("stripe", &[(8, 10)]));
        let snap = build_prior_snapshot("m_1", &parent, &[], 0, true, true, params());
        assert_eq!(snap.entries.len(), 1);
        assert_eq!(snap.entries[0].gateway, "adyen");
        assert_eq!(snap.pooled_entries.len(), 1);
        assert_eq!(snap.pooled_entries[0].observations, 110);
        assert_eq!(snap.lookback_secs, DEFAULT_PRIOR_PARENT_LOOKBACK_SECS);
        assert_eq!(
            snap.fit_lookback_secs,
            Some(DEFAULT_PRIOR_FIT_LOOKBACK_SECS)
        );
        let flags = sr_prior::ColdStartFlags {
            prior_enabled: true,
            learned_strength_enabled: true,
        };
        let stripe =
            sr_prior::resolve_prior(flags, Some(&snap), "m_1", 1, "CARD", "VISA", "stripe", None)
                .expect("pooled prior");
        assert_eq!(stripe.scope(), sr_prior::PriorScope::Pooled);
        assert!((stripe.parent_success_rate() - 89.0 / 112.0).abs() < 1e-12);
    }

    #[test]
    fn empty_analytics_produce_an_empty_snapshot() {
        let snap = build_prior_snapshot("m_1", &[], &[], 0, true, true, params());
        assert!(snap.entries.is_empty());
    }

    #[test]
    fn insufficient_training_data_records_the_reason() {
        let snap = build_prior_snapshot(
            "m_1",
            &rows("stripe", &[(40, 50)]),
            &rows("stripe", &[(40, 50)]),
            0,
            true,
            true,
            params(),
        );
        assert_eq!(snap.entries.len(), 1);
        assert_eq!(snap.entries[0].learned_prior_strength, None);
        assert!(snap.entries[0]
            .fit_error
            .as_deref()
            .is_some_and(|e| e.contains("insufficient")));
    }
}
