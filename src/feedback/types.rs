// Automatically converted from Haskell to Rust
// Generated on 2025-03-23 12:53:45

// Converted imports
// use eulerhs::prelude::*;
use serde::{Deserialize, Serialize};
use time::PrimitiveDateTime;
// use database::beam as B;
// use chrono::{Local, Utc};
use crate::types::gateway as ETG;
use crate::types::txn_details::types::{Offset, TransactionLatency, TxnStatus};
use std::string::String;
use time::OffsetDateTime;
// use eulerhs::types::MeshError;

// // Converted type synonyms
// // Original Haskell type: TxnDetail
// pub type TxnDetail = TxnDetailT<Identity>;

// // Original Haskell type: TxnCardInfo
// pub type TxnCardInfo = TxnCardInfoT<Identity>;

// // Original Haskell type: MerchantGatewayAccount
// pub type MerchantGatewayAccount = MerchantGatewayAccountT<Identity>;

// Converted data types
// Original Haskell data type: MandateTxnInfo
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct MandateTxnInfo {
    #[serde(rename = "mandateTxnInfo")]
    pub mandateTxnInfo: TxnInfo,
}

// Original Haskell data type: TxnInfo
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct TxnInfo {
    #[serde(rename = "debitAmount")]
    pub debitAmount: f64,
    #[serde(rename = "txnType")]
    pub txnType: MandateTxnType,
}

// Original Haskell data type: MandateTxnType
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum MandateTxnType {
    REGISTER,
    REGISTER_AND_DEBIT,
    DEFAULT,
}

// Original Haskell data type: MerchantScoringDetails
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, PartialOrd)]
pub struct MerchantScoringDetails {
    #[serde(rename = "merchantId")]
    pub merchantId: String,

    #[serde(rename = "transactionCount")]
    pub transactionCount: i32,

    #[serde(rename = "score")]
    pub score: f64,

    #[serde(rename = "lastResetTimestamp")]
    pub lastResetTimestamp: i32,
}

// Original Haskell data type: GatewayScoringKeyType
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct GatewayScoringKeyType {
    #[serde(rename = "key")]
    pub key: Option<String>,

    #[serde(rename = "ttl")]
    pub ttl: Option<i32>,

    #[serde(rename = "downThreshold")]
    pub downThreshold: Option<f64>,

    #[serde(rename = "eliminationMaxCount")]
    pub eliminationMaxCount: Option<i32>,

    #[serde(rename = "dimension")]
    pub dimension: Option<ScoringDimension>,

    #[serde(rename = "merchantId")]
    pub merchantId: String,

    #[serde(rename = "gateway")]
    pub gateway: String,

    #[serde(rename = "authType")]
    pub authType: Option<String>,

    #[serde(rename = "cardBin")]
    pub cardBin: Option<String>,

    #[serde(rename = "cardIssuerBankName")]
    pub cardIssuerBankName: Option<String>,

    #[serde(rename = "paymentMethodType")]
    pub paymentMethodType: Option<String>,

    #[serde(rename = "paymentMethod")]
    pub paymentMethod: Option<String>,

    #[serde(rename = "sourceObject")]
    pub sourceObject: Option<String>,

    #[serde(rename = "paymentSource")]
    pub paymentSource: Option<String>,

    #[serde(rename = "cardType")]
    pub cardType: Option<String>,

    #[serde(rename = "keyType")]
    pub keyType: KeyType,

    #[serde(rename = "scoreType")]
    pub scoreType: ScoreType,

    #[serde(rename = "softTTL")]
    pub softTTL: Option<i32>,
    
    #[serde(rename = "gatewayReferenceId")]
    pub gatewayReferenceId: Option<String>,
}

// Original Haskell data type: TxnDetailT
// #[derive(Debug, Serialize, Deserialize, PartialEq)]
// pub struct TxnDetailT<F> {
//     #[serde(rename = "_id")]
//     pub _id: Option<String>,

