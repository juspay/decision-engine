use serde::{Serialize, Deserialize};
use time::PrimitiveDateTime;
// use serde_json::Value as AValue;
// use std::collections::HashMap;
use std::option::Option;
use std::string::String;
use std::vec::Vec;
use std::time::SystemTime;
use std::fmt;

// use db::eulermeshimpl::mesh_config;
// use db::mesh::internal;
use crate::storage::types::TxnDetail as DBTxnDetail;
// use crate::storage::internal::primd_id_to_int;
// use types::utils::dbconfig::get_euler_db_conf;
use crate::types::currency::{Currency};
use crate::types::gateway::{Gateway, text_to_gateway};
use crate::types::merchant::id::{MerchantId, to_merchant_id};
use crate::types::merchant::merchant_gateway_account::{MerchantGwAccId, to_merchant_gw_acc_id};
use crate::types::money::internal::Money;
use crate::types::order::id::{OrderId, to_order_id};
// use juspay::extra::parsing::{Parsed, ParsingErrorType, Step, around, defaulting, lift_either, lift_pure, mandated, non_empty_text, non_negative, parse_field, project, to_utc};
use crate::types::source_object_id::{SourceObjectId, to_source_object_id};
// use juspay::extra::nonemptytext::NonEmptyText;
use crate::types::transaction::id::{TransactionId, to_transaction_id};
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
        write!(f, "{}", match self {
            TxnMode::Prod => "PROD",
            TxnMode::Test => "TEST",
        })
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
        write!(f, "{}", match self {
            TxnObjectType::OrderPayment => "ORDER_PAYMENT",
            TxnObjectType::MandateRegister => "MANDATE_REGISTER",
            TxnObjectType::EmandateRegister => "EMANDATE_REGISTER",
            TxnObjectType::MandatePayment => "MANDATE_PAYMENT",
            TxnObjectType::EmandatePayment => "EMANDATE_PAYMENT",
            TxnObjectType::TpvPayment => "TPV_PAYMENT",
            TxnObjectType::TpvEmandateRegister => "TPV_EMANDATE_REGISTER",
            TxnObjectType::TpvMandateRegister => "TPV_MANDATE_REGISTER",
            TxnObjectType::TpvEmandatePayment => "TPV_EMANDATE_PAYMENT",
            TxnObjectType::TpvMandatePayment => "TPV_MANDATE_PAYMENT",
            TxnObjectType::PartialCapture => "PARTIAL_CAPTURE",
            TxnObjectType::PartialVoid => "PARTIAL_VOID",
            TxnObjectType::VanPayment => "VAN_PAYMENT",
            TxnObjectType::MotoPayment => "MOTO_PAYMENT",
        })
    }
}

impl TxnObjectType {
    pub fn from_text(text: String) -> Option<Self> {
        match text.as_str() {
            "ORDER_PAYMENT" => Some(TxnObjectType::OrderPayment),
            "MANDATE_REGISTER" => Some(TxnObjectType::MandateRegister),
            "EMANDATE_REGISTER" => Some(TxnObjectType::EmandateRegister),
            "MANDATE_PAYMENT" => Some(TxnObjectType::MandatePayment),
            "EMANDATE_PAYMENT" => Some(TxnObjectType::EmandatePayment),
            "TPV_PAYMENT" => Some(TxnObjectType::TpvPayment),
            "TPV_EMANDATE_REGISTER" => Some(TxnObjectType::TpvEmandateRegister),
            "TPV_MANDATE_REGISTER" => Some(TxnObjectType::TpvMandateRegister),
            "TPV_EMANDATE_PAYMENT" => Some(TxnObjectType::TpvEmandatePayment),
            "TPV_MANDATE_PAYMENT" => Some(TxnObjectType::TpvMandatePayment),
            "PARTIAL_CAPTURE" => Some(TxnObjectType::PartialCapture),
            "PARTIAL_VOID" => Some(TxnObjectType::PartialVoid),
            "VAN_PAYMENT" => Some(TxnObjectType::VanPayment),
            "MOTO_PAYMENT" => Some(TxnObjectType::MotoPayment),
            _ => None,
        }
    }

