//! Redis-backed store for the plan (with a short process-local cache) and the scheduler's
//! per-merchant run lease. The plan is derived state; the lease only says who runs next.

use async_trait::async_trait;

use super::plan::SteeringPlan;

#[async_trait]
pub trait StateStore: Send + Sync {
    /// The latest plan, or None before the background loop has produced one.
    async fn load_plan(&self, merchant_id: &str) -> Option<SteeringPlan>;

    async fn store_plan(&self, merchant_id: &str, plan: &SteeringPlan);

    /// Forget the plan everywhere: a deactivated or replaced contract must stop steering now,
    /// not when the plan's TTL runs out.
    async fn clear_plan(&self, merchant_id: &str);

    /// Claim this merchant's next forecast for `ttl_secs`. False when another replica already
    /// holds it (or Redis cannot say), so K schedulers produce one run per interval, not K.
    async fn try_acquire_run_lease(&self, merchant_id: &str, ttl_secs: u64) -> bool;

    /// Give a lease back after a run that did not happen, so the next tick retries.
    async fn release_run_lease(&self, merchant_id: &str);

    /// Epoch seconds the live lease was taken at — when this merchant's last run started — or
    /// None once it has expired or was released.
    async fn last_run_started_at(&self, merchant_id: &str) -> Option<i64>;
}

/// Local cache TTL; short so pods notice a new forecast quickly — freshness itself is enforced by
/// `stale_after_epoch_secs`, not by this.
const PLAN_CACHE_TTL_MS: u64 = 5_000;

/// At most this many merchants' plans are held locally. Well above any realistic active-contract
/// count, so eviction is a safety valve rather than something the hot path meets.
const PLAN_CACHE_MAX: usize = 10_000;

/// Process-local front for the Redis plans, holding misses too (`None`): a flag-on merchant with
/// no plan — every rollover gap — would otherwise cost one Redis GET per payment. `try_lock`
/// inside means contention costs a Redis read rather than blocking a payment.
static PLAN_CACHE: once_cell::sync::Lazy<
    crate::redis::mem_cache::TypedCache<Option<SteeringPlan>>,
> = once_cell::sync::Lazy::new(|| {
    crate::redis::mem_cache::TypedCache::new(PLAN_CACHE_TTL_MS, PLAN_CACHE_MAX)
});

fn plan_key(merchant_id: &str) -> String {
    format!("vc_plan_{merchant_id}")
}

fn lease_key(merchant_id: &str) -> String {
    format!("vc_run_lease_{merchant_id}")
}

/// Redis is the source of truth (survives restarts, shared by all pods); the local cache only
/// spares a round trip per payment. A plan is derived state, so losing it costs one forecast.
pub struct RedisStateStore;

#[async_trait]
impl StateStore for RedisStateStore {
    async fn load_plan(&self, merchant_id: &str) -> Option<SteeringPlan> {
        let key = plan_key(merchant_id);
        if let Some(cached) = PLAN_CACHE.get(&key) {
            return cached;
        }

        let state = crate::app::get_tenant_app_state().await;
        // A Redis failure is not cached: it is transient, and the next payment may get through.
        let raw = state.redis_conn.get_key_string(&key).await.ok()?;

        // An absent key comes back as an empty string rather than an error.
        let plan = if raw.is_empty() {
            None
        } else {
            match serde_json::from_str::<SteeringPlan>(&raw) {
                Ok(plan) => Some(plan),
                Err(error) => {
                    // A plan written by an incompatible build. Steering simply stops until the
                    // next forecast overwrites it, which is the safe direction.
                    crate::logger::error!(
                        tag = "volume_commitment",
                        merchant_id = merchant_id,
                        "could not parse the stored plan: {error}"
                    );
                    None
                }
            }
        };
        // Misses are cached as well, so the absent-plan case stays off Redis for the TTL.
        PLAN_CACHE.store(key, plan.clone());
        plan
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
        PLAN_CACHE.store(key, Some(plan.clone()));
    }

    async fn clear_plan(&self, merchant_id: &str) {
        let key = plan_key(merchant_id);
        // This pod stops at once; the others notice within the local cache TTL.
        PLAN_CACHE.store(key.clone(), None);

        let state = crate::app::get_tenant_app_state().await;
        if let Err(error) = state.redis_conn.delete_key(&key).await {
            // Loud: until this succeeds, other pods keep steering on a contract that is gone.
            crate::logger::error!(
                tag = "volume_commitment",
                merchant_id = merchant_id,
                "could not delete the stored plan from redis: {error:?}"
            );
        }
    }

    async fn try_acquire_run_lease(&self, merchant_id: &str, ttl_secs: u64) -> bool {
        let state = crate::app::get_tenant_app_state().await;
        let started_at = chrono::Utc::now().timestamp().to_string();
        let ttl = i64::try_from(ttl_secs.max(1)).unwrap_or(i64::MAX);
        match state
            .redis_conn
            .set_key_if_not_exists(&lease_key(merchant_id), &started_at, ttl)
            .await
        {
            Ok(acquired) => acquired,
            Err(error) => {
                // Without Redis nobody can tell who holds the lease; running anyway would put
                // every replica to work at once, so no one runs.
                crate::logger::error!(
                    tag = "volume_commitment",
                    merchant_id = merchant_id,
                    "could not take the forecast lease in redis: {error:?}"
                );
                false
            }
        }
    }

    async fn release_run_lease(&self, merchant_id: &str) {
        let state = crate::app::get_tenant_app_state().await;
        if let Err(error) = state.redis_conn.delete_key(&lease_key(merchant_id)).await {
            crate::logger::warn!(
                tag = "volume_commitment",
                merchant_id = merchant_id,
                "could not release the forecast lease; the merchant waits out the interval: {error:?}"
            );
        }
    }

    async fn last_run_started_at(&self, merchant_id: &str) -> Option<i64> {
        let state = crate::app::get_tenant_app_state().await;
        let raw = state
            .redis_conn
            .get_key_string(&lease_key(merchant_id))
            .await
            .ok()?;
        raw.parse().ok()
    }
}
