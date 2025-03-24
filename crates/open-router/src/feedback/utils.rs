// Automatically converted from Haskell to Rust
// Generated on 2025-03-23 11:38:31

// Converted imports
use eulerhs::prelude::*;
use eulerhs::language as L;
use sequelize::Clause::{Is, And};
use sequelize::Term::Eq;
use database::beam::mysql::MySQL;
use juspay::extra::json::encode_json;
use db::storage::types::merchant_account::{MerchantAccountT, MerchantAccount};
use db::mesh::internal as EulerDBInternal;
use feedback::constants::*;
use feedback::types::{TxnCardInfo, Date, CachedGatewayScore, Error, Milliseconds, TxnDetail, TxnStatus, TxnObjectType, MandateTxnType, MandateTxnInfo};
use feedback::types as FT;
use control::exception as CE;
use sequelize::{ModelMeta, OrderBy, Set, Where};
use utils::errors::merchant_account_null;
use eulerhs::types as EulerHS;
use data::text::encoding as TE;
use db::euler_mesh_impl::mesh_config;
use utils::database::euler_db::get_euler_db_conf;
use utils::errors::predefined_errors as Errs;
use data::map::strict as MP;
use serde_json as A;
use data::text as T;
use data::list as DL;
use ghc::records::extra::HasField;
use gateway_decider::types::{RoutingFlowType, DetailedGatewayScoringType, GatewayScoringTypeLogData, GatewayScoringTypeLog, GatewayScoringData, ScoreKeyType, GatewayReferenceIdMap};
use gateway_decider::utils as GU;
use control::monad::extra::maybe_m;
use data::time::local_time as DTL;
use data::time::format as DTF;
use juspay::extra::json::decode_json;
use gateway_decider::utils::{either_decode_t, get_value};
use control::monad::except::{run_except, ExceptT};
use data::byte_string::lazy as BSL;
use ghc::generics::Generic;
use data::time::clock as DTC;
use data::time::format::iso8601 as ISO;
use data::time as Time;
use utils::redis as EWRedis;
use eulerhs::types as T;
use types::transaction as TXN;
use types::payment as ETP;
use eulerhs::tenant_redis_layer as RC;
use types::card as ETCa;
use types::txn_detail as ETTD;
use juspay::extra::secret::{SecretContext, make_secret};
use juspay::extra::parsing as P;
use prelude::Int;
use feedback::constants as C;
use control::exception as CE;
use types::order as ETO;
use types::currency as Curr;
use juspay::extra::non_empty_text as NE;
use types::merchant as ETM;
use types::money::{from_double, Money};
use optics::core::{preview, review};
use control::category::<<<;
use types::gateway as ETG;
use prelude::real_to_frac;
use data::time::clock::posix as DTP;


// Converted data types
// Original Haskell data type: GatewayScoringType
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum GatewayScoringType {
    PENALISE,
    PENALISE_SRV3,
    REWARD,
}


// Original Haskell data type: JuspayBankCode
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct JuspayBankCode {
    #[serde(rename = "juspayBankCode")]
    pub juspayBankCode: String,
}


// Converted functions
// Original Haskell function: convertMerchantGwAccountIdFlip
pub fn convertMerchantGwAccountIdFlip(x: i32) -> ETM::MerchantGwAccId {
    ETM::MerchantGwAccId::from(x as i64)
}


