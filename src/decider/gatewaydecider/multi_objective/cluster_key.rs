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
        // Normalize the card's issuer country into a pricing region bucket. This is what
        // separates the same-currency (USD) cost scenarios at a US merchant — regulated US
        // debit vs EU consumer vs international — that transaction_currency alone cannot.
        card_issuing_country: txn_card_info
            .card_issuer_country
            .as_ref()
            .and_then(|s| non_empty(s))
            .map(|s| issuer_region(&s)),
        // Cross-border = card issued outside the merchant's home region (here: anything that
        // normalizes to "intl"). Derived for completeness; pricing matches on the region.
        cross_border_flag: txn_card_info
            .card_issuer_country
            .as_ref()
            .and_then(|s| non_empty(s))
            .map(|s| issuer_region(&s) == "intl")
            .unwrap_or(false),
        psp_array: Vec::new(),
    }
}

/// Map an issuer country (ISO-3166 alpha-2, or a region bucket already) to the pricing
/// region the seed-cost tiers key on: "us", "eu", or "intl". Case-insensitive. EU/UK
/// consumer interchange is capped, so those issuers share the cheap "eu" bucket; the US
/// merchant's own region is "us"; everything else is cross-border "intl".
fn issuer_region(raw: &str) -> String {
    const EU: &[&str] = &[
        "eu", "gb", "uk", "ie", "de", "fr", "es", "it", "nl", "be", "pt", "at", "fi", "se", "dk",
        "pl", "cz", "gr", "hu", "ro", "sk", "bg", "hr", "si", "ee", "lv", "lt", "lu", "mt", "cy",
    ];
    let v = raw.trim().to_lowercase();
    match v.as_str() {
        "us" | "usa" => "us".to_string(),
        "intl" => "intl".to_string(),
        s if EU.contains(&s) => "eu".to_string(),
        _ => "intl".to_string(),
    }
}

fn non_empty(s: &str) -> Option<String> {
    if s.trim().is_empty() {
        None
    } else {
        Some(s.to_string())
    }
}
