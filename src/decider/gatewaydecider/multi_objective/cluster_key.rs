use masking::PeekInterface;
use serde::{Deserialize, Serialize};

use crate::types::card::txn_card_info::TxnCardInfo;
use crate::types::txn_details::types::TxnDetail;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ClusterKey {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_network: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_issuer_bank: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issuer_country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub billing_country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_program: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
}

pub fn derive_cluster_key(txn_detail: &TxnDetail, txn_card_info: &TxnCardInfo) -> ClusterKey {
    ClusterKey {
        payment_method: non_empty(&txn_card_info.paymentMethod),
        card_type: txn_card_info
            .card_type
            .as_ref()
            .map(|ct| format!("{:?}", ct).to_lowercase()),
        card_network: txn_card_info
            .cardSwitchProvider
            .as_ref()
            .map(|s| s.peek().to_lowercase()),
        card_issuer_bank: txn_card_info
            .cardIssuerBankName
            .as_ref()
            .and_then(|s| non_empty(s))
            .map(|s| s.to_lowercase()),
        issuer_country: None,
        billing_country: txn_detail.country.as_ref().map(|c| format!("{:?}", c)),
        card_program: None,
        currency: Some(format!("{:?}", txn_detail.currency)),
    }
}

fn non_empty(s: &str) -> Option<String> {
    if s.trim().is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn non_empty_filters_blanks() {
        assert_eq!(non_empty(""), None);
        assert_eq!(non_empty("   "), None);
        assert_eq!(non_empty("card"), Some("card".to_string()));
    }

    #[test]
    fn specificity_increases_with_set_fields() {
        let key = ClusterKey {
            payment_method: Some("card".into()),
            ..ClusterKey::default()
        };
        let serialized = serde_json::to_value(&key).unwrap();
        let obj = serialized.as_object().unwrap();
        assert!(obj.contains_key("payment_method"));
        assert!(!obj.contains_key("card_type"));
    }
}