// Original Haskell function: transformECTxnDetailToEulerTxnDetail
pub fn transformECTxnDetailToEulerTxnDetail(req: TxnDetail) -> ETTD::TxnDetail {
    let merchant_id = req.merchantId.as_ref().and_then(|id| convertMerchantIdFlip(id));
    let txn_type = req._type.as_ref().and_then(|t| preview(NE::nonEmpty, t));
    let txn_id = match P::parse(&req.txnId, TXN::toTransactionId) {
        P::Result(r) => Some(r),
        P::Failed(_) => None,
    };
    let currency = req.currency.as_ref().and_then(|c| convertCurrencyFlip(c));
    let txn_detail_id = req._id.as_ref().and_then(|id| fromString(id)).map(|id| ETTD::TxnDetailId(id as i64));

    ETTD::TxnDetail {
        id: txn_detail_id.unwrap_or_else(|| panic!("TxnDetailId is mandatory for TxnDetail")),
        dateCreated: req.dateCreated.as_ref()
            .and_then(|date| getDate(date))
            .unwrap_or_else(|| panic!("DateCreated is mandatory for TxnDetail")),
        orderId: ETO::OrderId(req.orderId.clone()),
        status: convertTxnStatusFlip(req.status.clone()),
        txnId: txn_id.unwrap_or_else(|| panic!("TxnId is mandatory for TxnDetail")),
        txnType: txn_type.unwrap_or_else(|| panic!("TxnType is mandatory for TxnDetail")),
        addToLocker: req.addToLocker.unwrap_or(false),
        merchantId: merchant_id.unwrap_or_else(|| panic!("MerchantId is mandatory for TxnDetail")),
        gateway: req.gateway.as_ref().and_then(|g| convertGatewayFlip(g)),
        expressCheckout: req.expressCheckout.unwrap_or(false),
        isEmi: req.isEmi.unwrap_or(false),
        emiBank: req.emiBank.clone(),
        emiTenure: req.emiTenure.map(|tenure| tenure as i64),
        txnUuid: req.txnUuid.clone().unwrap_or_default(),
        merchantGatewayAccountId: req.merchantGatewayAccountId.as_ref().and_then(|id| convertMerchantGwAccountIdFlip(id)),
        txnAmount: amountConvertToMoney(req.txnAmount).unwrap_or_default(),
        txnObjectType: convertTxnObjectTypeFlip(req.txnObjectType.clone()),
        sourceObject: req.sourceObject.clone(),
        sourceObjectId: req.sourceObjectId.as_ref().map(|id| ETTD::SourceObjectId(id.clone())),
        currency: currency.unwrap_or(Curr::INR),
        surchargeAmount: amountConvertToMoney(req.surchargeAmount),
        taxAmount: amountConvertToMoney(req.taxAmount),
        internalMetadata: req.internalMetadata.clone(),
        netAmount: amountConvertToMoney(req.netAmount).unwrap_or_default(),
        metadata: None,
        offerDeductionAmount: amountConvertToMoney(req.offerDeductionAmount),
        internalTrackingInfo: req.internalTrackingInfo.clone(),
        partitionKey: req.partitionKey.clone(),
        txnAmountBreakup: None,
    }
}


// Original Haskell function: amountConvertToMoney
pub fn amountConvertToMoney(money: Option<f64>) -> Option<Money> {
    money.map(|m| fromDouble(m))
}


// Original Haskell function: convertGatewayFlip
pub fn convertGatewayFlip(t: Text) -> Option<ETG::Gateway> {
    match P::parse(t, ETG::textToGateway) {
        P::Failed(_) => None,
        P::Result(r) => Some(r),
    }
}


// Original Haskell function: convertSuccessResponseIdFlip
pub fn convertSuccessResponseIdFlip(x: i32) -> ETTD::SuccessResponseId {
    ETTD::SuccessResponseId(x as i64)
}


// Original Haskell function: convertMerchantIdFlip
pub fn convertMerchantIdFlip(s: &str) -> Option<ETM::MerchantId> {
    preview(ETM::merchantIdText, s)
}


// Original Haskell function: convertCurrencyFlip
pub fn convertCurrencyFlip(s: Text) -> Option<Curr::Currency> {
    preview(Curr::textCurrency, s)
}


// Original Haskell function: fromString
pub fn fromString(s: Text) -> Option<i32> {
    s.to_string().parse::<i32>().ok()
}


