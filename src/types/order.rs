pub mod id;
pub mod udfs;

use crate::types::currency::Currency;
use crate::types::customer::CustomerId;
use crate::types::merchant::id::MerchantId;
use crate::types::money::internal::Money;
use crate::types::order::id::{OrderId, OrderPrimId, ProductId};
use crate::types::order::udfs::UDFs;
use crate::types::txn_details::types::TxnObjectType;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;
use std::option::Option;
use std::string::String;
use time::OffsetDateTime;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderStatus {
    #[serde(rename = "AUTHENTICATION_FAILED")]
    AuthenticationFailed,
    #[serde(rename = "AUTHORIZATION_FAILED")]
    AuthorizationFailed,
    #[serde(rename = "AUTHORIZED")]
    Authorized,
    #[serde(rename = "AUTHORIZING")]
    Authorizing,
    #[serde(rename = "AUTO_REFUNDED")]
    AutoRefunded,
    #[serde(rename = "CAPTURE_FAILED")]
    CaptureFailed,
    #[serde(rename = "CAPTURE_INITIATED")]
    CaptureInitiated,
    #[serde(rename = "COD_INITIATED")]
    CodInitiated,
    #[serde(rename = "CREATED")]
    Created,
    #[serde(rename = "ERROR")]
    Error,
    #[serde(rename = "JUSPAY_DECLINED")]
    JuspayDeclined,
    #[serde(rename = "NEW")]
    New,
    #[serde(rename = "NOT_FOUND")]
    NotFound,
    #[serde(rename = "PARTIAL_CHARGED")]
    PartialCharged,
    #[serde(rename = "TO_BE_CHARGED")]
    ToBeCharged,
    #[serde(rename = "PENDING_AUTHENTICATION")]
    PendingAuthentication,
    #[serde(rename = "SUCCESS")]
    Success,
    #[serde(rename = "VOID_FAILED")]
    VoidFailed,
    #[serde(rename = "VOID_INITIATED")]
    VoidInitiated,
    #[serde(rename = "VOIDED")]
    Voided,
    #[serde(rename = "MERCHANT_VOIDED")]
    MerchantVoided,
    #[serde(rename = "DECLINED")]
    Declined,
}

impl OrderStatus {
    pub fn to_text(&self) -> String {
        match self {
            Self::AuthenticationFailed => "AUTHENTICATION_FAILED".to_string(),
            Self::AuthorizationFailed => "AUTHORIZATION_FAILED".to_string(),
            Self::Authorized => "AUTHORIZED".to_string(),
            Self::Authorizing => "AUTHORIZING".to_string(),
            Self::AutoRefunded => "AUTO_REFUNDED".to_string(),
            Self::CaptureFailed => "CAPTURE_FAILED".to_string(),
            Self::CaptureInitiated => "CAPTURE_INITIATED".to_string(),
            Self::CodInitiated => "COD_INITIATED".to_string(),
            Self::Created => "CREATED".to_string(),
            Self::Error => "ERROR".to_string(),
            Self::JuspayDeclined => "JUSPAY_DECLINED".to_string(),
            Self::New => "NEW".to_string(),
            Self::NotFound => "NOT_FOUND".to_string(),
            Self::PartialCharged => "PARTIAL_CHARGED".to_string(),
            Self::ToBeCharged => "TO_BE_CHARGED".to_string(),
            Self::PendingAuthentication => "PENDING_AUTHENTICATION".to_string(),
            Self::Success => "SUCCESS".to_string(),
            Self::VoidFailed => "VOID_FAILED".to_string(),
            Self::VoidInitiated => "VOID_INITIATED".to_string(),
            Self::Voided => "VOIDED".to_string(),
            Self::MerchantVoided => "MERCHANT_VOIDED".to_string(),
            Self::Declined => "DECLINED".to_string(),
        }
    }

