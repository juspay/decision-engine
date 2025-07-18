use serde::{Deserialize, Serialize};

use crate::decider::network_decider::types::{CountryAlpha2, PanOrToken, RegulatedName};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PagosInterchangeDetails {
    pub regulated: Option<bool>,
    pub regulated_name: Option<RegulatedName>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PagosCostDetails {
    pub interchange: Option<PagosInterchangeDetails>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub enum PagosCardType {
    Credit,
    Debit,
    Prepaid,
    Charge,
    DeferredDebit,
    #[serde(other)]
    Unknown,
}

impl PagosCardType {
    pub fn to_domain_card_type(&self) -> Option<crate::decider::network_decider::types::CardType> {
        match self {
            PagosCardType::Credit => Some(crate::decider::network_decider::types::CardType::Credit),
            PagosCardType::Debit => Some(crate::decider::network_decider::types::CardType::Debit),
            _ => None,
        }
    }
}


#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PagosCardDetails {
    pub card_brand: Option<String>,
    #[serde(rename = "type")]
    pub card_type: Option<PagosCardType>,
    pub card_level: Option<String>,
    pub bank: Option<PagosBankDetails>,
    pub country: Option<PagosCountryDetails>,
    pub domestic_only: Option<bool>,
    pub prepaid: Option<bool>,
    pub additional_card_brands: Option<Vec<PagosAdditionalCardBrand>>,
    pub correlation_id: Option<String>,
    pub number: Option<PagosCardNumberDetails>,
    pub bin_length: Option<i16>,
    pub pagos_bin_length: Option<i16>,
    pub bin_max: Option<String>,
    pub bin_min: Option<String>,
    pub pan_or_token: Option<PanOrToken>,
    pub reloadable: Option<bool>,
    pub shared_bin: Option<bool>,
    pub cost: Option<PagosCostDetails>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PagosCardNumberDetails {
    pub length: Option<i32>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PagosAdditionalCardBrand {
    pub card_brand: Option<String>,
    pub bin_max: Option<String>,
    pub bin_min: Option<String>,
    pub card_brand_product: Option<String>,
    pub card_brand_bank_name: Option<String>,
    pub ecom_enabled: Option<bool>,
    pub billpay_enabled: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PagosBankDetails {
    pub name: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PagosCountryDetails {
    pub alpha2: Option<CountryAlpha2>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct PagosPanDetailsResponse {
    pub card: PagosCardDetails,
}