// Original Haskell function: convertTxnObjectTypeFlip
pub fn convertTxnObjectTypeFlip(txn_object_type: Option<TxnObjectType>) -> ETTD.TxnObjectType {
    match txn_object_type {
        Some(TxnObjectType::ORDER_PAYMENT) => ETTD.TxnObjectType::OrderPayment,
        Some(TxnObjectType::MANDATE_REGISTER) => ETTD.TxnObjectType::MandateRegister,
        Some(TxnObjectType::EMANDATE_REGISTER) => ETTD.TxnObjectType::EmandateRegister,
        Some(TxnObjectType::EMANDATE_PAYMENT) => ETTD.TxnObjectType::EmandatePayment,
        Some(TxnObjectType::MANDATE_PAYMENT) => ETTD.TxnObjectType::MandatePayment,
        Some(TxnObjectType::TPV_PAYMENT) => ETTD.TxnObjectType::TpvPayment,
        Some(TxnObjectType::PARTIAL_CAPTURE) => ETTD.TxnObjectType::PartialCapture,
        Some(TxnObjectType::TPV_EMANDATE_REGISTER) => ETTD.TxnObjectType::TpvEmandateRegister,
        Some(TxnObjectType::TPV_MANDATE_REGISTER) => ETTD.TxnObjectType::TpvMandateRegister,
        Some(TxnObjectType::TPV_EMANDATE_PAYMENT) => ETTD.TxnObjectType::TpvEmandatePayment,
        Some(TxnObjectType::TPV_MANDATE_PAYMENT) => ETTD.TxnObjectType::TpvMandatePayment,
        _ => ETTD.TxnObjectType::OrderPayment,
    }
}


// Original Haskell function: convertTxnStatusFlip
pub fn convertTxnStatusFlip(status: TxnStatus) -> ETTD::TxnStatus {
    match status {
        TxnStatus::STARTED => ETTD::TxnStatus::Started,
        TxnStatus::AUTHENTICATION_FAILED => ETTD::TxnStatus::AuthenticationFailed,
        TxnStatus::JUSPAY_DECLINED => ETTD::TxnStatus::JuspayDeclined,
        TxnStatus::PENDING_VBV => ETTD::TxnStatus::PendingVBV,
        TxnStatus::VBV_SUCCESSFUL => ETTD::TxnStatus::VBVSuccessful,
        TxnStatus::AUTHORIZED => ETTD::TxnStatus::Authorized,
        TxnStatus::AUTHORIZATION_FAILED => ETTD::TxnStatus::AuthorizationFailed,
        TxnStatus::CHARGED => ETTD::TxnStatus::Charged,
        TxnStatus::AUTHORIZING => ETTD::TxnStatus::Authorizing,
        TxnStatus::COD_INITIATED => ETTD::TxnStatus::CODInitiated,
        TxnStatus::VOIDED => ETTD::TxnStatus::Voided,
        TxnStatus::VOID_INITIATED => ETTD::TxnStatus::VoidInitiated,
        TxnStatus::NOP => ETTD::TxnStatus::Nop,
        TxnStatus::CAPTURE_INITIATED => ETTD::TxnStatus::CaptureInitiated,
        TxnStatus::CAPTURE_FAILED => ETTD::TxnStatus::CaptureFailed,
        TxnStatus::VOID_FAILED => ETTD::TxnStatus::VoidFailed,
        TxnStatus::AUTO_REFUNDED => ETTD::TxnStatus::AutoRefunded,
        TxnStatus::PARTIAL_CHARGED => ETTD::TxnStatus::PartialCharged,
        TxnStatus::PENDING => ETTD::TxnStatus::Pending,
        _ => ETTD::TxnStatus::Failure,
    }
}


// Original Haskell function: transformECTxncardInfoToEulertxncardInfo
pub fn transformECTxncardInfoToEulertxncardInfo(req: TxnCardInfo) -> ETCa::TxnCardInfo {
    let txnCardInfoId = req._id.as_ref().and_then(|id| fromString(id).map(|s| ETCa::TxnCardInfoPId(s as i64)));
    let txnDetailId = req._id.as_ref().and_then(|id| fromString(id).map(|s| ETTD::TxnDetailId(s as i64)));
    let txnId = match P::parse(&req.txnId, TXN::toTransactionId) {
        P::Result(r) => Some(r),
        P::Failed(_) => None,
    };

    ETCa::TxnCardInfo {
        id: txnCardInfoId.unwrap_or_else(|| panic!("TxnCardInfoId is mandatory for TxnCardInfo")),
        txnId: txnId.unwrap_or_else(|| panic!("TxnId is mandatory for TxnCardInfo")),
        cardIsin: req.cardIsin.clone(),
        cardIssuerBankName: req.cardIssuerBankName.clone(),
        cardSwitchProvider: req.cardSwitchProvider.as_ref().map(|s| makeSecret(s)),
        cardType: textToCardType(&req.cardType.clone().unwrap_or_else(|| "".to_string())),
        nameOnCard: req.nameOnCard.as_ref().map(|s| makeSecret(s)),
        txnDetailId: txnDetailId.unwrap_or_else(|| panic!("TxnDetailId is mandatory for TxnCardInfo")),
        dateCreated: req.dateCreated.as_ref().map(|d| getDate(d)).unwrap_or_else(|| panic!("DateCreated is mandatory for TxnCardInfo")),
        paymentMethodType: transformECPaymentMethodTypeToEulerPaymentMethodType(req.paymentMethodType.clone()),
        paymentMethod: req.paymentMethod.clone().unwrap_or_else(|| "".to_string()),
        paymentSource: req.paymentSource.clone(),
        authType: req.authType.as_ref().map(|s| makeSecret(&textToAuthType(s))),
        partitionKey: req.partitionKey.clone(),
    }
}


