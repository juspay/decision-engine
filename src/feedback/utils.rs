// Automatically converted from Haskell to Rust
// Generated on 2025-03-23 11:38:31

use crate::app::get_tenant_app_state;
use crate::error::StorageError;
use masking::{PeekInterface, Secret};
// Converted imports
// use eulerhs::prelude::*;
// use eulerhs::language as L;
// use sequelize::Clause::{Is, And};
// use sequelize::Term::Eq;
// use database::beam::mysql::MySQL;
// use juspay::extra::json::encode_json;
// use db::storage::types::merchant_account::{MerchantAccountT, MerchantAccount};
// use db::mesh::internal as EulerDBInternal;
// use feedback::constants::*;
use crate::feedback::types::{
    CachedGatewayScore, InternalMetadata, InternalTrackingInfo, MandateTxnInfo, MandateTxnType,
    UpdateScorePayload,
};
use crate::types::money::internal::Money;
use crate::types::order as ETO;
use crate::types::transaction::id as ETId;
use fred::prelude::{KeysInterface, ListInterface};
// use sequelize::{ModelMeta, OrderBy, Set, Where};
use crate::types::card as ETCa;
use crate::utils as EU;
use serde::{Deserialize, Serialize};
// use utils::errors::merchant_account_null;
// use eulerhs::types as EulerHS;
// use data::text::encoding as TE;
// use db::euler_mesh_impl::mesh_config;
// use utils::database::euler_db::get_euler_db_conf;
// use utils::errors::predefined_errors as Errs;
// use data::map::strict as MP;
use time::PrimitiveDateTime;
// use data::text as T;
// use data::list as DL;
// use ghc::records::extra::HasField;
use crate::decider::gatewaydecider::types::{
    DetailedGatewayScoringType, GatewayReferenceIdMap, GatewayScoringData, GatewayScoringTypeLog,
    GatewayScoringTypeLogData, RoutingFlowType, ScoreKeyType,
};
use crate::types::money::internal as ETMo;
// use gateway_decider::utils as GU;
// use control::monad::extra::maybe_m;
// use data::time::local_time as DTL;
// use data::time::format as DTF;
// use juspay::extra::json::decode_json;
use crate::decider::gatewaydecider::utils::get_unified_key;
// use control::monad::except::{run_except, ExceptT};
// use data::byte_string::lazy as BSL;
// use ghc::generics::Generic;
// use data::time::clock as DTC;
// use data::time::format::iso8601 as ISO;
// use data::time as Time;
// use utils::redis as EWRedis;
// use eulerhs::types as T;
// use types::transaction as TXN;
use crate::types::txn_details::types::{self as ETTD, TxnDetail, TxnObjectType};
// use juspay::extra::secret::{SecretContext, make_secret};
// use juspay::extra::parsing as P;
// use prelude::Int;
// use crate::control::exception as CE;
// use juspay::extra::non_empty_text as NE;
use crate::types::merchant as ETM;
// use types::money::{from_double, Money};
// use optics::core::{preview, review};
// use control::category::<<<;
// use prelude::real_to_frac;
// use data::time::clock::posix as DTP;
use crate::logger;
use crate::redis::feature::{
    RedisCompressionConfig, RedisCompressionConfigCombined, RedisDataStruct,
};
use time::format_description::well_known::Iso8601;
// Converted data types
// Original Haskell data type: GatewayScoringType
#[derive(Debug, Serialize, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GatewayScoringType {
    Penalise,
    PenaliseSrv3,
    Reward,
}

// Original Haskell data type: JuspayBankCode
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct JuspayBankCode {
    #[serde(rename = "juspayBankCode")]
    pub juspayBankCode: String,
}

// Converted functions
// Original Haskell function: convertMerchantGwAccountIdFlip
pub fn convertMerchantGwAccountIdFlip(x: i32) -> ETM::merchant_gateway_account::MerchantGwAccId {
    ETM::merchant_gateway_account::MerchantGwAccId {
        merchantGwAccId: x as i64,
    }
}

// Original Haskell function: transformECTxnDetailToEulerTxnDetail
// pub fn transformECTxnDetailToEulerTxnDetail(req: TxnDetail) -> ETTD::TxnDetail {
//     let merchant_id = req.merchantId;
//     let txn_type = req.txnType;
//     let txn_id = match P::parse(&req.txnId, TXN::toTransactionId) {
//         P::Result(r) => Some(r),
//         P::Failed(_) => None,
//     };
//     let currency = req.currency;
//     let txn_detail_id = req.id;