    pub fn to_text(&self) -> &str {
        match self {
            TxnObjectType::OrderPayment => "ORDER_PAYMENT",
            TxnObjectType::MandateRegister => "MANDATE_REGISTER",
            TxnObjectType::EmandateRegister => "EMANDATE_REGISTER",
            TxnObjectType::MandatePayment => "MANDATE_PAYMENT",
            TxnObjectType::EmandatePayment => "EMANDATE_PAYMENT",
            TxnObjectType::TpvPayment => "TPV_PAYMENT",
            TxnObjectType::TpvEmandateRegister => "TPV_EMANDATE_REGISTER",
            TxnObjectType::TpvMandateRegister => "TPV_MANDATE_REGISTER",
            TxnObjectType::TpvEmandatePayment => "TPV_EMANDATE_PAYMENT",
            TxnObjectType::TpvMandatePayment => "TPV_MANDATE_PAYMENT",
            TxnObjectType::PartialCapture => "PARTIAL_CAPTURE",
            TxnObjectType::PartialVoid => "PARTIAL_VOID",
            TxnObjectType::VanPayment => "VAN_PAYMENT",
            TxnObjectType::MotoPayment => "MOTO_PAYMENT",
        }
    }
}

impl TxnFlowType {
    pub fn from_text(text: String) -> Option<Self> {
        match text.as_str() {
            "INTENT" => Some(TxnFlowType::Intent),
            "COLLECT" => Some(TxnFlowType::Collect),
            "REDIRECT" => Some(TxnFlowType::Redirect),
            "PAY" => Some(TxnFlowType::Pay),
            "DIRECT_DEBIT" => Some(TxnFlowType::DirectDebit),
            "REDIRECT_DEBIT" => Some(TxnFlowType::RedirectDebit),
            "TOPUP_DIRECT_DEBIT" => Some(TxnFlowType::TopupDirectDebit),
            "TOPUP_REDIRECT_DEBIT" => Some(TxnFlowType::TopupRedirectDebit),
            "INAPP_DEBIT" => Some(TxnFlowType::InappDebit),
            "NET_BANKING" => Some(TxnFlowType::Netbanking),
            "EMI" => Some(TxnFlowType::Emi),
            "CARD_TRANSACTION" => Some(TxnFlowType::CardTransaction),
            "PAY_LATER" => Some(TxnFlowType::PayLater),
            "AADHAAR_PAY" => Some(TxnFlowType::AadhaarPay),
            "PAPERNACH" => Some(TxnFlowType::Papernach),
            "CASH_PAY" => Some(TxnFlowType::CashPay),
            "QR" => Some(TxnFlowType::Qr),
            "NATIVE" => Some(TxnFlowType::Native),
            "PAN" => Some(TxnFlowType::PAN),
            _ => None,
        }
    }

