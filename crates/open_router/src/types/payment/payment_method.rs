
use serde::{Serialize, Deserialize};
use serde_json::Value;
use db::euler_mesh_impl::mesh_config;
use db::mesh::internal::*;
use db::storage::types::paymentmethod as DB;
use types::utils::dbconfig::get_euler_db_conf;
use types::juspaybankcode::{JuspayBankCodeId, to_juspay_bank_code_id};
use juspay::extra::parsing::{Parsed, ParsingErrorType, Step, around, lift_either, lift_pure, mandated, non_negative, parse_field, project, to_utc};
use eulerhs::extra::combinators::to_domain_all;
use eulerhs::language::MonadFlow;

use std::string::String;
use std::option::Option;
use std::vec::Vec;
use std::time::SystemTime;
use std::convert::TryFrom;

#[derive(Debug, PartialEq, Eq, Ord, Serialize, Deserialize)]
pub enum PaymentMethodType {
    #[serde(rename = "WALLET")]
    Wallet,
    #[serde(rename = "UPI")]
    UPI,
    #[serde(rename = "NB")]
    NB,
    #[serde(rename = "CARD")]
    Card,
    #[serde(rename = "PAYLATER")]
    Paylater,
    #[serde(rename = "CONSUMER_FINANCE")]
    ConsumerFinance,
    #[serde(rename = "REWARD")]
    Reward,
    #[serde(rename = "CASH")]
    Cash,
    #[serde(rename = "ATM_CARD")]
    AtmCard,
    #[serde(rename = "AADHAAR")]
    Aadhaar,
    #[serde(rename = "MERCHANT_CONTAINER")]
    MerchantContainer,
    #[serde(rename = "WALLET_CONTAINER")]
    WalletContainer,
    #[serde(rename = "PAPERNACH")]
    Papernach,
    #[serde(rename = "VIRTUAL_ACCOUNT")]
    VirtualAccount,
    #[serde(rename = "OTC")]
    Otc,
    #[serde(rename = "RTP")]
    Rtp,
    #[serde(rename = "CRYPTO")]
    Crypto,
    #[serde(rename = "CARD_QR")]
    CardQr,
    #[serde(rename = "PAN")]
    PAN,
    #[serde(rename = "UNKNOWN")]
    Unknown,
}

impl PaymentMethodType {
    pub fn to_text(&self) -> &'static str {
        match self {
            PaymentMethodType::Wallet => "WALLET",
            PaymentMethodType::UPI => "UPI",
            PaymentMethodType::NB => "NB",
            PaymentMethodType::Card => "CARD",
            PaymentMethodType::Paylater => "PAYLATER",
            PaymentMethodType::ConsumerFinance => "CONSUMER_FINANCE",
            PaymentMethodType::Reward => "REWARD",
            PaymentMethodType::Cash => "CASH",
            PaymentMethodType::AtmCard => "ATM_CARD",
            PaymentMethodType::Aadhaar => "AADHAAR",
            PaymentMethodType::MerchantContainer => "MERCHANT_CONTAINER",
            PaymentMethodType::WalletContainer => "WALLET_CONTAINER",
            PaymentMethodType::Papernach => "PAPERNACH",
            PaymentMethodType::VirtualAccount => "VIRTUAL_ACCOUNT",
            PaymentMethodType::Otc => "OTC",
            PaymentMethodType::Rtp => "RTP",
            PaymentMethodType::Crypto => "CRYPTO",
            PaymentMethodType::CardQr => "CARD_QR",
            PaymentMethodType::PAN => "PAN",
            PaymentMethodType::Unknown => "UNKNOWN",
        }
    }
}

#[derive(Debug, PartialEq, Eq, Ord, Serialize, Deserialize)]
pub struct PaymentMethodId(pub i64);

#[derive(Debug, Serialize, Deserialize)]
pub struct PaymentMethod {
    pub id: PaymentMethodId,
    pub dateCreated: SystemTime,
    pub lastUpdated: SystemTime,
    pub name: String,
    pub pmType: PaymentMethodType,
    pub description: Option<String>,
    pub juspayBankCodeId: Option<JuspayBankCodeId>,
    pub displayName: Option<String>,
    pub nickName: Option<String>,
    pub subType: Option<PaymentMethodSubType>,
    pub dsl: Option<String>,
}