//     ETTD::TxnDetail {
//         id: txn_detail_id.unwrap_or_else(|| panic!("TxnDetailId is mandatory for TxnDetail")),
//         dateCreated: req.dateCreated,
//         orderId: ETO::OrderId(req.orderId.clone()),
//         status: convertTxnStatusFlip(req.status.clone()),
//         txnId: txn_id.unwrap_or_else(|| panic!("TxnId is mandatory for TxnDetail")),
//         txnType: txn_type.unwrap_or_else(|| panic!("TxnType is mandatory for TxnDetail")),
//         addToLocker: req.addToLocker.unwrap_or(false),
//         merchantId: merchant_id,
//         gateway: req.gateway,
//         expressCheckout: req.expressCheckout.unwrap_or(false),
//         isEmi: req.isEmi.unwrap_or(false),
//         emiBank: req.emiBank.clone(),
//         emiTenure: req.emiTenure.map(|tenure| tenure as i64),
//         txnUuid: req.txnUuid.clone().unwrap_or_default(),
//         merchantGatewayAccountId: req.merchantGatewayAccountId,
//         txnAmount: req.txnAmount,
//         txnObjectType: req.txnObjectType.clone(),
//         sourceObject: req.sourceObject.clone(),
//         sourceObjectId: req.sourceObjectId,
//         currency: currency.unwrap_or(Currency::INR),
//         surchargeAmount: req.surchargeAmount,
//         taxAmount: req.taxAmount,
//         internalMetadata: req.internalMetadata.clone(),
//         netAmount: req.netAmount,
//         metadata: None,
//         offerDeductionAmount: req.offerDeductionAmount,
//         internalTrackingInfo: req.internalTrackingInfo.clone(),
//         partitionKey: req.partitionKey.clone(),
//         txnAmountBreakup: None,
//     }
// }

// Original Haskell function: amountConvertToMoney
pub fn amountConvertToMoney(money: Option<f64>) -> Option<Money> {
    money.map(Money::from_double)
}

// Original Haskell function: convertGatewayFlip
// pub fn convertGatewayFlip(t: String) -> Option<ETG::Gateway> {
//     match P::parse(t, ETG::Gateway::gateway_to_text) {
//         P::Failed(_) => None,
//         P::Result(r) => Some(r),
//     }
// }

// Original Haskell function: convertSuccessResponseIdFlip
pub fn convertSuccessResponseIdFlip(x: i32) -> ETTD::SuccessResponseId {
    ETTD::SuccessResponseId(x as i64)
}

pub fn get_txn_detail_from_api_payload(
    api_payload: UpdateScorePayload,
    gateway_scoring_data: GatewayScoringData,
) -> Result<ETTD::TxnDetail, crate::error::ApiError> {
    let txn_detail = ETTD::TxnDetail {
        id: ETTD::to_txn_detail_id(1),
        dateCreated: gateway_scoring_data.dateCreated,
        orderId: ETO::id::to_order_id(api_payload.payment_id.clone()),
        status: api_payload.status.clone(),
        txnId: ETId::to_transaction_id(api_payload.payment_id.clone()),
        txnType: Some("NOT_DEFINED".to_string()),
        addToLocker: Some(false),
        merchantId: ETM::id::to_merchant_id(api_payload.merchant_id.clone()),
        gateway: Some(api_payload.gateway),
        expressCheckout: Some(false),
        isEmi: Some(false),
        emiBank: None,
        emiTenure: None,
        txnUuid: api_payload.payment_id.clone(),
        merchantGatewayAccountId: None,
        txnAmount: Some(ETMo::Money::from_double(0.0)),
        txnObjectType: Some(
            ETTD::TxnObjectType::from_text(gateway_scoring_data.orderType.clone())
                .unwrap_or_else(|| ETTD::TxnObjectType::OrderPayment),
        ),
        sourceObject: Some(gateway_scoring_data.paymentMethod.clone()),
        sourceObjectId: None,
        currency: gateway_scoring_data
            .currency
            .clone()
            .ok_or(crate::error::ApiError::MissingRequiredField("currency"))?,
        country: gateway_scoring_data.country.clone(),
        surchargeAmount: None,
        taxAmount: None,
        internalMetadata: Some(Secret::new(
            serde_json::to_string(&InternalMetadata {
                internal_tracking_info: InternalTrackingInfo {
                    routing_approach: gateway_scoring_data.routingApproach.unwrap_or_default(),
                },
            })
            .unwrap(),
        )),
        netAmount: Some(ETMo::Money::from_double(0.0)),
        metadata: None,
        offerDeductionAmount: None,
        internalTrackingInfo: None,
        partitionKey: None,
        txnAmountBreakup: None,
    };
    Ok(txn_detail)
}

pub fn get_txn_card_info_from_api_payload(
    api_payload: UpdateScorePayload,
    gateway_scoring_data: GatewayScoringData,
) -> ETCa::txn_card_info::TxnCardInfo {
    let txn_card_info = ETCa::txn_card_info::TxnCardInfo {
        id: ETCa::txn_card_info::to_txn_card_info_pid(1),
        card_isin: None,
        cardIssuerBankName: None,
        cardSwitchProvider: None,
        card_type: None,
        nameOnCard: None,
        dateCreated: gateway_scoring_data.dateCreated,
        paymentMethodType: gateway_scoring_data.paymentMethodType.clone(),
        paymentMethod: gateway_scoring_data.paymentMethod.clone(),
        paymentSource: gateway_scoring_data.paymentSource.clone(),
        authType: gateway_scoring_data
            .authType
            .clone()
            .and_then(|auth_type_text| {
                ETCa::txn_card_info::text_to_auth_type(&auth_type_text).ok()
            }),
        partitionKey: None,
    };
    txn_card_info
}
// Original Haskell function: convertMerchantIdFlip
// pub fn convertMerchantIdFlip(s: &str) -> Option<ETM::MerchantId> {
//     preview(ETM::merchantIdText, s)
// }

