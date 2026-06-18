use masking::PeekInterface;
use serde::{Deserialize, Serialize};

use crate::types::card::txn_card_info::TxnCardInfo;
use crate::types::txn_details::types::TxnDetail;

/// Merchant category code sent on every cost lookup. Fixed in code for now.
const DEFAULT_MCC: &str = "4722";

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ClusterKey {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transaction_currency: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payment_method_type: Option<String>,
    /// Card tier (e.g. "standard"); derived from the card program.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_network: Option<String>,
    pub mcc: String,
    pub cross_border_flag: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_bin: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_issuing_bank: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card_issuing_country: Option<String>,
    pub psp_array: Vec<String>,
}

pub fn derive_cluster_key(txn_detail: &TxnDetail, txn_card_info: &TxnCardInfo) -> ClusterKey {
    ClusterKey {
        amount: txn_detail.netAmount.as_ref().map(|m| m.to_double()),
        transaction_currency: Some(format!("{:?}", txn_detail.currency)),
        payment_method_type: txn_card_info
            .card_type
            .as_ref()
            .map(|ct| format!("{:?}", ct).to_lowercase()),
        card_type: txn_card_info
            .card_program
            .as_ref()
            .and_then(|s| non_empty(s))
            .map(|s| s.to_lowercase()),
        card_network: txn_card_info
            .cardSwitchProvider
            .as_ref()
            .map(|s| s.peek().to_lowercase()),
        mcc: DEFAULT_MCC.to_string(),
        cross_border_flag: false,
        card_bin: txn_card_info
            .card_isin
            .as_ref()
            .and_then(|s| non_empty(s))
            .and_then(|s| s.parse::<u64>().ok()),
        card_issuing_bank: txn_card_info
            .cardIssuerBankName
            .as_ref()
            .and_then(|s| non_empty(s))
            .map(|s| s.to_lowercase()),
        // Issuer country isn't available from txn data yet.
        card_issuing_country: None,
        psp_array: Vec::new(),
    }
}

fn non_empty(s: &str) -> Option<String> {
    if s.trim().is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}
