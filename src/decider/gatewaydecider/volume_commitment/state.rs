//! Where the plan lives between forecasts.
//!
//! Only the plan: steering used to also keep a running per-day counter here, which a rate makes
//! unnecessary — each payment now rolls independently, so there is nothing to accumulate.

use async_trait::async_trait;

use super::plan::SteeringPlan;

#[async_trait]
pub trait StateStore: Send + Sync {
    /// The latest plan, or None before the background loop has produced one.
    async fn load_plan(&self, merchant_id: &str) -> Option<SteeringPlan>;

    async fn store_plan(&self, merchant_id: &str, plan: &SteeringPlan);
}

/// How long a plan may sit in the process-local cache before it is re-read from Redis.
///
/// Short, because a pod that has cached a plan must notice the next forecast quickly — but
/// correctness never rests on it: every plan carries `stale_after_epoch_secs`, and the nudge
/// refuses an expired one regardless of where it came from.
const PLAN_CACHE_TTL_MS: u64 = 5_000;

/// At most this many merchants' plans are held locally. Well above any realistic active-contract
/// count, so eviction is a safety valve rather than something the hot path meets.
const PLAN_CACHE_MAX: usize = 10_000;

/// Process-local front for the Redis plans. `try_lock` inside means contention costs a Redis read
/// rather than blocking a payment.
static PLAN_CACHE: once_cell::sync::Lazy<crate::redis::mem_cache::TypedCache<SteeringPlan>> =
    once_cell::sync::Lazy::new(|| {
        crate::redis::mem_cache::TypedCache::new(PLAN_CACHE_TTL_MS, PLAN_CACHE_MAX)
    });

fn plan_key(merchant_id: &str) -> String {
    format!("vc_plan_{merchant_id}")
}

/// Redis-backed store with a read-through local cache.
///
/// Redis is the source of truth, which is what makes the feature survive a restart and behave the
/// same on every pod — the forecast runs once, and whichever process serves a payment reads that
/// same plan. The local cache exists because the plan is read on *every* payment: without it each
/// one would pay a network round trip, so Redis load would scale with payment volume instead of
/// with time.
///
/// A plan is derived state — a pure function of the contract document and delivered volume, both
/// durable elsewhere — so losing it costs one forecast, not data. That is why Redis with a TTL is
/// the right home for it, and why nothing here tries to be a system of record.
pub struct RedisStateStore;

#[async_trait]
impl StateStore for RedisStateStore {
    async fn load_plan(&self, merchant_id: &str) -> Option<SteeringPlan> {
        let key = plan_key(merchant_id);
        if let Some(cached) = PLAN_CACHE.get(&key) {
            return Some(cached);
        }

        let state = crate::app::get_tenant_app_state().await;
        let raw = state.redis_conn.get_key_string(&key).await.ok()?;
        // An absent key comes back as an empty string rather than an error.
        if raw.is_empty() {
            return None;
        }

        match serde_json::from_str::<SteeringPlan>(&raw) {
            Ok(plan) => {
                PLAN_CACHE.store(key, plan.clone());
                Some(plan)
            }
            Err(error) => {
                // A plan written by an incompatible build. Steering simply stops until the next
                // forecast overwrites it, which is the safe direction.
                crate::logger::error!(
                    tag = "volume_commitment",
                    merchant_id = merchant_id,
                    "could not parse the stored plan: {error}"
                );
                None
            }
        }
    }

    async fn store_plan(&self, merchant_id: &str, plan: &SteeringPlan) {
        let key = plan_key(merchant_id);

        // Expire the key exactly when the nudge would start refusing the plan, so Redis never
        // holds one that could only ever be rejected. Floored at a second so a plan built right on
        // a boundary still lands.
        let ttl_secs = (plan.stale_after_epoch_secs - chrono::Utc::now().timestamp()).max(1);

        let state = crate::app::get_tenant_app_state().await;
        match serde_json::to_string(plan) {
            Ok(raw) => {
                if let Err(error) = state.redis_conn.set_key_with_ttl(&key, raw, ttl_secs).await {
                    // Redis is the source of truth, so a failed write means other pods will not
                    // see this plan. Loud, and deliberately not cached locally either — a local
                    // copy of a plan Redis never received would make one pod behave differently
                    // from the rest, which is the failure this store exists to prevent.
                    crate::logger::error!(
                        tag = "volume_commitment",
                        merchant_id = merchant_id,
                        "could not store the plan in redis: {error:?}"
                    );
                    return;
                }
            }
            Err(error) => {
                crate::logger::error!(
                    tag = "volume_commitment",
                    merchant_id = merchant_id,
                    "could not serialize the plan: {error}"
                );
                return;
            }
        }

        // Populate locally only after Redis accepted it, so the cache can never be ahead of truth.
        PLAN_CACHE.store(key, plan.clone());
    }
}