// Original Haskell function: convertCurrencyFlip
// pub fn convertCurrencyFlip(s: Text) -> Option<Currency> {
//     preview(Curr::textCurrency, s)
// }

// Original Haskell function: fromString
// pub fn fromString(s: Text) -> Option<i32> {
//     s.to_string().parse::<i32>().ok()
// }

// Original Haskell function: convertTxnObjectTypeFli::
// pub fn convertTxnObjectTypeFlip(txn_object_type: Option<TxnObjectType>) -> ETTD::TxnObjectType {
//     match txn_object_type {
//         Some(TxnObjectType::OrderPayment) => ETTD::TxnObjectType::OrderPayment,
//         Some(TxnObjectType::MandateRegister) => ETTD::TxnObjectType::MandateRegister,
//         Some(TxnObjectType::EmandateRegister) => ETTD::TxnObjectType::EmandateRegister,
//         Some(TxnObjectType::EmandatePayment) => ETTD::TxnObjectType::EmandatePayment,
//         Some(TxnObjectType::MandatePayment) => ETTD::TxnObjectType::MandatePayment,
//         Some(TxnObjectType::TpvPayment) => ETTD::TxnObjectType::TpvPayment,
//         Some(TxnObjectType::PartialCapture) => ETTD::TxnObjectType::PartialCapture,
//         Some(TxnObjectType::TpvEmandateRegister) => ETTD::TxnObjectType::TpvEmandateRegister,
//         Some(TxnObjectType::TpvMandateRegister) => ETTD::TxnObjectType::TpvMandateRegister,
//         Some(TxnObjectType::TpvEmandatePayment) => ETTD::TxnObjectType::TpvEmandatePayment,
//         Some(TxnObjectType::TpvMandatePayment) => ETTD::TxnObjectType::TpvMandatePayment,
//         _ => ETTD::TxnObjectType::OrderPayment,
//     }
// }

// Original Haskell function: convertTxnStatusFlip
// pub fn convertTxnStatusFlip(status: TxnStatus) -> ETTD::TxnStatus {
//     match status {
//         TxnStatus::STARTED => ETTD::TxnStatus::Started,
//         TxnStatus::AUTHENTICATION_FAILED => ETTD::TxnStatus::AuthenticationFailed,
//         TxnStatus::JUSPAY_DECLINED => ETTD::TxnStatus::JuspayDeclined,
//         TxnStatus::PENDING_VBV => ETTD::TxnStatus::PendingVBV,
//         TxnStatus::VBV_SUCCESSFUL => ETTD::TxnStatus::VBVSuccessful,
//         TxnStatus::AUTHORIZED => ETTD::TxnStatus::Authorized,
//         TxnStatus::AUTHORIZATION_FAILED => ETTD::TxnStatus::AuthorizationFailed,
//         TxnStatus::CHARGED => ETTD::TxnStatus::Charged,
//         TxnStatus::AUTHORIZING => ETTD::TxnStatus::Authorizing,
//         TxnStatus::COD_INITIATED => ETTD::TxnStatus::CODInitiated,
//         TxnStatus::VOIDED => ETTD::TxnStatus::Voided,
//         TxnStatus::VOID_INITIATED => ETTD::TxnStatus::VoidInitiated,
//         TxnStatus::NOP => ETTD::TxnStatus::Nop,
//         TxnStatus::CAPTURE_INITIATED => ETTD::TxnStatus::CaptureInitiated,
//         TxnStatus::CAPTURE_FAILED => ETTD::TxnStatus::CaptureFailed,
//         TxnStatus::VOID_FAILED => ETTD::TxnStatus::VoidFailed,
//         TxnStatus::AUTO_REFUNDED => ETTD::TxnStatus::AutoRefunded,
//         TxnStatus::PARTIAL_CHARGED => ETTD::TxnStatus::PartialCharged,
//         TxnStatus::PENDING => ETTD::TxnStatus::Pending,
//         _ => ETTD::TxnStatus::Failure,
//     }
// }

// Original Haskell function: transformECTxncardInfoToEulertxncardInfo
// pub fn transformECTxncardInfoToEulertxncardInfo(req: TxnCardInfo) -> ETCa::TxnCardInfo {
//     let txnCardInfoId = req._id.as_ref().and_then(|id| fromString(id).map(|s| ETCa::TxnCardInfoPId(s as i64)));
//     let txnDetailId = req._id.as_ref().and_then(|id| fromString(id).map(|s| ETTD::TxnDetailId(s as i64)));
//     let txnId = match P::parse(&req.txnId, TXN::toTransactionId) {
//         P::Result(r) => Some(r),
//         P::Failed(_) => None,
//     };

