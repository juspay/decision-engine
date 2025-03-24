use masking::{Secret, SerializableSecret};
use serde::{Serialize, Deserialize};
use crate::error::ApiError;
use crate::types::card::card_type::CardType;
use crate::types::payment::payment_method::PaymentMethodType;
// use crate::types::transaction::id::TransactionId;
// use crate::types::txn_details::types::TxnDetailId;
// use juspay::extra::parsing::{Step, lift_either, lift_pure, ParsingErrorType};
// use juspay::extra::secret::{Secret, SecretContext};
use std::option::Option;
use std::string::String;
use std::time::{SystemTime, UNIX_EPOCH};
use std::fmt::Debug;

#[derive(Debug, PartialEq, Clone, Eq, Serialize, Deserialize)]
pub enum AuthType {
    #[serde(rename = "ATMPIN")]
    ATMPIN,
    #[serde(rename = "THREE_DS")]
    THREE_DS,
    #[serde(rename = "THREE_DS_2")]
    THREE_DS_2,
    #[serde(rename = "OTP")]
    OTP,
    #[serde(rename = "OBO_OTP")]
    OBO_OTP,
    #[serde(rename = "VIES")]
    VIES,
    #[serde(rename = "NO_THREE_DS")]
    NO_THREE_DS,
    #[serde(rename = "NETWORK_TOKEN")]
    NETWORK_TOKEN,
    #[serde(rename = "MOTO")]
    MOTO,
    #[serde(rename = "FIDO")]
    FIDO,
    #[serde(rename = "CTP")]
    CTP,
}

pub fn text_to_auth_type(ctx: &str) -> Result<AuthType, ApiError> {
    match ctx {
        "ATMPIN" => Ok(AuthType::ATMPIN),
        "THREE_DS" => Ok(AuthType::THREE_DS),
        "THREE_DS_2" => Ok(AuthType::THREE_DS_2),
        "OTP" => Ok(AuthType::OTP),
        "OBO_OTP" => Ok(AuthType::OBO_OTP),
        "VIES" => Ok(AuthType::VIES),
        "NO_THREE_DS" => Ok(AuthType::NO_THREE_DS),
        "NETWORK_TOKEN" => Ok(AuthType::NETWORK_TOKEN),
        "MOTO" => Ok(AuthType::MOTO),
        "FIDO" => Ok(AuthType::FIDO),
        "CTP" => Ok(AuthType::CTP),
        _ => Err(ApiError::ParsingError("Invalid Auth Type")),
    }
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EMIType {
    #[serde(rename = "NO_COST_EMI")]
    NO_COST_EMI,
    #[serde(rename = "LOW_COST_EMI")]
    LOW_COST_EMI,
    #[serde(rename = "STANDARD_EMI")]
    STANDARD_EMI,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmiDetails {
    #[serde(rename = "emi_type")]
    pub emi_type: EMIType,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct TxnCardInfoPId {
    #[serde(rename = "txnCardInfoPId")]
    pub txnCardInfoPId: i64,
}

pub fn to_txn_card_info_pid(ctx: i64) -> TxnCardInfoPId {
    TxnCardInfoPId { txnCardInfoPId: ctx }
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenBinPaymentSource {
    #[serde(rename = "is_token_bin")]
    pub is_token_bin: bool,
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
pub struct TxnCardInfo {
    #[serde(rename = "id")]
    pub id: TxnCardInfoPId,
    // #[serde(rename = "txnId")]
    // pub txnId: TransactionId,
    #[serde(rename = "cardIsin")]
    pub cardIsin: Option<String>,
    #[serde(rename = "cardIssuerBankName")]
    pub cardIssuerBankName: Option<String>,
    #[serde(rename = "cardSwitchProvider")]
    pub cardSwitchProvider: Option<Secret<String>>,
    #[serde(rename = "cardType")]
    pub cardType: Option<CardType>,
    #[serde(rename = "nameOnCard")]
    pub nameOnCard: Option<Secret<String>>,
    // #[serde(rename = "txnDetailId")]
    // pub txnDetailId: TxnDetailId,
    #[serde(rename = "dateCreated")]
    pub dateCreated: SystemTime,
    #[serde(rename = "paymentMethodType")]
    pub paymentMethodType: PaymentMethodType,
    #[serde(rename = "paymentMethod")]
    pub paymentMethod: String,
    #[serde(rename = "paymentSource")]
    pub paymentSource: Option<String>,
    #[serde(rename = "authType")]
    pub authType: Option<AuthType>,
    #[serde(rename = "partitionKey")]
    pub partitionKey: Option<SystemTime>,
}
