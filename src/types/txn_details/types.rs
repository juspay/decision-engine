use serde::{Deserialize, Deserializer, Serialize};
use time::{OffsetDateTime, PrimitiveDateTime};
// use serde_json::Value as AValue;
// use std::collections::HashMap;
use std::fmt;
use std::option::Option;
use std::string::String;
use std::vec::Vec;
use time::Date;
use time::Month;
use time::Time;
// use db::eulermeshimpl::mesh_config;
// use db::mesh::internal;
// use crate::storage::internal::primd_id_to_int;
// use types::utils::dbconfig::get_euler_db_conf;
use crate::feedback::types::Milliseconds;
use crate::types::country::country_iso::CountryISO2;
use crate::types::currency::Currency;
use crate::types::merchant::id::MerchantId;
use crate::types::merchant::merchant_gateway_account::MerchantGwAccId;
use crate::types::money::internal::Money;
use crate::types::order::id::OrderId;
// use juspay::extra::parsing::{Parsed, ParsingErrorType, Step, around, defaulting, lift_either, lift_pure, mandated, non_empty_text, non_negative, parse_field, project, to_utc};
use crate::types::source_object_id::SourceObjectId;
// use juspay::extra::nonemptytext::NonEmptyText;
use crate::types::transaction::id::TransactionId;
// use eulerhs::extra::combinators::to_domain_all;
// use eulerhs::extra::aeson::aeson_omit_nothing_fields;
// use eulerhs::language::MonadFlow;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TxnMode {
    #[serde(rename = "PROD")]
    Prod,
    #[serde(rename = "TEST")]
    Test,
}

impl fmt::Display for TxnMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Prod => "PROD",
                Self::Test => "TEST",
            }
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TxnObjectType {
    #[serde(rename = "ORDER_PAYMENT")]
    OrderPayment,
    #[serde(rename = "MANDATE_REGISTER")]
    MandateRegister,
    #[serde(rename = "EMANDATE_REGISTER")]
    EmandateRegister,
    #[serde(rename = "MANDATE_PAYMENT")]
    MandatePayment,
    #[serde(rename = "EMANDATE_PAYMENT")]
    EmandatePayment,
    #[serde(rename = "TPV_PAYMENT")]
    TpvPayment,
    #[serde(rename = "TPV_EMANDATE_REGISTER")]
    TpvEmandateRegister,
    #[serde(rename = "TPV_MANDATE_REGISTER")]
    TpvMandateRegister,
    #[serde(rename = "TPV_EMANDATE_PAYMENT")]
    TpvEmandatePayment,
    #[serde(rename = "TPV_MANDATE_PAYMENT")]
    TpvMandatePayment,
    #[serde(rename = "PARTIAL_CAPTURE")]
    PartialCapture,
    #[serde(rename = "PARTIAL_VOID")]
    PartialVoid,
    #[serde(rename = "VAN_PAYMENT")]
    VanPayment,
    #[serde(rename = "MOTO_PAYMENT")]
    MotoPayment,
}

impl fmt::Display for TxnObjectType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::OrderPayment => "ORDER_PAYMENT",
                Self::MandateRegister => "MANDATE_REGISTER",
                Self::EmandateRegister => "EMANDATE_REGISTER",
                Self::MandatePayment => "MANDATE_PAYMENT",
                Self::EmandatePayment => "EMANDATE_PAYMENT",
                Self::TpvPayment => "TPV_PAYMENT",
                Self::TpvEmandateRegister => "TPV_EMANDATE_REGISTER",
                Self::TpvMandateRegister => "TPV_MANDATE_REGISTER",
                Self::TpvEmandatePayment => "TPV_EMANDATE_PAYMENT",
                Self::TpvMandatePayment => "TPV_MANDATE_PAYMENT",
                Self::PartialCapture => "PARTIAL_CAPTURE",
                Self::PartialVoid => "PARTIAL_VOID",
                Self::VanPayment => "VAN_PAYMENT",
                Self::MotoPayment => "MOTO_PAYMENT",
            }
        )
    }
}