//     ETCa::TxnCardInfo {
//         id: txnCardInfoId.unwrap_or_else(|| panic!("TxnCardInfoId is mandatory for TxnCardInfo")),
//         txnId: txnId.unwrap_or_else(|| panic!("TxnId is mandatory for TxnCardInfo")),
//         cardIsin: req.cardIsin.clone(),
//         cardIssuerBankName: req.cardIssuerBankName.clone(),
//         cardSwitchProvider: req.cardSwitchProvider.as_ref().map(|s| makeSecret(s)),
//         cardType: textToCardType(&req.cardType.clone().unwrap_or_else(|| "".to_string())),
//         nameOnCard: req.nameOnCard.as_ref().map(|s| makeSecret(s)),
//         txnDetailId: txnDetailId.unwrap_or_else(|| panic!("TxnDetailId is mandatory for TxnCardInfo")),
//         dateCreated: req.dateCreated.as_ref().map(|d| getDate(d)).unwrap_or_else(|| panic!("DateCreated is mandatory for TxnCardInfo")),
//         paymentMethodType: transformECPaymentMethodTypeToEulerPaymentMethodType(req.paymentMethodType.clone()),
//         paymentMethod: req.paymentMethod.clone().unwrap_or_else(|| "".to_string()),
//         paymentSource: req.paymentSource.clone(),
//         authType: req.authType.as_ref().map(|s| makeSecret(&textToAuthType(s))),
//         partitionKey: req.partitionKey.clone(),
//     }
// }

// Original Haskell function: textToCardType
// pub fn textToCardType(t: Text) -> Option<ETCa.CardType> {
//     match P.parse(t, ETCa.toCardType) {
//         P.Failed(_) => None,
//         P.Result(r) => Some(r),
//     }
// }

// Original Haskell function: textToAuthType
// pub fn textToAuthType(auth_type: Option<Text>) -> Option<ETCa::AuthType> {
//     match auth_type.as_deref() {
//         Some("ATMPIN") => Some(ETCa::AuthType::Atmpin),
//         Some("THREE_DS") => Some(ETCa::AuthType::ThreeDs),
//         Some("THREE_DS_2") => Some(ETCa::AuthType::ThreeDs2),
//         Some("OTP") => Some(ETCa::AuthType::Otp),
//         Some("OBO_OTP") => Some(ETCa::AuthType::OboOtp),
//         Some("VIES") => Some(ETCa::AuthType::Vies),
//         Some("NO_THREE_DS") => Some(ETCa::AuthType::NoThreeDs),
//         Some("NETWORK_TOKEN") => Some(ETCa::AuthType::NetworkToken),
//         Some("MOTO") => Some(ETCa::AuthType::Moto),
//         Some("FIDO") => Some(ETCa::AuthType::Fido),
//         Some("CTP") => Some(ETCa::AuthType::Ctp),
//         _ => None,
//     }
// }

// Original Haskell function: transformECPaymentMethodTypeToEulerPaymentMethodType
// pub fn transformECPaymentMethodTypeToEulerPaymentMethodType(
//     payment_method_type: Option<FT::PaymentMethodType>,
// ) -> ETP::PaymentMethodType {
//     match payment_method_type {
//         Some(FT::PaymentMethodType::WALLET) => ETP::PaymentMethodType::Wallet,
//         Some(FT::PaymentMethodType::UPI) => ETP::PaymentMethodType::UPI,
//         Some(FT::PaymentMethodType::NB) => ETP::PaymentMethodType::NB,
//         Some(FT::PaymentMethodType::CARD) => ETP::PaymentMethodType::Card,
//         Some(FT::PaymentMethodType::PAYLATER) => ETP::PaymentMethodType::Paylater,
//         Some(FT::PaymentMethodType::CONSUMER_FINANCE) => ETP::PaymentMethodType::ConsumerFinance,
//         Some(FT::PaymentMethodType::REWARD) => ETP::PaymentMethodType::Reward,
//         Some(FT::PaymentMethodType::CASH) => ETP::PaymentMethodType::Cash,
//         Some(FT::PaymentMethodType::AADHAAR) => ETP::PaymentMethodType::Aadhaar,
//         Some(FT::PaymentMethodType::PAPERNACH) => ETP::PaymentMethodType::Papernach,
//         Some(FT::PaymentMethodType::PAN) => ETP::PaymentMethodType::PAN,
//         Some(FT::PaymentMethodType::UNKNOWN(ref val)) if val == "ATM_CARD" => ETP::PaymentMethodType::AtmCard,
//         Some(FT::PaymentMethodType::MerchantContainer) => ETP::PaymentMethodType::MerchantContainer,
//         Some(FT::PaymentMethodType::Virtual_Account) => ETP::PaymentMethodType::VirtualAccount,
//         Some(FT::PaymentMethodType::OTC) => ETP::PaymentMethodType::Otc,
//         Some(FT::PaymentMethodType::RTP) => ETP::PaymentMethodType::Rtp,
//         Some(FT::PaymentMethodType::CRYPTO) => ETP::PaymentMethodType::Crypto,
//         Some(FT::PaymentMethodType::CARD_QR) => ETP::PaymentMethodType::CardQr,
//         Some(FT::PaymentMethodType::UNKNOWN(_)) | None => ETP::PaymentMethodType::Unknown,
//     }
// }

