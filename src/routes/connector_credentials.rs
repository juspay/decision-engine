//! Merchant-facing API for connector settlement-ingestion credentials.
//!
//! A merchant configures, per `(connector, account)`, the webhook signing secret and the
//! report-download auth. Secrets are encrypted at rest (`ConnectorCredsStore`); this route never
//! returns them back. See `scratch/inhouse-cost-architecture.md` §7–8.

use axum::extract::Path;
use axum::http::StatusCode;
use axum::Json;
use masking::Secret;
use serde::{Deserialize, Serialize};

use crate::app::get_tenant_app_state;
use crate::cost_ingestion::{creds, ConnectorCreds, ConnectorCredsStore};

#[derive(Debug, Deserialize)]
pub struct SetCredentialsRequest {
    /// Connector-side account (e.g. Adyen `merchantAccountCode`).
    pub account: String,
    /// Secret used to verify inbound webhook signatures.
    pub webhook_secret: String,
    /// Credential used to authenticate report downloads. Either report-user Basic auth as
    /// "user:password", or a Report Service API key on its own (sent as `X-API-Key`).
    pub download_auth: String,
}

#[derive(Debug, Serialize)]
pub struct SetCredentialsResponse {
    pub merchant_id: String,
    pub connector: String,
    pub account: String,
    pub status: String,
}

/// `POST /merchant-account/:merchant_id/connectors/:connector/credentials`
pub async fn set_connector_credentials(
    Path((merchant_id, connector)): Path<(String, String)>,
    Json(body): Json<SetCredentialsRequest>,
) -> Result<Json<SetCredentialsResponse>, (StatusCode, String)> {
    let app_state = get_tenant_app_state().await;
    let cfg = &app_state.config.cost_ingestion;
    let store = ConnectorCredsStore::from_keyring(
        &cfg.creds_encryption_current,
        &cfg.creds_encryption_keys,
    )
    .ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "credential encryption keyring not configured".to_string(),
    ))?;

    let new = ConnectorCreds {
        webhook_secret: Secret::new(body.webhook_secret),
        download_auth: Secret::new(body.download_auth),
    };
    store
        .put(&connector, &body.account, &merchant_id, &new)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:?}")))?;

    Ok(Json(SetCredentialsResponse {
        merchant_id,
        connector,
        account: body.account,
        status: "saved".to_string(),
    }))
}

/// `DELETE /merchant-account/:merchant_id/connectors/:connector/credentials/:account`
///
/// Removes the stored credentials and the source-index entry so the pair drops off the configured
/// list. Idempotent — deleting a source that isn't there returns success.
pub async fn delete_connector_credentials(
    Path((merchant_id, connector, account)): Path<(String, String, String)>,
) -> Result<StatusCode, (StatusCode, String)> {
    creds::delete_source(&connector, &account, &merchant_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:?}")))?;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /merchant-account/:merchant_id/connectors` — configured sources with *masked* credential
/// previews (last-4 hints only, never full secrets). Falls back to source ids without hints when
/// the credential keyring isn't configured (nothing decryptable).
pub async fn list_connector_credentials(
    Path(merchant_id): Path<String>,
) -> Result<Json<Vec<creds::MaskedSource>>, (StatusCode, String)> {
    let app_state = get_tenant_app_state().await;
    let cfg = &app_state.config.cost_ingestion;
    match ConnectorCredsStore::from_keyring(
        &cfg.creds_encryption_current,
        &cfg.creds_encryption_keys,
    ) {
        Some(store) => store
            .list_masked(&merchant_id)
            .await
            .map(Json)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:?}"))),
        None => {
            let sources = creds::list_sources(&merchant_id)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:?}")))?;
            Ok(Json(
                sources
                    .into_iter()
                    .map(|s| creds::MaskedSource {
                        connector: s.connector,
                        account: s.account,
                        webhook_secret_hint: "—".to_string(),
                        download_auth_hint: "—".to_string(),
                    })
                    .collect(),
            ))
        }
    }
}
