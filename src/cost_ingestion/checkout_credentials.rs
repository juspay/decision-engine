//! Checkout dashboard input is a secret key and an environment, never a destination URL.

use masking::{PeekInterface, Secret};
use serde::{Deserialize, Serialize};

use crate::config::ConnectorReportApiConfig;

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CheckoutEnvironment {
    #[default]
    Prod,
    Sandbox,
}

impl CheckoutEnvironment {
    /// Preserve the credential format consumed by the existing Checkout report source.
    pub fn encode_credentials(
        self,
        secret_key: &Secret<String>,
        config: &ConnectorReportApiConfig,
    ) -> Secret<String> {
        let api_base_url = match self {
            Self::Prod => &config.checkout_production_api_base_url,
            Self::Sandbox => &config.checkout_sandbox_api_base_url,
        };
        Secret::new(
            serde_json::json!({
                "secret_key": secret_key.peek(),
                "api_base_url": api_base_url,
            })
            .to_string(),
        )
    }

    /// Match the stored Checkout destination to its configured environment.
    pub fn from_credentials(raw: &str, config: &ConnectorReportApiConfig) -> Option<Self> {
        let value: serde_json::Value = serde_json::from_str(raw).ok()?;
        let url = value.get("api_base_url")?.as_str()?;
        if url == config.checkout_production_api_base_url {
            Some(Self::Prod)
        } else if url == config.checkout_sandbox_api_base_url {
            Some(Self::Sandbox)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkout_endpoints_load_from_nested_development_config() -> Result<(), toml::de::Error> {
        let config: toml::Value = toml::from_str(include_str!("../../config/development.toml"))?;
        let ingestion: crate::config::CostIngestionConfig =
            config["cost_ingestion"].clone().try_into()?;
        assert_eq!(
            ingestion.connector.checkout_production_api_base_url,
            "https://api.checkout.com"
        );
        assert_eq!(
            ingestion.connector.checkout_sandbox_api_base_url,
            "https://api.sandbox.checkout.com"
        );
        Ok(())
    }

    #[test]
    fn checkout_environment_uses_configured_urls_and_preserves_storage_shape() {
        let config = ConnectorReportApiConfig {
            checkout_production_api_base_url: "https://configured-prod.example".into(),
            checkout_sandbox_api_base_url: "https://configured-sandbox.example".into(),
        };
        for (environment, expected_url) in [
            (
                CheckoutEnvironment::Prod,
                &config.checkout_production_api_base_url,
            ),
            (
                CheckoutEnvironment::Sandbox,
                &config.checkout_sandbox_api_base_url,
            ),
        ] {
            let encoded = environment.encode_credentials(&Secret::new("test_key".into()), &config);
            let value =
                serde_json::from_str::<serde_json::Value>(encoded.peek()).unwrap_or_default();
            assert_eq!(value["secret_key"], "test_key");
            assert_eq!(value["api_base_url"], expected_url.as_str());
            assert_eq!(
                CheckoutEnvironment::from_credentials(encoded.peek(), &config),
                Some(environment)
            );
        }
    }

    #[test]
    fn checkout_keeps_key_opaque_and_uses_only_configured_destination() {
        let config = ConnectorReportApiConfig {
            checkout_production_api_base_url: "https://configured-prod.example".into(),
            ..ConnectorReportApiConfig::default()
        };
        let raw = r#"{"secret_key":"test","api_base_url":"https://other.example"}"#;
        let encoded =
            CheckoutEnvironment::Prod.encode_credentials(&Secret::new(raw.into()), &config);
        let value = serde_json::from_str::<serde_json::Value>(encoded.peek()).unwrap_or_default();
        assert_eq!(value["secret_key"], raw);
        assert_eq!(
            value["api_base_url"],
            config.checkout_production_api_base_url
        );
    }
}