// Original Haskell function: updateScore
pub async fn updateScore(redis: String, key: String, should_score_increase: bool) -> () {
    let app_state = get_tenant_app_state().await;
    let either_res = if should_score_increase {
        app_state.redis_conn.increment_key(&key).await
    } else {
        app_state.redis_conn.decrement_key(&key).await
    };

    match either_res {
        Ok(_int_val) => (),
        Err(err) => {
            logger::error!(
                action = "updateScore",
                tag = "updateScore",
                "Error while updating score in redis - returning Nothing {}",
                err
            );
        }
    }
}

// Original Haskell function: isKeyExistsRedis
pub async fn isKeyExistsRedis(key: String) -> bool {
    let app_state = get_tenant_app_state().await;
    let either_is_in_redis: Result<bool, error_stack::Report<redis_interface::errors::RedisError>> =
        app_state.redis_conn.exists(&key).await;
    match either_is_in_redis {
        Ok(val) => val,
        Err(err) => {
            logger::error!(
                action = "isKeyExistsRedis",
                tag = "isKeyExistsRedis",
                "Error while checking key exists in redis - returning False {}",
                err
            );
            false
        }
    }
}
// Original Haskell function: updateQueue
pub async fn updateQueue(
    redis_name: String,
    queue_key: String,
    score_key: String,
    value: String,
) -> Result<Option<String>, error_stack::Report<redis_interface::errors::RedisError>> {
    let app_state = get_tenant_app_state().await;
    let value_clone = value.clone();
    let r: Result<Vec<String>, error_stack::Report<redis_interface::errors::RedisError>> =
        app_state
            .redis_conn
            .multi(false, |transaction| {
                Box::pin(async move {
                    // Append value to the start of the list
                    transaction
                        .lpush::<(), _, _>(
                            &fred::types::RedisKey::from(queue_key.clone()),
                            vec![&value],
                        )
                        .await?;

                    // Set expiration for queue_key and score_key
                    transaction.expire::<(), _>(&queue_key, 10000000).await?;
                    transaction.expire::<(), _>(&score_key, 10000000).await?;

                    // Remove from the end of the list
                    transaction.rpop::<String, _>(&queue_key, None).await?;

                    Ok(())
                })
            })
            .await;

    match r {
        Ok(result) => {
            logger::debug!(
                action = "updateQueue",
                tag = "updateQueue",
                "Successfully updated queue in Redis: {:?}",
                result
            );
            Ok(Some(result.last().cloned().unwrap_or_default()))
        }
        Err(e) => {
            logger::error!(
                action = "updateQueue",
                tag = "updateQueue",
                "Error while updating queue in Redis: {:?}",
                e
            );
            Err(e)
        }
    }
}

// Original Haskell function: updateMovingWindow
pub async fn updateMovingWindow(
    redis_name: String,
    queue_key: String,
    score_key: String,
    value: String,
) -> String {
    let either_res = updateQueue(redis_name, queue_key, score_key, value.clone()).await;
    match either_res {
        Ok(maybe_val) => maybe_val.unwrap_or(value),
        Err(err) => {
            logger::error!(
                action = "updateMovingWindow",
                tag = "updateMovingWindow",
                "Error while updating queue in redis - returning input value: {:?}",
                err
            );
            value
        }
    }
}

// Original Haskell function: getTrueString
pub fn getTrueString(val: Option<String>) -> Option<String> {
    match val {
        Some(ref value) if value.is_empty() => None,
        Some(value) => Some(value),
        None => None,
    }
}

// Original Haskell function: isTrueString
pub fn isTrueString(val: Option<String>) -> bool {
    match val.map(|v| v.trim().to_string()) {
        Some(ref v) if v.is_empty() => false,
        Some(_) => true,
        None => false,
    }
}

// Original Haskell function: dateInIST
pub fn dateInIST(db_date: String, format: String) -> Option<String> {
    // Parse input date uisng primitivedatetime
    let format_description = match time::format_description::parse(&format) {
        Ok(desc) => desc,
        Err(_) => return None,
    };
    let date = PrimitiveDateTime::parse(&db_date, &format_description);
    let utc_time = match date {
        Ok(d) => d,
        Err(_) => return None,
    };

    // Convert to UTC then to IST (+5:30)
    let result = utc_time + time::Duration::hours(5) + time::Duration::minutes(30);
    match time::format_description::parse(&format) {
        Ok(parsed_format) => Some(
            result
                .format(&parsed_format)
                .unwrap_or_else(|_| "Invalid format".to_string()),
        ),
        Err(_) => None,
    }
}