#[derive(Debug, PartialEq, Eq, Ord, Serialize, Deserialize)]
pub enum PaymentMethodSubType {
    #[serde(rename = "WALLET")]
    WALLET,
    #[serde(rename = "CF_BNPL")]
    CF_BNPL,
    #[serde(rename = "GIFT_CARD")]
    GIFT_CARD,
    #[serde(rename = "CF_EMI")]
    CF_EMI,
    #[serde(rename = "REAL_TIME")]
    REAL_TIME,
    #[serde(rename = "CF_POD")]
    CF_POD,
    #[serde(rename = "REWARD")]
    REWARD,
    #[serde(rename = "VAN")]
    VAN,
    #[serde(rename = "STORE")]
    STORE,
    #[serde(rename = "POS")]
    POS,
    #[serde(rename = "CF_LSP")]
    CF_LSP,
    #[serde(rename = "FPX")]
    FPX,
    #[serde(rename = "UNKNOWN")]
    UNKNOWN,
}

impl PaymentMethodSubType {
    pub fn to_text(&self) -> &'static str {
        match self {
            PaymentMethodSubType::WALLET => "WALLET",
            PaymentMethodSubType::CF_BNPL => "CF_BNPL",
            PaymentMethodSubType::GIFT_CARD => "GIFT_CARD",
            PaymentMethodSubType::CF_EMI => "CF_EMI",
            PaymentMethodSubType::REAL_TIME => "REAL_TIME",
            PaymentMethodSubType::CF_POD => "CF_POD",
            PaymentMethodSubType::REWARD => "REWARD",
            PaymentMethodSubType::VAN => "VAN",
            PaymentMethodSubType::STORE => "STORE",
            PaymentMethodSubType::POS => "POS",
            PaymentMethodSubType::CF_LSP => "CF_LSP",
            PaymentMethodSubType::FPX => "FPX",
            PaymentMethodSubType::UNKNOWN => "UNKNOWN",
        }
    }
}

#[derive(Debug, PartialEq, Eq, Ord, Serialize, Deserialize)]
pub struct SubType(pub String);

pub fn parse_payment_method(db_type: DB::PaymentMethod) -> Parsed<PaymentMethod> {
    PaymentMethod {
        id: parse_field(&db_type, "id").and_then(|id| mandated(id).and_then(non_negative).map(|id| PaymentMethodId(id))),
        dateCreated: parse_field(&db_type, "dateCreated").map(to_utc),
        lastUpdated: parse_field(&db_type, "lastUpdated").map(to_utc),
        name: parse_field(&db_type, "name"),
        pmType: parse_field(&db_type, "pmType").and_then(|pm_type| text_to_payment_method_type(pm_type)),
        description: parse_field(&db_type, "description"),
        juspayBankCodeId: parse_field(&db_type, "juspayBankCodeId").map(|code| around(to_juspay_bank_code_id(code))),
        displayName: parse_field(&db_type, "displayName"),
        nickName: parse_field(&db_type, "nickName"),
        subType: parse_field(&db_type, "subType").map(|sub_type| around(text_to_payment_method_sub_type(sub_type))),
        dsl: parse_field(&db_type, "dsl"),
    }
}

pub async fn get_by_name_db(name: String) -> Result<Option<DB::PaymentMethod>, MeshError> {
    let db_conf = get_euler_db_conf::<DB::PaymentMethodT>().await?;
    find_one_row(db_conf, mesh_config, vec![Clause::Is(DB::name, Term::Eq(name))]).await
}

pub async fn get_by_name(name: String) -> Option<PaymentMethod> {
    match get_by_name_db(name).await {
        Ok(Some(db_payment_method)) => to_domain_all(db_payment_method, parse_payment_method, "getByName", "parsePaymentMethod"),
        _ => None,
    }
}
