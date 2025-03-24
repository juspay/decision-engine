use serde::{Serialize, Deserialize};
use serde_json::Value;
use crate::app::get_tenant_app_state;
// use db::euler_mesh_impl::mesh_config;
// use db::mesh::internal::*;
// use db::storage::types::paymentmethod as DB;
// use types::utils::dbconfig::get_euler_db_conf;
// use types::juspaybankcode::{JuspayBankCodeId, to_juspay_bank_code_id};
// use juspay::extra::parsing::{Parsed, ParsingErrorType, Step, around, lift_either, lift_pure, mandated, non_negative, parse_field, project, to_utc};
// use eulerhs::extra::combinators::to_domain_all;
// use eulerhs::language::MonadFlow;
use crate::error::ApiError;
use crate::storage::types::PaymentMethod as DBPaymentMethod;
use crate::types::bank_code::{BankCodeId, to_bank_code_id};

use std::string::String;
use std::option::Option;
use std::vec::Vec;
use time::PrimitiveDateTime;
use std::convert::TryFrom;
use crate::storage::schema::payment_method::dsl;
use diesel::*;
use diesel::associations::HasTable;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

    pub fn from_text(ctx: &str) -> Result<PaymentMethodType, ApiError> {
        match ctx {
            "WALLET" => Ok(PaymentMethodType::Wallet),
            "UPI" => Ok(PaymentMethodType::UPI),
            "NB" => Ok(PaymentMethodType::NB),
            "CARD" => Ok(PaymentMethodType::Card),
            "PAYLATER" => Ok(PaymentMethodType::Paylater),
            "CONSUMER_FINANCE" => Ok(PaymentMethodType::ConsumerFinance),
            "REWARD" => Ok(PaymentMethodType::Reward),
            "CASH" => Ok(PaymentMethodType::Cash),
            "ATM_CARD" => Ok(PaymentMethodType::AtmCard),
            "AADHAAR" => Ok(PaymentMethodType::Aadhaar),
            "MERCHANT_CONTAINER" => Ok(PaymentMethodType::MerchantContainer),
            "WALLET_CONTAINER" => Ok(PaymentMethodType::WalletContainer),
            "PAPERNACH" => Ok(PaymentMethodType::Papernach),
            "VIRTUAL_ACCOUNT" => Ok(PaymentMethodType::VirtualAccount),
            "OTC" => Ok(PaymentMethodType::Otc),
            "RTP" => Ok(PaymentMethodType::Rtp),
            "CRYPTO" => Ok(PaymentMethodType::Crypto),
            "CARD_QR" => Ok(PaymentMethodType::CardQr),
            "PAN" => Ok(PaymentMethodType::PAN),
            "UNKNOWN" => Ok(PaymentMethodType::Unknown),
            _ => Err(ApiError::ParsingError("Invalid Payment Method Type")),
        }
    }
}

pub fn text_to_payment_method_type(payment_method_type: String) -> Result<PaymentMethodType, ApiError> {
    PaymentMethodType::from_text(payment_method_type.as_str())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaymentMethodId(pub i64);

pub fn to_payment_method_id(id: i64) -> PaymentMethodId {
    PaymentMethodId(id)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentMethod {
    pub id: PaymentMethodId,
    pub dateCreated: PrimitiveDateTime,
    pub lastUpdated: PrimitiveDateTime,
    pub name: String,
    pub pmType: PaymentMethodType,
    pub description: Option<String>,
    pub juspayBankCodeId: Option<BankCodeId>,
    pub displayName: Option<String>,
    pub nickName: Option<String>,
    pub subType: Option<PaymentMethodSubType>,
    pub dsl: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

    pub fn from_text(ctx: &str) -> Result<PaymentMethodSubType, ApiError> {
        match ctx {
            "WALLET" => Ok(PaymentMethodSubType::WALLET),
            "CF_BNPL" => Ok(PaymentMethodSubType::CF_BNPL),
            "GIFT_CARD" => Ok(PaymentMethodSubType::GIFT_CARD),
            "CF_EMI" => Ok(PaymentMethodSubType::CF_EMI),
            "REAL_TIME" => Ok(PaymentMethodSubType::REAL_TIME),
            "CF_POD" => Ok(PaymentMethodSubType::CF_POD),
            "REWARD" => Ok(PaymentMethodSubType::REWARD),
            "VAN" => Ok(PaymentMethodSubType::VAN),
            "STORE" => Ok(PaymentMethodSubType::STORE),
            "POS" => Ok(PaymentMethodSubType::POS),
            "CF_LSP" => Ok(PaymentMethodSubType::CF_LSP),
            "FPX" => Ok(PaymentMethodSubType::FPX),
            "UNKNOWN" => Ok(PaymentMethodSubType::UNKNOWN),
            _ => Err(ApiError::ParsingError("Invalid Payment Method Sub Type")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubType(pub String);

impl TryFrom<DBPaymentMethod> for PaymentMethod {
    type Error = ApiError;

    fn try_from(value: DBPaymentMethod) -> Result<Self, ApiError> {
        Ok(PaymentMethod {
            id: PaymentMethodId(value.id),
            dateCreated: value.date_created,
            lastUpdated: value.last_updated,
            name: value.name,
            pmType: PaymentMethodType::from_text(&value.pm_type)?,
            description: value.description,
            juspayBankCodeId: value.juspay_bank_code_id.map(|id| to_bank_code_id(id)),
            displayName: value.display_name,
            nickName: value.nick_name,
            subType: value.sub_type.map(|sub_type| PaymentMethodSubType::from_text(&sub_type)).transpose()?,
            dsl: value.dsl,
        })
    }
}

// #TOD: Implement DB Calls

// pub async fn get_by_name_db(name: String) -> Result<Option<DB::PaymentMethod>, MeshError> {
//     let db_conf = get_euler_db_conf::<DB::PaymentMethodT>().await?;
//     find_one_row(db_conf, mesh_config, vec![Clause::Is(DB::name, Term::Eq(name))]).await
// }

// pub async fn get_by_name(name: String) -> Option<PaymentMethod> {
//     match get_by_name_db(name).await {
//         Ok(Some(db_payment_method)) => to_domain_all(db_payment_method, parse_payment_method, "getByName", "parsePaymentMethod"),
//         _ => None,
//     }
// }


pub async fn get_by_name(
    name: String,
) -> Option<PaymentMethod> {
    let app_state = get_tenant_app_state().await;
    match crate::generics::generic_find_one_optional::<
        <DBPaymentMethod as HasTable>::Table,
        _,
        DBPaymentMethod
    >(
        &app_state.db,
        dsl::name.eq(name)
    ).await {
        Ok(Some(db_payment_method)) => PaymentMethod::try_from(db_payment_method).ok(),
        _ => None,
    }
}