// Original Haskell function: hush
// pub fn hush<A, B>(e: Either<A, B>) -> Option<B> {
//     match e {
//         Either::Left(_) => None,
//         Either::Right(b) => Some(b),
//     }
// }

// Original Haskell function: getJuspayBankCodeFromInternalMetadata
// pub fn getJuspayBankCodeFromInternalMetadata<R>(object: R) -> Option<String>
// where
//     R: HasField<"internalMetadata", Option<String>>,
// {
//     if let Some(metadata) = object.internalMetadata() {
//         if let Ok(bank_code) = serde_json::from_slice::<JuspayBankCode>(metadata.as_bytes()) {
//             return Some(juspayBankCode(bank_code));
//         }
//     }
//     None
// }

// Original Haskell function: fetchJuspayBankCodeFromPaymentSource

// Original Haskell function: formatLT
// pub fn formatLT(spec: String, u: PrimitiveDateTime) -> String {
//     T::pack(&DTF::format_time(DTF::default_time_locale(), &spec, &u))
// }

// Original Haskell function: getDateTimeFormat
pub fn getDateTimeFormat(format: &str) -> &str {
    match format {
        "YYYY-MM-DD HH:mm:ss" => "%F %T",
        "YYYY/MM/DD" => "%Y/%m/%d",
        "YYYY/MM/DD HH:MM:SS" => "%Y/%m/%d %H:%M:%S",
        "YYYY-MM-DD" => "%F",
        "DD-MM-YYYY HH:mm:ss" => "%d-%m-%Y %H:%M:%S",
        "DD-MM-YYYY HH:mm" => "%d-%m-%Y %R",
        "YYYYMMDDHHMMSS" => "%Y%m%d%H%M%S",
        "DDMMYYYYHHMMSS" => "%d%m%Y%H%M%S",
        "YYYYMMDD" => "%Y%m%d",
        "DD-MM-YYYY" => "%d-%m-%Y",
        "DD/MM/YYYY hh:mm A" => "%d/%m/%Y %I:%M %p",
        "YYYY-MM-DD HH:mm:ss.ms Z" => "%Y-%m-%d %H:%M:%S %z",
        "X" => "%s",
        "DDMMYYYY" => "%d%m%Y",
        "YYYY:MM:DD HH:mm:ssZ" => "%Y:%m:%d %H:%M:%S %z",
        "YYYY-MM-DD HH:mm:ss.SZ" => "%F %H:%M:%S %z",
        "YYYYDDMMHHmmssZ" => "%Y%d%m%H%M%S%z",
        "DD/MM/YYYY" => "%d/%m/%Y",
        "DD/MM/YYYY HH:mm" => "%d/%m/%Y %H:%M",
        "DD/MM/YYYY HH:MM:SS" => "%d/%m/%Y %H:%M:%S",
        "yyyy:MM:DD-HH:mm:ss" => "%Y:%m:%d-%H:%M:%S",
        "ddd, DD MMM YYYY hh:mm:ss [GMT]" => "%a, %d %b %Y %H:%M:%S GMT",
        "MMM DD, YYYY HH:MM:SS A" => "%b %d, %Y %H:%M:%S %p",
        "YYYY-MM-DD HH:mm:SS.sss" => "%Y-%m-%d %H:%M:%S.%q",
        "DD-MM-YYYY hh:mm A" => "%d-%m-%Y %I:%M %p",
        _ => "%F %T",
    }
}

// Original Haskell function: getCurrentIstDateWithFormat
pub fn getCurrentIstDateWithFormat(format: String) -> String {
    let current_time = time::OffsetDateTime::now_utc();
    let format_description = match time::format_description::parse(&format) {
        Ok(desc) => desc,
        Err(_) => return "Invalid format".to_string(),
    };

    current_time
        .format(&format_description)
        .unwrap_or_else(|_| "Invalid format".to_string())
}

// Original Haskell function: getProducerKey
pub async fn getProducerKey(
    txn_detail: TxnDetail,
    redis_gateway_score_data: Option<GatewayScoringData>,
    score_key_type: ScoreKeyType,
    enforce1d: bool,
    gateway_reference_id: Option<String>,
) -> Option<String> {
    match redis_gateway_score_data {
        Some(gateway_score_data) => {
            let is_gri_enabled = if [ScoreKeyType::EliminationMerchantKey].contains(&score_key_type)
            {
                gateway_score_data.isGriEnabledForElimination
            } else if [ScoreKeyType::SrV2Key, ScoreKeyType::SrV3Key].contains(&score_key_type) {
                gateway_score_data.isGriEnabledForSrRouting
            } else {
                false
            };

            let gateway = txn_detail.gateway.unwrap_or_default();

            let gateway_and_reference_id = if is_gri_enabled {
                let mut map = GatewayReferenceIdMap::new();
                map.insert(
                    gateway.clone(),
                    Some(gateway_reference_id.unwrap_or_else(|| "NULL".to_string())),
                );
                map
            } else {
                let mut map = GatewayReferenceIdMap::new();
                map.insert(gateway.clone(), None);
                map
            };

            let gateway_key = get_unified_key(
                gateway_score_data,
                None,
                score_key_type,
                enforce1d,
                gateway_and_reference_id,
            )
            .await;

            let (_, key) = gateway_key.into_iter().next().unwrap();
            logger::debug!(tag = "getProducerKey", "UNIFIED_KEY {}", key);
            Some(key)
        }
        None => {
            logger::error!(
                action = "GATEWAY_SCORING_DATA_NOT_FOUND",
                tag = "GATEWAY_SCORING_DATA_NOT_FOUND",
                "Gateway scoring data is not found in redis"
            );
            None
        }
    }
}