// Original Haskell function: textToCardType
pub fn textToCardType(t: Text) -> Option<ETCa.CardType> {
    match P.parse(t, ETCa.toCardType) {
        P.Failed(_) => None,
        P.Result(r) => Some(r),
    }
}


// Original Haskell function: textToAuthType
pub fn textToAuthType(auth_type: Option<Text>) -> Option<ETCa::AuthType> {
    match auth_type.as_deref() {
        Some("ATMPIN") => Some(ETCa::AuthType::ATMPIN),
        Some("THREE_DS") => Some(ETCa::AuthType::THREE_DS),
        Some("THREE_DS_2") => Some(ETCa::AuthType::THREE_DS_2),
        Some("OTP") => Some(ETCa::AuthType::OTP),
        Some("OBO_OTP") => Some(ETCa::AuthType::OBO_OTP),
        Some("VIES") => Some(ETCa::AuthType::VIES),
        Some("NO_THREE_DS") => Some(ETCa::AuthType::NO_THREE_DS),
        Some("NETWORK_TOKEN") => Some(ETCa::AuthType::NETWORK_TOKEN),
        Some("MOTO") => Some(ETCa::AuthType::MOTO),
        Some("FIDO") => Some(ETCa::AuthType::FIDO),
        Some("CTP") => Some(ETCa::AuthType::CTP),
        _ => None,
    }
}


// Original Haskell function: transformECPaymentMethodTypeToEulerPaymentMethodType
pub fn transformECPaymentMethodTypeToEulerPaymentMethodType(
    payment_method_type: Option<FT::PaymentMethodType>,
) -> ETP::PaymentMethodType {
    match payment_method_type {
        Some(FT::PaymentMethodType::WALLET) => ETP::PaymentMethodType::Wallet,
        Some(FT::PaymentMethodType::UPI) => ETP::PaymentMethodType::UPI,
        Some(FT::PaymentMethodType::NB) => ETP::PaymentMethodType::NB,
        Some(FT::PaymentMethodType::CARD) => ETP::PaymentMethodType::Card,
        Some(FT::PaymentMethodType::PAYLATER) => ETP::PaymentMethodType::Paylater,
        Some(FT::PaymentMethodType::CONSUMER_FINANCE) => ETP::PaymentMethodType::ConsumerFinance,
        Some(FT::PaymentMethodType::REWARD) => ETP::PaymentMethodType::Reward,
        Some(FT::PaymentMethodType::CASH) => ETP::PaymentMethodType::Cash,
        Some(FT::PaymentMethodType::AADHAAR) => ETP::PaymentMethodType::Aadhaar,
        Some(FT::PaymentMethodType::PAPERNACH) => ETP::PaymentMethodType::Papernach,
        Some(FT::PaymentMethodType::PAN) => ETP::PaymentMethodType::PAN,
        Some(FT::PaymentMethodType::UNKNOWN(ref val)) if val == "ATM_CARD" => ETP::PaymentMethodType::AtmCard,
        Some(FT::PaymentMethodType::MERCHANT_CONTAINER) => ETP::PaymentMethodType::MerchantContainer,
        Some(FT::PaymentMethodType::Virtual_Account) => ETP::PaymentMethodType::VirtualAccount,
        Some(FT::PaymentMethodType::OTC) => ETP::PaymentMethodType::Otc,
        Some(FT::PaymentMethodType::RTP) => ETP::PaymentMethodType::Rtp,
        Some(FT::PaymentMethodType::CRYPTO) => ETP::PaymentMethodType::Crypto,
        Some(FT::PaymentMethodType::CARD_QR) => ETP::PaymentMethodType::CardQr,
        Some(FT::PaymentMethodType::UNKNOWN(_)) | None => ETP::PaymentMethodType::Unknown,
    }
}


