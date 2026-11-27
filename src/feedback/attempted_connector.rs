//! `attemptedConnector` on `/update-gateway-score`: the connector account a payment was attempted
//! on, in the shape `/decide` returns connectors. An update that sends it is checked against the
//! connectors `/decide` offered for the payment before it is accepted, so an outcome is never
//! scored under a connector the payment was not routed to.

use crate::decider::gatewaydecider::types::{ErrorResponse, UnifiedError};
use crate::feedback::gateway_scoring_service::ScoringContextRead;
use crate::feedback::types::UpdateScorePayload;

/// Why an update naming an `attemptedConnector` was refused.
#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    /// `gateway` was sent too and names another connector.
    Mismatch { attempted: String },
    /// `/decide` offered the payment other connectors.
    NotOffered { offered: Vec<String> },
}

impl Refusal {
    pub fn into_error_response(self, payload: &UpdateScorePayload) -> ErrorResponse {
        let (status, code, message) = match self {
            Self::Mismatch { attempted } => (
                "400",
                "CONNECTOR_MISMATCH",
                format!(
                    "`gateway` ({}) and `attemptedConnector` ({attempted}) name different connectors; \
                     send attemptedConnector alone.",
                    payload.gateway
                ),
            ),
            Self::NotOffered { offered } => (
                "422",
                "CONNECTOR_NOT_OFFERED",
                format!(
                    "{} was not among the connectors /decide offered for payment {}: {}.",
                    payload.gateway,
                    payload.payment_id,
                    offered.join(", ")
                ),
            ),
        };
        ErrorResponse {
            status: status.to_string(),
            error_code: code.to_string(),
            error_message: message.clone(),
            priority_logic_tag: None,
            routing_approach: None,
            filter_wise_gateways: None,
            error_info: UnifiedError {
                code: code.to_string(),
                user_message: message.clone(),
                developer_message: message,
            },
            priority_logic_output: None,
            is_dynamic_mga_enabled: false,
        }
    }
}

/// Checks an update that names its `attemptedConnector`, against the payment's scoring `context`:
/// a `gateway` sent with it must name the same connector, and the connector must be one `/decide`
/// offered the payment.
///
/// Only a stored decision with a connector list is checked against. A payment with no stored
/// decision (routed by its rule with rule-routed scoring off, or past its context's TTL), a
/// decision stored without a list (the decider found no connector, so the caller's fallback
/// answered), and a context that could not be read are not; their updates go on to be scored, or
/// skipped, as an update that names only `gateway` would be.
pub fn check(payload: &UpdateScorePayload, context: &ScoringContextRead) -> Result<(), Refusal> {
    let Some(attempted) = payload.attempted_connector.as_ref() else {
        return Ok(());
    };
    let attempted = attempted.identifier();
    if payload.gateway != attempted {
        return Err(Refusal::Mismatch { attempted });
    }
    match context.found() {
        Some(decision) => offered_check(&attempted, decision.eligibleConnectors.as_deref()),
        None => Ok(()),
    }
}