// Original Haskell function: logGatewayScoreType
pub fn log_gateway_score_type(
    gateway_score_type: GatewayScoringType,
    routing_flow_type: RoutingFlowType,
    txn_detail: TxnDetail,
) {
    let detailed_gateway_score_type = match routing_flow_type {
        RoutingFlowType::EliminationFlow => match gateway_score_type {
            GatewayScoringType::Reward => DetailedGatewayScoringType::EliminationReward,
            _ => DetailedGatewayScoringType::EliminationPenalise,
        },
        RoutingFlowType::Srv2Flow => match gateway_score_type {
            GatewayScoringType::Reward => DetailedGatewayScoringType::Srv2Reward,
            _ => DetailedGatewayScoringType::Srv2Penalise,
        },
        _ => match gateway_score_type {
            GatewayScoringType::Reward => DetailedGatewayScoringType::Srv3Reward,
            _ => DetailedGatewayScoringType::Srv3Penalise,
        },
    };

    let txn_creation_time = match &time::OffsetDateTime::now_utc().format(&Iso8601::DEFAULT) {
        Ok(dt) => dt.to_string(),
        Err(_) => "Invalid format".to_string(),
    };

    let log_data = GatewayScoringTypeLogData {
        dateCreated: txn_creation_time,
        score_type: detailed_gateway_score_type,
    };

    let log_json = serde_json::json!({
        "data": log_data,
    });

    logger::info!(
        action = "GATEWAY_SCORE_UPDATED",
        tag = "GATEWAY_SCORE_UPDATED",
        "{}",
        log_json.to_string()
    );
}

// Original Haskell function: writeToCacheWithTTL
pub async fn writeToCacheWithTTL(
    key: String,
    cached_gateway_score: CachedGatewayScore,
    ttl: i64,
    redis_compression_config: Option<RedisCompressionConfigCombined>,
) -> Result<i32, StorageError> {
    //from CachedGatewayScore convert encoded_score to a encoded json that can be used as a value for redis sextx
    let encoded_score =
        serde_json::to_string(&cached_gateway_score).unwrap_or_else(|_| "".to_string());

    let primary_write = addToCacheWithExpiry(
        "kv_redis".to_string(),
        key.clone(),
        encoded_score,
        ttl,
        redis_compression_config,
    )
    .await;

    match primary_write {
        Ok(_) => Ok(0),
        Err(err) => Err(err),
    }
}

// Original Haskell function: addToCacheWithExpiry
pub async fn addToCacheWithExpiry(
    redis_name: String,
    key: String,
    value: String,
    ttl: i64,
    redis_compression_config: Option<RedisCompressionConfigCombined>,
) -> Result<(), StorageError> {
    let app_state = get_tenant_app_state().await;
    let cached_resp = app_state
        .redis_conn
        .setx(
            &key,
            &value,
            ttl,
            redis_compression_config,
            RedisDataStruct::STRING,
        )
        .await;
    match cached_resp {
        Ok(_) => Ok(()),
        Err(error) => Err(StorageError::InsertError),
    }
}

// Original Haskell function: deleteFromCache
pub async fn deleteFromCache(redis_name: String, key: String) -> Result<i32, StorageError> {
    let either_res = delCache(redis_name, key).await;
    // return either_res
    either_res
}

// Original Haskell function: delCache
pub async fn delCache(dbName: String, key: String) -> Result<i32, StorageError> {
    let app_state = get_tenant_app_state().await;
    let data = app_state.redis_conn.conn.delete_key(&key).await;
    // convert data to Result<StorageError, i32>
    match data {
        Ok(res) => Ok(res as i32),
        Err(err) => {
            logger::error!(
                action = "delCache",
                tag = "delCache",
                "Error while deleting score key in redis: {}",
                err
            );
            Err(StorageError::DeleteError)
        }
    }
    // match data{
    //     Ok(res) => (),
    //     Err(err) => {
    //         // Log an error if there's an issue deleting the score key
    //         // L::log_error_v(
    //         //     "deleteScoreKeyIfBucketSizeChanges",
    //         //     "Error while deleting score key in redis",
    //         //     err
    //         // ).await;
    //         ()
    //     }
    // }
    // let result = RC::rDel(dbName, vec![key]).await;
    // result.map_err(replyToError).map(|v| v as i32)
}

// Original Haskell function: replyToError
// pub fn replyToError(reply: Result<DelReply, Report<RedisError>>) -> StorageError {
//     match reply {
//         Ok(_) => StorageError::None,
//         Err(err) => StorageError::RedisError(err),
//     }
// }

