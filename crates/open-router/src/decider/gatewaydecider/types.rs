use serde::ser::SerializeStruct;
use serde::{Serialize, Deserialize};
use serde_json::Value as AValue;
use time::PrimitiveDateTime;
use std::collections::HashMap as HMap;
use std::option::Option;
use std::vec::Vec;
use std::string::String;
use std::i64;

use crate::app::TenantAppState;
use crate::error::ApiError;
// use eulerhs::prelude::*;
// use eulerhs::language::MonadFlow;
// use juspay::extra::secret::SecretContext;
// use data::reflection::Given;
// use data::time::{UTCTime, LocalTime};
// use unsafe_coerce::unsafeCoerce;
use crate::types::card as ETCa;
use crate::types::gateway as ETG;
use crate::types::merchant::id::MerchantId;
use crate::types::merchant as ETM;
use crate::types::order as ETO;
use crate::types::order_metadata_v2 as ETOMV2;
use crate::types::txn_details::types as ETTD;
use crate::types::txn_offer_detail as ETTOD;
use crate::types::txn_offer_info as ETTOI;
// use utils::framework::capture as Capture;
use crate::types::gateway_routing_input as ETGRI;
// use eulerhs::language as L;
// use juspay::extra::parsing as Parsing;
use crate::types::payment as ETP;
// use utils::errors as Errors;
// use eulerhs::tenantredislayer as RC;

