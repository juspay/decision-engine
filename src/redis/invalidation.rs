//! Cross-pod invalidation of in-process caches over Redis pub/sub.

use fred::interfaces::PubsubInterface;
use fred::types::RedisValue;
use once_cell::sync::Lazy;

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
            .publish::<i64, _, _>(channel, payload)
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
pub fn spawn_subscriber(clickhouse: ClickHouseAnalyticsConfig) {
    tokio::spawn(async move {
        let state = get_tenant_app_state().await;
        let cache_config = state.config.cache_config.clone();
        let channel = cache_config.invalidation_channel.clone();
        let subscriber = state.redis_conn.conn.subscriber.clone();

        let mut messages = subscriber.on_message();
        match subscriber.subscribe::<(), _>(channel.as_str()).await {
            Ok(()) => {
                // Resubscribes after a Redis reconnect, otherwise invalidations silently stop arriving.
                let _resubscriber = subscriber.manage_subscriptions();
                logger::info!(tag = "cache_invalidation", channel = %channel, "Listening for cache invalidations");

                while let Ok(message) = messages.recv().await {
                    if *message.channel == *channel {
                        handle(&message.value, &cache_config, &clickhouse);
                    }
                }
            }
            Err(error) => {
                logger::error!(tag = "cache_invalidation", ?error, channel = %channel, "Subscribe failed; caches fall back to TTL expiry");
            }
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