// Original Haskell function: updateScore
pub fn updateScore(
    redis_name: String,
    key: String,
    should_score_increase: bool,
) -> () {
    let either_res = if should_score_increase {
        EWRedis::incr(&redis_name, &key)
    } else {
        EWRedis::decr(&redis_name, &key)
    };

    match either_res {
        Ok(_int_val) => (),
        Err(err) => {
            L::logInfoV(
                "updateScore",
                &format!("Error while updating score in redis - returning Nothing: {:?}", err),
            );
        }
    }
}


// Original Haskell function: isKeyExistsRedis
pub fn isKeyExistsRedis(redis_name: Text, key: Text) -> Bool {
    let either_is_in_redis = EWRedis::keyExistsCache(redis_name, key);
    match either_is_in_redis {
        Ok(val) => val,
        Err(err) => {
            L::logErrorV(
                "isKeyExistsRedis",
                &("Error while checking key exists in redis - returning False ", err),
            );
            false
        }
    }
}


// Original Haskell function: updateQueue
pub fn updateQueue(
    db_name: Text,
    queue_key: Text,
    score_key: Text,
    value: Text,
) -> Either<Text, Option<Text>> {
    let result = RC::multiExec(db_name, |k| {
        RC::lpushTx(TE::encodeUtf8(queue_key), vec![TE::encodeUtf8(value)], k);
        RC::expireTx(TE::encodeUtf8(queue_key), 10000000, k);
        RC::expireTx(TE::encodeUtf8(score_key), 10000000, k);
        RC::rpopTx(TE::encodeUtf8(queue_key), k)
    });

    match result {
        Err(_) => Either::Left("Error".into()),
        Ok(T::TxSuccess(x)) => Either::Right(x.map(TE::decodeUtf8)),
        Ok(T::TxAborted) => Either::Left("Error".into()),
        Ok(T::TxError(_)) => Either::Left("Error".into()),
    }
}