    pub fn from_text(text: String) -> Option<Self> {
        match text.as_str() {
            "AUTHENTICATION_FAILED" => Some(Self::AuthenticationFailed),
            "AUTHORIZATION_FAILED" => Some(Self::AuthorizationFailed),
            "AUTHORIZED" => Some(Self::Authorized),
            "AUTHORIZING" => Some(Self::Authorizing),
            "AUTO_REFUNDED" => Some(Self::AutoRefunded),
            "CAPTURE_FAILED" => Some(Self::CaptureFailed),
            "CAPTURE_INITIATED" => Some(Self::CaptureInitiated),
            "COD_INITIATED" => Some(Self::CodInitiated),
            "CREATED" => Some(Self::Created),
            "ERROR" => Some(Self::Error),
            "JUSPAY_DECLINED" => Some(Self::JuspayDeclined),
            "NEW" => Some(Self::New),
            "NOT_FOUND" => Some(Self::NotFound),
            "PARTIAL_CHARGED" => Some(Self::PartialCharged),
            "TO_BE_CHARGED" => Some(Self::ToBeCharged),
            "PENDING_AUTHENTICATION" => Some(Self::PendingAuthentication),
            "SUCCESS" => Some(Self::Success),
            "VOID_FAILED" => Some(Self::VoidFailed),
            "VOID_INITIATED" => Some(Self::VoidInitiated),
            "VOIDED" => Some(Self::Voided),
            "MERCHANT_VOIDED" => Some(Self::MerchantVoided),
            "DECLINED" => Some(Self::Declined),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OrderType {
    #[serde(rename = "MANDATE_REGISTER")]
    MandateRegister,
    #[serde(rename = "EMANDATE_REGISTER")]
    EmandateRegister,
    #[serde(rename = "MANDATE_PAYMENT")]
    MandatePayment,
    #[serde(rename = "ORDER_PAYMENT")]
    OrderPayment,
    #[serde(rename = "TPV_PAYMENT")]
    TpvPayment,
    #[serde(rename = "TPV_MANDATE_REGISTER")]
    TpvMandateRegister,
    #[serde(rename = "TPV_MANDATE_PAYMENT")]
    TpvMandatePayment,
    #[serde(rename = "VAN_PAYMENT")]
    VanPayment,
    #[serde(rename = "MOTO_PAYMENT")]
    MotoPayment,
}

impl OrderType {
    pub fn to_text(&self) -> String {
        match self {
            Self::MandateRegister => "MANDATE_REGISTER".to_string(),
            Self::EmandateRegister => "EMANDATE_REGISTER".to_string(),
            Self::MandatePayment => "MANDATE_PAYMENT".to_string(),
            Self::OrderPayment => "ORDER_PAYMENT".to_string(),
            Self::TpvPayment => "TPV_PAYMENT".to_string(),
            Self::TpvMandateRegister => "TPV_MANDATE_REGISTER".to_string(),
            Self::TpvMandatePayment => "TPV_MANDATE_PAYMENT".to_string(),
            Self::VanPayment => "VAN_PAYMENT".to_string(),
            Self::MotoPayment => "MOTO_PAYMENT".to_string(),
        }
    }

    pub fn from_text(text: String) -> Option<Self> {
        match text.as_str() {
            "MANDATE_REGISTER" => Some(Self::MandateRegister),
            "EMANDATE_REGISTER" => Some(Self::EmandateRegister),
            "MANDATE_PAYMENT" => Some(Self::MandatePayment),
            "ORDER_PAYMENT" => Some(Self::OrderPayment),
            "TPV_PAYMENT" => Some(Self::TpvPayment),
            "TPV_MANDATE_REGISTER" => Some(Self::TpvMandateRegister),
            "TPV_MANDATE_PAYMENT" => Some(Self::TpvMandatePayment),
            "VAN_PAYMENT" => Some(Self::VanPayment),
            "MOTO_PAYMENT" => Some(Self::MotoPayment),
            _ => None,
        }
    }
    pub fn from_txn_object_type(txn_type: TxnObjectType) -> Self {
        match txn_type {
            TxnObjectType::OrderPayment => Self::OrderPayment,
            TxnObjectType::MandateRegister => Self::MandateRegister,
            TxnObjectType::EmandateRegister => Self::EmandateRegister,
            TxnObjectType::MandatePayment => Self::MandatePayment,
            TxnObjectType::EmandatePayment => Self::MandatePayment,
            TxnObjectType::TpvPayment => Self::TpvPayment,
            TxnObjectType::TpvEmandateRegister => Self::TpvMandateRegister,
            TxnObjectType::TpvMandateRegister => Self::TpvMandateRegister,
            TxnObjectType::TpvEmandatePayment => Self::TpvMandatePayment,
            TxnObjectType::TpvMandatePayment => Self::TpvMandatePayment,
            TxnObjectType::PartialCapture => Self::OrderPayment,
            TxnObjectType::PartialVoid => Self::OrderPayment,
            TxnObjectType::VanPayment => Self::VanPayment,
            TxnObjectType::MotoPayment => Self::MotoPayment,
        }
    }
}

pub fn deserialize_udfs_to_hashmap<'de, D>(deserializer: D) -> Result<UDFs, D::Error>
where
    D: Deserializer<'de>,
{
    // Deserialize the input as a Vec<String>
    let raw_vec: Vec<Option<String>> = Vec::deserialize(deserializer)?;

    // Convert the Vec<String> to a HashMap<i32, String>
    let hashmap: HashMap<i32, String> = raw_vec
        .into_iter()
        .enumerate()
        .filter(|(_, value)| value.is_some())
        .map(|(index, value)| (index as i32, value.unwrap_or("".to_string())))
        .collect();

    Ok(UDFs(hashmap))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Order {
    pub id: OrderPrimId,
    pub amount: Money,
    pub currency: Currency,
    #[serde(with = "time::serde::iso8601")]
    pub dateCreated: OffsetDateTime,
    pub merchantId: MerchantId,
    pub orderId: OrderId,
    pub status: OrderStatus,
    pub customerId: Option<CustomerId>,
    pub description: Option<String>,
    #[serde(deserialize_with = "deserialize_udfs_to_hashmap")]
    pub udfs: UDFs,
    pub preferredGateway: Option<String>,
    pub productId: Option<ProductId>,
    pub orderType: OrderType,
    pub metadata: Option<String>,
    pub internalMetadata: Option<String>,
}

// #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
// pub struct CustomerName {
//     pub customerName: Option<String>,
// }
