//! Cross-pod invalidation of in-process caches over Redis pub/sub.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use fred::interfaces::{EventInterface, PubsubInterface};
use fred::types::RedisValue;
use once_cell::sync::Lazy;
use tokio::sync::broadcast::error::RecvError;

use crate::app::get_tenant_app_state;
use crate::config::{CacheConfig, ClickHouseAnalyticsConfig};
use crate::logger;
use crate::metrics::CACHE_INVALIDATION_COUNTER;

/// An in-process cache entry other pods may hold a stale copy of.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum CacheKind {
    MerchantAccount(String),
    CostModel(String),
    ServiceConfig(String),
}

impl CacheKind {
    fn label(&self) -> &'static str {
        match self {
            Self::MerchantAccount(_) => "merchant_account",
            Self::CostModel(_) => "cost_model",
            Self::ServiceConfig(_) => "service_config",
        }
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct CacheRedact {
    origin: String,
    kind: CacheKind,
}

// Lets a pod skip its own messages; the writer has already updated its local copy.
static INSTANCE_ID: Lazy<String> = Lazy::new(|| uuid::Uuid::new_v4().to_string());

const SUBSCRIBE_RETRY_START: Duration = Duration::from_secs(1);
const SUBSCRIBE_RETRY_MAX: Duration = Duration::from_secs(30);
/// How long a subscribe waits for its own probe before it counts as refused.
const PROBE_TIMEOUT: Duration = Duration::from_secs(2);

/// Tell every other pod to drop or rebuild `kind`; best effort, the cache TTL is the backstop.
pub async fn publish(kind: CacheKind) {
    let label = kind.label();
    let state = get_tenant_app_state().await;
    let channel = state.config.cache_config.invalidation_channel.as_str();

    let published = match serde_json::to_string(&CacheRedact {
        origin: INSTANCE_ID.clone(),
        kind,
    }) {
        Ok(payload) => state
            .redis_conn
            .conn
            .publisher
            .publish(channel, redis_interface::RedisValue::new(payload.into()))
            .await
            .map_err(|error| format!("{error:?}")),
        Err(error) => Err(format!("encode failed: {error}")),
    };

    match published {
        Ok(receivers) => {
            CACHE_INVALIDATION_COUNTER
                .with_label_values(&[label, "published"])
                .inc();
            logger::debug!(
                tag = "cache_invalidation",
                kind = label,
                receivers,
                "Published invalidation"
            );
        }
        Err(error) => {
            CACHE_INVALIDATION_COUNTER
                .with_label_values(&[label, "publish_failed"])
                .inc();
            logger::warn!(
                tag = "cache_invalidation",
                kind = label,
                error = %error,
                "Failed to publish invalidation"
            );
        }
    }
}

/// Apply other pods' invalidations to this process's caches; spawn once, after `APP_STATE` is set.
///
/// Pub/sub keeps no backlog: whatever is published while this pod is disconnected never reaches
/// it, and nothing says which entries those messages were about. So after every reconnect, and
/// whenever the buffer overflows, the pod resubscribes and then drops everything the messages
/// would have corrected.
pub fn spawn_subscriber(clickhouse: ClickHouseAnalyticsConfig) {
    tokio::spawn(async move {
        let state = get_tenant_app_state().await;
        let cache_config = state.config.cache_config.clone();
        let channel = cache_config.invalidation_channel.clone();
        let subscriber = state.redis_conn.conn.subscriber.clone();

        let mut messages = subscriber.message_rx();
        // Taken before subscribing, so a reconnect during the first subscribe is still seen.
        let mut reconnects = EventInterface::reconnect_rx(&**subscriber);

        subscribe(&state, &mut messages, &cache_config, &clickhouse).await;
        logger::info!(tag = "cache_invalidation", channel = %channel, "Listening for cache invalidations");

        loop {
            tokio::select! {
                message = messages.recv() => match message {
                    Ok(message) => {
                        if message.channel == channel {
                            handle(&message.value, &cache_config, &clickhouse);
                        }
                    }
                    Err(RecvError::Lagged(skipped)) => {
                        logger::warn!(tag = "cache_invalidation", skipped, "Invalidations dropped");
                        flush(FlushReason::Lagged, &clickhouse);
                    }
                    Err(RecvError::Closed) => {
                        logger::error!(
                            tag = "cache_invalidation",
                            "Subscriber stream closed; caches fall back to TTL expiry"
                        );
                        break;
                    }
                },
                reconnect = reconnects.recv() => match reconnect {
                    // A lagged receiver still means at least one reconnect happened.
                    Ok(_) | Err(RecvError::Lagged(_)) => {
                        // The new connection holds no subscription. Flushing only once it does means
                        // anything published from here on arrives, and everything published before
                        // was committed before the flush, so the next read finds it.
                        subscribe(&state, &mut messages, &cache_config, &clickhouse).await;
                        flush(FlushReason::Reconnect, &clickhouse);
                    }
                    // The client closes its event streams only when it quits, at shutdown.
                    Err(RecvError::Closed) => break,
                },
            }
        }
    });
}

/// Subscribe to `channel`, retrying with backoff until the subscription is confirmed live. Until
/// then nothing is flushed, so a pod that cannot subscribe keeps serving its cache on TTL alone.
///
/// The client reports a subscribe as done once it is sent, before Redis accepts or refuses it, so
/// each attempt publishes a probe and counts only once the probe comes back. Every message
/// published after the probe then reaches this pod; ones that arrive while it waits are applied.
async fn subscribe(
    state: &crate::app::TenantAppState,
    messages: &mut tokio::sync::broadcast::Receiver<redis_interface::PubSubMessage>,
    cache_config: &CacheConfig,
    clickhouse: &ClickHouseAnalyticsConfig,
) {
    let channel = cache_config.invalidation_channel.as_str();
    let subscriber = &state.redis_conn.conn.subscriber;
    let mut retry_in = SUBSCRIBE_RETRY_START;
    loop {
        let attempt = match PubsubInterface::subscribe(&***subscriber, channel).await {
            Ok(()) => confirm_subscribed(state, messages, cache_config, clickhouse).await,
            Err(error) => Err(format!("{error:?}")),
        };
        match attempt {
            Ok(()) => return,
            Err(error) => {
                logger::warn!(tag = "cache_invalidation", error = %error, channel = %channel, retry_in_secs = retry_in.as_secs(), "Subscribe failed; retrying");
                tokio::time::sleep(retry_in).await;
                retry_in = (retry_in * 2).min(SUBSCRIBE_RETRY_MAX);
            }
        }
    }
}

/// Publish a probe only this pod can recognise and wait for it to arrive, applying any
/// invalidation that comes first. Other pods ignore the probe: it does not parse as one.
async fn confirm_subscribed(
    state: &crate::app::TenantAppState,
    messages: &mut tokio::sync::broadcast::Receiver<redis_interface::PubSubMessage>,
    cache_config: &CacheConfig,
    clickhouse: &ClickHouseAnalyticsConfig,
) -> Result<(), String> {
    let channel = cache_config.invalidation_channel.as_str();
    let probe = serde_json::json!({ "probe": uuid::Uuid::new_v4().to_string() }).to_string();
    state
        .redis_conn
        .conn
        .publisher
        .publish(
            channel,
            redis_interface::RedisValue::new(probe.clone().into()),
        )
        .await
        .map_err(|error| format!("probe publish failed: {error:?}"))?;

    tokio::time::timeout(PROBE_TIMEOUT, async {
        loop {
            match messages.recv().await {
                Ok(message) if message.channel != channel => {}
                Ok(message) if message.value.as_bytes() == Some(probe.as_bytes()) => {
                    return Ok(());
                }
                Ok(message) => handle(&message.value, cache_config, clickhouse),
                Err(RecvError::Lagged(_)) => flush(FlushReason::Lagged, clickhouse),
                Err(RecvError::Closed) => return Err("subscriber stream closed".to_string()),
            }
        }
    })
    .await
    .map_err(|_| "probe not received; the subscription was not accepted".to_string())?
}

#[derive(Clone, Copy)]
enum FlushReason {
    Reconnect,
    Lagged,
}

impl FlushReason {
    fn label(self) -> &'static str {
        match self {
            Self::Reconnect => "flushed_after_reconnect",
            Self::Lagged => "flushed_after_lag",
        }
    }
}

