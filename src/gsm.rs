//! GSM lookups: the pod's own rules, or Hyperswitch's GSM API with `[gsm] hyperswitch_lookup` on.

use std::sync::OnceLock;
use std::time::Duration;

use masking::PeekInterface;

use crate::app::get_tenant_app_state;
use crate::logger;

pub use gsm::{options, GsmConfig, GsmDecision, GsmErrorInfo, GsmInfo, GsmOptionRow};

/// A source of GSM rules: the pod's loaded rules or Hyperswitch's GSM API.
#[async_trait::async_trait]
pub trait GsmSource: Send + Sync {
    /// The rule matching a failure, if any: issuer code first, then the connector's error code.
    async fn find_rule(&self, connector: &str, error_info: &GsmErrorInfo) -> Option<GsmInfo>;
}

/// Holds the GSM source this process uses, chosen once at startup from `[gsm]`.
pub struct Gsm;

/// The chosen source; unset only before startup, when the pod source applies.
static SOURCE: OnceLock<Box<dyn GsmSource>> = OnceLock::new();

impl Gsm {
    /// Picks the GSM source from `[gsm]`, loading rules into the pod only for the pod source.
    pub async fn init_source(config: &GsmConfig) {
        let source: Box<dyn GsmSource> = if config.hyperswitch_lookup {
            logger::info!(
                tag = "GSM",
                "GSM rules are looked up in Hyperswitch; none are loaded in the pod"
            );
            Box::new(HyperswitchGsmSource)
        } else {
            gsm::init(config).await;
            Box::new(PodGsmSource)
        };
        let _ = SOURCE.set(source);
    }

    /// The GSM source to look failures up in.
    pub fn source() -> &'static dyn GsmSource {
        SOURCE.get().map_or(&PodGsmSource, |source| source.as_ref())
    }
}

/// A gateway identifier as Decision Engine writes it: `connector` or `connector:merchant_connector_id`.
pub trait GatewayIdentifier {
    /// The connector it names, as GSM rules key it: `checkout:mca_cko` -> `checkout`.
    fn connector_name(&self) -> &str;
}

impl GatewayIdentifier for str {
    fn connector_name(&self) -> &str {
        self.split(':').next().unwrap_or(self)
    }
}

/// GSM source backed by rules loaded into each pod at startup from `[gsm] source`.
pub struct PodGsmSource;

#[async_trait::async_trait]
impl GsmSource for PodGsmSource {
    async fn find_rule(&self, connector: &str, error_info: &GsmErrorInfo) -> Option<GsmInfo> {
        gsm::lookup(connector, error_info)
    }
}

/// GSM source backed by Hyperswitch's GSM API, called on every lookup; nothing kept in the pod.
pub struct HyperswitchGsmSource;

#[async_trait::async_trait]
impl GsmSource for HyperswitchGsmSource {
    async fn find_rule(&self, connector: &str, error_info: &GsmErrorInfo) -> Option<GsmInfo> {
        // The same two steps Hyperswitch takes; its API answers one exact key at a time.
        let issuer_rule = match (
            error_info.card_network.as_deref(),
            error_info.issuer_error_code.as_deref(),
        ) {
            (Some(network), Some(issuer_code)) => {
                let key = gsm::issuer_lookup_key(network, issuer_code);
                self.find_rule_by_key(connector, error_info, &key, &key)
                    .await
            }
            _ => None,
        };
        match issuer_rule {
            Some(info) => Some(info),
            None => {
                self.find_rule_by_key(
                    connector,
                    error_info,
                    error_info.error_code.as_deref().unwrap_or(""),
                    error_info.error_message.as_deref().unwrap_or(""),
                )
                .await
            }
        }
    }
}

/// The body of Hyperswitch's `POST /gsm/get`: one exact rule key.
#[derive(serde::Serialize)]
struct HyperswitchGsmRequest<'a> {
    connector: &'a str,
    flow: &'a str,
    sub_flow: &'a str,
    code: &'a str,
    message: &'a str,
}

/// The part of Hyperswitch's `GsmResponse` that Decision Engine returns as `gsm_info`.
#[derive(serde::Deserialize)]
struct HyperswitchGsmResponse {
    decision: String,
    step_up_possible: bool,
    clear_pan_possible: bool,
    unified_code: Option<String>,
    unified_message: Option<String>,
    error_category: Option<String>,
    standardised_code: Option<String>,
    description: Option<String>,
    user_guidance_message: Option<String>,
    feature_data: Option<HyperswitchGsmFeatureData>,
}

/// Hyperswitch's `feature_data`; only its retry part carries `alternate_network_possible`.
#[derive(serde::Deserialize)]
struct HyperswitchGsmFeatureData {
    retry: Option<HyperswitchRetryFeatureData>,
}

/// Hyperswitch's `feature_data.retry`.
#[derive(serde::Deserialize)]
struct HyperswitchRetryFeatureData {
    alternate_network_possible: bool,
}

// A Hyperswitch rule as the `gsm_info` Decision Engine returns.
impl From<HyperswitchGsmResponse> for GsmInfo {
    fn from(rule: HyperswitchGsmResponse) -> Self {
        Self {
            decision: rule.decision,
            step_up_possible: rule.step_up_possible,
            clear_pan_possible: rule.clear_pan_possible,
            alternate_network_possible: rule
                .feature_data
                .and_then(|data| data.retry)
                .is_some_and(|retry| retry.alternate_network_possible),
            unified_code: rule.unified_code,
            unified_message: rule.unified_message,
            error_category: rule.error_category,
            standardised_code: rule.standardised_code,
            description: rule.description,
            user_guidance_message: rule.user_guidance_message,
            retry_connectors: None,
        }
    }
}

