use std::sync::atomic::AtomicBool;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};

use super::*;
use crate::analytics::flow::{AnalyticsFlowContext, AnalyticsRoute, ApiFlow, FlowType};
use crate::error::ApiError;

struct RecordingStore(mpsc::Sender<Vec<DomainAnalyticsEvent>>);

#[async_trait]
impl AnalyticsWriteStore for RecordingStore {
    async fn persist_domain_events(&self, events: &[DomainAnalyticsEvent]) -> Result<(), ApiError> {
        self.0.send(events.to_vec()).await.unwrap();
        Ok(())
    }

    async fn persist_api_events(&self, _: &[ApiEvent]) -> Result<(), ApiError> {
        Ok(())
    }

    fn sink_name(&self) -> &'static str {
        "test"
    }
}

fn runtime(
    enabled: bool,
    capacity: usize,
) -> (
    Arc<AnalyticsRuntime>,
    mpsc::Receiver<PendingDomainEvent>,
    mpsc::Receiver<Vec<DomainAnalyticsEvent>>,
) {
    let mut config = AnalyticsConfig::default();
    config.kafka.enabled = enabled;
    let (domain_tx, domain_rx) = mpsc::channel(capacity);
    let (api_tx, _) = mpsc::channel(1);
    let (published_tx, published_rx) = mpsc::channel(4);
    let runtime = Arc::new(AnalyticsRuntime {
        config,
        read_store: Arc::new(UnavailableAnalyticsReadStore),
        write_store: Arc::new(RecordingStore(published_tx)),
        domain_tx,
        deferred_details: Arc::new(Semaphore::new(MAX_DEFERRED_DETAILS)),
        api_tx,
        domain_depth: Arc::new(AtomicUsize::new(0)),
        api_depth: Arc::new(AtomicUsize::new(0)),
    });
    (runtime, domain_rx, published_rx)
}

fn event(id: &str, details: Option<String>) -> DomainAnalyticsEvent {
    let mut event = DomainAnalyticsEvent::request_hit(
        AnalyticsFlowContext::new(
            ApiFlow::RuleBasedRouting,
            FlowType::RoutingEvaluateRequestHit,
        ),
        AnalyticsRoute::RoutingEvaluate,
        Some("merchant".to_string()),
        Some("payment".to_string()),
        Some(id.to_string()),
        None,
        None,
        None,
        1234,
    );
    event.event_id = id.to_string();
    event.details = details;
    event
}

#[test]
fn bounded_details_preserve_unicode_and_handle_tiny_limits() {
    let small = json!({ "note": "é🙂" });
    let encoded = serde_json::to_string(&small).unwrap();
    for limit in 0..=encoded.len() {
        let captured = serialize_bounded_details(&small, limit);
        if limit == encoded.len() {
            assert_eq!(captured.as_deref(), Some(encoded.as_str()));
        } else {
            assert!(captured.is_none());
        }
    }

    let large = json!({ "note": "é".repeat(40_000) });
    let encoded = serde_json::to_string(&large).unwrap();
    assert!(serialize_bounded_details(&large, 65_536).is_none());
    let captured = bound_domain_details(Some(encoded.clone()), 65_536).unwrap();
    assert!(captured.len() <= 65_536);
    assert_eq!(
        serde_json::from_str::<Value>(&captured).unwrap(),
        json!({ "truncated": true, "original_bytes": encoded.len() }),
    );
    assert!(bound_domain_details(Some(encoded), 1).is_none());
}

#[test]
fn disabled_or_full_queue_does_not_build_deferred_details() {
    for enabled in [false, true] {
        let (runtime, mut pending, _) = runtime(enabled, 1);
        if enabled {
            runtime.enqueue_domain_event(event("existing", None));
        }
        let calls = AtomicUsize::new(0);
        runtime.enqueue_domain_event_with_details(event("deferred", None), || {
            calls.fetch_add(1, Ordering::Relaxed);
            |_| Some("{}".to_string())
        });
        assert_eq!(calls.load(Ordering::Relaxed), 0);
        assert_eq!(
            runtime.deferred_details.available_permits(),
            MAX_DEFERRED_DETAILS
        );
        if enabled {
            assert_eq!(pending.try_recv().unwrap().event.event_id, "existing");
        }
        assert!(pending.try_recv().is_err());
    }
}

