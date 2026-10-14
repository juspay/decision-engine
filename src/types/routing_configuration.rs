use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoutingRule {
    pub merchant_id: String,
    pub config: ConfigVariant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetchRoutingRule {
    pub merchant_id: String,
    pub algorithm: AlgorithmType,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AlgorithmType {
    SuccessRate,
    Elimination,
    DebitRouting,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
#[serde(rename_all = "camelCase")]
pub enum ConfigVariant {
    SuccessRate(SuccessRateData),
    Elimination(EliminationData),
    DebitRouting(DebitRoutingData),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuccessRateData {
    pub default_latency_threshold: Option<f64>,
    pub default_bucket_size: Option<i32>,
    pub default_hedging_percent: Option<f64>,
    pub default_lower_reset_factor: Option<f64>,
    pub default_upper_reset_factor: Option<f64>,
    pub default_gateway_extra_score: Option<Vec<GatewayWiseExtraScore>>,
    pub sub_level_input_config: Option<Vec<SRSubLevelInputConfig>>,
    /// Merchant margin (fraction of ticket, e.g. 0.20). Used in the multi-objective
    /// expected-value ranking `EV = auth·(margin − cost/10_000)`; there is no auth
    /// band or admission gate. Defaults to [`crate::decider::gatewaydecider::
    /// multi_objective::DEFAULT_MARGIN`] when unset.
    pub margin: Option<f64>,
}

impl SuccessRateData {
    /// Checks each sub-level row's payment method in Hyperswitch's meaning and stores it as the
    /// engine's codes: `paymentMethod` is the category (`card` → `CARD`), required, and
    /// `paymentMethodType` the specific method within it (`credit` → `CREDIT`), optional.
    pub fn normalised(mut self) -> Result<Self, String> {
        for row in self.sub_level_input_config.iter_mut().flatten() {
            let method = row.payment_method.as_deref().ok_or_else(|| {
                "each subLevelInputConfig row names its paymentMethod (card, wallet, upi...)"
                    .to_string()
            })?;
            match row.payment_method_type.as_deref() {
                Some(method_type) => {
                    let instrument =
                        crate::types::payment_instrument::PaymentInstrument::from_hyperswitch(
                            method,
                            method_type,
                        )
                        .map_err(|error| error.to_string())?;
                    row.payment_method = Some(instrument.method_code().to_string());
                    row.payment_method_type = Some(instrument.method_type_code().into_owned());
                }
                None => {
                    let category =
                        crate::types::payment_instrument::PaymentMethod::from_hyperswitch(method)
                            .ok_or_else(|| {
                            format!(
                                "paymentMethod `{method}` is not a payment method (card, \
                                     wallet, upi...)"
                            )
                        })?;
                    row.payment_method = Some(category.code().to_string());
                }
            }
        }
        Ok(self)
    }
}

/// One sub-level SR override. `paymentMethod` is the payment method category (`CARD`) and
/// `paymentMethodType` the specific method within it (`CREDIT`), in Hyperswitch's meaning; the
/// row applies to payments matching every dimension it sets.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SRSubLevelInputConfig {
    pub payment_method_type: Option<String>,
    pub payment_method: Option<String>,
    // Optional cluster dimensions (serialize as cardNetwork/cardIsIn/currency/country/authType).
    // Required here so dimension-scoped sub-level overrides round-trip through /rule/get and
    // /rule/update instead of being silently dropped by serde.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card_network: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card_is_in: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth_type: Option<String>,
    /// Provenance: "autopilot" for entries the auto-calibrator created/manages, absent for
    /// human-authored ones. Lets Hard refresh wipe only auto entries and lets the job avoid
    /// overwriting manual overrides.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub latency_threshold: Option<f64>,
    pub bucket_size: Option<i32>,
    pub hedging_percent: Option<f64>,
    pub lower_reset_factor: Option<f64>,
    pub upper_reset_factor: Option<f64>,
    pub gateway_extra_score: Option<Vec<GatewayWiseExtraScore>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayWiseExtraScore {
    pub gateway_name: String,
    pub gateway_sigma_factor: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EliminationData {
    pub threshold: f64,
    pub txnLatency: Option<TransactionLatencyThreshold>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DebitRoutingData {
    pub merchant_category_code: String,
    pub acquirer_country: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionLatencyThreshold {
    /// To have a hard threshold for latency in millis, which is used to filter out gateways that exceed this threshold.
    pub gatewayLatency: Option<f64>,
}

#[cfg(test)]
#[allow(clippy::panic_in_result_fn)]
mod success_rate_normalisation_tests {
    use super::SuccessRateData;
    use serde_json::json;

    fn config(rows: serde_json::Value) -> Result<SuccessRateData, serde_json::Error> {
        serde_json::from_value(json!({ "defaultBucketSize": 200, "subLevelInputConfig": rows }))
    }

    fn row(method: &str, method_type: Option<&str>) -> serde_json::Value {
        json!({
            "paymentMethod": method, "paymentMethodType": method_type,
            "bucketSize": 50, "hedgingPercent": null, "latencyThreshold": null,
            "lowerResetFactor": null, "upperResetFactor": null, "gatewayExtraScore": null,
        })
    }

    #[test]
    fn rows_are_stored_as_codes() -> Result<(), serde_json::Error> {
        let normalised =
            config(json!([row("card", Some("credit")), row("wallet", None)]))?.normalised();
        let rows = normalised
            .ok()
            .and_then(|config| config.sub_level_input_config)
            .unwrap_or_default();
        let pairs: Vec<_> = rows
            .iter()
            .map(|row| (row.payment_method.clone(), row.payment_method_type.clone()))
            .collect();
        assert_eq!(
            pairs,
            [
                (Some("CARD".to_string()), Some("CREDIT".to_string())),
                (Some("WALLET".to_string()), None),
            ]
        );
        Ok(())
    }

    #[test]
    fn a_legacy_order_row_is_rejected() -> Result<(), serde_json::Error> {
        let error = config(json!([row("CREDIT", Some("CARD"))]))?
            .normalised()
            .err()
            .unwrap_or_default();
        assert!(error.contains("looks swapped"), "{error}");
        let error = config(json!([row("card", Some("google_pay"))]))?
            .normalised()
            .err()
            .unwrap_or_default();
        assert!(
            error.contains("is not a card payment method type"),
            "{error}"
        );
        Ok(())
    }
}
