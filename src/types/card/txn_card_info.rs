use crate::error::ApiError;
use crate::types::card::card_type::CardType;
use crate::utils::StringExt;
use masking::Secret;
use serde::{Deserialize, Deserializer, Serialize};
use time::{OffsetDateTime, PrimitiveDateTime};
// use crate::types::transaction::id::TransactionId;
// use crate::types::txn_details::types::TxnDetailId;
// use juspay::extra::parsing::{Step, lift_either, lift_pure, ParsingErrorType};
// use juspay::extra::secret::{Secret, SecretContext};
use std::fmt::Debug;
use std::option::Option;
use std::string::String;

#[derive(Debug, PartialEq, Clone, Eq, Serialize, Deserialize, Hash)]
pub enum AuthType {
    #[serde(rename = "ATMPIN")]
    ATMPIN,
    #[serde(rename = "THREE_DS")]
    ThreeDs,
    #[serde(rename = "THREE_DS_2")]
    ThreeDs2,
    #[serde(rename = "OTP")]
    OTP,
    #[serde(rename = "OBO_OTP")]
    OboOtp,
    #[serde(rename = "VIES")]
    VIES,
    #[serde(rename = "NO_THREE_DS")]
    NoThreeDs,
    #[serde(rename = "NETWORK_TOKEN")]
    NetworkToken,
    #[serde(rename = "MOTO")]
    MOTO,
    #[serde(rename = "FIDO")]
    FIDO,
    #[serde(rename = "CTP")]
    CTP,
}

impl std::fmt::Display for AuthType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ATMPIN => write!(f, "ATMPIN"),
            Self::ThreeDs => write!(f, "THREE_DS"),
            Self::ThreeDs2 => write!(f, "THREE_DS_2"),
            Self::OTP => write!(f, "OTP"),
            Self::OboOtp => write!(f, "OBO_OTP"),
            Self::VIES => write!(f, "VIES"),
            Self::NoThreeDs => write!(f, "NO_THREE_DS"),
            Self::NetworkToken => write!(f, "NETWORK_TOKEN"),
            Self::MOTO => write!(f, "MOTO"),
            Self::FIDO => write!(f, "FIDO"),
            Self::CTP => write!(f, "CTP"),
        }
    }
}