/// Set while a full cost-model rebuild is queued and has not started, so a burst of reconnects
/// queues one rebuild rather than one per reconnect.
static COST_REBUILD_QUEUED: AtomicBool = AtomicBool::new(false);

/// Drop every cache an invalidation message could have corrected; the next reads go to the
/// database, and the cost models are rebuilt from ClickHouse.
fn flush(reason: FlushReason, clickhouse: &ClickHouseAnalyticsConfig) {
    CACHE_INVALIDATION_COUNTER
        .with_label_values(&["all", reason.label()])
        .inc();
    logger::warn!(
        tag = "cache_invalidation",
        reason = reason.label(),
        "Invalidations may have been missed; dropping cached service configs and merchant accounts and rebuilding cost models"
    );

    if let Err(error) = crate::redis::mem_cache::GLOBAL_CACHE.clear() {
        logger::error!(tag = "cache_invalidation", error = %error, "Failed to clear the in-memory cache");
    }

    if COST_REBUILD_QUEUED.swap(true, Ordering::AcqRel) {
        return;
    }
    let clickhouse = clickhouse.clone();
    tokio::spawn(async move {
        // A flush from here on queues another rebuild; every earlier one is covered by this one,
        // whose reads all come after it.
        COST_REBUILD_QUEUED.store(false, Ordering::Release);
        if let Err(error) = crate::cost_ingestion::serving::refresh(&clickhouse).await {
            logger::warn!(tag = "cache_invalidation", error = %error, "Cost model rebuild after a missed invalidation failed");
        }
    });
}

