//! Connector-generic settlement-report webhook ingress:
//! `POST /webhooks/settlement/:merchant_id/:connector`.
//!
//! A connector (Adyen first) calls this when a settlement report is ready. We verify the
//! signature, ACK immediately, and enqueue — every heavy step (download, parse, fit) is deferred
//! to the ingest worker so the connector always gets a fast response. Public (unauthenticated by
//! our API key): the caller authenticates via its own signature, checked here.
//!
//! The merchant is in the path because it cannot be recovered from the payload: a connector's
//! notification names only its own account, and one account can be shared by two of our merchants
//! (each registering their own endpoint, with their own HMAC key, at the connector). The path
//! merchant selects *which* credentials to verify against — so a caller cannot pass another
//! merchant's id and get anywhere without also holding that merchant's signing secret.
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
    Path((merchant_id, connector)): Path<(String, String)>,
    headers: HeaderMap,
    body: Bytes, // must be the last extractor — it consumes the request body
) -> impl IntoResponse {
    match handle(&merchant_id, &connector, &headers, &body).await {
        Ok(created) => {
            logger::info!(
                tag = "settlement_webhook",
                "accepted {} settlement webhook for {} (new_job={})",
                connector,
                merchant_id,
                created
            );
            // Adyen expects the literal body "[accepted]"; harmless for other connectors.
            (StatusCode::OK, "[accepted]")
        }
        Err(e) => {
            let status = status_for(&e);
            // Rejections stay at debug so an unauthenticated caller cannot flood the logs.
            let connector = sanitize_for_log(&connector);
            let merchant_id = sanitize_for_log(&merchant_id);
            let detail = sanitize_for_log(&format!("{e:?}"));
            if status == StatusCode::INTERNAL_SERVER_ERROR {
                logger::warn!(
                    tag = "settlement_webhook",
                    "failed {} settlement webhook for {}: {}",
                    connector,
                    merchant_id,
                    detail
                );
            } else {
                logger::debug!(
                    tag = "settlement_webhook",
                    "rejected {} settlement webhook for {}: {}",
                    connector,
                    merchant_id,
                    detail
                );
            }
            (status, "rejected")
        }
    }
}

/// Verify + enqueue. Everything here is cheap (a couple of DB round-trips + an HMAC); the report
/// download and fit happen later in the worker.
async fn handle(
    merchant_id: &str,
    connector: &str,
    headers: &HeaderMap,
    body: &[u8],
) -> Result<bool, IngestError> {
    let registry = ConnectorRegistry::with_builtins();
    let source = registry.get(connector)?;

    // 1. Read the connector-side account from the *unverified* body. Together with the path
    //    merchant it names which credentials to verify against.
    let account = source.peek_account(body)?;

    // 2. Load this merchant's credentials for that account. Scoped to the path merchant, so a
    //    second merchant sharing the same connector account keeps their own signing secret.
    let app_state = get_tenant_app_state().await;
    let cfg = &app_state.config.cost_ingestion;
    let creds_store = ConnectorCredsStore::from_keyring(
        &cfg.creds_encryption_current,
        &cfg.creds_encryption_keys,
    )
    .ok_or_else(|| IngestError::Storage("credential encryption keyring not configured".into()))?;
    // The signature below, checked against this merchant's own secret, is what authorizes the
    // path merchant; nothing is written until it verifies.
    let merchant = crate::auth::AuthorizedMerchant::signed_callback(merchant_id);
    let resolved = creds_store
        .get(&merchant, connector, &account)
        .await?
        .ok_or_else(|| {
            IngestError::UnknownSource(format!("{merchant_id}/{connector}/{account}"))
        })?;

    // 3. Verify the signature against that merchant's secret and extract the report handle. An
    //    unverified body got us this far; nothing past this point runs on an unsigned notification.
    let note =
        source.verify_and_parse_notification(headers, body, &resolved.creds.webhook_secret)?;

    // 4. Enqueue (idempotent on the merchant's notification id — the same notification delivered to
    //    another merchant's endpoint is a separate job, not a duplicate).
    store::enqueue_pending(
        connector,
        &account,
        &merchant,
        &note.notification_id,
        &note.report_ref,
        "webhook",
    )
    .await
}

/// A bad signature is the caller's fault (401), a bad payload too (400), everything else is our
/// side (500). Either way the response is fast and carries no internal detail.
///
/// An unknown source shares the 401 so an unauthenticated caller cannot enumerate which
/// merchant+connector+account tuples are onboarded. `MalformedNotification` stays a 400 because it
/// is also raised *after* the HMAC verifies, where the caller has earned an accurate status.
fn status_for(e: &IngestError) -> StatusCode {
    match e {
        IngestError::SignatureMismatch | IngestError::UnknownSource(_) => StatusCode::UNAUTHORIZED,
        IngestError::UnknownConnector(_) | IngestError::MalformedNotification(_) => {
            StatusCode::BAD_REQUEST
        }
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// Clean a string an unauthenticated caller controls before it reaches a log line. Control
/// characters would otherwise let it forge whole log records.
fn sanitize_for_log(s: &str) -> String {
    const MAX: usize = 256;
    let mut out: String = s
        .chars()
        .take(MAX)
        .map(|c| if c.is_control() { '.' } else { c })
        .collect();
    if s.chars().nth(MAX).is_some() {
        out.push_str("..");
    }
    out
}