//     #[serde(rename = "orderId")]
//     pub orderId: BC<F, String>,

//     #[serde(rename = "status")]
//     pub status: BC<F, TxnStatus>,

//     #[serde(rename = "dateCreated")]
//     pub dateCreated: BC<F, Option<Date>>,

//     #[serde(rename = "txnId")]
//     pub txnId: BC<F, String>,

//     #[serde(rename = "_type")]
//     pub _type: BC<F, String>,

//     #[serde(rename = "addToLocker")]
//     pub addToLocker: BC<F, Option<bool>>,

//     #[serde(rename = "merchantId")]
//     pub merchantId: BC<F, Option<String>>,

//     #[serde(rename = "gateway")]
//     pub gateway: BC<F, Option<String>>,

//     #[serde(rename = "expressCheckout")]
//     pub expressCheckout: BC<F, Option<bool>>,

//     #[serde(rename = "isEmi")]
//     pub isEmi: BC<F, Option<bool>>,

//     #[serde(rename = "emiBank")]
//     pub emiBank: BC<F, Option<String>>,

//     #[serde(rename = "emiTenure")]
//     pub emiTenure: BC<F, Option<i32>>,

//     #[serde(rename = "txnUuid")]
//     pub txnUuid: BC<F, Option<String>>,

//     #[serde(rename = "merchantGatewayAccountId")]
//     pub merchantGatewayAccountId: BC<F, Option<i32>>,

//     #[serde(rename = "txnAmount")]
//     pub txnAmount: BC<F, Option<f64>>,

//     #[serde(rename = "txnObjectType")]
//     pub txnObjectType: BC<F, Option<TxnObjectType>>,

//     #[serde(rename = "sourceObject")]
//     pub sourceObject: BC<F, Option<String>>,

//     #[serde(rename = "sourceObjectId")]
//     pub sourceObjectId: BC<F, Option<String>>,

//     #[serde(rename = "currency")]
//     pub currency: BC<F, Option<String>>,

//     #[serde(rename = "netAmount")]
//     pub netAmount: BC<F, Option<f64>>,

//     #[serde(rename = "surchargeAmount")]
//     pub surchargeAmount: BC<F, Option<f64>>,

//     #[serde(rename = "taxAmount")]
//     pub taxAmount: BC<F, Option<f64>>,

//     #[serde(rename = "offerDeductionAmount")]
//     pub offerDeductionAmount: BC<F, Option<f64>>,

//     #[serde(rename = "metadata")]
//     pub metadata: BC<F, Option<String>>,

//     #[serde(rename = "internalMetadata")]
//     pub internalMetadata: BC<F, Option<String>>,

//     #[serde(rename = "txnLinkUuid")]
//     pub txnLinkUuid: BC<F, Option<String>>,

//     #[serde(rename = "internalTrackingInfo")]
//     pub internalTrackingInfo: BC<F, Option<String>>,

//     #[serde(rename = "partitionKey")]
//     pub partitionKey: BC<F, Option<LocalTime>>,
// }