fn offered_check(gateway: &str, offered: Option<&[String]>) -> Result<(), Refusal> {
    match offered.filter(|offered| !offered.is_empty()) {
        Some(offered) if !offered.iter().any(|connector| connector == gateway) => {
            Err(Refusal::NotOffered {
                offered: offered.to_vec(),
            })
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::{check, offered_check, Refusal};
    use crate::decider::gatewaydecider::types::{GatewayScoringData, GatewayScoringDataV2};
    use crate::feedback::gateway_scoring_service::{ScoringContext, ScoringContextRead};
    use crate::feedback::types::UpdateScorePayload;

    fn update(fields: serde_json::Value) -> UpdateScorePayload {
        let mut body =
            serde_json::json!({"merchantId": "m", "status": "FAILURE", "paymentId": "p"});
        if let (Some(body), Some(fields)) = (body.as_object_mut(), fields.as_object()) {
            body.extend(fields.clone());
        }
        serde_json::from_value(body).expect("an update")
    }

    fn attempted(connector: &str, account: Option<&str>) -> UpdateScorePayload {
        update(serde_json::json!({
            "attemptedConnector": {"connector": connector, "merchant_connector_id": account}
        }))
    }

    fn read(context: ScoringContext) -> ScoringContextRead {
        ScoringContextRead {
            redis_key: "gateway_scoring_data_m_p".to_string(),
            context,
        }
    }

    fn decided(eligible: Option<Vec<&str>>) -> ScoringContextRead {
        let mut stored = serde_json::json!({"merchantId": "m", "paymentMethod": "CARD",
            "paymentMethodType": "CREDIT", "orderType": "ORDER_PAYMENT", "dateCreated": 0});
        if let Some(eligible) = eligible {
            stored["eligibleConnectors"] = serde_json::json!(eligible);
        }
        let data: GatewayScoringDataV2 = serde_json::from_value(stored).expect("a stored context");
        read(ScoringContext::Found(Box::new(GatewayScoringData::from(
            data,
        ))))
    }

    #[test]
    fn a_stored_decision_with_connectors_is_checked() {
        let decision = decided(Some(vec!["stripe:mca_a", "adyen:mca_c"]));
        assert_eq!(check(&attempted("adyen", Some("mca_c")), &decision), Ok(()));
        assert!(matches!(
            check(&attempted("stripe", Some("mca_b")), &decision),
            Err(Refusal::NotOffered { .. })
        ));
        assert!(matches!(
            check(&attempted("stripe", None), &decision),
            Err(Refusal::NotOffered { .. })
        ));
    }

    #[test]
    fn without_a_stored_connector_list_nothing_is_refused() {
        let payload = attempted("stripe", Some("mca_b"));
        for context in [
            decided(None),
            read(ScoringContext::Absent),
            read(ScoringContext::OtherMerchant),
            read(ScoringContext::Unreadable("timeout".to_string())),
        ] {
            assert_eq!(check(&payload, &context), Ok(()));
        }
    }

    #[test]
    fn a_gateway_naming_another_connector_is_refused_whatever_is_stored() {
        let payload = update(serde_json::json!({
            "gateway": "stripe",
            "attemptedConnector": {"connector": "stripe", "merchant_connector_id": "mca_a"}
        }));
        for context in [
            decided(Some(vec!["stripe:mca_a"])),
            read(ScoringContext::Absent),
        ] {
            assert_eq!(
                check(&payload, &context),
                Err(Refusal::Mismatch {
                    attempted: "stripe:mca_a".to_string()
                })
            );
        }
    }

    #[test]
    fn an_update_naming_only_gateway_is_not_checked() {
        let payload = update(serde_json::json!({"gateway": "stripe:mca_b"}));
        assert_eq!(
            check(&payload, &decided(Some(vec!["stripe:mca_a"]))),
            Ok(())
        );
    }

    fn offered() -> Vec<String> {
        vec!["stripe:mca_a".to_string(), "adyen:mca_c".to_string()]
    }

    #[test]
    fn an_offered_connector_is_accepted() {
        assert_eq!(offered_check("stripe:mca_a", Some(&offered())), Ok(()));
        assert_eq!(offered_check("adyen:mca_c", Some(&offered())), Ok(()));
    }

    #[test]
    fn another_account_or_a_bare_name_is_refused() {
        for gateway in ["stripe:mca_b", "stripe", "checkout:mca_k"] {
            assert_eq!(
                offered_check(gateway, Some(&offered())),
                Err(Refusal::NotOffered { offered: offered() }),
                "{gateway}"
            );
        }
    }

    #[test]
    fn a_decision_without_a_connector_list_is_not_checked() {
        assert_eq!(offered_check("stripe:mca_b", None), Ok(()));
        assert_eq!(offered_check("stripe:mca_b", Some(&[])), Ok(()));
    }
}