#[test]
fn deferred_limit_rejects_before_capture_and_recovers_after_panic() {
    let (runtime, mut pending, _) = runtime(true, MAX_DEFERRED_DETAILS + 1);
    let captures = AtomicUsize::new(0);
    for index in 0..MAX_DEFERRED_DETAILS {
        runtime.enqueue_domain_event_with_details(event(&index.to_string(), None), || {
            captures.fetch_add(1, Ordering::Relaxed);
            move |_| {
                assert_ne!(index, 0, "injected serialization panic");
                Some("{}".to_string())
            }
        });
    }
    runtime.enqueue_domain_event_with_details(event("rejected", None), || {
        captures.fetch_add(1, Ordering::Relaxed);
        |_| Some("{}".to_string())
    });
    assert_eq!(captures.load(Ordering::Relaxed), MAX_DEFERRED_DETAILS);
    assert_eq!(runtime.deferred_details.available_permits(), 0);

    let prepared = pending.try_recv().unwrap().prepare(1024);
    assert_eq!(prepared.event_id, "0");
    assert!(prepared.details.is_none());
    assert_eq!(runtime.deferred_details.available_permits(), 1);

    runtime.enqueue_domain_event_with_details(event("replacement", None), || {
        captures.fetch_add(1, Ordering::Relaxed);
        |_| Some("{}".to_string())
    });
    assert_eq!(captures.load(Ordering::Relaxed), MAX_DEFERRED_DETAILS + 1);
    assert_eq!(runtime.deferred_details.available_permits(), 0);
    drop(pending);
    assert_eq!(
        runtime.deferred_details.available_permits(),
        MAX_DEFERRED_DETAILS
    );
}

#[tokio::test]
async fn publisher_isolates_details_panic_and_keeps_publishing() {
    let (runtime, pending, mut published) = runtime(true, 4);
    let request_thread = std::thread::current().id();
    let prepared_off_thread = Arc::new(AtomicBool::new(false));
    let observed = prepared_off_thread.clone();
    runtime.enqueue_domain_event_with_details(event("panicking", None), || {
        move |_| {
            observed.store(
                std::thread::current().id() != request_thread,
                Ordering::Relaxed,
            );
            panic!("injected serialization panic");
        }
    });
    runtime.enqueue_domain_event(event("neighbor", Some("{\"ok\":true}".to_string())));
    assert!(!prepared_off_thread.load(Ordering::Relaxed));
    runtime.spawn_domain_publisher(pending);

    let first = tokio::time::timeout(Duration::from_secs(5), published.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first.len(), 2);
    assert_eq!(first[0].event_id, "panicking");
    assert!(first[0].details.is_none());
    assert_eq!(first[0].created_at_ms, 1234);
    assert_eq!(first[0].payment_id.as_deref(), Some("payment"));
    assert_eq!(first[1].event_id, "neighbor");
    assert_eq!(first[1].details.as_deref(), Some("{\"ok\":true}"));
    assert!(prepared_off_thread.load(Ordering::Relaxed));
    assert_eq!(
        runtime.deferred_details.available_permits(),
        MAX_DEFERRED_DETAILS
    );

    runtime.enqueue_domain_event(event("later", Some("{}".to_string())));
    let later = tokio::time::timeout(Duration::from_secs(5), published.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(later.len(), 1);
    assert_eq!(later[0].event_id, "later");
    assert_eq!(later[0].details.as_deref(), Some("{}"));
    assert_eq!(runtime.domain_depth.load(Ordering::Relaxed), 0);
}
