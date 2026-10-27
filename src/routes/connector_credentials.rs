//! Merchant-facing API for connector settlement-ingestion credentials.
//!
//! A merchant configures, per `(connector, account)`, the webhook signing secret and the
//! report-download auth. Secrets are encrypted at rest (`ConnectorCredsStore`); this route never
//! returns them back. See `scratch/inhouse-cost-architecture.md` §7–8.

use crate::custom_extractors::MerchantFromPath;

use crate::custom_extractors::Json;
use axum::extract::Path;
use axum::http::StatusCode;
use masking::Secret;
use serde::{Deserialize, Serialize};

use crate::app::get_tenant_app_state;
use crate::cost_ingestion::{creds, ConnectorCreds, ConnectorCredsStore};
use crate::error::RouteError;

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
    MerchantFromPath(merchant): MerchantFromPath,
    Path((_, connector)): Path<(String, String)>,
    Json(body): Json<SetCredentialsRequest>,
) -> Result<Json<SetCredentialsResponse>, RouteError> {
    let merchant_id = merchant.as_str().to_string();
    let app_state = get_tenant_app_state().await;
    let cfg = &app_state.config.cost_ingestion;
    let store = ConnectorCredsStore::from_keyring(
        &cfg.creds_encryption_current,
        &cfg.creds_encryption_keys,
    )
    .ok_or(RouteError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        "credential encryption keyring not configured",
    ))?;

    let new = ConnectorCreds {
        webhook_secret: Secret::new(body.webhook_secret),
        download_auth: Secret::new(body.download_auth),
    };
    store
        .put(&connector, &body.account, &merchant, &new)
        .await
        .map_err(RouteError::internal)?;

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
    MerchantFromPath(merchant): MerchantFromPath,
    Path((_, connector, account)): Path<(String, String, String)>,
) -> Result<StatusCode, RouteError> {
    creds::delete_source(&connector, &account, &merchant)
        .await
        .map_err(RouteError::internal)?;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /merchant-account/:merchant_id/connectors` — configured sources with *masked* credential
/// previews (last-4 hints only, never full secrets). Falls back to source ids without hints when
/// the credential keyring isn't configured (nothing decryptable).
pub async fn list_connector_credentials(
    MerchantFromPath(merchant): MerchantFromPath,
) -> Result<Json<Vec<creds::MaskedSource>>, RouteError> {
    let app_state = get_tenant_app_state().await;
    let cfg = &app_state.config.cost_ingestion;
    match ConnectorCredsStore::from_keyring(
        &cfg.creds_encryption_current,
        &cfg.creds_encryption_keys,
    ) {
        Some(store) => store
            .list_masked(&merchant)
            .await
            .map(Json)
            .map_err(RouteError::internal),
        None => {
            let sources = creds::list_sources(&merchant)
                .await
                .map_err(RouteError::internal)?;
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
