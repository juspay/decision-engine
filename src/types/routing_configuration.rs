use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, crate::auth::MerchantScoped)]
pub struct RoutingRule {
    #[merchant]
    pub merchant_id: String,
    pub config: ConfigVariant,
}

/// Read response: an existing merchant may use defaults without a saved configuration.
/// Write requests continue to require a concrete `RoutingRule::config`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleConfigGetResponse {
    pub merchant_id: String,
    pub config: Option<ConfigVariant>,
}

#[derive(Debug, Clone, Serialize, Deserialize, crate::auth::MerchantScoped)]
pub struct FetchRoutingRule {
    #[merchant]
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
    /// Checks each sub-level row's payment method as `/decide-gateway` checks a request's, and
    /// stores it as the engine's codes: `paymentMethod` is the category (`card` → `CARD`),
    /// required, and `paymentMethodType` the specific method within it (`credit` → `CREDIT`),
    /// optional.
    pub fn normalised(mut self) -> Result<Self, String> {
        for row in self.sub_level_input_config.iter_mut().flatten() {
            let instrument = crate::types::payment_instrument::PaymentInstrument::parse(
                row.payment_method.as_deref().unwrap_or_default(),
                row.payment_method_type.as_deref(),
            )
            .map_err(|error| error.to_string())?;
            let method_type = instrument.method_type_code();
            row.payment_method = Some(instrument.method_code().into_owned());
            row.payment_method_type = (!method_type.is_empty()).then(|| method_type.into_owned());
        }
        Ok(self)
    }

    /// Applies an update without touching the autopilot's rows. Rows marked `source: autopilot`
    /// belong to the auto-calibrator, which retunes them in place and treats a missing row as a
    /// cluster it has never tuned. A caller's copy of them is a snapshot that may be stale (a
    /// form loaded before the last retune) or absent (a client that only sends its own rows), so
    /// the stored autopilot rows are kept and the caller's are ignored. A human row for the same
    /// cluster takes it over, as the calibrator never touches human rows. Hard refresh, which
    /// clears autopilot rows on purpose, writes the config directly and is unaffected.
    pub fn keeping_autopilot_rows(mut self, stored: Option<&Self>) -> Self {
        let is_autopilot = |row: &SRSubLevelInputConfig| {
            row.source.as_deref() == Some(crate::sr_auto_calibration::AUTOPILOT_SOURCE)
        };
        let mut rows: Vec<SRSubLevelInputConfig> = self
            .sub_level_input_config
            .take()
            .unwrap_or_default()
            .into_iter()
            .filter(|row| !is_autopilot(row))
            .collect();
        let kept: Vec<SRSubLevelInputConfig> = stored
            .and_then(|stored| stored.sub_level_input_config.as_ref())
            .into_iter()
            .flatten()
            .filter(|row| is_autopilot(row) && !rows.iter().any(|own| own.same_cluster(row)))
            .cloned()
            .collect();
        rows.extend(kept);
        self.sub_level_input_config = (!rows.is_empty()).then_some(rows);
        self
    }
}

/// One sub-level SR override. `paymentMethod` is the payment method category (`CARD`) and
/// `paymentMethodType` the specific method within it (`CREDIT`), in the standard meaning; the
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

impl SRSubLevelInputConfig {
    /// Whether both rows scope the same cluster: every dimension equal, case-insensitively, with
    /// an absent or empty dimension meaning "any".
    fn same_cluster(&self, other: &Self) -> bool {
        fn same(a: &Option<String>, b: &Option<String>) -> bool {
            fn set(v: &Option<String>) -> Option<&str> {
                v.as_deref().filter(|s| !s.is_empty())
            }
            match (set(a), set(b)) {
                (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
                (None, None) => true,
                _ => false,
            }
        }
        same(&self.payment_method, &other.payment_method)
            && same(&self.payment_method_type, &other.payment_method_type)
            && same(&self.card_network, &other.card_network)
            && same(&self.card_is_in, &other.card_is_in)
            && same(&self.currency, &other.currency)
            && same(&self.country, &other.country)
            && same(&self.auth_type, &other.auth_type)
    }
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
        assert!(error.contains("look swapped"), "{error}");
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

#[cfg(test)]
#[allow(clippy::panic_in_result_fn)]
mod autopilot_row_tests {
    use super::SuccessRateData;
    use serde_json::json;

    fn config(rows: serde_json::Value) -> Result<SuccessRateData, serde_json::Error> {
        serde_json::from_value(json!({ "defaultBucketSize": 200, "subLevelInputConfig": rows }))
    }

    fn row(network: &str, bucket: i32, source: Option<&str>) -> serde_json::Value {
        json!({
            "paymentMethod": "CARD", "paymentMethodType": "DEBIT", "cardNetwork": network,
            "currency": "USD", "authType": "THREE_DS", "source": source, "bucketSize": bucket,
            "hedgingPercent": 20.0, "latencyThreshold": null, "lowerResetFactor": null,
            "upperResetFactor": null, "gatewayExtraScore": null,
        })
    }

    /// (card network, bucket size, source) of each row, in order.
    fn rows(config: &SuccessRateData) -> Vec<(String, i32, Option<String>)> {
        config
            .sub_level_input_config
            .iter()
            .flatten()
            .map(|row| {
                (
                    row.card_network.clone().unwrap_or_default(),
                    row.bucket_size.unwrap_or_default(),
                    row.source.clone(),
                )
            })
            .collect()
    }

    fn autopilot() -> Option<String> {
        Some("autopilot".to_string())
    }

    #[test]
    fn an_update_without_autopilot_rows_keeps_them() -> Result<(), serde_json::Error> {
        let stored = config(json!([row("VISA", 300, Some("autopilot"))]))?;
        let update = config(json!([row("MASTERCARD", 50, None)]))?;
        let merged = update.keeping_autopilot_rows(Some(&stored));
        assert_eq!(
            rows(&merged),
            [
                ("MASTERCARD".to_string(), 50, None),
                ("VISA".to_string(), 300, autopilot()),
            ]
        );
        // A client that sends no rows at all doesn't clear them either.
        let merged = config(json!(null))?.keeping_autopilot_rows(Some(&stored));
        assert_eq!(rows(&merged), [("VISA".to_string(), 300, autopilot())]);
        Ok(())
    }

    #[test]
    fn a_stale_copy_of_an_autopilot_row_does_not_revert_it() -> Result<(), serde_json::Error> {
        // The form loaded at 250; the calibrator has since retuned the cluster to 300.
        let stored = config(json!([row("VISA", 300, Some("autopilot"))]))?;
        let update = config(json!([row("VISA", 250, Some("autopilot"))]))?;
        let merged = update.keeping_autopilot_rows(Some(&stored));
        assert_eq!(rows(&merged), [("VISA".to_string(), 300, autopilot())]);
        // Nor does it bring back a row that Hard refresh cleared.
        let update = config(json!([row("VISA", 250, Some("autopilot"))]))?;
        assert!(rows(&update.keeping_autopilot_rows(None)).is_empty());
        Ok(())
    }

    #[test]
    fn a_human_row_for_the_cluster_takes_it_over() -> Result<(), serde_json::Error> {
        let stored = config(json!([row("VISA", 300, Some("autopilot"))]))?;
        let update = config(json!([row("visa", 40, None)]))?;
        let merged = update.keeping_autopilot_rows(Some(&stored));
        assert_eq!(rows(&merged), [("visa".to_string(), 40, None)]);
        Ok(())
    }
}

// These requests name no payment.
crate::impl_api_event_payload!(FetchRoutingRule, RoutingRule);