impl TxnObjectType {
    pub fn from_text(text: String) -> Option<Self> {
        match text.as_str() {
            "ORDER_PAYMENT" => Some(Self::OrderPayment),
            "MANDATE_REGISTER" => Some(Self::MandateRegister),
            "EMANDATE_REGISTER" => Some(Self::EmandateRegister),
            "MANDATE_PAYMENT" => Some(Self::MandatePayment),
            "EMANDATE_PAYMENT" => Some(Self::EmandatePayment),
            "TPV_PAYMENT" => Some(Self::TpvPayment),
            "TPV_EMANDATE_REGISTER" => Some(Self::TpvEmandateRegister),
            "TPV_MANDATE_REGISTER" => Some(Self::TpvMandateRegister),
            "TPV_EMANDATE_PAYMENT" => Some(Self::TpvEmandatePayment),
            "TPV_MANDATE_PAYMENT" => Some(Self::TpvMandatePayment),
            "PARTIAL_CAPTURE" => Some(Self::PartialCapture),
            "PARTIAL_VOID" => Some(Self::PartialVoid),
            "VAN_PAYMENT" => Some(Self::VanPayment),
            "MOTO_PAYMENT" => Some(Self::MotoPayment),
            _ => None,
        }
    }

    pub fn to_text(&self) -> &str {
        match self {
            Self::OrderPayment => "ORDER_PAYMENT",
            Self::MandateRegister => "MANDATE_REGISTER",
            Self::EmandateRegister => "EMANDATE_REGISTER",
            Self::MandatePayment => "MANDATE_PAYMENT",
            Self::EmandatePayment => "EMANDATE_PAYMENT",
            Self::TpvPayment => "TPV_PAYMENT",
            Self::TpvEmandateRegister => "TPV_EMANDATE_REGISTER",
            Self::TpvMandateRegister => "TPV_MANDATE_REGISTER",
            Self::TpvEmandatePayment => "TPV_EMANDATE_PAYMENT",
            Self::TpvMandatePayment => "TPV_MANDATE_PAYMENT",
            Self::PartialCapture => "PARTIAL_CAPTURE",
            Self::PartialVoid => "PARTIAL_VOID",
            Self::VanPayment => "VAN_PAYMENT",
            Self::MotoPayment => "MOTO_PAYMENT",
        }
    }
}

impl TxnFlowType {
    pub fn from_text(text: String) -> Option<Self> {
        match text.as_str() {
            "INTENT" => Some(Self::Intent),
            "COLLECT" => Some(Self::Collect),
            "REDIRECT" => Some(Self::Redirect),
            "PAY" => Some(Self::Pay),
            "DIRECT_DEBIT" => Some(Self::DirectDebit),
            "REDIRECT_DEBIT" => Some(Self::RedirectDebit),
            "TOPUP_DIRECT_DEBIT" => Some(Self::TopupDirectDebit),
            "TOPUP_REDIRECT_DEBIT" => Some(Self::TopupRedirectDebit),
            "INAPP_DEBIT" => Some(Self::InappDebit),
            "NET_BANKING" => Some(Self::Netbanking),
            "EMI" => Some(Self::Emi),
            "CARD_TRANSACTION" => Some(Self::CardTransaction),
            "PAY_LATER" => Some(Self::PayLater),
            "AADHAAR_PAY" => Some(Self::AadhaarPay),
            "PAPERNACH" => Some(Self::Papernach),
            "CASH_PAY" => Some(Self::CashPay),
            "QR" => Some(Self::Qr),
            "NATIVE" => Some(Self::Native),
            "PAN" => Some(Self::PAN),
            _ => None,
        }
    }

