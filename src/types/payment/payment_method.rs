use crate::app::get_tenant_app_state;
use serde::{Deserialize, Serialize};
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
use crate::types::bank_code::{to_bank_code_id, BankCodeId};

#[cfg(feature = "mysql")]
use crate::storage::schema::payment_method::dsl;
#[cfg(feature = "postgres")]
use crate::storage::schema_pg::payment_method::dsl;
use diesel::associations::HasTable;
use diesel::*;
use std::convert::TryFrom;
use std::option::Option;
use std::string::String;
use time::PrimitiveDateTime;

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
            Self::Wallet => "WALLET",
            Self::UPI => "UPI",
            Self::NB => "NB",
            Self::Card => "CARD",
            Self::Paylater => "PAYLATER",
            Self::ConsumerFinance => "CONSUMER_FINANCE",
            Self::Reward => "REWARD",
            Self::Cash => "CASH",
            Self::AtmCard => "ATM_CARD",
            Self::Aadhaar => "AADHAAR",
            Self::MerchantContainer => "MERCHANT_CONTAINER",
            Self::WalletContainer => "WALLET_CONTAINER",
            Self::Papernach => "PAPERNACH",
            Self::VirtualAccount => "VIRTUAL_ACCOUNT",
            Self::Otc => "OTC",
            Self::Rtp => "RTP",
            Self::Crypto => "CRYPTO",
            Self::CardQr => "CARD_QR",
            Self::PAN => "PAN",
            Self::Unknown => "UNKNOWN",
        }
    }

    pub fn from_text(ctx: &str) -> Result<Self, ApiError> {
        match ctx {
            "WALLET" => Ok(Self::Wallet),
            "UPI" => Ok(Self::UPI),
            "NB" => Ok(Self::NB),
            "CARD" => Ok(Self::Card),
            "PAYLATER" => Ok(Self::Paylater),
            "CONSUMER_FINANCE" => Ok(Self::ConsumerFinance),
            "REWARD" => Ok(Self::Reward),
            "CASH" => Ok(Self::Cash),
            "ATM_CARD" => Ok(Self::AtmCard),
            "AADHAAR" => Ok(Self::Aadhaar),
            "MERCHANT_CONTAINER" => Ok(Self::MerchantContainer),
            "WALLET_CONTAINER" => Ok(Self::WalletContainer),
            "PAPERNACH" => Ok(Self::Papernach),
            "VIRTUAL_ACCOUNT" => Ok(Self::VirtualAccount),
            "OTC" => Ok(Self::Otc),
            "RTP" => Ok(Self::Rtp),
            "CRYPTO" => Ok(Self::Crypto),
            "CARD_QR" => Ok(Self::CardQr),
            "PAN" => Ok(Self::PAN),
            "UNKNOWN" => Ok(Self::Unknown),
            _ => Err(ApiError::ParsingError("Invalid Payment Method Type")),
        }
    }
}

pub fn text_to_payment_method_type(
    payment_method_type: String,
) -> Result<PaymentMethodType, ApiError> {
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
            Self::WALLET => "WALLET",
            Self::CF_BNPL => "CF_BNPL",
            Self::GIFT_CARD => "GIFT_CARD",
            Self::CF_EMI => "CF_EMI",
            Self::REAL_TIME => "REAL_TIME",
            Self::CF_POD => "CF_POD",
            Self::REWARD => "REWARD",
            Self::VAN => "VAN",
            Self::STORE => "STORE",
            Self::POS => "POS",
            Self::CF_LSP => "CF_LSP",
            Self::FPX => "FPX",
            Self::UNKNOWN => "UNKNOWN",
        }
    }

    pub fn from_text(ctx: &str) -> Result<Self, ApiError> {
        match ctx {
            "WALLET" => Ok(Self::WALLET),
            "CF_BNPL" => Ok(Self::CF_BNPL),
            "GIFT_CARD" => Ok(Self::GIFT_CARD),
            "CF_EMI" => Ok(Self::CF_EMI),
            "REAL_TIME" => Ok(Self::REAL_TIME),
            "CF_POD" => Ok(Self::CF_POD),
            "REWARD" => Ok(Self::REWARD),
            "VAN" => Ok(Self::VAN),
            "STORE" => Ok(Self::STORE),
            "POS" => Ok(Self::POS),
            "CF_LSP" => Ok(Self::CF_LSP),
            "FPX" => Ok(Self::FPX),
            "UNKNOWN" => Ok(Self::UNKNOWN),
            _ => Err(ApiError::ParsingError("Invalid Payment Method Sub Type")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubType(pub String);

impl TryFrom<DBPaymentMethod> for PaymentMethod {
    type Error = ApiError;

    fn try_from(value: DBPaymentMethod) -> Result<Self, ApiError> {
        Ok(Self {
            id: PaymentMethodId(value.id),
            dateCreated: value.date_created,
            lastUpdated: value.last_updated,
            name: value.name,
            pmType: PaymentMethodType::from_text(&value.pm_type)?,
            description: value.description,
            juspayBankCodeId: value.juspay_bank_code_id.map(to_bank_code_id),
            displayName: value.display_name,
            nickName: value.nick_name,
            subType: value
                .sub_type
                .map(|sub_type| PaymentMethodSubType::from_text(&sub_type))
                .transpose()?,
            dsl: value.payment_dsl,
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

pub async fn get_by_name(name: String) -> Option<PaymentMethod> {
    let app_state = get_tenant_app_state().await;
    match crate::generics::generic_find_one_optional::<
        <DBPaymentMethod as HasTable>::Table,
        _,
        DBPaymentMethod,
    >(&app_state.db, dsl::name.eq(name))
    .await
    {
        Ok(Some(db_payment_method)) => PaymentMethod::try_from(db_payment_method).ok(),
        _ => None,
    }
}
