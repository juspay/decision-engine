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
    pub pmType: String,
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
    CfBnpl,
    #[serde(rename = "GIFT_CARD")]
    GiftCard,
    #[serde(rename = "CF_EMI")]
    CfEmi,
    #[serde(rename = "REAL_TIME")]
    RealTime,
    #[serde(rename = "CF_POD")]
    CfPod,
    #[serde(rename = "REWARD")]
    REWARD,
    #[serde(rename = "VAN")]
    VAN,
    #[serde(rename = "STORE")]
    STORE,
    #[serde(rename = "POS")]
    POS,
    #[serde(rename = "CF_LSP")]
    CfLsp,
    #[serde(rename = "FPX")]
    FPX,
    #[serde(rename = "UNKNOWN")]
    UNKNOWN,
}

impl PaymentMethodSubType {
    pub fn to_text(&self) -> &'static str {
        match self {
            Self::WALLET => "WALLET",
            Self::CfBnpl => "CF_BNPL",
            Self::GiftCard => "GIFT_CARD",
            Self::CfEmi => "CF_EMI",
            Self::RealTime => "REAL_TIME",
            Self::CfPod => "CF_POD",
            Self::REWARD => "REWARD",
            Self::VAN => "VAN",
            Self::STORE => "STORE",
            Self::POS => "POS",
            Self::CfLsp => "CF_LSP",
            Self::FPX => "FPX",
            Self::UNKNOWN => "UNKNOWN",
        }
    }

    pub fn from_text(ctx: &str) -> Result<Self, ApiError> {
        match ctx {
            "WALLET" => Ok(Self::WALLET),
            "CF_BNPL" => Ok(Self::CfBnpl),
            "GIFT_CARD" => Ok(Self::GiftCard),
            "CF_EMI" => Ok(Self::CfEmi),
            "REAL_TIME" => Ok(Self::RealTime),
            "CF_POD" => Ok(Self::CfPod),
            "REWARD" => Ok(Self::REWARD),
            "VAN" => Ok(Self::VAN),
            "STORE" => Ok(Self::STORE),
            "POS" => Ok(Self::POS),
            "CF_LSP" => Ok(Self::CfLsp),
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
            pmType: value.pm_type,
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