// Original Haskell function: getCachedVal
// pub fn getCachedVal<T: serde::de::DeserializeOwned>(
//     identifier: String,
//     fallback_identifier: String,
//     key: String,
// ) -> Option<T> {
//     match getCache(&identifier, &key) {
//         Err(err) => {
// logger::error!(
//     tag = "redis_fetch_error",
//     "Error while getting value from cache {}_: {}",
//     key,
//     err
// );
//             None
//         }
//         Ok(None) => {
// logger::debug!(
//     tag = "redis_fetch_noexist",
//     "Could not find value in cache {}",
//     key
// );

//             None
//         }
//         Ok(Some(val)) => match serde_json::from_slice::<T>(&val.into_bytes()) {
//             Ok(typed_val) => Some(typed_val),
//             Err(_) => {
//                logger::error!(
//     tag = "decode_error",
//     "Error while decoding cached value for {}_",
//     key
// );
//                 None
//             }
//         },
//     }
// }

// Original Haskell function: recurringTxnObjectTypes
pub fn recurringTxnObjectTypes() -> Vec<TxnObjectType> {
    vec![
        TxnObjectType::MandatePayment,
        TxnObjectType::EmandatePayment,
        TxnObjectType::TpvMandatePayment,
        TxnObjectType::TpvEmandatePayment,
    ]
}

// Original Haskell function: mandateRegisterTxnObjectTypes
pub fn mandateRegisterTxnObjectTypes() -> Vec<TxnObjectType> {
    vec![
        TxnObjectType::MandateRegister,
        TxnObjectType::EmandateRegister,
        TxnObjectType::TpvEmandateRegister,
        TxnObjectType::TpvMandateRegister,
    ]
}

pub fn isPennyMandateRegTxn(txn_detail: TxnDetail) -> bool {
    if let Some(txn_object_type) = txn_detail.clone().txnObjectType {
        if isMandateRegTxn(txn_object_type) {
            isPennyTxnType(txn_detail.clone())
        } else {
            false
        }
    } else {
        false
    }
}

// Original Haskell function: getTxnTypeFromInternalMetadata
pub fn getTxnTypeFromInternalMetadata(internal_metadata: Option<Secret<String>>) -> MandateTxnType {
    match internal_metadata {
        None => {
            logger::debug!(
                action = "APP_DEBUG",
                tag = "APP_DEBUG",
                "FETCH_TXN_TYPE_FROM_IM_FLOW"
            );
            MandateTxnType::Default
        }
        Some(internal_metadata) => {
            match serde_json::from_str::<MandateTxnInfo>(internal_metadata.peek()) {
                Ok(txn_info) => txn_info.mandateTxnInfo.txnType,
                Err(_) => MandateTxnType::Default,
            }
        }
    }
}

// Original Haskell function: isMandateRegTxn
pub fn isMandateRegTxn(txn_object_type: TxnObjectType) -> bool {
    mandateRegisterTxnObjectTypes().contains(&txn_object_type)
}

// Original Haskell function: isPennyTxnType
pub fn isPennyTxnType(txn_detail: TxnDetail) -> bool {
    let mandate = getTxnTypeFromInternalMetadata(txn_detail.internalMetadata);
    match mandate {
        MandateTxnType::Register => true,
        _ => false,
    }
}

// Original Haskell function: isRecurringTxn
pub fn isRecurringTxn(txn_object_type: Option<TxnObjectType>) -> bool {
    match txn_object_type {
        Some(t) => recurringTxnObjectTypes().contains(&t),
        None => false,
    }
}

// Original Haskell function: getCache
// pub fn getCache(db_name: String, key: String) -> Result<Option<String>, Error> {
//     let result = RC::rGetB(db_name, TE::encodeUtf8(key));
//     match result {
//         Ok(bytes) => Ok(bytes.map(TE::decodeUtf8)),
//         Err(e) => Err(e),
//     }
// }

// Original Haskell function: getTimeFromTxnCreatedInMills
pub fn get_time_from_txn_created_in_mills(txn: TxnDetail) -> u128 {
    let date_created = txn.dateCreated.unix_timestamp_nanos() as u128 / 1_000_000;
    let current_time = EU::get_current_date_in_millis();
    current_time.saturating_sub(date_created)
}

// Original Haskell function: dateToMilliSeconds
// pub fn dateToMilliSeconds(date: Date) -> f64 {
//     1000.0 * DTC::nominalDiffTimeToSeconds(DTP::utcTimeToPOSIXSeconds(date.getDate())) as f64
// }

// Original Haskell function: getCurrentDateInMillis
// pub fn dateToMilliSeconds(date: Date) -> f64 {
//     date.getDate.millisecond()
// }

// // Original Haskell function: getCurrentDateInMillis
// pub fn getCurrentDateInMillis() -> f64 {
//     L.getPOSIXTime().map(|posix_time| (posix_time * 1000.0) as f64)
// }