pub fn text_to_auth_type(ctx: &str) -> Result<AuthType, ApiError> {
    match ctx {
        "ATMPIN" => Ok(AuthType::ATMPIN),
        "THREE_DS" => Ok(AuthType::ThreeDs),
        "THREE_DS_2" => Ok(AuthType::ThreeDs2),
        "OTP" => Ok(AuthType::OTP),
        "OBO_OTP" => Ok(AuthType::OboOtp),
        "VIES" => Ok(AuthType::VIES),
        "NO_THREE_DS" => Ok(AuthType::NoThreeDs),
        "NETWORK_TOKEN" => Ok(AuthType::NetworkToken),
        "MOTO" => Ok(AuthType::MOTO),
        "FIDO" => Ok(AuthType::FIDO),
        "CTP" => Ok(AuthType::CTP),
        _ => Err(ApiError::ParsingError("Invalid Auth Type")),
    }
}
pub fn auth_type_to_text(ctx: &AuthType) -> String {
    match ctx {
        AuthType::ATMPIN => "ATMPIN".to_string(),
        AuthType::ThreeDs => "THREE_DS".to_string(),
        AuthType::ThreeDs2 => "THREE_DS_2".to_string(),
        AuthType::OTP => "OTP".to_string(),
        AuthType::OboOtp => "OBO_OTP".to_string(),
        AuthType::VIES => "VIES".to_string(),
        AuthType::NoThreeDs => "NO_THREE_DS".to_string(),
        AuthType::NetworkToken => "NETWORK_TOKEN".to_string(),
        AuthType::MOTO => "MOTO".to_string(),
        AuthType::FIDO => "FIDO".to_string(),
        AuthType::CTP => "CTP".to_string(),
    }
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EMIType {
    #[serde(rename = "NO_COST_EMI")]
    NoCostEmi,
    #[serde(rename = "LOW_COST_EMI")]
    LowCostEmi,
    #[serde(rename = "STANDARD_EMI")]
    StandardEmi,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmiDetails {
    #[serde(rename = "emi_type")]
    pub emi_type: EMIType,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TxnCardInfoPId(i64);

pub fn to_txn_card_info_pid(ctx: i64) -> TxnCardInfoPId {
    TxnCardInfoPId(ctx)
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenBinPaymentSource {
    #[serde(rename = "is_token_bin")]
    pub is_token_bin: bool,
}

pub fn deserialize_optional_primitive_datetime<'de, D>(
    deserializer: D,
) -> Result<Option<PrimitiveDateTime>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Deserialize::deserialize(deserializer)?;
    if s.is_none() {
        return Ok(None);
    }

    let format = time::macros::format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]");

    match time::PrimitiveDateTime::parse(&s.unwrap(), &format) {
        Ok(o) => Ok(Some(o)),
        Err(err) => {
            crate::logger::debug!("Error: {:?}", err);
            Ok(None)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct TxnCardInfo {
    #[serde(rename = "id")]
    pub id: TxnCardInfoPId,
    // #[serde(rename = "txnId")]
    // pub txnId: TransactionId,
    #[serde(rename = "cardIsin")]
    pub card_isin: Option<String>,
    #[serde(rename = "cardIssuerBankName")]
    pub cardIssuerBankName: Option<String>,
    #[serde(rename = "cardSwitchProvider")]
    pub cardSwitchProvider: Option<Secret<String>>,
    #[serde(rename = "cardType")]
    pub card_type: Option<CardType>,
    #[serde(rename = "nameOnCard")]
    pub nameOnCard: Option<Secret<String>>,
    // #[serde(rename = "txnDetailId")]
    // pub txnDetailId: TxnDetailId,
    #[serde(with = "time::serde::iso8601")]
    #[serde(rename = "dateCreated")]
    pub dateCreated: OffsetDateTime,
    #[serde(rename = "paymentMethodType")]
    pub paymentMethodType: String,
    #[serde(rename = "paymentMethod")]
    pub paymentMethod: String,
    #[serde(rename = "paymentSource")]
    pub paymentSource: Option<String>,
    #[serde(rename = "authType")]
    pub authType: Option<AuthType>,
    #[serde(rename = "partitionKey")]
    #[serde(deserialize_with = "deserialize_optional_primitive_datetime")]
    pub partitionKey: Option<PrimitiveDateTime>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct SafeTxnCardInfo {
    #[serde(rename = "id")]
    pub id: String,
    #[serde(rename = "cardIsin")]
    pub card_isin: Option<String>,
    #[serde(rename = "cardIssuerBankName")]
    pub cardIssuerBankName: Option<String>,
    #[serde(rename = "cardSwitchProvider")]
    pub cardSwitchProvider: Option<Secret<String>>,
    #[serde(rename = "cardType")]
    pub card_type: Option<CardType>,
    #[serde(rename = "nameOnCard")]
    pub nameOnCard: Option<Secret<String>>,
    #[serde(with = "time::serde::iso8601")]
    #[serde(rename = "dateCreated")]
    pub dateCreated: OffsetDateTime,
    #[serde(rename = "paymentMethodType")]
    pub paymentMethodType: String,
    #[serde(rename = "paymentMethod")]
    pub paymentMethod: String,
    #[serde(rename = "paymentSource")]
    pub paymentSource: Option<String>,
    #[serde(rename = "authType")]
    pub authType: Option<String>,
    #[serde(rename = "partitionKey")]
    #[serde(deserialize_with = "deserialize_optional_primitive_datetime")]
    pub partitionKey: Option<PrimitiveDateTime>,
}

pub fn convert_safe_to_txn_card_info(
    safe_info: SafeTxnCardInfo,
) -> Result<TxnCardInfo, crate::error::ApiError> {
    let id_i64 = safe_info
        .id
        .parse::<i64>()
        .map_err(|_| crate::error::ApiError::ParsingError("id"))?;

    Ok(TxnCardInfo {
        id: TxnCardInfoPId(id_i64),
        card_isin: safe_info.card_isin,
        cardIssuerBankName: safe_info.cardIssuerBankName,
        cardSwitchProvider: safe_info.cardSwitchProvider,
        card_type: safe_info.card_type,
        nameOnCard: safe_info.nameOnCard,
        dateCreated: safe_info.dateCreated,
        paymentMethodType: safe_info.paymentMethodType,
        paymentMethod: safe_info.paymentMethod,
        paymentSource: safe_info.paymentSource,
        authType: safe_info
            .authType
            .and_then(|auth| text_to_auth_type(&auth).ok()),
        partitionKey: safe_info.partitionKey,
    })
}
