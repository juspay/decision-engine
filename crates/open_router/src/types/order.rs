
use serde::{Serialize, Deserialize};
use serde_json::Value;
use std::option::Option;
use std::string::String;
use std::time::SystemTime;
use types::currency::Currency;
use types::customer::id::CustomerId;
use types::gateway::Gateway;
use types::merchant::id::MerchantId;
use types::money::Money;
use types::order::basket::{OrderBasket, OrderBasketPrimId};
use types::order::id::{OrderId, OrderPrimId, ProductId, toOrderId, toOrderPrimId, toProductId};
use types::order::udfs::{UDFs, getUDF};
use juspay::extra::parsing::{Step, liftEither, ParsingErrorType};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
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
            OrderStatus::AuthenticationFailed => "AUTHENTICATION_FAILED".to_string(),
            OrderStatus::AuthorizationFailed => "AUTHORIZATION_FAILED".to_string(),
            OrderStatus::Authorized => "AUTHORIZED".to_string(),
            OrderStatus::Authorizing => "AUTHORIZING".to_string(),
            OrderStatus::AutoRefunded => "AUTO_REFUNDED".to_string(),
            OrderStatus::CaptureFailed => "CAPTURE_FAILED".to_string(),
            OrderStatus::CaptureInitiated => "CAPTURE_INITIATED".to_string(),
            OrderStatus::CodInitiated => "COD_INITIATED".to_string(),
            OrderStatus::Created => "CREATED".to_string(),
            OrderStatus::Error => "ERROR".to_string(),
            OrderStatus::JuspayDeclined => "JUSPAY_DECLINED".to_string(),
            OrderStatus::New => "NEW".to_string(),
            OrderStatus::NotFound => "NOT_FOUND".to_string(),
            OrderStatus::PartialCharged => "PARTIAL_CHARGED".to_string(),
            OrderStatus::ToBeCharged => "TO_BE_CHARGED".to_string(),
            OrderStatus::PendingAuthentication => "PENDING_AUTHENTICATION".to_string(),
            OrderStatus::Success => "SUCCESS".to_string(),
            OrderStatus::VoidFailed => "VOID_FAILED".to_string(),
            OrderStatus::VoidInitiated => "VOID_INITIATED".to_string(),
            OrderStatus::Voided => "VOIDED".to_string(),
            OrderStatus::MerchantVoided => "MERCHANT_VOIDED".to_string(),
            OrderStatus::Declined => "DECLINED".to_string(),
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
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
            OrderType::MandateRegister => "MANDATE_REGISTER".to_string(),
            OrderType::EmandateRegister => "EMANDATE_REGISTER".to_string(),
            OrderType::MandatePayment => "MANDATE_PAYMENT".to_string(),
            OrderType::OrderPayment => "ORDER_PAYMENT".to_string(),
            OrderType::TpvPayment => "TPV_PAYMENT".to_string(),
            OrderType::TpvMandateRegister => "TPV_MANDATE_REGISTER".to_string(),
            OrderType::TpvMandatePayment => "TPV_MANDATE_PAYMENT".to_string(),
            OrderType::VanPayment => "VAN_PAYMENT".to_string(),
            OrderType::MotoPayment => "MOTO_PAYMENT".to_string(),
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Order {
    pub id: OrderPrimId,
    pub amount: Money,
    pub currency: Currency,
    pub dateCreated: SystemTime,
    pub merchantId: MerchantId,
    pub orderId: OrderId,
    pub status: OrderStatus,
    pub customerId: Option<CustomerId>,
    pub description: Option<String>,
    pub udfs: UDFs,
    pub preferredGateway: Option<Gateway>,
    pub productId: Option<ProductId>,
    pub orderType: OrderType,
    pub metadata: Option<String>,
    pub internalMetadata: Option<String>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct CustomerName {
    pub customerName: Option<String>,
}