#[derive(Debug, Serialize, Deserialize)]
pub enum DeciderFilterName {
    GetFunctionalGateways,
    FilterFunctionalGatewaysForCurrency,
    FilterBySurcharge,
    FilterFunctionalGatewaysForBrand,
    FilterFunctionalGatewaysForAuthType,
    FilterFunctionalGatewaysForValidationType,
    FilterFunctionalGatewaysForEmi,
    FilterFunctionalGatewaysForTxnOfferDetails,
    FilterFunctionalGatewaysForPaymentMethod,
    FilterFunctionalGatewaysForTokenProvider,
    FilterFunctionalGatewaysForTxnOfferInfo,
    FilterFunctionalGatewaysForWallet,
    FilterFunctionalGatewaysForNbOnly,
    FilterFunctionalGatewaysForConsumerFinance,
    FilterFunctionalGatewaysForUpi,
    FilterFunctionalGatewaysForTxnType,
    FilterFunctionalGatewaysForTxnDetailType,
    FilterFunctionalGatewaysForReward,
    FilterFunctionalGatewaysForCash,
    FilterFunctionalGatewaysForSplitSettlement,
    FilterFunctionalGateways,
    FinalFunctionalGateways,
    FilterByPriorityLogic,
    PreferredGateway,
    FilterEnforcement,
    GatewayPriorityList,
    FilterFunctionalGatewaysForMerchantRequiredFlow,
    FilterGatewaysForMGASelectionIntegrity,
    FilterGatewaysForEMITenureSpecficGatewayCreds,
    FilterFunctionalGatewaysForReversePennyDrop,
    FilterFunctionalGatewaysForOTM,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum DeciderScoringName {
    UpdateScoreForIssuer,
    UpdateScoreForIsin,
    UpdateScoreForCardBrand,
    UpdateScoreWithHealth,
    UpdateScoreIfLastTxnFailure,
    UpdateScoreForOutage,
    ScoringByGatewayScoreBasedOnGlobalSuccessRate,
    UpdateGatewayScoreBasedOnSuccessRate,
    FinalScoring,
    GetScoreWithPriority,
    GetCachedScoresBasedOnSuccessRate,
    GetCachedScoresBasedOnSrV3,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum DetailedGatewayScoringType {
    ELIMINATION_PENALISE,
    ELIMINATION_REWARD,
    SRV2_PENALISE,
    SRV2_REWARD,
    SRV3_PENALISE,
    SRV3_REWARD,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum RoutingFlowType {
    ELIMINATION_FLOW,
    SRV2_FLOW,
    SRV3_FLOW,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ScoreUpdateStatus {
    PENALISED,
    REWARDED,
    NOT_INITIATED,
}

pub type GatewayScoreMap = HMap<ETG::Gateway, f64>;
pub type GatewayList = Vec<ETG::Gateway>;
pub type GatewayReferenceIdMap = HMap<Gateway, Option<String>>;
pub type GatewayRedisKeyMap = HMap<Gateway, RedisKey>;
pub type Gateway = String;
pub type RedisKey = String;

#[derive(Debug, Serialize, Deserialize)]
pub struct GatewayScoringTypeLogData {
    pub dateCreated: String,
    pub score_type: DetailedGatewayScoringType,
}

#[derive(Debug)]
pub struct GatewayScoringTypeLog {
    pub log_data: AValue,
}

impl Serialize for GatewayScoringTypeLog {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer {
        let mut state = serializer.serialize_struct("GatewayScoringTypeLog", 1)?;
        state.serialize_field("data", &self.log_data)?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for GatewayScoringTypeLog {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let data = deserializer.deserialize_struct("GatewayScoringTypeLog", &["data"], GatewayScoringTypeLogVisitor)?;
        Ok(GatewayScoringTypeLog { log_data: data })
    }
}

struct GatewayScoringTypeLogVisitor;

impl<'de> serde::de::Visitor<'de> for GatewayScoringTypeLogVisitor {
    type Value = AValue;

    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
        formatter.write_str("struct GatewayScoringTypeLog")
    }

    fn visit_map<V>(self, mut map: V) -> Result<AValue, V::Error>
    where
        V: serde::de::MapAccess<'de>,
    {
        let mut data = None;
        while let Some(key) = map.next_key()? {
            match key {
                "data" => {
                    if data.is_some() {
                        return Err(serde::de::Error::duplicate_field("data"));
                    }
                    data = Some(map.next_value()?);
                }
                _ => {
                    let _: serde::de::IgnoredAny = map.next_value()?;
                }
            }
        }
        let data = data.ok_or_else(|| serde::de::Error::missing_field("data"))?;
        Ok(data)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SRMetricLogData {
    #[serde(rename = "gateway_after_evaluation")]
    pub gatewayAfterEvaluation: Option<ETG::Gateway>,
    #[serde(rename = "gateway_before_evaluation")]
    pub gatewayBeforeEvaluation: Option<ETG::Gateway>,
    #[serde(rename = "merchant_gateway_score")]
    pub merchantGatewayScore: Option<AValue>,
    #[serde(rename = "downtime_status")]
    pub downtimeStatus: Vec<ETG::Gateway>,
    #[serde(rename = "date_created")]
    pub dateCreated: String,
    #[serde(rename = "gateway_before_downtime_evaluation")]
    pub gatewayBeforeDowntimeEvaluation: Option<ETG::Gateway>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeciderGatewayWiseSuccessRateBasedRoutingInput {
    pub gateway: ETG::Gateway,
    pub eliminationThreshold: Option<f64>,
    pub eliminationMaxCountThreshold: Option<i64>,
    pub selectionMaxCountThreshold: Option<i64>,
    pub softTxnResetCount: Option<i64>,
    pub gatewayLevelEliminationThreshold: Option<f64>,
    pub eliminationLevel: Option<ETGRI::EliminationLevel>,
    pub currentScore: Option<f64>,
    pub lastResetTimeStamp: Option<i64>,
}

pub fn transform_gateway_wise_success_rate_based_routing(
    gateway_wise_success_rate_input: ETGRI::GatewayWiseSuccessRateBasedRoutingInput,
) -> DeciderGatewayWiseSuccessRateBasedRoutingInput {
    DeciderGatewayWiseSuccessRateBasedRoutingInput {
        gateway: gateway_wise_success_rate_input.gateway,
        eliminationThreshold: gateway_wise_success_rate_input.eliminationThreshold,
        eliminationMaxCountThreshold: gateway_wise_success_rate_input.eliminationMaxCountThreshold,
        selectionMaxCountThreshold: gateway_wise_success_rate_input.selectionMaxCountThreshold,
        softTxnResetCount: gateway_wise_success_rate_input.softTxnResetCount,
        gatewayLevelEliminationThreshold: gateway_wise_success_rate_input.gatewayLevelEliminationThreshold,
        eliminationLevel: gateway_wise_success_rate_input.eliminationLevel,
        currentScore: gateway_wise_success_rate_input.currentScore,
        lastResetTimeStamp: gateway_wise_success_rate_input.lastResetTimeStamp,
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeciderApproachLogData {
    pub decided_gateway: Option<ETG::Gateway>,
    pub routing_approach: GatewayDeciderApproach,
    pub gateway_before_downtime_evaluation: Option<ETG::Gateway>,
    pub elimination_level_info: String,
    pub isPrimary_approach: Option<bool>,
    pub functional_gateways_before_scoring_flow: Vec<ETG::Gateway>,
    pub experimentTag: Option<String>,
    pub dateCreated: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct MessageFormat {
    pub model: String,
    pub log_type: String,
    pub payment_method: String,
    pub payment_method_type: String,
    pub payment_source: Option<String>,
    pub source_object: Option<String>,
    pub txn_detail_id: ETTD::TxnDetailId,
    pub stage: String,
    pub merchant_id: String,
    pub txn_uuid: String,
    pub order_id: String,
    pub card_type: String,
    pub auth_type: Option<String>,
    pub bank_code: Option<String>,
    pub x_request_id: Option<String>,
    pub log_data: AValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionScoreInfo {
    pub gateway: String,
    pub currentScore: f64,
    pub scoreScope: String,
    pub selectionMerchantTxnCountThreshold: i64,
    pub selectionMaxCountThreshold: Option<i64>,
    pub transactionCount: Option<i64>,
    pub eliminationLevel: Option<ETGRI::EliminationLevel>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeciderState {
    pub functionalGateways: Vec<ETG::Gateway>,
    pub metadata: Option<HMap<String, String>>,
    pub mgas: Option<Vec<ETM::merchant_gateway_account::MerchantGatewayAccount>>,
    pub cardBrand: Option<String>,
    pub gwScoreMap: GatewayScoreMap,
    pub debugFilterList: DebugFilterList,
    pub debugScoringList: DebugScoringList,
    pub selectionScoreMetricInfo: Vec<SelectionScoreInfo>,
    pub merchantSRScores: Vec<ETGRI::GatewayWiseSuccessRateBasedRoutingInput>,
    pub resetGatewayList: Vec<ETG::Gateway>,
    pub srMetricLogData: SRMetricLogData,
    pub gwDeciderApproach: GatewayDeciderApproach,
    pub srElminiationApproachInfo: Vec<String>,
    pub allMgas: Option<Vec<ETM::merchant_gateway_account::MerchantGatewayAccount>>,
    pub paymentFlowList: Vec<String>,
    pub internalMetaData: Option<InternalMetadata>,
    pub topGatewayBeforeSRDowntimeEvaluation: Option<ETG::Gateway>,
    pub isOptimizedBasedOnSRMetricEnabled: bool,
    pub isSrV3MetricEnabled: bool,
    pub isPrimaryGateway: Option<bool>,
    pub experimentTag: Option<String>,
    pub resetApproach: ResetApproach,
    pub routingDimension: Option<String>,
    pub routingDimensionLevel: Option<String>,
    pub isScheduledOutage: bool,
    pub isDynamicMGAEnabled: bool,
    pub outageDimension: Option<String>,
    pub eliminationDimension: Option<String>,
    pub srGatewayScores: Option<Vec<GatewayScore>>,
    pub eliminationScores: Option<Vec<GatewayScore>>,
    pub srv3BucketSize: Option<i32>,
    pub srV3HedgingPercent: Option<f64>,
    pub gatewayReferenceId: Option<String>,
}

pub fn initial_decider_state(date_created: String) -> DeciderState {
    DeciderState {
        functionalGateways: vec![],
        metadata: None,
        mgas: None,
        cardBrand: None,
        gwScoreMap: HMap::new(),
        debugFilterList: vec![],
        debugScoringList: vec![],
        selectionScoreMetricInfo: vec![],
        merchantSRScores: vec![],
        resetGatewayList: vec![],
        srMetricLogData: SRMetricLogData {
            gatewayAfterEvaluation: None,
            gatewayBeforeEvaluation: None,
            merchantGatewayScore: None,
            downtimeStatus: vec![],
            dateCreated: date_created,
            gatewayBeforeDowntimeEvaluation: None,
        },
        gwDeciderApproach: GatewayDeciderApproach::NONE,
        srElminiationApproachInfo: vec![],
        allMgas: None,
        paymentFlowList: vec![],
        internalMetaData: None,
        topGatewayBeforeSRDowntimeEvaluation: None,
        isOptimizedBasedOnSRMetricEnabled: false,
        isSrV3MetricEnabled: false,
        isPrimaryGateway: Some(true),
        experimentTag: None,
        resetApproach: ResetApproach::NO_RESET,
        routingDimension: None,
        routingDimensionLevel: None,
        isScheduledOutage: false,
        isDynamicMGAEnabled: false,
        outageDimension: None,
        eliminationDimension: None,
        srGatewayScores: None,
        eliminationScores: None,
        srv3BucketSize: None,
        srV3HedgingPercent: None,
        gatewayReferenceId: None,
    }
}



#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct GatewayScoringData {
    pub merchantId: String,
    pub paymentMethodType: String,
    pub paymentMethod: String,
    pub orderType: String,
    pub cardType: Option<String>,
    pub bankCode: Option<String>,
    pub authType: Option<String>,
    pub paymentSource: Option<String>,
    pub isPaymentSourceEnabledForSrRouting: bool,
    pub isAuthLevelEnabledForSrRouting: bool,
    pub isBankLevelEnabledForSrRouting: bool,
    pub isGriEnabledForElimination: bool,
    pub isGriEnabledForSrRouting: bool,
}

#[derive(Debug)]
pub struct MetricsStreamKeyShard(String, i32);

#[derive(Debug)]
pub struct MetricsStreamKey(String);

// # TODO - Implement RedisKey for MetricsStreamKeyShard
// impl RC::RedisKey for MetricsStreamKeyShard {
//     fn get_key(&self) -> String {
//         let MetricsStreamKeyShard(txnUuid, shardNumber) = self;
//         let slot = unsafe { std::mem::transmute::<_, u16>(L::key_to_slot(txnUuid.as_bytes())) };
//         let shardStream = slot % (*shardNumber as u16);
//         format!("routing_etl{{shard-{}}}", shardStream)
//     }
// }

// impl RC::RedisKey for MetricsStreamKey {
//     fn get_key(&self) -> String {
//         self.0.clone()
//     }
// }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ScoreKeyType {
    ELIMINATION_GLOBAL_KEY,
    ELIMINATION_MERCHANT_KEY,
    OUTAGE_GLOBAL_KEY,
    OUTAGE_MERCHANT_KEY,
    SR_V2_KEY,
    SR_V3_KEY,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum GatewayDeciderApproach {
    SR_SELECTION,
    SR_SELECTION_V2_ROUTING,
    SR_SELECTION_V3_ROUTING,
    PRIORITY_LOGIC,
    DEFAULT,
    NONE,
    MERCHANT_PREFERENCE,
    PL_ALL_DOWNTIME_ROUTING,
    PL_DOWNTIME_ROUTING,
    PL_GLOBAL_DOWNTIME_ROUTING,
    SR_V2_ALL_DOWNTIME_ROUTING,
    SR_V2_DOWNTIME_ROUTING,
    SR_V2_GLOBAL_DOWNTIME_ROUTING,
    SR_V2_HEDGING,
    SR_V2_ALL_DOWNTIME_HEDGING,
    SR_V2_DOWNTIME_HEDGING,
    SR_V2_GLOBAL_DOWNTIME_HEDGING,
    SR_V3_ALL_DOWNTIME_ROUTING,
    SR_V3_DOWNTIME_ROUTING,
    SR_V3_GLOBAL_DOWNTIME_ROUTING,
    SR_V3_HEDGING,
    SR_V3_ALL_DOWNTIME_HEDGING,
    SR_V3_DOWNTIME_HEDGING,
    SR_V3_GLOBAL_DOWNTIME_HEDGING,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum DownTime {
    ALL_DOWNTIME,
    GLOBAL_DOWNTIME,
    DOWNTIME,
    NO_DOWNTIME,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum ResetApproach {
    ELIMINATION_RESET,
    SRV2_RESET,
    SRV3_RESET,
    NO_RESET,
    SRV2_ELIMINATION_RESET,
    SRV3_ELIMINATION_RESET,
}

// pub type DeciderFlow<R> = for<'a> fn(&'a mut (dyn MonadFlow + 'a)) -> ReaderT<DeciderParams, StateT<DeciderState, &'a mut (dyn MonadFlow + 'a)>, R>;

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiDeciderRequest {
    pub orderReference: ApiOrderReference,
    pub orderMetadata: ApiOrderMetadataV2,
    pub txnDetail: ApiTxnDetail,
    pub txnCardInfo: ApiTxnCardInfo,
    pub card_token: Option<String>,
    pub txn_type: Option<String>,
    pub should_create_mandate: Option<bool>,
    pub enforce_gateway_list: Option<Vec<AValue>>,
    pub priority_logic_script: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiMerchantAccount {
    pub id: i64,
    pub merchantId: String,
    pub city: Option<String>,
    pub officeLine2: Option<String>,
    pub officeLine1: Option<String>,
    pub merchantZip: Option<String>,
    pub contactPersonPrimary: Option<String>,
    pub mobile: Option<String>,
    pub country: Option<String>,
    pub website: Option<String>,
    pub contactPersonEmail: Option<String>,
    pub resellerId: Option<String>,
    pub enableSendingCardIsin: Option<bool>,
    pub returnUrl: Option<String>,
    pub merchantName: Option<String>,
    pub realModeOnly: bool,
    pub autoRefundConflictTransactions: Option<bool>,
    pub autoRefundMultipleChargedTransactions: bool,
    pub autoRefundConflictThresholdInMins: Option<i32>,
    pub enableSaveCardBeforeAuth: Option<bool>,
    pub webHookAPIVersion: Option<String>,
    pub webHookURL: Option<String>,
    pub lockerId: Option<String>,
    pub gatewayDecidedByHealthEnabled: Option<bool>,
    pub gatewayPriority: Option<String>,
    pub gatewayPriorityLogic: String,
    pub useCodeForGatewayPriority: bool,
    pub enableTransactionFilter: Option<bool>,
    pub enabledInstantRefund: bool,
    pub cardEncodingKey: Option<String>,
    pub shouldAddSurcharge: Option<bool>,
    pub enableGatewayReferenceIdBasedRouting: Option<bool>,
    pub enableSuccessRateBasedGatewayElimination: bool,
    pub gatewaySuccessRateBasedDeciderInput: String,
    pub gatewaySuccessRateBasedOutageInput: String,
    pub secondaryMerchantAccountId: Option<i64>,
    pub internalHashKey: Option<String>,
    pub tokenLockerId: Option<String>,
    pub executeMandateAutoRetryEnabled: Option<bool>,
    pub autoRevokeMandate: Option<bool>,
    pub mandateRetryConfig: Option<String>,
    pub fingerprintOnTokenize: Option<bool>,
    pub mustUseGivenOrderIdForTxn: Option<bool>,
    pub externalMetadata: Option<String>,
    pub internalMetadata: Option<String>,
    pub userId: Option<i64>,
    pub installmentEnabled: Option<bool>,
    pub basiliskKeyId: Option<String>,
    pub encryptionKeyIds: Option<String>,
    pub tenantAccountId: Option<String>,
    pub priorityLogicConfig: Option<String>,
}

// #TODO - Implement A::FromJSON and A::ToJSON for ApiMerchantAccount
// impl A::FromJSON for ApiMerchantAccount {
//     fn parse_json(value: A::Value) -> Result<Self, A::Error> {
//         A::generic_parse_json(A::default_options().field_label_modifier(|x| if x == "merchantZip" { "zip" } else { x }), value)
//     }
// }

// impl A::ToJSON for ApiMerchantAccount {
//     fn to_json(&self) -> A::Value {
//         A::generic_to_json(A::default_options().field_label_modifier(|x| if x == "merchantZip" { "zip" } else { x }), self)
//     }
// }

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiTxnDetail {
    pub id: Option<String>,
    pub version: i64,
    pub errorMessage: Option<String>,
    pub orderId: String,
    pub status: String,
    pub txnId: String,
    pub txnType: String,
    pub dateCreated: Option<PrimitiveDateTime>,
    pub lastModified: Option<PrimitiveDateTime>,
    pub successResponseId: Option<String>,
    pub txnMode: Option<String>,
    pub addToLocker: Option<bool>,
    pub merchantId: Option<String>,
    pub bankErrorCode: Option<String>,
    pub bankErrorMessage: Option<String>,
    pub gateway: Option<String>,
    pub expressCheckout: Option<bool>,
    pub redirect: Option<bool>,
    pub gatewayPayload: Option<String>,
    pub isEmi: Option<bool>,
    pub emiBank: Option<String>,
    pub emiTenure: Option<i32>,
    pub username: Option<String>,
    pub txnUuid: Option<String>,
    pub merchantGatewayAccountId: Option<i64>,
    pub txnAmount: Option<f64>,
    pub txnObjectType: Option<String>,
    pub sourceObject: Option<String>,
    pub sourceObjectId: Option<String>,
    pub currency: Option<String>,
    pub netAmount: Option<f64>,
    pub surchargeAmount: Option<f64>,
    pub taxAmount: Option<f64>,
    pub internalMetadata: Option<String>,
    pub metadata: Option<String>,
    pub txnFlowType: Option<String>,
    pub txnFlowSubType: Option<String>,
    pub offerDeductionAmount: Option<f64>,
    pub txnLinkUuid: Option<String>,
    pub internalTrackingInfo: Option<String>,
    pub responseCode: Option<String>,
    pub responseMessage: Option<String>,
    pub compactPgResponse: Option<String>,
    pub partitionKey: Option<PrimitiveDateTime>,
    pub paymentMethodSubDetail: Option<String>,
    pub txnAmountBreakup: Option<String>,
}

// #TODO - Implement A::FromJSON and A::ToJSON for ApiTxnDetail

// impl A::FromJSON for ApiTxnDetail {
//     fn parse_json(value: A::Value) -> Result<Self, A::Error> {
//         A::generic_parse_json(A::default_options().field_label_modifier(|x| if x == "txnType" { "type" } else { x }), value)
//     }
// }

// impl A::ToJSON for ApiTxnDetail {
//     fn to_json(&self) -> A::Value {
//         A::generic_to_json(A::default_options().field_label_modifier(|x| if x == "txnType" { "type" } else { x }), self)
//     }
// }

// impl Capture::Requireable for ApiTxnDetail {
//     fn from_url() -> Result<Self, Capture::Error> {
//         Err(Capture::CustomError("fromURL instance for ApiTxnDetail not implemented".to_string()))
//     }

//     fn from_json(value: A::Value) -> Result<Self, Capture::Error> {
//         match A::from_json(value) {
//             Ok(a) => Ok(a),
//             Err(err) => Err(Capture::CustomError(err.to_string())),
//         }
//     }
// }

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiOrderMetadataV2 {
    pub id: Option<String>,
    pub browser: Option<String>,
    pub browserVersion: Option<String>,
    pub dateCreated: PrimitiveDateTime,
    pub device: Option<String>,
    pub lastUpdated: PrimitiveDateTime,
    pub metadata: Option<HMap<String, AValue>>,
    pub mobile: Option<bool>,
    pub operatingSystem: Option<String>,
    pub orderReferenceId: String,
    pub ipAddress: Option<String>,
    pub referer: Option<String>,
    pub userAgent: Option<String>,
    pub partitionKey: Option<PrimitiveDateTime>,
}

// #TODO - Implement A::FromJSON and A::ToJSON for ApiOrderMetadataV2
// impl Capture::Requireable for ApiOrderMetadataV2 {
//     fn from_url() -> Result<Self, Capture::Error> {
//         Err(Capture::CustomError("fromURL instance for ApiOrderMetadataV2 not implemented".to_string()))
//     }

//     fn from_json(value: A::Value) -> Result<Self, Capture::Error> {
//         match A::from_json(value) {
//             Ok(a) => Ok(a),
//             Err(err) => Err(Capture::CustomError(err.to_string())),
//         }
//     }
// }

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiTxnCardInfo {
    pub id: Option<String>,
    pub txnId: String,
    pub cardIsin: Option<String>,
    pub cardIssuerBankName: Option<String>,
    pub cardExpYear: Option<String>,
    pub cardExpMonth: Option<String>,
    pub cardSwitchProvider: Option<String>,
    pub cardType: Option<String>,
    pub cardLastFourDigits: Option<String>,
    pub nameOnCard: Option<String>,
    pub cardFingerprint: Option<String>,
    pub cardReferenceId: Option<String>,
    pub txnDetailId: Option<String>,
    pub dateCreated: Option<PrimitiveDateTime>,
    pub paymentMethodType: Option<String>,
    pub paymentMethod: Option<String>,
    pub cardGlobalFingerprint: Option<String>,
    pub paymentSource: Option<String>,
    pub authType: Option<String>,
    pub partitionKey: Option<PrimitiveDateTime>,
}

// #TODO - Implement A::FromJSON and A::ToJSON for ApiTxnCardInfo
// impl Capture::Requireable for ApiTxnCardInfo {
//     fn from_url() -> Result<Self, Capture::Error> {
//         Err(Capture::CustomError("fromURL instance for ApiTxnCardInfo not implemented".to_string()))
//     }

//     fn from_json(value: A::Value) -> Result<Self, Capture::Error> {
//         match A::from_json(value) {
//             Ok(a) => Ok(a),
//             Err(err) => Err(Capture::CustomError(err.to_string())),
//         }
//     }
// }

#[derive(Debug, Serialize, Deserialize)]
pub struct DomainDeciderRequest {
    pub orderReference: ETO::Order,
    pub orderMetadata: ETOMV2::OrderMetadataV2,
    pub txnDetail: ETTD::TxnDetail,
    pub txnOfferDetails: Option<Vec<ETTOD::TxnOfferDetail>>,
    pub txnCardInfo: ETCa::txn_card_info::TxnCardInfo,
    pub merchantAccount: ETM::merchant_account::MerchantAccount,
    pub cardToken: Option<String>,
    pub txnType: Option<String>,
    pub shouldCreateMandate: Option<bool>,
    pub enforceGatewayList: Option<Vec<ETG::Gateway>>,
    pub priorityLogicOutput: Option<GatewayPriorityLogicOutput>,
    pub priorityLogicScript: Option<String>,
    pub isEdccApplied: Option<bool>,
}

// impl Given<SecretContext> for DomainDeciderRequest {}

#[derive(Debug, Serialize, Deserialize)]
pub struct DomainDeciderRequestForApiCall {
    pub orderReference: ETO::Order,
    pub orderMetadata: ETOMV2::OrderMetadataV2,
    pub txnDetail: ETTD::TxnDetail,
    pub txnCardInfo: ETCa::txn_card_info::TxnCardInfo,
    pub card_token: Option<String>,
    pub txn_type: Option<String>,
    pub should_create_mandate: Option<bool>,
    pub enforce_gateway_list: Option<Vec<AValue>>,
    pub priority_logic_script: Option<String>,
}

// impl Given<SecretContext> for DomainDeciderRequestForApiCall {}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiOrderReference {
    pub id: Option<String>,
    pub version: Option<i32>,
    pub amount: Option<f64>,
    pub currency: Option<String>,
    pub dateCreated: PrimitiveDateTime,
    pub lastModified: PrimitiveDateTime,
    pub merchantId: Option<String>,
    pub orderId: Option<String>,
    pub status: String,
    pub customerEmail: Option<String>,
    pub customerId: Option<String>,
    pub browser: Option<String>,
    pub browserVersion: Option<String>,
    pub popupLoaded: Option<bool>,
    pub popupLoadedTime: Option<String>,
    pub description: Option<String>,
    pub udf1: Option<String>,
    pub udf10: Option<String>,
    pub udf2: Option<String>,
    pub udf3: Option<String>,
    pub udf4: Option<String>,
    pub udf5: Option<String>,
    pub udf6: Option<String>,
    pub udf7: Option<String>,
    pub udf8: Option<String>,
    pub udf9: Option<String>,
    pub returnUrl: Option<String>,
    pub amountRefunded: Option<f64>,
    pub refundedEntirely: Option<bool>,
    pub preferredGateway: Option<String>,
    pub customerPhone: Option<String>,
    pub productId: Option<String>,
    pub billingAddressId: Option<String>,
    pub shippingAddressId: Option<String>,
    pub orderUuid: Option<String>,
    pub lastSynced: Option<PrimitiveDateTime>,
    pub orderType: Option<String>,
    pub mandateFeature: Option<String>,
    pub autoRefund: Option<bool>,
    pub partitionKey: Option<PrimitiveDateTime>,
    pub parentOrderId: Option<String>,
    pub internalMetadata: Option<String>,
    pub metadata: Option<String>,
    pub amountInfo: Option<String>,
}

// #TODO - Implement A::FromJSON and A::ToJSON for ApiOrderReference
// impl Capture::Requireable for ApiOrderReference {
//     fn from_url() -> Result<Self, Capture::Error> {
//         Err(Capture::CustomError("fromURL instance for ApiOrderReference not implemented".to_string()))
//     }

//     fn from_json(value: A::Value) -> Result<Self, Capture::Error> {
//         match A::from_json(value) {
//             Ok(a) => Ok(a),
//             Err(err) => Err(Capture::CustomError(err.to_string())),
//         }
//     }
// }


#[derive(Debug, Serialize, Deserialize)]
pub struct SrV3InputConfig {
    pub defaultLatencyThreshold: Option<f64>,
    pub defaultBucketSize: Option<i32>,
    pub defaultHedgingPercent: Option<f64>,
    pub defaultLowerResetFactor: Option<f64>,
    pub defaultUpperResetFactor: Option<f64>,
    pub defaultGatewayExtraScore: Option<Vec<GatewayWiseExtraScore>>,
    pub subLevelInputConfig: Option<Vec<SrV3SubLevelInputConfig>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SrV3SubLevelInputConfig {
    pub paymentMethodType: Option<String>,
    pub paymentMethod: Option<String>,
    pub latencyThreshold: Option<f64>,
    pub bucketSize: Option<i32>,
    pub hedgingPercent: Option<f64>,
    pub lowerResetFactor: Option<f64>,
    pub upperResetFactor: Option<f64>,
    pub gatewayExtraScore: Option<Vec<GatewayWiseExtraScore>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GatewayWiseExtraScore {
    pub gatewayName: String,
    pub gatewaySigmaFactor: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub status: String,
    pub error_code: String,
    pub error_message: String,
    pub priority_logic_tag: Option<String>,
    pub routing_approach: Option<GatewayDeciderApproach>,
    pub filter_wise_gateways: Option<AValue>,
    pub error_info: String, // #TDOO - Change to ErrorInfo
    pub priority_logic_output: Option<GatewayPriorityLogicOutput>,
    pub is_dynamic_mga_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugFilterEntry {
    pub filterName: String,
    pub gateways: Vec<ETG::Gateway>,
}

pub type DebugFilterList = Vec<DebugFilterEntry>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayScore {
    pub gateway: ETG::Gateway,
    pub score: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DebugScoringEntry {
    pub scoringName: String,
    pub gatewayScores: Vec<GatewayScore>,
}

pub type DebugScoringList = Vec<DebugScoringEntry>;

pub fn toListOfGatewayScore(m: GatewayScoreMap) -> Vec<GatewayScore> {
    m.into_iter().map(|(k, v)| GatewayScore { gateway: k, score: v }).collect()
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct DecidedGateway {
    pub decided_gateway: ETG::Gateway,
    pub gateway_priority_map: Option<AValue>,
    pub filter_wise_gateways: Option<AValue>,
    pub priority_logic_tag: Option<String>,
    pub routing_approach: GatewayDeciderApproach,
    pub gateway_before_evaluation: Option<ETG::Gateway>,
    pub priority_logic_output: Option<GatewayPriorityLogicOutput>,
    pub reset_approach: ResetApproach,
    pub routing_dimension: Option<String>,
    pub routing_dimension_level: Option<String>,
    pub is_scheduled_outage: bool,
    pub is_dynamic_mga_enabled: bool,
    pub gateway_mga_id_map: Option<AValue>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeciderParams {
    pub dpMerchantAccount: ETM::merchant_account::MerchantAccount,
    pub dpOrder: ETO::Order,
    pub dpTxnDetail: ETTD::TxnDetail,
    pub dpTxnOfferDetails: Option<Vec<ETTOD::TxnOfferDetail>>,
    pub dpTxnCardInfo: ETCa::txn_card_info::TxnCardInfo,
    pub dpTxnOfferInfo: Option<ETTOI::TxnOfferInfo>,
    pub dpVaultProvider: Option<ETCa::vault_provider::VaultProvider>,
    pub dpTxnType: Option<String>,
    pub dpMerchantPrefs: ETM::merchant_iframe_preferences::MerchantIframePreferences,
    pub dpOrderMetadata: ETOMV2::OrderMetadataV2,
    pub dpEnforceGatewayList: Option<Vec<ETG::Gateway>>,
    pub dpPriorityLogicOutput: Option<GatewayPriorityLogicOutput>,
    pub dpPriorityLogicScript: Option<String>,
    pub dpEDCCApplied: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TxnsApiResponse {
    pub order_id: String,
    pub txn_id: String,
    pub txn_uuid: String,
    pub status: String,
    pub resp_code: Option<String>,
    pub resp_message: Option<String>,
    pub offer_details: Offers,
    pub payment: AuthResp,
    pub juspay: Option<OrderTokenResp>,
    pub merchant_return_url: Option<String>,
    pub native_godel_support: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OrderTokenResp {
    pub client_auth_token: Option<String>,
    pub client_auth_token_expiry: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AuthResp {
    pub authentication: Params,
    pub sdk_params: Option<AValue>,
    pub qr_code: Option<AValue>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Params {
    pub url: String,
    pub method: String,
    pub params: Option<AValue>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Offers {
    pub offers: Vec<Option<Offer>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Offer {
    pub offer_id: String,
    pub status: String,
    pub error_code: Option<String>,
    pub error_message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EmiType;

#[derive(Debug, Serialize, Deserialize)]
pub struct ValidationType;

#[derive(Debug, Serialize, Deserialize)]
pub struct Marketplace {
    pub amount: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Vendor {
    pub split: Vec<VendorSplit>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct VendorSplit {
    pub sub_mid: String,
    pub amount: f64,
    pub merchant_commission: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SplitSettlementDetails {
    pub mdr_borne_by: String,
    pub marketplace: Marketplace,
    pub vendor: Vendor,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiDeciderFullRequest {
    pub captures: ApiDeciderRequest,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct GatewayPriorityLogicOutput {
    pub isEnforcement: bool,
    pub gws: Vec<ETG::Gateway>,
    pub priorityLogicTag: Option<String>,
    pub gatewayReferenceIds: HMap<String, String>,
    pub primaryLogic: Option<PriorityLogicData>,
    pub fallbackLogic: Option<PriorityLogicData>,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct PriorityLogicData {
    pub name: Option<String>,
    pub status: Status,
    pub failureReason: PriorityLogicFailure,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct PriorityLogicFailure;

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Status;

#[derive(Debug, Serialize, Deserialize)]
pub struct Dimension;

#[derive(Debug, Serialize, Deserialize)]
pub struct ResetGatewayInput {
    pub gateway: ETG::Gateway,
    pub eliminationThreshold: Option<f64>,
    pub eliminationMaxCount: Option<i64>,
    pub gatewayEliminationThreshold: Option<f64>,
    pub gatewayReferenceId: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResetCallParams {
    pub txn_detail_id: String,
    pub txn_id: String,
    pub merchant_id: String,
    pub order_id: String,
    pub resetGatewayScoreReqArr: Vec<ResetGatewayInput>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OptimizationRedisBlockData {
    pub aggregate: Vec<GatewayScoreDetails>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GatewayScoreDetails {
    pub timestamp: f64,
    pub block_total_txn: i64,
    pub transactions_detail: Vec<GatewayDetails>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GatewayDetails {
    pub success_txn_count: i64,
    pub total_txn_count: i64,
    pub gateway_name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SuccessRateData {
    pub successTxnCount: i64,
    pub totalTxn: i64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LogCurrScore {
    pub gateway: String,
    pub current_score: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InternalMetadata {
    pub isCvvLessTxn: Option<bool>,
    pub storedCardVaultProvider: Option<String>,
    pub paymentOption: Option<String>,
    pub tokenProvider: Option<String>,
    pub paymentChannel: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaymentFlowInfoInInternalTrackingInfo {
    pub paymentFlowInfo: PaymentFlowInfo,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PaymentFlowInfo {
    pub paymentFlows: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DefaultSRBasedGatewayEliminationInput {
    pub gatewayWiseInputs: Option<Vec<ETGRI::GatewayWiseSuccessRateBasedRoutingInput>>,
    pub defaultEliminationThreshold: f64,
    pub defaultEliminationLevel: ETGRI::EliminationLevel,
    pub enabledPaymentMethodTypes: Option<Vec<ETP::payment_method::PaymentMethodType>>,
    pub globalGatewayWiseInputs: Option<Vec<ETGRI::GatewayWiseSuccessRateBasedRoutingInput>>,
    pub defaultGlobalEliminationThreshold: Option<f64>,
    pub defaultGlobalEliminationMaxCountThreshold: Option<i64>,
    pub defaultGlobalEliminationLevel: Option<ETGRI::EliminationLevel>,
    pub defaultGlobalSelectionMaxCountThreshold: Option<i64>,
    pub selectionTransactionCountThreshold: Option<i64>,
    pub defaultGlobalSoftTxnResetCount: Option<i64>,
    pub defaultGatewayLevelEliminationThreshold: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GlobalSREvaluationScoreLog {
    pub transactionCount: i64,
    pub currentScore: f64,
    pub merchantId: MerchantId,
    pub eliminationThreshold: f64,
    pub eliminationMaxCountThreshold: i64,
    pub gateway: ETG::Gateway,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SRStaleScoreLog {
    pub score_key: String,
    pub merchant_id: String,
    pub gateway_scores: Vec<(ETG::Gateway, f64)>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigurableBlock {
    pub max_blocks_allowed: i32,
    pub block_timeperiod: f64,
    pub max_transactions_per_block: i32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Weights {
    pub index: i32,
    pub value: f64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JuspayBankCodeInternalMetadata {
    pub juspayBankCode: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ExecutePriorityLogicRequest {
    pub order: ETO::Order,
    pub orderMetadata: ETOMV2::OrderMetadataV2,
    pub txnDetail: ETTD::TxnDetail,
    pub txnCardInfo: ETCa::txn_card_info::TxnCardInfo,
    pub merchantAccount: ETM::merchant_account::MerchantAccount,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GatewayInfo {
    pub name: String,
    pub offer_code: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GatewayRule {
    pub force_routing: Option<bool>,
    pub gateway_info: Vec<GatewayInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SuccessRate1AndNConfig {
    pub successRate: f64,
    pub nValue: f64,
    pub paymentMethodType: String,
    pub paymentMethod: Option<String>,
    pub txnObjectType: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FilterLevel;

#[derive(Debug, Serialize, Deserialize)]
pub struct ConfigSource;

#[derive(Debug, Serialize, Deserialize)]
pub struct BooleanOrString;

#[derive(Debug, Serialize, Deserialize)]
pub struct EMIAccountDetails {
    pub emiTenure: Option<i32>,
    pub isEmi: Option<BooleanOrString>,
}



pub struct DeciderFlow<'a> {
    pub reader: Reader<DeciderParams>,
    pub writer: &'a mut DeciderState,
}

impl DeciderFlow<'_> {
    pub fn get(&self) -> &DeciderParams {
        &self.reader.reader
    }

    pub fn state(&self) -> &TenantAppState {
        &self.reader.tenant_state
    }
}


struct Reader<T> {
    reader: T,
    tenant_state: TenantAppState,
}