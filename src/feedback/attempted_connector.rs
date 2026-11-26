//! `attemptedConnector` on `/update-gateway-score`: the connector account a payment was attempted
//! on, in the shape `/decide` returns connectors. An update that sends it is checked against the
//! connectors `/decide` offered for the payment before it is accepted, so an outcome is never
//! scored under a connector the payment was not routed to.

use crate::decider::gatewaydecider::types::{ErrorResponse, UnifiedError};
use crate::feedback::gateway_scoring_service::{load_scoring_context, ScoringContext};
use crate::feedback::types::UpdateScorePayload;

/// Why an update naming an `attemptedConnector` was refused.
#[derive(Debug, PartialEq, Eq)]
pub enum Refusal {
    /// `gateway` was sent too and names another connector.
    Mismatch { attempted: String },
    /// `/decide` stored nothing for the payment, or what it stored has expired.
    NoDecision,
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
            Self::NoDecision => (
                "404",
                "DECISION_NOT_FOUND",
                format!(
                    "No /decide call is stored for payment {}, so there is no score to update. \
                     Decisions are kept for the scoring-context TTL.",
                    payload.payment_id
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

/// Checks an update that names its `attemptedConnector`: a `gateway` sent with it must name the
/// same connector, and the connector must be one `/decide` offered for the payment. An update that
/// names only `gateway` is not checked, nor is one whose stored decision has no connector list
/// (stored before the list was kept) or could not be read; those are scored as before.
pub async fn check(payload: &UpdateScorePayload) -> Result<(), Refusal> {
    let Some(attempted) = payload.attempted_connector.as_ref() else {
        return Ok(());
    };
    let attempted = attempted.identifier();
    if payload.gateway != attempted {
        return Err(Refusal::Mismatch { attempted });
    }
    match load_scoring_context(&payload.merchant_id, &payload.payment_id).await {
        ScoringContext::Found(data) => offered_check(&payload.gateway, data.eligibleConnectors),
        ScoringContext::Absent => Err(Refusal::NoDecision),
        ScoringContext::Unreadable => Ok(()),
    }
}

fn offered_check(gateway: &str, offered: Option<Vec<String>>) -> Result<(), Refusal> {
    match offered.filter(|offered| !offered.is_empty()) {
        Some(offered) if !offered.iter().any(|connector| connector == gateway) => {
            Err(Refusal::NotOffered { offered })
        }
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::{offered_check, Refusal};

    fn offered() -> Option<Vec<String>> {
        Some(vec!["stripe:mca_a".to_string(), "adyen:mca_c".to_string()])
    }

    #[test]
    fn an_offered_connector_is_accepted() {
        assert_eq!(offered_check("stripe:mca_a", offered()), Ok(()));
        assert_eq!(offered_check("adyen:mca_c", offered()), Ok(()));
    }

    #[test]
    fn another_account_or_a_bare_name_is_refused() {
        for gateway in ["stripe:mca_b", "stripe", "checkout:mca_k"] {
            assert_eq!(
                offered_check(gateway, offered()),
                Err(Refusal::NotOffered {
                    offered: offered().unwrap_or_default()
                }),
                "{gateway}"
            );
        }
    }

    #[test]
    fn a_decision_without_a_connector_list_is_not_checked() {
        assert_eq!(offered_check("stripe:mca_b", None), Ok(()));
        assert_eq!(offered_check("stripe:mca_b", Some(Vec::new())), Ok(()));
    }
}
