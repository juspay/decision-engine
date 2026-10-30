use crate::decider::gatewaydecider::types::{DomainDeciderRequestForApiCallV2, RankingAlgorithm};
use crate::euclid::ast::{ConnectorInfo, MetadataValue, ValueType};
use crate::euclid::types::{KeyDataType, RoutingRequest, TomlConfig};
use crate::types::hybrid_routing::HybridRoutingRequest;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

pub mod response;

#[derive(Debug, Clone, Serialize, Deserialize, crate::auth::MerchantScoped)]
#[serde(deny_unknown_fields)]
pub struct DecideRequest {
    pub payment_id: String,
    #[merchant]
    pub merchant_id: String,
    pub amount: u64,
    pub currency: String,
    pub payment_method_type: Option<String>,
    pub payment_method_sub_type: Option<String>,
    pub authentication_type: Option<String>,
    pub capture_method: Option<String>,
    pub mandate_type: Option<String>,
    pub mandate_acceptance_type: Option<String>,
    pub setup_future_usage: Option<String>,
    pub transaction_initiator: Option<String>,
    pub surcharge_amount: Option<u64>,
    pub business_country: Option<String>,
    pub billing_country: Option<String>,
    pub business_label: Option<String>,
    pub card: Option<DecideCard>,
    pub customer_device_platform: Option<String>,
    pub customer_device_type: Option<String>,
    pub customer_device_display_size: Option<String>,
    pub pay_later: Option<String>,
    pub wallet: Option<String>,
    pub upi: Option<String>,
    pub voucher: Option<String>,
    pub bank_redirect: Option<String>,
    pub bank_debit: Option<String>,
    pub bank_transfer: Option<String>,
    pub real_time_payment: Option<String>,
    pub gift_card: Option<String>,
    pub card_redirect: Option<String>,
    pub open_banking: Option<String>,
    pub mobile_payment: Option<String>,
    pub crypto: Option<String>,
    pub reward: Option<String>,
    pub network_token: Option<String>,
    pub metadata: Option<BTreeMap<String, String>>,
    pub country: Option<String>,
    pub customer_id: Option<String>,
    pub routing_config: DecideRoutingConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecideCard {
    pub card_isin: Option<String>,
    pub card_extended_bin: Option<String>,
    pub card_network: Option<String>,
    pub card_type: Option<String>,
    pub card_issuer: Option<String>,
    pub card_issuing_country_code: Option<String>,
    pub card_subtype: Option<String>,
    pub card_discovery: Option<String>,
    pub card_switch_provider: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecideRoutingConfig {
    pub algorithm_for: String,
    pub elimination_enabled: Option<bool>,
    pub enable_multi_objective: Option<bool>,
    pub ranking_algorithm: Option<RankingAlgorithm>,
    pub eligible_connectors: Vec<DecideConnector>,
    pub preferred_connectors: Option<Vec<DecideConnector>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DecideConnector {
    pub gateway_name: String,
    /// The connector account. A connector named without one is matched and scored by its
    /// gateway name alone, as a hybrid `fallback_output` entry without a `gateway_id` is.
    #[serde(default)]
    pub payment_merchant_connector_id: Option<String>,
}

impl DecideConnector {
    pub fn identifier(&self) -> String {
        self.to_legacy().identifier()
    }

    /// Reads the decider's `"name:id"` identifier, or a bare `"name"`.
    pub fn from_identifier(identifier: &str) -> Self {
        ConnectorInfo::from_identifier(identifier).into()
    }

    fn to_legacy(&self) -> ConnectorInfo {
        ConnectorInfo {
            gateway_name: self.gateway_name.clone(),
            gateway_id: self.payment_merchant_connector_id.clone(),
        }
    }
}

impl From<ConnectorInfo> for DecideConnector {
    fn from(connector: ConnectorInfo) -> Self {
        Self {
            gateway_name: connector.gateway_name,
            payment_merchant_connector_id: connector.gateway_id,
        }
    }
}

fn routing_key(key: &str) -> &str {
    match key {
        "payment_method_type" => "payment_method",
        "payment_method_sub_type" => "payment_method_type",
        "card_isin" => "card_bin",
        "card_extended_bin" => "extended_card_bin",
        "card_issuer" => "issuer_name",
        "card_issuing_country_code" => "issuer_country",
        key => key,
    }
}

fn routing_value(
    key: &str,
    raw: &serde_json::Value,
    config: &TomlConfig,
) -> Result<Option<ValueType>, String> {
    let Some(field) = config.keys.keys.get(key) else {
        return Ok(None);
    };
    if raw.is_null() {
        return Ok(None);
    }
    match field.data_type {
        KeyDataType::Integer => raw.as_u64().map(ValueType::Number),
        KeyDataType::Enum => raw.as_str().map(|text| {
            let values: Vec<&str> = field
                .values
                .as_deref()
                .unwrap_or_default()
                .split(',')
                .map(str::trim)
                .collect();
            let canonical = values
                .iter()
                .find(|value| **value == text)
                .or_else(|| values.iter().find(|value| value.eq_ignore_ascii_case(text)))
                .copied();
            let country_name = matches!(
                key,
                "billing_country" | "business_country" | "issuer_country"
            )
            .then(|| {
                crate::types::country::routing_names::routing_country_name(
                    &text.to_ascii_uppercase(),
                )
            })
            .flatten();
            ValueType::EnumVariant(canonical.or(country_name).unwrap_or(text).to_string())
        }),
        KeyDataType::GlobalRef => raw
            .as_str()
            .map(|value| ValueType::GlobalRef(value.to_string())),
        KeyDataType::StrValue | KeyDataType::Udf => raw
            .as_str()
            .map(|value| ValueType::StrValue(value.to_string())),
    }
    .map(Some)
    .ok_or_else(|| format!("Invalid value for routing field {key}"))
}

impl DecideRequest {
    pub fn to_hybrid_request(&self, config: &TomlConfig) -> Result<HybridRoutingRequest, String> {
        for (name, value) in [
            ("payment_id", self.payment_id.as_str()),
            ("merchant_id", self.merchant_id.as_str()),
            (
                "routing_config.algorithm_for",
                self.routing_config.algorithm_for.as_str(),
            ),
        ] {
            if value.trim().is_empty() {
                return Err(format!("{name} must not be empty"));
            }
        }
        for connector in self
            .routing_config
            .eligible_connectors
            .iter()
            .chain(self.routing_config.preferred_connectors.iter().flatten())
        {
            if connector.gateway_name.trim().is_empty()
                || connector
                    .payment_merchant_connector_id
                    .as_deref()
                    .is_some_and(|id| id.trim().is_empty())
            {
                return Err(
                    "Connector names and payment_merchant_connector_id must not be empty"
                        .to_string(),
                );
            }
        }

        let value = serde_json::to_value(self).map_err(|error| error.to_string())?;
        let mut parameters = HashMap::new();
        if let Some(fields) = value.as_object() {
            for (key, value) in fields {
                if matches!(
                    key.as_str(),
                    "merchant_id" | "routing_config" | "card" | "metadata"
                ) {
                    continue;
                }
                let key = routing_key(key);
                if let Some(value) = routing_value(key, value, config)? {
                    parameters.insert(key.to_string(), Some(value));
                }
            }
            if let Some(value) = routing_value("payment_amount", &self.amount.into(), config)? {
                parameters.insert("payment_amount".to_string(), Some(value));
            }
            if let Some(card) = fields.get("card").and_then(serde_json::Value::as_object) {
                for (key, value) in card {
                    let key = routing_key(key);
                    if let Some(value) = routing_value(key, value, config)? {
                        parameters.insert(key.to_string(), Some(value));
                    }
                }
            }
            for (key, value) in self.metadata.iter().flatten() {
                if !fields.contains_key(key) && !config.keys.keys.contains_key(key) {
                    parameters.insert(
                        key.clone(),
                        Some(ValueType::MetadataVariant(MetadataValue {
                            key: key.clone(),
                            value: value.clone(),
                        })),
                    );
                }
            }
        }
        let static_request = RoutingRequest {
            payment_id: Some(self.payment_id.clone()),
            created_by: self.merchant_id.clone(),
            fallback_output: Some(
                self.routing_config
                    .eligible_connectors
                    .iter()
                    .map(DecideConnector::to_legacy)
                    .collect(),
            ),
            parameters,
            algorithm_for: Some(self.routing_config.algorithm_for.clone()),
        };
        let dynamic = DomainDeciderRequestForApiCallV2::from_decide(self, &static_request)?;
        Ok(HybridRoutingRequest {
            static_routing_request: Some(static_request),
            dynamic_routing_request: Some(dynamic),
        })
    }
}

#[cfg(test)]
#[allow(clippy::panic_in_result_fn)]
mod tests {
    use super::*;
    use serde_json::{json, Value};

    fn config() -> Result<TomlConfig, Box<dyn std::error::Error>> {
        let document: toml::Value = toml::from_str(include_str!("../../config/development.toml"))?;
        let mut config: TomlConfig = document["routing_config"].clone().try_into()?;
        config.keys = config.keys.with_payment_method_keys();
        Ok(config)
    }

    fn request() -> Result<DecideRequest, serde_json::Error> {
        serde_json::from_str(include_str!(
            "../../tests/fixtures/contracts/decide-request.json"
        ))
    }

    #[test]
    fn decide_maps_the_full_contract_to_existing_rule_keys(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let request = request()?;
        let actual = serde_json::to_value(request.to_hybrid_request(&config()?)?)?;
        let static_request = &actual["static_routing_request"];
        assert_eq!(
            static_request,
            &json!({
                "payment_id": "pay_demo",
                "created_by": "pro_demo",
                "algorithm_for": "payment",
                "fallback_output": [
                    {"gateway_name": "worldpay", "gateway_id": "mca_wp"},
                    {"gateway_name": "checkout", "gateway_id": "mca_cko"}
                ],
                "parameters": {
                    "amount": {"type": "number", "value": 1000},
                    "payment_amount": {"type": "number", "value": 1000},
                    "currency": {"type": "enum_variant", "value": "USD"},
                    "payment_method": {"type": "enum_variant", "value": "card"},
                    "payment_method_type": {"type": "enum_variant", "value": "credit"},
                    "billing_country": {"type": "enum_variant", "value": "UnitedStatesOfAmerica"},
                    "card_bin": {"type": "str_value", "value": "001234"},
                    "extended_card_bin": {"type": "str_value", "value": "00123456"},
                    "card_network": {"type": "enum_variant", "value": "Visa"},
                    "card_type": {"type": "enum_variant", "value": "credit"},
                    "issuer_country": {"type": "enum_variant", "value": "UnitedStatesOfAmerica"},
                    "segment": {"type": "metadata_variant", "value": {"key": "segment", "value": "premium"}}
                }
            })
        );
        assert_eq!(
            actual["dynamic_routing_request"]["paymentInfo"]["cardIsin"],
            "001234"
        );
        assert_eq!(
            actual["dynamic_routing_request"]["paymentInfo"]["country"],
            "US"
        );
        assert_eq!(
            actual["dynamic_routing_request"]["eligibleGatewayList"],
            json!(["worldpay:mca_wp", "checkout:mca_cko"])
        );
        Ok(())
    }

    #[test]
    fn decide_preserves_hybrid_defaults_for_an_equivalent_request(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let flat: DecideRequest = serde_json::from_value(json!({
            "payment_id": "pay_1", "merchant_id": "pro_1", "amount": 100, "currency": "USD",
            "payment_method_type": "card", "payment_method_sub_type": "credit",
            "routing_config": {"algorithm_for": "payment",
                "eligible_connectors": [{"gateway_name": "stripe", "payment_merchant_connector_id": "mca_1"}]}
        }))?;
        let legacy: RoutingRequest = serde_json::from_value(json!({
            "payment_id": "pay_1", "created_by": "pro_1", "algorithm_for": "payment",
            "parameters": {
                "amount": {"type": "number", "value": 100},
                "payment_amount": {"type": "number", "value": 100},
                "currency": {"type": "enum_variant", "value": "USD"},
                "payment_method": {"type": "enum_variant", "value": "card"},
                "payment_method_type": {"type": "enum_variant", "value": "credit"}
            },
            "fallback_output": [{"gateway_name": "stripe", "gateway_id": "mca_1"}]
        }))?;
        let mapped = flat.to_hybrid_request(&config()?)?;
        assert_eq!(
            serde_json::to_value(mapped.static_routing_request)?,
            serde_json::to_value(&legacy)?
        );
        assert_eq!(
            serde_json::to_value(mapped.dynamic_routing_request)?,
            serde_json::to_value(DomainDeciderRequestForApiCallV2::from((&legacy, None)))?
        );
        Ok(())
    }

    #[test]
    fn decide_dynamic_fields_do_not_depend_on_the_static_key_catalog(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let request = request()?;
        let actual = serde_json::to_value(
            request
                .to_hybrid_request(&TomlConfig::default())?
                .dynamic_routing_request,
        )?;
        let info = &actual["paymentInfo"];
        assert_eq!(info["amount"], 1000.0);
        assert_eq!(info["currency"], "USD");
        assert_eq!(info["paymentMethod"], "CARD");
        assert_eq!(info["paymentMethodType"], "CREDIT");
        assert_eq!(info["cardIsin"], "001234");
        assert_eq!(info["cardType"], "CREDIT");
        assert!(!info["cardSwitchProvider"].is_null());
        Ok(())
    }

    #[test]
    fn decide_forwards_optional_dynamic_fields_and_distinct_accounts(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut value = serde_json::to_value(request()?)?;
        value["customer_id"] = json!("cus_1");
        value["authentication_type"] = json!("no_three_ds");
        value["country"] = json!("IN");
        value["card"]["card_issuer"] = json!("Example Bank");
        value["card"]["card_subtype"] = json!("commercial");
        value["routing_config"]["elimination_enabled"] = json!(false);
        value["routing_config"]["enable_multi_objective"] = json!(true);
        value["routing_config"]["ranking_algorithm"] = json!("NTW_SR_HYBRID_ROUTING");
        value["routing_config"]["eligible_connectors"][1] =
            json!({"gateway_name": "worldpay", "payment_merchant_connector_id": "mca_wp_2"});
        value["routing_config"]["preferred_connectors"] = json!([
            {"gateway_name": "worldpay", "payment_merchant_connector_id": "mca_wp_2"}
        ]);
        let request: DecideRequest = serde_json::from_value(value)?;
        let actual = serde_json::to_value(
            request
                .to_hybrid_request(&config()?)?
                .dynamic_routing_request,
        )?;
        let info = &actual["paymentInfo"];
        assert_eq!(info["customerId"], "cus_1");
        assert!(info["udfs"].is_null());
        assert!(info["metadata"].is_null());
        assert!(info["paymentSource"].is_null());
        assert_eq!(info["authType"], "NO_THREE_DS");
        assert_eq!(info["country"], "IN");
        assert!(info["channel"].is_null());
        assert_eq!(info["cardIssuerBankName"], "Example Bank");
        assert_eq!(info["cardProgram"], "commercial");
        assert_eq!(info["preferredConnectors"], json!(["worldpay:mca_wp_2"]));
        assert_eq!(
            actual["eligibleGatewayList"],
            json!(["worldpay:mca_wp", "worldpay:mca_wp_2"])
        );
        assert_eq!(actual["eliminationEnabled"], false);
        assert_eq!(actual["enableMultiObjective"], true);
        assert_eq!(actual["rankingAlgorithm"], "NTW_SR_HYBRID_ROUTING");
        Ok(())
    }

    #[test]
    fn decide_rejects_legacy_fields_and_incorrect_types() -> Result<(), Box<dyn std::error::Error>>
    {
        let baseline = serde_json::to_value(request()?)?;
        for key in [
            "context",
            "payment_type",
            "is_emi",
            "emi_bank",
            "emi_tenure",
            "transaction_type",
            "internal_metadata",
            "card_bin",
            "created_by",
            "payment_amount",
            "order_amount",
            "udfs",
            "payment_metadata",
            "payment_source",
            "auth_type",
            "channel",
            "profile_id",
            "attempt_id",
        ] {
            let mut value = baseline.clone();
            value[key] = Value::Null;
            assert!(
                serde_json::from_value::<DecideRequest>(value).is_err(),
                "accepted {key}"
            );
        }
        for value in [json!(-1), json!(1.2), json!("100")] {
            let mut request = baseline.clone();
            request["amount"] = value;
            assert!(serde_json::from_value::<DecideRequest>(request).is_err());
        }
        let mut value = baseline.clone();
        value["routing_config"]["created_by"] = json!("pro_old");
        assert!(serde_json::from_value::<DecideRequest>(value).is_err());
        let mut value = baseline.clone();
        value["routing_config"]["fallback_connectors"] = json!([]);
        assert!(serde_json::from_value::<DecideRequest>(value).is_err());
        let mut value = baseline;
        value["routing_config"]["eligible_connectors"][0]["gateway_id"] = json!("mca_1");
        assert!(serde_json::from_value::<DecideRequest>(value).is_err());
        Ok(())
    }

    #[test]
    fn decide_metadata_cannot_override_payment_parameters() -> Result<(), Box<dyn std::error::Error>>
    {
        let mut request = request()?;
        request.metadata = Some(BTreeMap::from([
            ("amount".into(), "9999".into()),
            ("payment_amount".into(), "9999".into()),
            ("segment".into(), "premium".into()),
        ]));
        let actual = serde_json::to_value(request.to_hybrid_request(&config()?)?)?;
        let parameters = &actual["static_routing_request"]["parameters"];
        assert_eq!(parameters["amount"]["value"], 1000);
        assert_eq!(parameters["payment_amount"]["value"], 1000);
        assert_eq!(parameters["segment"]["value"]["value"], "premium");
        Ok(())
    }

    #[test]
    fn decide_accepts_connectors_named_without_an_account_like_hybrid(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let flat: DecideRequest = serde_json::from_value(json!({
            "payment_id": "pay_1", "merchant_id": "pro_1", "amount": 100, "currency": "USD",
            "routing_config": {"algorithm_for": "payment",
                "eligible_connectors": [{"gateway_name": "stripe"}, {"gateway_name": "adyen", "payment_merchant_connector_id": null}],
                "preferred_connectors": [{"gateway_name": "adyen"}]}
        }))?;
        let mapped = serde_json::to_value(flat.to_hybrid_request(&config()?)?)?;
        assert_eq!(
            mapped["static_routing_request"]["fallback_output"],
            json!([
                {"gateway_name": "stripe", "gateway_id": null},
                {"gateway_name": "adyen", "gateway_id": null}
            ])
        );
        assert_eq!(
            mapped["dynamic_routing_request"]["eligibleGatewayList"],
            json!(["stripe", "adyen"])
        );
        assert_eq!(
            mapped["dynamic_routing_request"]["paymentInfo"]["preferredConnectors"],
            json!(["adyen"])
        );

        let mut blank = flat;
        blank.routing_config.eligible_connectors[0].payment_merchant_connector_id =
            Some(" ".into());
        assert!(blank.to_hybrid_request(&config()?).is_err());
        Ok(())
    }
}