impl HyperswitchGsmSource {
    /// Asks Hyperswitch for the rule with one exact key.
    async fn find_rule_by_key(
        &self,
        connector: &str,
        error_info: &GsmErrorInfo,
        code: &str,
        message: &str,
    ) -> Option<GsmInfo> {
        self.call_gsm_get_api(&HyperswitchGsmRequest {
            connector,
            flow: &error_info.flow,
            sub_flow: &error_info.sub_flow,
            code,
            message,
        })
        .await
    }

    /// Calls Hyperswitch's `POST /gsm/get`; `None` on 404/400 or a failure, which is logged.
    async fn call_gsm_get_api(&self, request: &HyperswitchGsmRequest<'_>) -> Option<GsmInfo> {
        let app_state = get_tenant_app_state().await;
        let cfg = &app_state.config.hyperswitch_service;
        if cfg.api_key.peek().is_empty() {
            logger::warn!(
                tag = "GSM",
                "hyperswitch_service.api_key not configured; skipping the Hyperswitch GSM lookup"
            );
            None
        } else {
            let url = cfg.gsm_url();
            let call = crate::types::card::card_info_api::client()
                .post(&url)
                .header("api-key", cfg.api_key.peek().as_str())
                .header("x-tenant-id", cfg.tenant_id.as_str())
                .json(request)
                .send();
            match tokio::time::timeout(Duration::from_millis(cfg.timeout_ms), call).await {
                Ok(Ok(response)) => Self::read_gsm_get_response(&url, response).await,
                Ok(Err(error)) => {
                    logger::warn!(tag = "GSM", url, error = %error.without_url(), "Hyperswitch GSM request failed");
                    None
                }
                Err(_) => {
                    logger::warn!(
                        tag = "GSM",
                        url,
                        "Hyperswitch GSM lookup timed out after {}ms",
                        cfg.timeout_ms
                    );
                    None
                }
            }
        }
    }

    /// Reads `/gsm/get`'s answer: the rule on success, otherwise a logged `None`.
    async fn read_gsm_get_response(url: &str, response: reqwest::Response) -> Option<GsmInfo> {
        let status = response.status();
        match response.error_for_status() {
            Ok(response) => match response.json::<HyperswitchGsmResponse>().await {
                Ok(rule) => Some(rule.into()),
                Err(error) => {
                    logger::warn!(tag = "GSM", url, error = %error.without_url(), "Hyperswitch GSM response unreadable");
                    None
                }
            },
            Err(_) => {
                logger::warn!(
                    tag = "GSM",
                    url,
                    status = status.as_u16(),
                    "{}",
                    Self::gsm_get_failure_reason(status)
                );
                None
            }
        }
    }

    /// Why `/gsm/get` returned no rule, by its non-2xx status.
    fn gsm_get_failure_reason(status: reqwest::StatusCode) -> &'static str {
        match status {
            reqwest::StatusCode::NOT_FOUND => "No GSM rule found in Hyperswitch for this key",
            reqwest::StatusCode::BAD_REQUEST => {
                "Hyperswitch rejected the GSM lookup (unknown connector or bad key)"
            }
            _ => "Hyperswitch GSM lookup failed",
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{GsmInfo, HyperswitchGsmResponse};

    // Hyperswitch's response maps to `gsm_info`; alternate network comes only from `feature_data.retry`.
    #[test]
    fn a_hyperswitch_rule_maps_to_gsm_info() {
        let rule: HyperswitchGsmResponse = serde_json::from_value(serde_json::json!({
            "connector": "adyen", "flow": "Payment", "sub_flow": "Authorize", "code": "15_013",
            "message": "m", "status": "Failure", "router_error": null, "decision": "retry",
            "step_up_possible": true, "unified_code": "UE_3000",
            "unified_message": "Technical issue with PSP", "error_category": "processor_downtime",
            "clear_pan_possible": false, "feature": "retry",
            "feature_data": {"retry": {"step_up_possible": true, "clear_pan_possible": false,
                "alternate_network_possible": true, "decision": "retry"}},
            "standardised_code": null, "description": null, "user_guidance_message": null
        }))
        .expect("a Hyperswitch GsmResponse");
        let info = GsmInfo::from(rule);
        assert_eq!(info.decision, "retry");
        assert!(info.step_up_possible && info.alternate_network_possible);
        assert_eq!(info.unified_code.as_deref(), Some("UE_3000"));
        assert!(info.retry_connectors.is_none());

        let without_feature_data: HyperswitchGsmResponse =
            serde_json::from_value(serde_json::json!({
                "decision": "do_default", "step_up_possible": false, "clear_pan_possible": false,
                "unified_code": null, "unified_message": null, "error_category": null,
                "feature_data": null, "standardised_code": null, "description": null,
                "user_guidance_message": null
            }))
            .expect("a Hyperswitch GsmResponse without feature_data");
        assert!(!GsmInfo::from(without_feature_data).alternate_network_possible);
    }
}
