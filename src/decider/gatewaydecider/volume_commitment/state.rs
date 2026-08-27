//! Redis-backed plan store with a short process-local cache; the plan is the only state.

use async_trait::async_trait;

use super::plan::SteeringPlan;

#[async_trait]
pub trait StateStore: Send + Sync {
    /// The latest plan, or None before the background loop has produced one.
    async fn load_plan(&self, merchant_id: &str) -> Option<SteeringPlan>;

    async fn store_plan(&self, merchant_id: &str, plan: &SteeringPlan);
}

/// Local cache TTL; short so pods notice a new forecast quickly — freshness itself is enforced by
/// `stale_after_epoch_secs`, not by this.
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

/// Redis is the source of truth (survives restarts, shared by all pods); the local cache only
/// spares a round trip per payment. A plan is derived state, so losing it costs one forecast.
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

        // TTL = time until the nudge would refuse the plan anyway, floored at 1s.
        let ttl_secs = (plan.stale_after_epoch_secs - chrono::Utc::now().timestamp()).max(1);

        let state = crate::app::get_tenant_app_state().await;
        match serde_json::to_string(plan) {
            Ok(raw) => {
                if let Err(error) = state.redis_conn.set_key_with_ttl(&key, raw, ttl_secs).await {
                    // Not cached locally on failure: a plan Redis never saw must not steer on one pod only.
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