    pub fn to_text(&self) -> &str {
        match self {
            TxnFlowType::Intent => "INTENT",
            TxnFlowType::Collect => "COLLECT",
            TxnFlowType::Redirect => "REDIRECT",
            TxnFlowType::Pay => "PAY",
            TxnFlowType::DirectDebit => "DIRECT_DEBIT",
            TxnFlowType::RedirectDebit => "REDIRECT_DEBIT",
            TxnFlowType::TopupDirectDebit => "TOPUP_DIRECT_DEBIT",
            TxnFlowType::TopupRedirectDebit => "TOPUP_REDIRECT_DEBIT",
            TxnFlowType::InappDebit => "INAPP_DEBIT",
            TxnFlowType::Netbanking => "NET_BANKING",
            TxnFlowType::Emi => "EMI",
            TxnFlowType::CardTransaction => "CARD_TRANSACTION",
            TxnFlowType::PayLater => "PAY_LATER",
            TxnFlowType::AadhaarPay => "AADHAAR_PAY",
            TxnFlowType::Papernach => "PAPERNACH",
            TxnFlowType::CashPay => "CASH_PAY",
            TxnFlowType::Qr => "QR",
            TxnFlowType::Native => "NATIVE",
            TxnFlowType::PAN => "PAN",
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
        write!(f, "{}", match self {
            TxnFlowType::Intent => "INTENT",
            TxnFlowType::Collect => "COLLECT",
            TxnFlowType::Redirect => "REDIRECT",
            TxnFlowType::Pay => "PAY",
            TxnFlowType::DirectDebit => "DIRECT_DEBIT",
            TxnFlowType::RedirectDebit => "REDIRECT_DEBIT",
            TxnFlowType::TopupDirectDebit => "TOPUP_DIRECT_DEBIT",
            TxnFlowType::TopupRedirectDebit => "TOPUP_REDIRECT_DEBIT",
            TxnFlowType::InappDebit => "INAPP_DEBIT",
            TxnFlowType::Netbanking => "NET_BANKING",
            TxnFlowType::Emi => "EMI",
            TxnFlowType::CardTransaction => "CARD_TRANSACTION",
            TxnFlowType::PayLater => "PAY_LATER",
            TxnFlowType::AadhaarPay => "AADHAAR_PAY",
            TxnFlowType::Papernach => "PAPERNACH",
            TxnFlowType::CashPay => "CASH_PAY",
            TxnFlowType::Qr => "QR",
            TxnFlowType::Native => "NATIVE",
            TxnFlowType::PAN => "PAN",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TxnDetailId {
    pub txnDetailId: i64,
}

pub fn to_txn_detail_id(id: i64) -> TxnDetailId {
    TxnDetailId {
        txnDetailId: id,
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SuccessResponseId {
    pub successResponseId: i64,
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
            "STARTED" => Some(TxnStatus::Started),
            "AUTHENTICATION_FAILED" => Some(TxnStatus::AuthenticationFailed),
            "JUSPAY_DECLINED" => Some(TxnStatus::JuspayDeclined),
            "PENDING_VBV" => Some(TxnStatus::PendingVBV),
            "VBV_SUCCESSFUL" => Some(TxnStatus::VBVSuccessful),
            "AUTHORIZED" => Some(TxnStatus::Authorized),
            "AUTHORIZATION_FAILED" => Some(TxnStatus::AuthorizationFailed),
            "CHARGED" => Some(TxnStatus::Charged),
            "AUTHORIZING" => Some(TxnStatus::Authorizing),
            "COD_INITIATED" => Some(TxnStatus::CODInitiated),
            "VOIDED" => Some(TxnStatus::Voided),
            "VOID_INITIATED" => Some(TxnStatus::VoidInitiated),
            "NOP" => Some(TxnStatus::Nop),
            "CAPTURE_INITIATED" => Some(TxnStatus::CaptureInitiated),
            "CAPTURE_FAILED" => Some(TxnStatus::CaptureFailed),
            "VOID_FAILED" => Some(TxnStatus::VoidFailed),
            "AUTO_REFUNDED" => Some(TxnStatus::AutoRefunded),
            "PARTIAL_CHARGED" => Some(TxnStatus::PartialCharged),
            "TO_BE_CHARGED" => Some(TxnStatus::ToBeCharged),
            "PENDING" => Some(TxnStatus::Pending),
            "FAILURE" => Some(TxnStatus::Failure),
            "DECLINED" => Some(TxnStatus::Declined),
            _ => None,
        }
    }

    pub fn to_text(&self) -> &str {
        match self {
            TxnStatus::Started => "STARTED",
            TxnStatus::AuthenticationFailed => "AUTHENTICATION_FAILED",
            TxnStatus::JuspayDeclined => "JUSPAY_DECLINED",
            TxnStatus::PendingVBV => "PENDING_VBV",
            TxnStatus::VBVSuccessful => "VBV_SUCCESSFUL",
            TxnStatus::Authorized => "AUTHORIZED",
            TxnStatus::AuthorizationFailed => "AUTHORIZATION_FAILED",
            TxnStatus::Charged => "CHARGED",
            TxnStatus::Authorizing => "AUTHORIZING",
            TxnStatus::CODInitiated => "COD_INITIATED",
            TxnStatus::Voided => "VOIDED",
            TxnStatus::VoidInitiated => "VOID_INITIATED",
            TxnStatus::Nop => "NOP",
            TxnStatus::CaptureInitiated => "CAPTURE_INITIATED",
            TxnStatus::CaptureFailed => "CAPTURE_FAILED",
            TxnStatus::VoidFailed => "VOID_FAILED",
            TxnStatus::AutoRefunded => "AUTO_REFUNDED",
            TxnStatus::PartialCharged => "PARTIAL_CHARGED",
            TxnStatus::ToBeCharged => "TO_BE_CHARGED",
            TxnStatus::Pending => "PENDING",
            TxnStatus::Failure => "FAILURE",
            TxnStatus::Declined => "DECLINED",
        }
    }
}

// #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
// pub struct TxnDetail {
//     pub id: TxnDetailId,
//     pub dateCreated: PrimitiveDateTime,
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
//     pub partitionKey: Option<PrimitiveDateTime>,
//     pub txnAmountBreakup: Option<Vec<TransactionCharge>>,
// }


#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TxnDetail {
    #[serde(rename = "id")]
    pub id: TxnDetailId,
    #[serde(rename = "dateCreated")]
    pub dateCreated: PrimitiveDateTime,
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
    pub gateway: Option<Gateway>,
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
    #[serde(rename = "partitionKey")]
    pub partitionKey: Option<PrimitiveDateTime>,
    #[serde(rename = "txnAmountBreakup")]
    pub txnAmountBreakup: Option<Vec<TransactionCharge>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TxnUuid {
    #[serde(rename = "txnUuid")]
    pub txnUuid: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StartingDate {
    #[serde(rename = "startingDate")]
    pub startingDate: Option<PrimitiveDateTime>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EndDate {
    #[serde(rename = "endingDate")]
    pub endingDate: Option<PrimitiveDateTime>,
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