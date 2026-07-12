//! Connector-generic settlement-report webhook ingress: `POST /webhooks/settlement/:connector`.
//!
//! A connector (Adyen first) calls this when a settlement report is ready. We verify the
//! signature, ACK immediately, and enqueue — every heavy step (download, parse, fit) is deferred
//! to the ingest worker so the connector always gets a fast response. Public (unauthenticated by
//! our API key): the caller authenticates via its own signature, checked here.
//!
//! See `scratch/inhouse-cost-architecture.md` §7.

use axum::body::Bytes;
use axum::extract::Path;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;

use crate::app::get_tenant_app_state;
use crate::cost_ingestion::{store, ConnectorCredsStore, ConnectorRegistry, IngestError};
use crate::logger;

pub async fn settlement_webhook(
    Path(connector): Path<String>,
    headers: HeaderMap,
    body: Bytes, // must be the last extractor — it consumes the request body
) -> impl IntoResponse {
    match handle(&connector, &headers, &body).await {
        Ok(created) => {
            logger::info!(
                tag = "settlement_webhook",
                "accepted {} settlement webhook (new_job={})",
                connector,
                created
            );
            // Adyen expects the literal body "[accepted]"; harmless for other connectors.
            (StatusCode::OK, "[accepted]")
        }
        Err(e) => {
            logger::warn!(
                tag = "settlement_webhook",
                "rejected {} settlement webhook: {:?}",
                connector,
                e
            );
            (status_for(&e), "rejected")
        }
    }
}

/// Verify + enqueue. Everything here is cheap (a couple of DB round-trips + an HMAC); the report
/// download and fit happen later in the worker.
async fn handle(connector: &str, headers: &HeaderMap, body: &[u8]) -> Result<bool, IngestError> {
    let registry = ConnectorRegistry::with_builtins();
    let source = registry.get(connector)?;

    // 1. Read the connector-side account from the *unverified* body, to find whose secret to use.
    let account = source.peek_account(body)?;

    // 2. Load that (connector, account)'s credentials + the merchant that owns it.
    let app_state = get_tenant_app_state().await;
    let cfg = &app_state.config.cost_ingestion;
    let creds_store = ConnectorCredsStore::from_keyring(
        &cfg.creds_encryption_current,
        &cfg.creds_encryption_keys,
    )
    .ok_or_else(|| IngestError::Storage("credential encryption keyring not configured".into()))?;
    let resolved = creds_store.get(connector, &account).await?.ok_or_else(|| {
        IngestError::MalformedNotification(format!(
            "no credentials stored for {connector}/{account}"
        ))
    })?;

    // 3. Verify the signature against that account's secret and extract the report handle.
    let note =
        source.verify_and_parse_notification(headers, body, &resolved.creds.webhook_secret)?;

    // 4. Enqueue (idempotent on the notification id).
    store::enqueue_pending(
        connector,
        &account,
        &resolved.merchant_id,
        &note.notification_id,
        &note.report_ref,
        "webhook",
    )
    .await
}

/// A bad signature is the caller's fault (401); everything else is our side (500). Either way the
/// response is fast and carries no internal detail.
fn status_for(e: &IngestError) -> StatusCode {
    match e {
        IngestError::SignatureMismatch => StatusCode::UNAUTHORIZED,
        IngestError::UnknownConnector(_) | IngestError::MalformedNotification(_) => {
            StatusCode::BAD_REQUEST
        }
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}