// Original Haskell data type: TxnObjectType
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TxnObjectType {
    MANDATE_REGISTER,
    MANDATE_PAYMENT,
    ORDER_PAYMENT,
    EMANDATE_REGISTER,
    EMANDATE_PAYMENT,
    TPV_PAYMENT,
    PARTIAL_CAPTURE,
    PARTIAL_VOID,
    TPV_EMANDATE_REGISTER,
    TPV_MANDATE_REGISTER,
    TPV_EMANDATE_PAYMENT,
    TPV_MANDATE_PAYMENT,
    VAN_PAYMENT,
    MOTO_PAYMENT,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UpdateScorePayload {
    pub merchantId: String,
    pub gateway: String,
    pub gatewayReferenceId: Option<String>,
    pub status: TxnStatus,
    pub paymentId: String,
    pub enforceDynamicRoutingFailure: Option<bool>,
    pub txnLatency: Option<TransactionLatency>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UpdateScoreResponse {
    pub message: String,
    pub merchant_id: String,
    pub gateway: String,
    pub payment_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InternalMetadata {
    #[serde(rename = "internal_tracking_info")]
    pub internal_tracking_info: InternalTrackingInfo,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InternalTrackingInfo {
    #[serde(rename = "routing_approach")]
    pub routing_approach: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TransactionLatencyThreshold {
    #[serde(rename = "gatewayLatency")]
    pub gatewayLatency: Option<f64>,
}

// Original Haskell data type: Error
// #[derive(Debug, Serialize, Deserialize, PartialEq)]
// #[serde(tag = "type", content = "value")]
// pub enum Error {
//     #[serde(rename = "ErrorText")]
//     ErrorText(String),

//     #[serde(rename = "ShimException")]
//     ShimException(String, String),

//     #[serde(rename = "DB_Error")]
//     DB_Error(EulerHSMeshError),

//     #[serde(rename = "MissingFieldException")]
//     MissingFieldException(String),

//     #[serde(rename = "NumberConversionFailed")]
//     NumberConversionFailed(String),
// }

// Original Haskell data type: ScoringDimension
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum ScoringDimension {
    #[serde(rename = "FIRST")]
    FIRST,

    #[serde(rename = "SECOND")]
    SECOND,

    #[serde(rename = "THIRD")]
    THIRD,

    #[serde(rename = "FOURTH")]
    FOURTH,
}

// Original Haskell data type: KeyType
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum KeyType {
    #[serde(rename = "GLOBAL")]
    Global,

    #[serde(rename = "MERCHANT")]
    Merchant,
}

// Original Haskell data type: ScoreType
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum ScoreType {
    #[serde(rename = "GATEWAY")]
    Gateway,

    #[serde(rename = "OUTAGE")]
    Outage,
}

// Original Haskell data type: CachedGatewayScore
#[derive(Debug, Serialize, Clone, Deserialize, PartialEq)]
pub struct CachedGatewayScore {
    #[serde(rename = "merchants")]
    pub merchants: Option<Vec<MerchantScoringDetails>>,

    #[serde(rename = "score")]
    pub score: Option<f64>,

    #[serde(rename = "timestamp")]
    pub timestamp: u128,

    #[serde(rename = "lastResetTimestamp")]
    pub lastResetTimestamp: Option<u128>,

    #[serde(rename = "transactionCount")]
    pub transactionCount: Option<i32>,
}

// Original Haskell data type: SrV3DebugBlock
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct SrV3DebugBlock {
    #[serde(rename = "txn_uuid")]
    pub txn_uuid: String,

    #[serde(rename = "order_id")]
    pub order_id: String,

    #[serde(rename = "date_created")]
    pub date_created: String,

    #[serde(rename = "current_time")]
    pub current_time: String,

    #[serde(rename = "txn_status")]
    pub txn_status: String,
}

// Original Haskell data type: TxnCardInfoT
// #[derive(Debug, Serialize, Deserialize, PartialEq)]
// pub struct TxnCardInfoT<F> {
//     #[serde(rename = "_id")]
//     pub _id: BC<F, Option<String>>,

//     #[serde(rename = "txnId")]
//     pub txnId: BC<F, String>,

//     #[serde(rename = "cardIsin")]
//     pub cardIsin: BC<F, Option<String>>,

//     #[serde(rename = "cardIssuerBankName")]
//     pub cardIssuerBankName: BC<F, Option<String>>,

//     #[serde(rename = "cardExpYear")]
//     pub cardExpYear: BC<F, Option<String>>,

//     #[serde(rename = "cardExpMonth")]
//     pub cardExpMonth: BC<F, Option<String>>,

//     #[serde(rename = "cardSwitchProvider")]
//     pub cardSwitchProvider: BC<F, Option<String>>,

//     #[serde(rename = "cardType")]
//     pub cardType: BC<F, Option<String>>,

//     #[serde(rename = "cardLastFourDigits")]
//     pub cardLastFourDigits: BC<F, Option<String>>,

//     #[serde(rename = "nameOnCard")]
//     pub nameOnCard: BC<F, Option<String>>,

//     #[serde(rename = "cardFingerprint")]
//     pub cardFingerprint: BC<F, Option<String>>,

//     #[serde(rename = "cardReferenceId")]
//     pub cardReferenceId: BC<F, Option<String>>,

//     #[serde(rename = "txnDetailId")]
//     pub txnDetailId: BC<F, Option<String>>,

//     #[serde(rename = "dateCreated")]
//     pub dateCreated: BC<F, Option<Date>>,

//     #[serde(rename = "paymentMethodType")]
//     pub paymentMethodType: BC<F, Option<PaymentMethodType>>,

//     #[serde(rename = "paymentMethod")]
//     pub paymentMethod: BC<F, Option<String>>,

//     #[serde(rename = "cardGlobalFingerprint")]
//     pub cardGlobalFingerprint: BC<F, Option<String>>,

//     #[serde(rename = "paymentSource")]
//     pub paymentSource: BC<F, Option<String>>,

//     #[serde(rename = "authType")]
//     pub authType: BC<F, Option<String>>,

//     #[serde(rename = "partitionKey")]
//     pub partitionKey: BC<F, Option<LocalTime>>,
// }

// Original Haskell data type: PaymentMethodType
// #[derive(Debug, Serialize, Deserialize)]
// #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
// pub enum PaymentMethodType {
//     WALLET,
//     UPI,
//     NB,
//     CARD,
//     PAYLATER,
//     CONSUMER_FINANCE,
//     REWARD,
//     CASH,
//     AADHAAR,
//     PAPERNACH,
//     PAN,
//     MERCHANT_CONTAINER,
//     Virtual_Account,
//     OTC,
//     RTP,
//     CRYPTO,
//     CARD_QR,
//     #[serde(rename = "UNKNOWN")]
//     UNKNOWN(String),
// }

// Original Haskell data type: MerchantGatewayAccountT
// #[derive(Debug, Serialize, Deserialize, PartialEq)]
// pub struct MerchantGatewayAccountT<F> {
//     #[serde(rename = "_id")]
//     pub _id: BC<F, Option<i32>>,

//     #[serde(rename = "version")]
//     pub version: BC<F, i32>,

//     #[serde(rename = "gateway")]
//     pub gateway: BC<F, String>,

//     #[serde(rename = "merchantId")]
//     pub merchantId: BC<F, String>,

//     #[serde(rename = "paymentMethods")]
//     pub paymentMethods: BC<F, Option<String>>,

//     #[serde(rename = "testMode")]
//     pub testMode: BC<F, Option<bool>>,

//     #[serde(rename = "disabled")]
//     pub disabled: BC<F, Option<bool>>,

//     #[serde(rename = "disabledBy")]
//     pub disabledBy: BC<F, Option<String>>,

//     #[serde(rename = "disabledAt")]
//     pub disabledAt: BC<F, Option<Date>>,

//     #[serde(rename = "isJuspayAccount")]
//     pub isJuspayAccount: BC<F, Option<bool>>,

//     #[serde(rename = "enforcePaymentMethodAcceptance")]
//     pub enforcePaymentMethodAcceptance: BC<F, Option<bool>>,

//     #[serde(rename = "referenceId")]
//     pub referenceId: BC<F, Option<String>>,

//     #[serde(rename = "supportedCurrencies")]
//     pub supportedCurrencies: BC<F, Option<String>>,

//     #[serde(rename = "gatewayIdentifier")]
//     pub gatewayIdentifier: BC<F, Option<String>>,

//     #[serde(rename = "gatewayType")]
//     pub gatewayType: BC<F, Option<String>>,

//     #[serde(rename = "lastModified")]
//     pub lastModified: BC<F, Option<Date>>,

//     #[serde(rename = "dateCreated")]
//     pub dateCreated: BC<F, Option<Date>>,

//     #[serde(rename = "masterAccountDetailId")]
//     pub masterAccountDetailId: BC<F, Option<String>>,

//     #[serde(rename = "merchantIdentifier")]
//     pub merchantIdentifier: BC<F, Option<String>>,

//     #[serde(rename = "supportedTxnType")]
//     pub supportedTxnType: BC<F, Option<String>>,
// }

// Converted newtypes
// Original Haskell newtype: Milliseconds
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, PartialOrd)]
pub struct Milliseconds {
    #[serde(rename = "milliseconds")]
    pub milliseconds: f64,
}

// Original Haskell newtype: Date
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct Date {
    #[serde(rename = "getDate")]
    pub getDate: PrimitiveDateTime,
}

// Converted functions
// Original Haskell function: defaultEnumEncodeT
// pub fn defaultEnumEncodeT(str: PaymentMethodType) -> Text {
//     match str {
//         PaymentMethodType::WALLET => "WALLET".into(),
//         PaymentMethodType::UPI => "UPI".into(),
//         PaymentMethodType::NB => "NB".into(),
//         PaymentMethodType::CARD => "CARD".into(),
//         PaymentMethodType::PAYLATER => "PAYLATER".into(),
//         PaymentMethodType::CONSUMER_FINANCE => "CONSUMER_FINANCE".into(),
//         PaymentMethodType::REWARD => "REWARD".into(),
//         PaymentMethodType::CASH => "CASH".into(),
//         PaymentMethodType::AADHAAR => "AADHAAR".into(),
//         PaymentMethodType::PAPERNACH => "PAPERNACH".into(),
//         PaymentMethodType::PAN => "PAN".into(),
//         PaymentMethodType::MERCHANT_CONTAINER => "MERCHANT_CONTAINER".into(),
//         PaymentMethodType::Virtual_Account => "VIRTUAL_ACCOUNT".into(),
//         PaymentMethodType::OTC => "OTC".into(),
//         PaymentMethodType::RTP => "RTP".into(),
//         PaymentMethodType::CRYPTO => "CRYPTO".into(),
//         PaymentMethodType::CARD_QR => "CARD_QR".into(),
//         PaymentMethodType::UNKNOWN(txt) => txt,
//     }
// }

// Original Haskell function: defaultEnumDecodeT
// pub fn defaultEnumDecodeT(str: &str) -> PaymentMethodType {
//     match str.to_uppercase().as_str() {
//         "WALLET" => PaymentMethodType::WALLET,
//         "UPI" => PaymentMethodType::UPI,
//         "NB" => PaymentMethodType::NB,
//         "CARD" => PaymentMethodType::CARD,
//         "PAYLATER" => PaymentMethodType::PAYLATER,
//         "CONSUMER_FINANCE" => PaymentMethodType::CONSUMER_FINANCE,
//         "REWARD" => PaymentMethodType::REWARD,
//         "CASH" => PaymentMethodType::CASH,
//         "AADHAAR" => PaymentMethodType::AADHAAR,
//         "PAPERNACH" => PaymentMethodType::PAPERNACH,
//         "PAN" => PaymentMethodType::PAN,
//         "MERCHANT_CONTAINER" => PaymentMethodType::MERCHANT_CONTAINER,
//         "VIRTUAL_ACCOUNT" => PaymentMethodType::Virtual_Account,
//         "OTC" => PaymentMethodType::OTC,
//         "RTP" => PaymentMethodType::RTP,
//         "CRYPTO" => PaymentMethodType::CRYPTO,
//         "CARD_QR" => PaymentMethodType::CARD_QR,
//         x => PaymentMethodType::UNKNOWN(x.to_string()),
//     }
// }