    pub fn to_text(&self) -> &str {
        match self {
            Self::Intent => "INTENT",
            Self::Collect => "COLLECT",
            Self::Redirect => "REDIRECT",
            Self::Pay => "PAY",
            Self::DirectDebit => "DIRECT_DEBIT",
            Self::RedirectDebit => "REDIRECT_DEBIT",
            Self::TopupDirectDebit => "TOPUP_DIRECT_DEBIT",
            Self::TopupRedirectDebit => "TOPUP_REDIRECT_DEBIT",
            Self::InappDebit => "INAPP_DEBIT",
            Self::Netbanking => "NET_BANKING",
            Self::Emi => "EMI",
            Self::CardTransaction => "CARD_TRANSACTION",
            Self::PayLater => "PAY_LATER",
            Self::AadhaarPay => "AADHAAR_PAY",
            Self::Papernach => "PAPERNACH",
            Self::CashPay => "CASH_PAY",
            Self::Qr => "QR",
            Self::Native => "NATIVE",
            Self::PAN => "PAN",
        }
    }
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TxnFlowType {
    #[serde(rename = "INTENT")]
    Intent,
    #[serde(rename = "COLLECT")]
    Collect,
    #[serde(rename = "REDIRECT")]
    Redirect,
    #[serde(rename = "PAY")]
    Pay,
    #[serde(rename = "DIRECT_DEBIT")]
    DirectDebit,
    #[serde(rename = "REDIRECT_DEBIT")]
    RedirectDebit,
    #[serde(rename = "TOPUP_DIRECT_DEBIT")]
    TopupDirectDebit,
    #[serde(rename = "TOPUP_REDIRECT_DEBIT")]
    TopupRedirectDebit,
    #[serde(rename = "INAPP_DEBIT")]
    InappDebit,
    #[serde(rename = "NET_BANKING")]
    Netbanking,
    #[serde(rename = "EMI")]
    Emi,
    #[serde(rename = "CARD_TRANSACTION")]
    CardTransaction,
    #[serde(rename = "PAY_LATER")]
    PayLater,
    #[serde(rename = "AADHAAR_PAY")]
    AadhaarPay,
    #[serde(rename = "PAPERNACH")]
    Papernach,
    #[serde(rename = "CASH_PAY")]
    CashPay,
    #[serde(rename = "QR")]
    Qr,
    #[serde(rename = "NATIVE")]
    Native,
    #[serde(rename = "PAN")]
    PAN,
}

impl fmt::Display for TxnFlowType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Intent => "INTENT",
                Self::Collect => "COLLECT",
                Self::Redirect => "REDIRECT",
                Self::Pay => "PAY",
                Self::DirectDebit => "DIRECT_DEBIT",
                Self::RedirectDebit => "REDIRECT_DEBIT",
                Self::TopupDirectDebit => "TOPUP_DIRECT_DEBIT",
                Self::TopupRedirectDebit => "TOPUP_REDIRECT_DEBIT",
                Self::InappDebit => "INAPP_DEBIT",
                Self::Netbanking => "NET_BANKING",
                Self::Emi => "EMI",
                Self::CardTransaction => "CARD_TRANSACTION",
                Self::PayLater => "PAY_LATER",
                Self::AadhaarPay => "AADHAAR_PAY",
                Self::Papernach => "PAPERNACH",
                Self::CashPay => "CASH_PAY",
                Self::Qr => "QR",
                Self::Native => "NATIVE",
                Self::PAN => "PAN",
            }
        )
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TxnDetailId(pub i64);

