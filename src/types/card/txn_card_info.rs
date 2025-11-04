use crate::error::ApiError;
use crate::types::card::card_type::CardType;
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
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AuthType {
    Atmpin,
    ThreeDs,
    ThreeDs2,
    Otp,
    OboOtp,
    Vies,
    NoThreeDs,
    NetworkToken,
    Moto,
    Fido,
    Ctp,
}

impl std::fmt::Display for AuthType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Atmpin => write!(f, "ATMPIN"),
            Self::ThreeDs => write!(f, "THREE_DS"),
            Self::ThreeDs2 => write!(f, "THREE_DS_2"),
            Self::Otp => write!(f, "OTP"),
            Self::OboOtp => write!(f, "OBO_OTP"),
            Self::Vies => write!(f, "VIES"),
            Self::NoThreeDs => write!(f, "NO_THREE_DS"),
            Self::NetworkToken => write!(f, "NETWORK_TOKEN"),
            Self::Moto => write!(f, "MOTO"),
            Self::Fido => write!(f, "FIDO"),
            Self::Ctp => write!(f, "CTP"),
        }
    }
}

pub fn text_to_auth_type(ctx: &str) -> Result<AuthType, ApiError> {
    match ctx {
        "ATMPIN" => Ok(AuthType::Atmpin),
        "THREE_DS" => Ok(AuthType::ThreeDs),
        "THREE_DS_2" => Ok(AuthType::ThreeDs2),
        "OTP" => Ok(AuthType::Otp),
        "OBO_OTP" => Ok(AuthType::OboOtp),
        "VIES" => Ok(AuthType::Vies),
        "NO_THREE_DS" => Ok(AuthType::NoThreeDs),
        "NETWORK_TOKEN" => Ok(AuthType::NetworkToken),
        "MOTO" => Ok(AuthType::Moto),
        "FIDO" => Ok(AuthType::Fido),
        "CTP" => Ok(AuthType::Ctp),
        _ => Err(ApiError::ParsingError("Invalid Auth Type")),
    }
}
pub fn auth_type_to_text(ctx: &AuthType) -> String {
    match ctx {
        AuthType::Atmpin => "ATMPIN".to_string(),
        AuthType::ThreeDs => "THREE_DS".to_string(),
        AuthType::ThreeDs2 => "THREE_DS_2".to_string(),
        AuthType::Otp => "OTP".to_string(),
        AuthType::OboOtp => "OBO_OTP".to_string(),
        AuthType::Vies => "VIES".to_string(),
        AuthType::NoThreeDs => "NO_THREE_DS".to_string(),
        AuthType::NetworkToken => "NETWORK_TOKEN".to_string(),
        AuthType::Moto => "MOTO".to_string(),
        AuthType::Fido => "FIDO".to_string(),
        AuthType::Ctp => "CTP".to_string(),
    }
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EMIType {
    NoCostEmi,
    LowCostEmi,
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
    pub card_issuer_bank_name: Option<String>,
    #[serde(rename = "cardSwitchProvider")]
    pub card_switch_provider: Option<Secret<String>>,
    #[serde(rename = "cardType")]
    pub card_type: Option<CardType>,
    #[serde(rename = "nameOnCard")]
    pub name_on_card: Option<Secret<String>>,
    // #[serde(rename = "txnDetailId")]
    // pub txnDetailId: TxnDetailId,
    #[serde(with = "time::serde::iso8601")]
    #[serde(rename = "dateCreated")]
    pub date_created: OffsetDateTime,
    #[serde(rename = "paymentMethodType")]
    pub payment_method_type: String,
    #[serde(rename = "paymentMethod")]
    pub payment_method: String,
    #[serde(rename = "paymentSource")]
    pub payment_source: Option<String>,
    #[serde(rename = "authType")]
    pub auth_type: Option<AuthType>,
    #[serde(rename = "partitionKey")]
    #[serde(deserialize_with = "deserialize_optional_primitive_datetime")]
    pub partition_key: Option<PrimitiveDateTime>,
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
        card_issuer_bank_name: safe_info.cardIssuerBankName,
        card_switch_provider: safe_info.cardSwitchProvider,
        card_type: safe_info.card_type,
        name_on_card: safe_info.nameOnCard,
        date_created: safe_info.dateCreated,
        payment_method_type: safe_info.paymentMethodType,
        payment_method: safe_info.paymentMethod,
        payment_source: safe_info.paymentSource,
        auth_type: safe_info
            .authType
            .and_then(|auth| text_to_auth_type(&auth).ok()),
        partition_key: safe_info.partitionKey,
    })
}