fn handle(value: &RedisValue, cache_config: &CacheConfig, clickhouse: &ClickHouseAnalyticsConfig) {
    let message = value
        .as_bytes()
        .and_then(|bytes| serde_json::from_slice::<CacheRedact>(bytes).ok());

    match message {
        // Unknown or malformed messages (e.g. a kind added by a newer version) are ignored.
        None => {
            logger::debug!(
                tag = "cache_invalidation",
                "Ignoring unrecognised invalidation message"
            );
        }
        Some(message) if message.origin == *INSTANCE_ID => {}
        Some(message) => apply(message.kind, cache_config, clickhouse),
    }
}

fn apply(kind: CacheKind, cache_config: &CacheConfig, clickhouse: &ClickHouseAnalyticsConfig) {
    CACHE_INVALIDATION_COUNTER
        .with_label_values(&[kind.label(), "applied"])
        .inc();
    logger::debug!(tag = "cache_invalidation", kind = ?kind, "Applying invalidation from another pod");
    match kind {
        CacheKind::MerchantAccount(merchant_id) => {
            crate::types::merchant::merchant_account::evict_cached(&merchant_id);
        }
        CacheKind::ServiceConfig(name) => {
            crate::redis::cache::evict_service_config_from_memory(cache_config, &name);
        }
        CacheKind::CostModel(merchant_id) => {
            // Off the receive loop, so a slow ClickHouse doesn't hold up other invalidations.
            let clickhouse = clickhouse.clone();
            tokio::spawn(async move {
                if let Err(error) =
                    crate::cost_ingestion::serving::refresh_merchant(&clickhouse, &merchant_id)
                        .await
                {
                    logger::warn!(tag = "cache_invalidation", merchant_id = %merchant_id, error = %error, "Cost model refresh from invalidation failed");
                }
            });
        }
    }
}