pub fn to_txn_detail_id(ctx: i64) -> TxnDetailId {
    TxnDetailId(ctx)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SuccessResponseId(pub i64);

pub fn convertSuccessResponseIdFlip(ctx: i64) -> SuccessResponseId {
    SuccessResponseId(ctx)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TxnStatus {
    #[serde(rename = "STARTED")]
    Started,
    #[serde(rename = "AUTHENTICATION_FAILED")]
    AuthenticationFailed,
    #[serde(rename = "JUSPAY_DECLINED")]
    JuspayDeclined,
    #[serde(rename = "PENDING_VBV")]
    PendingVBV,
    #[serde(rename = "VBV_SUCCESSFUL")]
    VBVSuccessful,
    #[serde(rename = "AUTHORIZED")]
    Authorized,
    #[serde(rename = "AUTHORIZATION_FAILED")]
    AuthorizationFailed,
    #[serde(rename = "CHARGED")]
    Charged,
    #[serde(rename = "AUTHORIZING")]
    Authorizing,
    #[serde(rename = "COD_INITIATED")]
    CODInitiated,
    #[serde(rename = "VOIDED")]
    Voided,
    #[serde(rename = "VOID_INITIATED")]
    VoidInitiated,
    #[serde(rename = "NOP")]
    Nop,
    #[serde(rename = "CAPTURE_INITIATED")]
    CaptureInitiated,
    #[serde(rename = "CAPTURE_FAILED")]
    CaptureFailed,
    #[serde(rename = "VOID_FAILED")]
    VoidFailed,
    #[serde(rename = "AUTO_REFUNDED")]
    AutoRefunded,
    #[serde(rename = "PARTIAL_CHARGED")]
    PartialCharged,
    #[serde(rename = "TO_BE_CHARGED")]
    ToBeCharged,
    #[serde(rename = "PENDING")]
    Pending,
    #[serde(rename = "FAILURE")]
    Failure,
    #[serde(rename = "DECLINED")]
    Declined,
}

impl TxnStatus {
    pub fn from_text(text: String) -> Option<Self> {
        match text.as_str() {
            "STARTED" => Some(Self::Started),
            "AUTHENTICATION_FAILED" => Some(Self::AuthenticationFailed),
            "JUSPAY_DECLINED" => Some(Self::JuspayDeclined),
            "PENDING_VBV" => Some(Self::PendingVBV),
            "VBV_SUCCESSFUL" => Some(Self::VBVSuccessful),
            "AUTHORIZED" => Some(Self::Authorized),
            "AUTHORIZATION_FAILED" => Some(Self::AuthorizationFailed),
            "CHARGED" => Some(Self::Charged),
            "AUTHORIZING" => Some(Self::Authorizing),
            "COD_INITIATED" => Some(Self::CODInitiated),
            "VOIDED" => Some(Self::Voided),
            "VOID_INITIATED" => Some(Self::VoidInitiated),
            "NOP" => Some(Self::Nop),
            "CAPTURE_INITIATED" => Some(Self::CaptureInitiated),
            "CAPTURE_FAILED" => Some(Self::CaptureFailed),
            "VOID_FAILED" => Some(Self::VoidFailed),
            "AUTO_REFUNDED" => Some(Self::AutoRefunded),
            "PARTIAL_CHARGED" => Some(Self::PartialCharged),
            "TO_BE_CHARGED" => Some(Self::ToBeCharged),
            "PENDING" => Some(Self::Pending),
            "FAILURE" => Some(Self::Failure),
            "DECLINED" => Some(Self::Declined),
            _ => None,
        }
    }

    pub fn to_text(&self) -> &str {
        match self {
            Self::Started => "STARTED",
            Self::AuthenticationFailed => "AUTHENTICATION_FAILED",
            Self::JuspayDeclined => "JUSPAY_DECLINED",
            Self::PendingVBV => "PENDING_VBV",
            Self::VBVSuccessful => "VBV_SUCCESSFUL",
            Self::Authorized => "AUTHORIZED",
            Self::AuthorizationFailed => "AUTHORIZATION_FAILED",
            Self::Charged => "CHARGED",
            Self::Authorizing => "AUTHORIZING",
            Self::CODInitiated => "COD_INITIATED",
            Self::Voided => "VOIDED",
            Self::VoidInitiated => "VOID_INITIATED",
            Self::Nop => "NOP",
            Self::CaptureInitiated => "CAPTURE_INITIATED",
            Self::CaptureFailed => "CAPTURE_FAILED",
            Self::VoidFailed => "VOID_FAILED",
            Self::AutoRefunded => "AUTO_REFUNDED",
            Self::PartialCharged => "PARTIAL_CHARGED",
            Self::ToBeCharged => "TO_BE_CHARGED",
            Self::Pending => "PENDING",
            Self::Failure => "FAILURE",
            Self::Declined => "DECLINED",
        }
    }
}

// #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
// pub struct TxnDetail {
//     pub id: TxnDetailId,
//     pub dateCreated: OffsetDateTime,
//     pub orderId: OrderId,
//     pub status: TxnStatus,
//     pub txnId: TransactionId,
//     pub txnType: NonEmptyText,
//     pub addToLocker: bool,
//     pub merchantId: MerchantId,
//     pub gateway: Option<Gateway>,
//     pub expressCheckout: bool,
//     pub isEmi: bool,
//     pub emiBank: Option<String>,
//     pub emiTenure: Option<i32>,
//     pub txnUuid: String,
//     pub merchantGatewayAccountId: Option<MerchantGwAccId>,
//     pub netAmount: Money,
//     pub txnAmount: Money,
//     pub txnObjectType: TxnObjectType,
//     pub sourceObject: Option<String>,
//     pub sourceObjectId: Option<SourceObjectId>,
//     pub currency: Currency,
//     pub surchargeAmount: Option<Money>,
//     pub taxAmount: Option<Money>,
//     pub internalMetadata: Option<String>,
//     pub metadata: Option<String>,
//     pub offerDeductionAmount: Option<Money>,
//     pub internalTrackingInfo: Option<String>,
//     pub partitionKey: Option<OffsetDateTime>,
//     pub txnAmountBreakup: Option<Vec<TransactionCharge>>,
// }

pub fn deserialize_optional_primitive_datetime<'de, D>(
    deserializer: D,
) -> Result<Option<PrimitiveDateTime>, D::Error>
where
    D: Deserializer<'de>,
{
    let partition_key_str: Option<String> = Option::deserialize(deserializer)?;

    partition_key_str
        .map(|s| {
            // Split the datetime string
            let parts: Vec<&str> = s.split('T').collect();
            if parts.len() != 2 {
                return Err(serde::de::Error::custom("Invalid datetime format"));
            }

            // Parse date
            let date_parts: Vec<&str> = parts[0].split('-').collect();
            if date_parts.len() != 3 {
                return Err(serde::de::Error::custom("Invalid date format"));
            }

            let year: i32 = date_parts[0].parse().map_err(serde::de::Error::custom)?;
            let month: Month = match date_parts[1]
                .parse::<u8>()
                .map_err(serde::de::Error::custom)?
            {
                1 => Month::January,
                2 => Month::February,
                3 => Month::March,
                4 => Month::April,
                5 => Month::May,
                6 => Month::June,
                7 => Month::July,
                8 => Month::August,
                9 => Month::September,
                10 => Month::October,
                11 => Month::November,
                12 => Month::December,
                _ => return Err(serde::de::Error::custom("Invalid month")),
            };
            let day: u8 = date_parts[2].parse().map_err(serde::de::Error::custom)?;

            // Parse time
            let time_parts: Vec<&str> = parts[1].split(':').collect();
            if time_parts.len() != 3 {
                return Err(serde::de::Error::custom("Invalid time format"));
            }

            let hour: u8 = time_parts[0].parse().map_err(serde::de::Error::custom)?;
            let minute: u8 = time_parts[1].parse().map_err(serde::de::Error::custom)?;
            let second: u8 = time_parts[2].parse().map_err(serde::de::Error::custom)?;

            // Create Date
            let date =
                Date::from_calendar_date(year, month, day).map_err(serde::de::Error::custom)?;

            // Create Time
            let time = Time::from_hms(hour, minute, second).map_err(serde::de::Error::custom)?;

            // Create PrimitiveDateTime
            Ok(PrimitiveDateTime::new(date, time))
        })
        .transpose()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TxnDetail {
    #[serde(rename = "id")]
    pub id: TxnDetailId,
    #[serde(with = "time::serde::iso8601")]
    #[serde(rename = "dateCreated")]
    pub dateCreated: OffsetDateTime,
    #[serde(rename = "orderId")]
    pub orderId: OrderId,
    #[serde(rename = "status")]
    pub status: TxnStatus,
    #[serde(rename = "txnId")]
    pub txnId: TransactionId,
    #[serde(rename = "txnType")]
    pub txnType: String,
    #[serde(rename = "addToLocker")]
    pub addToLocker: bool,
    #[serde(rename = "merchantId")]
    pub merchantId: MerchantId,
    #[serde(rename = "gateway")]
    pub gateway: Option<String>,
    #[serde(rename = "expressCheckout")]
    pub expressCheckout: bool,
    #[serde(rename = "isEmi")]
    pub isEmi: bool,
    #[serde(rename = "emiBank")]
    pub emiBank: Option<String>,
    #[serde(rename = "emiTenure")]
    pub emiTenure: Option<i32>,
    #[serde(rename = "txnUuid")]
    pub txnUuid: String,
    #[serde(rename = "merchantGatewayAccountId")]
    pub merchantGatewayAccountId: Option<MerchantGwAccId>,
    #[serde(rename = "netAmount")]
    pub netAmount: Money,
    #[serde(rename = "txnAmount")]
    pub txnAmount: Money,
    #[serde(rename = "txnObjectType")]
    pub txnObjectType: TxnObjectType,
    #[serde(rename = "sourceObject")]
    pub sourceObject: Option<String>,
    #[serde(rename = "sourceObjectId")]
    pub sourceObjectId: Option<SourceObjectId>,
    #[serde(rename = "currency")]
    pub currency: Currency,
    #[serde(rename = "country")]
    pub country: Option<CountryISO2>,
    #[serde(rename = "surchargeAmount")]
    pub surchargeAmount: Option<Money>,
    #[serde(rename = "taxAmount")]
    pub taxAmount: Option<Money>,
    #[serde(rename = "internalMetadata")]
    pub internalMetadata: Option<String>,
    #[serde(rename = "metadata")]
    pub metadata: Option<String>,
    #[serde(rename = "offerDeductionAmount")]
    pub offerDeductionAmount: Option<Money>,
    #[serde(rename = "internalTrackingInfo")]
    pub internalTrackingInfo: Option<String>,
    #[serde(deserialize_with = "deserialize_optional_primitive_datetime")]
    #[serde(rename = "partitionKey")]
    pub partitionKey: Option<PrimitiveDateTime>,
    #[serde(rename = "txnAmountBreakup")]
    pub txnAmountBreakup: Option<Vec<TransactionCharge>>,
    #[serde(rename = "txnLatency")]
    pub txnLatency: Option<Milliseconds>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TxnUuid {
    #[serde(rename = "txnUuid")]
    pub txnUuid: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StartingDate {
    #[serde(rename = "startingDate")]
    pub startingDate: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EndDate {
    #[serde(rename = "endingDate")]
    pub endingDate: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Offset {
    #[serde(rename = "offset")]
    pub offset: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Limit {
    #[serde(rename = "limit")]
    pub limit: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TransactionCharge {
    #[serde(rename = "name")]
    pub name: ChargeName,
    #[serde(rename = "amount")]
    pub amount: f64,
    #[serde(rename = "sno")]
    pub sno: i32,
    #[serde(rename = "method")]
    pub method: ChargeMethod,
    #[serde(rename = "desc")]
    pub desc: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ChargeMethod {
    ADD,
    SUBTRACT,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ChargeName {
    BASE,
    SURCHARGE,
    TAX_ON_SURCHARGE,
    OFFER,
    ADD_ON,
    GATEWAY_ADJUSTMENT,
}