// Original Haskell function: updateMovingWindow
pub fn updateMovingWindow(
    redis_name: Text,
    queue_key: Text,
    score_key: Text,
    value: Text,
) -> Text {
    let either_res = updateQueue(redis_name, queue_key, score_key, value.clone()).await;
    match either_res {
        Ok(maybe_val) => maybe_val.unwrap_or(value),
        Err(err) => {
            L::logErrorV(
                "updateMovingWindow",
                &format!("Error while updating queue in redis - returning input value: {}", err),
            )
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
pub fn isTrueString(val: Option<Text>) -> bool {
    match val.map(|v| v.trim()) {
        Some(ref v) if v.is_empty() => false,
        Some(_) => true,
        None => false,
    }
}


// Original Haskell function: dateInIST
pub fn dateInIST(db_date: String, format: String) -> Option<String> {
    // Parse input date
    let utc_time = match chrono::NaiveDateTime::parse_from_str(&db_date, "%Y-%m-%d %H:%M:%S") {
        Ok(t) => t,
        Err(_) => return None,
    };

    // Convert to UTC then to IST (+5:30)
    let utc = chrono::DateTime::<chrono::Utc>::from_utc(utc_time, chrono::Utc);
    let ist = utc.with_timezone(&chrono::FixedOffset::east(5 * 3600 + 30 * 60));
    let result = ist.format(&format).to_string();

    Some(result)
}


// Original Haskell function: hush
pub fn hush<A, B>(e: Either<A, B>) -> Option<B> {
    match e {
        Either::Left(_) => None,
        Either::Right(b) => Some(b),
    }
}


// Original Haskell function: getJuspayBankCodeFromInternalMetadata
pub fn getJuspayBankCodeFromInternalMetadata<R>(object: R) -> Option<String>
where
    R: HasField<"internalMetadata", Option<String>>,
{
    if let Some(metadata) = object.internalMetadata() {
        if let Ok(bank_code) = serde_json::from_slice::<JuspayBankCode>(metadata.as_bytes()) {
            return Some(juspayBankCode(bank_code));
        }
    }
    None
}


// Original Haskell function: fetchJuspayBankCodeFromPaymentSource
pub fn fetchJuspayBankCodeFromPaymentSource(txnCardInfo: TxnCardInfo) -> Option<Text> {
    txnCardInfo.paymentSource.and_then(|source| getValue("juspay_bank_code", source))
}


// Original Haskell function: formatLT
pub fn formatLT(spec: String, u: DTL::LocalTime) -> Text {
    T::pack(&DTF::format_time(DTF::default_time_locale(), &spec, &u))
}


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
pub fn getCurrentIstDateWithFormat(format: Text) -> Text {
    let date = getCurrentTime();
    let utc_time = DTL::utcToLocalTime(DTL::TimeZone::new(330, false, "IST"), date);
    formatLT(getDateTimeFormat(&format.to_string()), utc_time)
}


// Original Haskell function: getProducerKey
pub fn getProducerKey(
    txn_detail: TxnDetail,
    redis_gateway_score_data: Option<GatewayScoringData>,
    score_key_type: ScoreKeyType,
    enforce1d: bool,
) -> Option<String> {
        match redis_gateway_score_data {
            Some(gateway_score_data) => {
                let is_gri_enabled = if [ELIMINATION_MERCHANT_KEY].contains(&score_key_type) {
                    gateway_score_data.is_gri_enabled_for_elimination
                } else if [SR_V2_KEY, SR_V3_KEY].contains(&score_key_type) {
                    gateway_score_data.is_gri_enabled_for_sr_routing
                } else {
                    false
                };

                let gateway = txn_detail.gateway.unwrap_or_else(|| "".to_string());

                let gateway_and_reference_id = if is_gri_enabled {
                    let merchant_gateway_account = txn_detail.merchant_gateway_account_id.and_then(|mga_id| {
                        // Replace `undefined` with the actual implementation
                        Some(()) // Placeholder for actual implementation
                    });

                    let gw_ref_id = merchant_gateway_account.and_then(|_| {
                        // Replace `undefined` with the actual implementation
                        Some("NULL".to_string()) // Placeholder for actual implementation
                    }).unwrap_or_else(|| "NULL".to_string());

                    let mut map = GatewayReferenceIdMap::new();
                    map.insert(gateway.clone(), Some(gw_ref_id));
                    map
                } else {
                    let mut map = GatewayReferenceIdMap::new();
                    map.insert(gateway.clone(), None);
                    map
                };

                let gateway_key = GU::getUnifiedKey(
                    gateway_score_data,
                    score_key_type,
                    enforce1d,
                    gateway_and_reference_id,
                );

                let (_, key) = gateway_key.into_iter().next().unwrap();
                L::logInfoV("UNIFIED_KEY", &key);
                Some(key)
            }
            None => {
                L::logErrorV("GATEWAY_SCORING_DATA_NOT_FOUND", "Gateway scoring data is not found in redis");
                None
            }
        }
}


// Original Haskell function: logGatewayScoreType
pub fn logGatewayScoreType(
    gateway_score_type: GatewayScoringType,
    routing_flow_type: RoutingFlowType,
    txn_detail: TxnDetail,
) -> () {
    let detailed_gateway_score_type = match routing_flow_type {
        RoutingFlowType::ELIMINATION_FLOW => match gateway_score_type {
            GatewayScoringType::REWARD => GatewayScoringType::ELIMINATION_REWARD,
            _ => GatewayScoringType::ELIMINATION_PENALISE,
        },
        RoutingFlowType::SRV2_FLOW => match gateway_score_type {
            GatewayScoringType::REWARD => GatewayScoringType::SRV2_REWARD,
            _ => GatewayScoringType::SRV2_PENALISE,
        },
        _ => match gateway_score_type {
            GatewayScoringType::REWARD => GatewayScoringType::SRV3_REWARD,
            _ => GatewayScoringType::SRV3_PENALISE,
        },
    };

    let txn_creation_time = txn_detail.date_created
        .to_string()
        .replace(" ", "T")
        .replace(" UTC", "Z");

    let log_data = GatewayScoringTypeLogData {
        txn_creation_time,
        detailed_gateway_score_type,
    };

    let log_entry = GatewayScoringTypeLog {
        log_data: A.to_json(log_data),
    };

    L.logInfoV::<String>("GATAEWAY_SCORE_UPDATED", log_entry)
}


// Original Haskell function: writeToCacheWithTTL
pub fn writeToCacheWithTTL(
    enforce_kv_redis: bool,
    disable_fallback: bool,
    key: Text,
    cached_gateway_score: CachedGatewayScore,
    ttl: f64,
) -> Result<i32, Error> {
    let ttl_ms = Milliseconds(ttl);
    let encoded_score = encodeJSON(cached_gateway_score);

    let primary_cache = if enforce_kv_redis {
        (C.kvRedis, C.kvRedis2)
    } else {
        (C.ecRedis, C.ecRedis2)
    };

    let fallback_cache = if enforce_kv_redis {
        (C.ecRedis, C.ecRedis2)
    } else {
        (C.kvRedis, C.kvRedis2)
    };

    let primary_write = addToCacheWithExpiry(primary_cache.0, primary_cache.1, key.clone(), encoded_score.clone(), ttl_ms);
    let fallback_write = addToCacheWithExpiry(fallback_cache.0, fallback_cache.1, key.clone(), encoded_score.clone(), ttl_ms);

    let primary_delete = deleteFromCache(primary_cache.0, primary_cache.1, key.clone());
    let fallback_delete = deleteFromCache(fallback_cache.0, fallback_cache.1, key.clone());

    match primary_write {
        Ok(_) => {
            if disable_fallback {
                Ok(0)
            } else {
                primary_delete.map_err(|e| e.into())
            }
        }
        Err(err) => {
            if disable_fallback {
                Err(err)
            } else {
                match fallback_write {
                    Ok(_) => Ok(0),
                    Err(_) => Err(err),
                }
            }
        }
    }
}


// Original Haskell function: addToCacheWithExpiry
pub fn addToCacheWithExpiry(
    redis_name: String,
    fallback_redis_name: String,
    key: String,
    value: String,
    ttl: Milliseconds,
) -> Result<(), Error> {
    let cached_resp = setCacheWithExpiry(redis_name, key, value, ttl);
    match cached_resp {
        Ok(_) => cached_resp,
        Err(error) => cached_resp,
    }
}


// Original Haskell function: deleteFromCache
pub fn deleteFromCache(
    redis_name: Text,
    fallback_redis_name: Text,
    key: Text,
) -> Result<Int, Error> {
        let either_res = delCache(redis_name, key).await;
        either_res
}


// Original Haskell function: delCache
pub fn delCache(
    dbName: Text,
    key: Text,
) -> Either<Error, i32> {
        let result = RC::rDel(dbName, vec![key]).await;
        result.map_err(replyToError).map(|v| v as i32)
}


// Original Haskell function: replyToError
pub fn replyToError(reply: EulerHS::KVDBReply) -> Error {
    CE::throw(ErrorText(show(reply)))
}


// Original Haskell function: getCachedVal
pub fn getCachedVal<T: serde::de::DeserializeOwned>(
    identifier: String,
    fallback_identifier: String,
    key: String,
) -> Option<T> {
        match getCache(&identifier, &key) {
            Err(err) => {
                L::logErrorV::<String>(
                    "redis_fetch_error",
                    &format!("Error while getting value from cache {}_", key),
                    &err,
                );
                None
            }
            Ok(None) => {
                L::logDebugV::<String>(
                    "redis_fetch_noexist",
                    &format!("Could not find value in cache {}", key),
                );
                None
            }
            Ok(Some(val)) => match serde_json::from_slice::<T>(&val.into_bytes()) {
                Ok(typed_val) => Some(typed_val),
                Err(_) => {
                    L::logErrorV::<String>(
                        "decode_error",
                        &format!("Error while decoding cached value for {}_", key),
                    );
                    None
                }
            },
        }
}


// Original Haskell function: recurringTxnObjectTypes
pub fn recurringTxnObjectTypes() -> Vec<TxnObjectType> {
    vec![
        TxnObjectType::MANDATE_PAYMENT,
        TxnObjectType::EMANDATE_PAYMENT,
        TxnObjectType::TPV_MANDATE_PAYMENT,
        TxnObjectType::TPV_EMANDATE_PAYMENT,
    ]
}


// Original Haskell function: mandateRegisterTxnObjectTypes
pub fn mandateRegisterTxnObjectTypes() -> Vec<TxnObjectType> {
    vec![
        TxnObjectType::MANDATE_REGISTER,
        TxnObjectType::EMANDATE_REGISTER,
        TxnObjectType::TPV_EMANDATE_REGISTER,
        TxnObjectType::TPV_MANDATE_REGISTER,
    ]
}

pub fn isPennyMandateRegTxn(txn_detail: TxnDetail) -> Bool {  
    if isMandateRegTxn(txn_detail.txnObjectType.unwrap_or(ORDER_PAYMENT)) {  
        isPennyTxnType(txn_detail)  
    } else {  
        false  
    }  
}  

// Original Haskell function: getTxnTypeFromInternalMetadata
pub fn getTxnTypeFromInternalMetadata(
    internal_metadata: Option<String>,
) -> MandateTxnType {
    match internal_metadata {
        None => {
            logDebugT("APP_DEBUG", "FETCH_TXN_TYPE_FROM_IM_FLOW");
                    (DEFAULT)
        }
        Some(internal_metadata) => {
            let decoded: Result<MandateTxnInfo, _> = serde_json::from_slice(internal_metadata.as_bytes());
            match decoded {
                    (txn_info.mandateTxnInfo.txnType),
                Err(err) => {
                    // logErrorV::<String>("DECODE_ERROR", &format!("Failed to decode mandate info: {}", err));
                    (DEFAULT)
                }
            }
        }
    }
}


// Original Haskell function: isMandateRegTxn
pub fn isMandateRegTxn(txn_object_type: TxnObjectType) -> bool {
    mandateRegisterTxnObjectTypes.contains(&txn_object_type)
}


// Original Haskell function: isPennyTxnType
pub fn isPennyTxnType(txn_detail: TxnDetail) -> bool {
        let mandate = getTxnTypeFromInternalMetadata(txn_detail.internalMetadata).await;
        match mandate {
            TxnType::REGISTER => true,
            _ => false,
        }
}


// Original Haskell function: isRecurringTxn
pub fn isRecurringTxn(txn_object_type: Option<TxnObjectType>) -> bool {
    match txn_object_type {
        Some(t) => recurringTxnObjectTypes.contains(&t),
        None => false,
    }
}


// Original Haskell function: getCache
pub fn getCache(
    db_name: Text,
    key: Text,
) -> Result<Option<Text>, Error> {
        let result = RC::rGetB(db_name, TE::encodeUtf8(key));
        match result {
            Ok(bytes) => Ok(bytes.map(TE::decodeUtf8)),
            Err(e) => Err(e),
        }
}


// Original Haskell function: getTimeFromTxnCreatedInMills
pub fn getTimeFromTxnCreatedInMills(txn: TxnDetail) -> Double {
        match txn.dateCreated {
            None => 0.0,
            Some(date) => {
                let txnCreatedInMillis = dateToMilliSeconds(date);
                let currentMillis = getCurrentDateInMillis();
                currentMillis - txnCreatedInMillis
            }
        }
}


// Original Haskell function: dateToMilliSeconds
pub fn dateToMilliSeconds(date: Date) -> f64 {
    1000.0 * DTC::nominalDiffTimeToSeconds(DTP::utcTimeToPOSIXSeconds(date.getDate())) as f64
}


// Original Haskell function: getCurrentDateInMillis
pub fn getCurrentDateInMillis() -> f64 {
    L.getPOSIXTime().map(|posix_time| (posix_time * 1000.0) as f64)
}

