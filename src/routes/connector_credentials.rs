//! Merchant-facing API for connector settlement-ingestion credentials.
//!
//! A merchant configures, per `(connector, account)`, the webhook signing secret and the
//! report-download auth. Secrets are encrypted at rest (`ConnectorCredsStore`); this route never
//! returns them back. See `scratch/inhouse-cost-architecture.md` §7–8.

use crate::custom_extractors::Json;
use axum::extract::Path;
use axum::http::StatusCode;
use masking::Secret;
use serde::{Deserialize, Serialize};

use crate::app::get_tenant_app_state;
use crate::cost_ingestion::checkout_credentials::CheckoutEnvironment;
use crate::cost_ingestion::{creds, ConnectorCreds, ConnectorCredsStore};
use crate::error::TextError;

#[derive(Debug, Deserialize)]
pub struct SetCredentialsRequest {
    /// Connector-side account (e.g. Adyen `merchantAccountCode`).
    pub account: String,
    /// Secret used to verify inbound webhook signatures.
    pub webhook_secret: String,
    /// Checkout: plain secret key (sk_…). Adyen: report-user Basic auth as
    /// "user:password", or a Report Service API key on its own (sent as `X-API-Key`).
    pub download_auth: Secret<String>,
    /// Checkout only: omitted means production. The client cannot supply a report API URL.
    #[serde(default)]
    pub environment: Option<CheckoutEnvironment>,
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
) -> Result<Json<SetCredentialsResponse>, TextError> {
    let app_state = get_tenant_app_state().await;
    let cfg = &app_state.config.cost_ingestion;
    let download_auth = if connector == "checkout" {
        body.environment
            .unwrap_or_default()
            .encode_credentials(&body.download_auth, &cfg.connector)
    } else {
        body.download_auth
    };
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
        download_auth,
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
) -> Result<StatusCode, TextError> {
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
) -> Result<Json<Vec<creds::MaskedSource>>, TextError> {
    let app_state = get_tenant_app_state().await;
    let cfg = &app_state.config.cost_ingestion;
    match ConnectorCredsStore::from_keyring(
        &cfg.creds_encryption_current,
        &cfg.creds_encryption_keys,
    ) {
        Some(store) => store
            .list_masked(&merchant_id, cfg)
            .await
            .map(Json)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("{e:?}")).into()),
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
                        environment: None,
                    })
                    .collect(),
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkout_credentials_request_defaults_to_production() -> Result<(), serde_json::Error> {
        let request: SetCredentialsRequest = serde_json::from_value(serde_json::json!({
            "account": "ent_test",
            "webhook_secret": "test_webhook",
            "download_auth": "sk_test",
        }))?;
        assert_eq!(
            request.environment.unwrap_or_default(),
            CheckoutEnvironment::Prod
        );
        Ok(())
    }

    #[test]
    fn checkout_credentials_request_rejects_unknown_environments() {
        assert!(
            serde_json::from_value::<SetCredentialsRequest>(serde_json::json!({
                "account": "ent_test",
                "webhook_secret": "test_webhook",
                "download_auth": "sk_test",
                "environment": "https://custom.example",
            }))
            .is_err()
        );
    }
}
