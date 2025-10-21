use crate::app::{get_tenant_app_state, TenantAppState};
use crate::decider::network_decider;
use crate::types::country::country_iso::CountryISO2;
use crate::types::currency::Currency;
use crate::types::money::internal as ETMo;
use crate::types::order::udfs::UDFs;
use crate::types::transaction::id as ETId;
use crate::types::txn_details::types::TxnObjectType;
use masking::Secret;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize};
use serde_json::Value as AValue;
use std::collections::HashMap as HMap;
use std::collections::HashMap;
use std::i64;
use std::option::Option;
use std::string::String;
use std::vec::Vec;
use time::{OffsetDateTime, PrimitiveDateTime};
// use eulerhs::prelude::*;
// use eulerhs::language::MonadFlow;
// use juspay::extra::secret::SecretContext;
// use data::reflection::Given;
// use data::time::{UTCTime, LocalTime};
// use unsafe_coerce::unsafeCoerce;
use crate::types::card as ETCa;
use crate::types::gateway as ETG;
use crate::types::merchant as ETM;
use crate::types::merchant::id::MerchantId;
use crate::types::order as ETO;
use crate::types::order_metadata_v2 as ETOMV2;
use crate::types::txn_details::types as ETTD;
use crate::types::txn_offer_detail as ETTOD;
use crate::types::txn_offer_info as ETTOI;
// use utils::framework::capture as Capture;
use crate::types::gateway_routing_input as ETGRI;
// use eulerhs::language as L;
// use juspay::extra::parsing as Parsing;
use crate::types::customer as ETCu;
use crate::types::payment as ETP;
use diesel::sql_types;
use std::fmt;

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

impl fmt::Display for DeciderFilterName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GetFunctionalGateways => write!(f, "GetFunctionalGateways"),
            Self::FilterFunctionalGatewaysForCurrency => {
                write!(f, "FilterFunctionalGatewaysForCurrency")
            }
            Self::FilterBySurcharge => write!(f, "FilterBySurcharge"),
            Self::FilterFunctionalGatewaysForBrand => {
                write!(f, "FilterFunctionalGatewaysForBrand")
            }
            Self::FilterFunctionalGatewaysForAuthType => {
                write!(f, "FilterFunctionalGatewaysForAuthType")
            }
            Self::FilterFunctionalGatewaysForValidationType => {
                write!(f, "FilterFunctionalGatewaysForValidationType")
            }
            Self::FilterFunctionalGatewaysForEmi => {
                write!(f, "FilterFunctionalGatewaysForEmi")
            }
            Self::FilterFunctionalGatewaysForTxnOfferDetails => {
                write!(f, "FilterFunctionalGatewaysForTxnOfferDetails")
            }
            Self::FilterFunctionalGatewaysForPaymentMethod => {
                write!(f, "FilterFunctionalGatewaysForPaymentMethod")
            }
            Self::FilterFunctionalGatewaysForTokenProvider => {
                write!(f, "FilterFunctionalGatewaysForTokenProvider")
            }
            Self::FilterFunctionalGatewaysForTxnOfferInfo => {
                write!(f, "FilterFunctionalGatewaysForTxnOfferInfo")
            }
            Self::FilterFunctionalGatewaysForWallet => {
                write!(f, "FilterFunctionalGatewaysForWallet")
            }
            Self::FilterFunctionalGatewaysForNbOnly => {
                write!(f, "FilterFunctionalGatewaysForNbOnly")
            }
            Self::FilterFunctionalGatewaysForConsumerFinance => {
                write!(f, "FilterFunctionalGatewaysForConsumerFinance")
            }
            Self::FilterFunctionalGatewaysForUpi => {
                write!(f, "FilterFunctionalGatewaysForUpi")
            }
            Self::FilterFunctionalGatewaysForTxnType => {
                write!(f, "FilterFunctionalGatewaysForTxnType")
            }
            Self::FilterFunctionalGatewaysForTxnDetailType => {
                write!(f, "FilterFunctionalGatewaysForTxnDetailType")
            }
            Self::FilterFunctionalGatewaysForReward => {
                write!(f, "FilterFunctionalGatewaysForReward")
            }
            Self::FilterFunctionalGatewaysForCash => {
                write!(f, "FilterFunctionalGatewaysForCash")
            }
            Self::FilterFunctionalGatewaysForSplitSettlement => {
                write!(f, "FilterFunctionalGatewaysForSplitSettlement")
            }
            Self::FilterFunctionalGateways => write!(f, "FilterFunctionalGateways"),
            Self::FinalFunctionalGateways => write!(f, "FinalFunctionalGateways"),
            Self::FilterByPriorityLogic => write!(f, "FilterByPriorityLogic"),
            Self::PreferredGateway => write!(f, "PreferredGateway"),
            Self::FilterEnforcement => write!(f, "FilterEnforcement"),
            Self::GatewayPriorityList => write!(f, "GatewayPriorityList"),
            Self::FilterFunctionalGatewaysForMerchantRequiredFlow => {
                write!(f, "FilterFunctionalGatewaysForMerchantRequiredFlow")
            }
            Self::FilterGatewaysForMGASelectionIntegrity => {
                write!(f, "FilterGatewaysForMGASelectionIntegrity")
            }
            Self::FilterGatewaysForEMITenureSpecficGatewayCreds => {
                write!(f, "FilterGatewaysForEMITenureSpecficGatewayCreds")
            }
            Self::FilterFunctionalGatewaysForReversePennyDrop => {
                write!(f, "FilterFunctionalGatewaysForReversePennyDrop")
            }
            Self::FilterFunctionalGatewaysForOTM => {
                write!(f, "FilterFunctionalGatewaysForOTM")
            }
        }
    }
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

pub type GatewayScoreMap = HMap<String, f64>;
pub type GatewayList = Vec<String>;
pub type GatewayReferenceIdMap = HMap<Gateway, Option<String>>;
pub type GatewayRedisKeyMap = HMap<Gateway, RedisKey>;
pub type Gateway = String;
pub type RedisKey = String;

#[derive(Debug, Serialize, Deserialize)]
pub struct GBESV2Metadata {
    pub supported_networks: Option<Vec<NETWORK>>,
}

/// Enum representing different network types.
#[derive(
    Debug,
    Clone,
    Serialize,
    Deserialize,
    Eq,
    Hash,
    PartialEq,
    diesel::AsExpression,
    diesel::FromSqlRow,
    strum::EnumString,
    strum::Display,
)]
#[diesel(sql_type = sql_types::Text)]
pub enum NETWORK {
    #[serde(alias = "visa", alias = "Visa")]
    VISA,
    #[serde(alias = "amex", alias = "Amex")]
    AMEX,
    #[serde(alias = "dinersclub", alias = "DinersClub")]
    DINERS,
    #[serde(alias = "rupay", alias = "RuPay")]
    RUPAY,
    #[serde(alias = "mastercard", alias = "Mastercard")]
    MASTERCARD,
    #[serde(alias = "star", alias = "Star")]
    STAR,
    #[serde(alias = "pulse", alias = "Pulse")]
    PULSE,
    #[serde(alias = "accel", alias = "Accel")]
    ACCEL,
    #[serde(alias = "nyce", alias = "Nyce")]
    NYCE,
}

#[cfg(feature = "mysql")]
crate::impl_to_sql_from_sql_text_mysql!(NETWORK);
#[cfg(feature = "postgres")]
crate::impl_to_sql_from_sql_text_pg!(NETWORK);

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
        S: serde::Serializer,
    {
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
        let data = deserializer.deserialize_struct(
            "GatewayScoringTypeLog",
            &["data"],
            GatewayScoringTypeLogVisitor,
        )?;
        Ok(Self { log_data: data })
    }
}

struct GatewayScoringTypeLogVisitor;

impl<'de> serde::de::Visitor<'de> for GatewayScoringTypeLogVisitor {
    type Value = AValue;

    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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
    pub gatewayAfterEvaluation: Option<String>,
    #[serde(rename = "gateway_before_evaluation")]
    pub gatewayBeforeEvaluation: Option<String>,
    #[serde(rename = "merchant_gateway_score")]
    pub merchantGatewayScore: Option<AValue>,
    #[serde(rename = "downtime_status")]
    pub downtimeStatus: Vec<String>,
    #[serde(rename = "date_created")]
    pub dateCreated: String,
    #[serde(rename = "gateway_before_downtime_evaluation")]
    pub gatewayBeforeDowntimeEvaluation: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeciderGatewayWiseSuccessRateBasedRoutingInput {
    pub gateway: String,
    #[serde(rename = "elimination_threshold")]
    pub eliminationThreshold: Option<f64>,
    #[serde(rename = "elimination_max_count_threshold")]
    pub eliminationMaxCountThreshold: Option<i64>,
    #[serde(rename = "selection_max_count_threshold")]
    pub selectionMaxCountThreshold: Option<i64>,
    #[serde(rename = "soft_txn_reset_count")]
    pub softTxnResetCount: Option<i64>,
    #[serde(rename = "gateway_level_elimination_threshold")]
    pub gatewayLevelEliminationThreshold: Option<f64>,
    #[serde(rename = "elimination_level")]
    pub eliminationLevel: Option<ETGRI::EliminationLevel>,
    #[serde(rename = "current_score")]
    pub currentScore: Option<f64>,
    #[serde(rename = "last_reset_time_stamp")]
    pub lastResetTimeStamp: Option<i64>,
}

pub fn transform_gateway_wise_success_rate_based_routing(
    gateway_wise_success_rate_input: &ETGRI::GatewayWiseSuccessRateBasedRoutingInput,
) -> DeciderGatewayWiseSuccessRateBasedRoutingInput {
    DeciderGatewayWiseSuccessRateBasedRoutingInput {
        gateway: gateway_wise_success_rate_input.gateway.clone(),
        eliminationThreshold: gateway_wise_success_rate_input.eliminationThreshold.clone(),
        eliminationMaxCountThreshold: gateway_wise_success_rate_input
            .eliminationMaxCountThreshold
            .clone(),
        selectionMaxCountThreshold: gateway_wise_success_rate_input
            .selectionMaxCountThreshold
            .clone(),
        softTxnResetCount: gateway_wise_success_rate_input.softTxnResetCount.clone(),
        gatewayLevelEliminationThreshold: gateway_wise_success_rate_input
            .gatewayLevelEliminationThreshold
            .clone(),
        eliminationLevel: gateway_wise_success_rate_input.eliminationLevel.clone(),
        currentScore: gateway_wise_success_rate_input.currentScore.clone(),
        lastResetTimeStamp: gateway_wise_success_rate_input.lastResetTimeStamp.clone(),
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeciderApproachLogData {
    pub decided_gateway: Option<String>,
    pub routing_approach: GatewayDeciderApproach,
    pub gateway_before_downtime_evaluation: Option<String>,
    pub elimination_level_info: String,
    pub isPrimary_approach: Option<bool>,
    pub functional_gateways_before_scoring_flow: Vec<String>,
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
    #[serde(rename = "data")]
    pub log_data: AValue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectionScoreInfo {
    pub gateway: String,
    #[serde(rename = "current_score")]
    pub currentScore: f64,
    #[serde(rename = "score_scope")]
    pub scoreScope: String,
    #[serde(rename = "selection_merchant_txn_count_threshold")]
    pub selectionMerchantTxnCountThreshold: i64,
    #[serde(rename = "selection_max_count_threshold")]
    pub selectionMaxCountThreshold: Option<i64>,
    #[serde(rename = "transaction_count")]
    pub transactionCount: Option<i64>,
    #[serde(rename = "elimination_level")]
    pub eliminationLevel: Option<ETGRI::EliminationLevel>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeciderState {
    pub functionalGateways: Vec<String>,
    pub metadata: Option<HMap<String, String>>,
    pub mgas: Option<Vec<ETM::merchant_gateway_account::MerchantGatewayAccount>>,
    pub cardBrand: Option<String>,
    pub gwScoreMap: GatewayScoreMap,
    pub debugFilterList: DebugFilterList,
    pub debugScoringList: DebugScoringList,
    pub selectionScoreMetricInfo: Vec<SelectionScoreInfo>,
    pub merchantSRScores: Vec<ETGRI::GatewayWiseSuccessRateBasedRoutingInput>,
    pub resetGatewayList: Vec<String>,
    pub srMetricLogData: SRMetricLogData,
    pub gwDeciderApproach: GatewayDeciderApproach,
    pub srElminiationApproachInfo: Vec<String>,
    pub allMgas: Option<Vec<ETM::merchant_gateway_account::MerchantGatewayAccount>>,
    pub paymentFlowList: Vec<String>,
    pub internalMetaData: Option<InternalMetadata>,
    pub topGatewayBeforeSRDowntimeEvaluation: Option<String>,
    pub isOptimizedBasedOnSRMetricEnabled: bool,
    pub isSrV3MetricEnabled: bool,
    pub isPrimaryGateway: Option<bool>,
    pub experiment_tag: Option<String>,
    pub reset_approach: ResetApproach,
    pub routing_dimension: Option<String>,
    pub routing_dimension_level: Option<String>,
    pub isScheduledOutage: bool,
    pub is_dynamic_mga_enabled: bool,
    pub outage_dimension: Option<String>,
    pub elimination_dimension: Option<String>,
    pub sr_gateway_scores: Option<Vec<GatewayScore>>,
    pub elimination_scores: Option<Vec<GatewayScore>>,
    pub srv3_bucket_size: Option<i32>,
    pub sr_v3_hedging_percent: Option<f64>,
    pub gateway_reference_id: Option<String>,
    pub gateway_scoring_data: GatewayScoringData,
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
        experiment_tag: None,
        reset_approach: ResetApproach::NO_RESET,
        routing_dimension: None,
        routing_dimension_level: None,
        isScheduledOutage: false,
        is_dynamic_mga_enabled: false,
        outage_dimension: None,
        elimination_dimension: None,
        sr_gateway_scores: None,
        elimination_scores: None,
        srv3_bucket_size: None,
        sr_v3_hedging_percent: None,
        gateway_reference_id: None,
        gateway_scoring_data: GatewayScoringData {
            merchantId: String::new(),
            paymentMethodType: String::new(),
            paymentMethod: String::new(),
            orderType: String::new(),
            cardType: None,
            bankCode: None,
            authType: None,
            paymentSource: None,
            isPaymentSourceEnabledForSrRouting: false,
            isAuthLevelEnabledForSrRouting: false,
            isBankLevelEnabledForSrRouting: false,
            isGriEnabledForElimination: false,
            isGriEnabledForSrRouting: false,
            routingApproach: None,
            dateCreated: OffsetDateTime::now_utc(),
            eliminationEnabled: false,
            cardIsIn: None,
            cardSwitchProvider: None,
            currency: None,
            country: None,
            is_legacy_decider_flow: false,
        },
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
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
    pub routingApproach: Option<String>,
    pub dateCreated: OffsetDateTime,
    pub eliminationEnabled: bool,
    pub cardIsIn: Option<String>,
    pub cardSwitchProvider: Option<Secret<String>>,
    pub currency: Option<Currency>,
    pub country: Option<CountryISO2>,
    pub is_legacy_decider_flow: bool,
}

#[derive(Debug)]
#[allow(dead_code)]
pub struct MetricsStreamKeyShard(String, i32);

#[derive(Debug)]
#[allow(dead_code)]
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
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
    NTW_BASED_ROUTING,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub enum DownTime {
    ALL_DOWNTIME,
    GLOBAL_DOWNTIME,
    DOWNTIME,
    NO_DOWNTIME,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ResetApproach {
    ELIMINATION_RESET,
    SRV2_RESET,
    SRV3_RESET,
    NO_RESET,
    SRV2_ELIMINATION_RESET,
    SRV3_ELIMINATION_RESET,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum RankingAlgorithm {
    SR_BASED_ROUTING,
    PL_BASED_ROUTING,
    NTW_BASED_ROUTING,
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
    pub dateCreated: Option<OffsetDateTime>,
    pub lastModified: Option<OffsetDateTime>,
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
    pub country: Option<String>,
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
    pub dateCreated: OffsetDateTime,
    pub device: Option<String>,
    pub lastUpdated: OffsetDateTime,
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
    pub dateCreated: Option<OffsetDateTime>,
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

#[derive(Debug, Serialize, Deserialize, Clone)]
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
    pub enforceGatewayList: Option<Vec<String>>,
    pub priorityLogicOutput: Option<GatewayPriorityLogicOutput>,
    pub priorityLogicScript: Option<String>,
    pub isEdccApplied: Option<bool>,
    pub shouldConsumeResult: Option<bool>,
}

// impl Given<SecretContext> for DomainDeciderRequest {}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainDeciderRequestForApiCallV2 {
    pub paymentInfo: PaymentInfo,
    pub merchantId: String,
    pub eligibleGatewayList: Option<Vec<String>>,
    pub rankingAlgorithm: Option<RankingAlgorithm>,
    pub eliminationEnabled: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaymentInfo {
    paymentId: String,
    pub amount: f64,
    currency: Currency,
    country: Option<CountryISO2>,
    customerId: Option<ETCu::CustomerId>,
    udfs: Option<UDFs>,
    preferredGateway: Option<String>,
    paymentType: TxnObjectType,
    pub metadata: Option<String>,
    internalMetadata: Option<String>,
    isEmi: Option<bool>,
    emiBank: Option<String>,
    emiTenure: Option<i32>,
    paymentMethodType: String,
    paymentMethod: String,
    paymentSource: Option<String>,
    authType: Option<ETCa::txn_card_info::AuthType>,
    cardIssuerBankName: Option<String>,
    pub cardIsin: Option<String>,
    cardType: Option<ETCa::card_type::CardType>,
    cardSwitchProvider: Option<Secret<String>>,
}

// write a function to transfer DomainDeciderRequestForApiCallV2 to DomainDeciderRequest

impl DomainDeciderRequestForApiCallV2 {
    pub async fn to_domain_decider_request(&self) -> DomainDeciderRequest {
        DomainDeciderRequest {
            orderReference: ETO::Order {
                id: ETO::id::to_order_prim_id(1),
                amount: ETMo::Money::from_double(self.paymentInfo.amount),
                currency: self.paymentInfo.currency.clone(),
                dateCreated: OffsetDateTime::now_utc(),
                merchantId: ETM::id::to_merchant_id(self.merchantId.clone()),
                orderId: ETO::id::to_order_id(self.paymentInfo.paymentId.clone()),
                status: ETO::OrderStatus::Created,
                description: None,
                customerId: self.paymentInfo.customerId.clone(),
                udfs: self
                    .paymentInfo
                    .udfs
                    .clone()
                    .unwrap_or(UDFs(HashMap::new())),
                preferredGateway: self.paymentInfo.preferredGateway.clone(),
                productId: None,
                orderType: ETO::OrderType::from_txn_object_type(
                    self.paymentInfo.paymentType.clone(),
                ),
                metadata: self.paymentInfo.metadata.clone(),
                internalMetadata: self.paymentInfo.internalMetadata.clone(),
            },
            shouldConsumeResult: None,
            orderMetadata: ETOMV2::OrderMetadataV2 {
                id: ETOMV2::to_order_metadata_v2_pid(1),
                date_created: OffsetDateTime::now_utc(),
                last_updated: OffsetDateTime::now_utc(),
                metadata: self.paymentInfo.metadata.clone(),
                order_reference_id: 1,
                ip_address: None,
                partition_key: None,
            },
            txnDetail: ETTD::TxnDetail {
                id: ETTD::to_txn_detail_id(1),
                orderId: ETO::id::to_order_id(self.paymentInfo.paymentId.clone()),
                status: ETTD::TxnStatus::Started,
                txnId: ETId::to_transaction_id(self.paymentInfo.paymentId.clone()),
                txnType: Some("NOT_DEFINED".to_string()),
                dateCreated: OffsetDateTime::now_utc(),
                addToLocker: Some(false),
                merchantId: ETM::id::to_merchant_id(self.merchantId.clone()),
                gateway: None,
                expressCheckout: Some(false),
                isEmi: Some(self.paymentInfo.isEmi.clone().unwrap_or(false)),
                emiBank: self.paymentInfo.emiBank.clone(),
                emiTenure: self.paymentInfo.emiTenure.clone(),
                txnUuid: self.paymentInfo.paymentId.clone(),
                merchantGatewayAccountId: None,
                txnAmount: Some(ETMo::Money::from_double(self.paymentInfo.amount)),
                txnObjectType: Some(self.paymentInfo.paymentType.clone()),
                sourceObject: Some(self.paymentInfo.paymentMethod.clone()),
                sourceObjectId: None,
                currency: self.paymentInfo.currency.clone(),
                country: self.paymentInfo.country.clone(),
                netAmount: Some(ETMo::Money::from_double(self.paymentInfo.amount)),
                surchargeAmount: None,
                taxAmount: None,
                internalMetadata: self.paymentInfo.internalMetadata.clone().map(Secret::new),
                metadata: self.paymentInfo.metadata.clone().map(Secret::new),
                offerDeductionAmount: None,
                internalTrackingInfo: None,
                partitionKey: None,
                txnAmountBreakup: None,
            },
            txnOfferDetails: None,
            txnCardInfo: ETCa::txn_card_info::TxnCardInfo {
                id: ETCa::txn_card_info::to_txn_card_info_pid(1),
                card_isin: self.paymentInfo.cardIsin.clone(),
                cardIssuerBankName: self.paymentInfo.cardIssuerBankName.clone(),
                cardSwitchProvider: self.paymentInfo.cardSwitchProvider.clone(),
                card_type: self.paymentInfo.cardType.clone(),
                nameOnCard: None,
                dateCreated: OffsetDateTime::now_utc(),
                paymentMethodType: self.paymentInfo.paymentMethodType.to_string(),
                paymentMethod: self.paymentInfo.paymentMethod.clone(),
                paymentSource: self.paymentInfo.paymentSource.clone(),
                authType: self.paymentInfo.authType.clone(),
                partitionKey: None,
            },
            merchantAccount: ETM::merchant_account::load_merchant_by_merchant_id(
                self.merchantId.clone(),
            )
            .await
            .expect("Merchant account not found"),
            cardToken: None,
            txnType: None,
            shouldCreateMandate: None,
            enforceGatewayList: None,
            priorityLogicOutput: None,
            priorityLogicScript: None,
            isEdccApplied: Some(false),
        }
    }
}

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
    pub dateCreated: OffsetDateTime,
    pub lastModified: OffsetDateTime,
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
    pub lastSynced: Option<OffsetDateTime>,
    pub orderType: Option<String>,
    pub mandateFeature: Option<String>,
    pub autoRefund: Option<bool>,
    pub partitionKey: Option<OffsetDateTime>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SrV3InputConfig {
    pub defaultLatencyThreshold: Option<f64>,
    pub defaultBucketSize: Option<i32>,
    pub defaultHedgingPercent: Option<f64>,
    pub defaultLowerResetFactor: Option<f64>,
    pub defaultUpperResetFactor: Option<f64>,
    pub defaultGatewayExtraScore: Option<Vec<GatewayWiseExtraScore>>,
    pub subLevelInputConfig: Option<Vec<SrV3SubLevelInputConfig>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SrV3SubLevelInputConfig {
    pub paymentMethodType: Option<String>,
    pub paymentMethod: Option<String>,
    pub cardNetwork: Option<String>,
    pub cardIsIn: Option<String>,
    pub currency: Option<String>,
    pub country: Option<String>,
    pub authType: Option<String>,
    pub latencyThreshold: Option<f64>,
    pub bucketSize: Option<i32>,
    pub hedgingPercent: Option<f64>,
    pub lowerResetFactor: Option<f64>,
    pub upperResetFactor: Option<f64>,
    pub gatewayExtraScore: Option<Vec<GatewayWiseExtraScore>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GatewayWiseExtraScore {
    pub gatewayName: String,
    pub gatewaySigmaFactor: f64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TransactionLatencyThreshold {
    /// To have a hard threshold for latency, which is used to filter out gateways that exceed this threshold.
    pub gatewayLatency: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UnifiedError {
    pub code: String,
    pub user_message: String,
    pub developer_message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub status: String,
    pub error_code: String,
    pub error_message: String,
    pub priority_logic_tag: Option<String>,
    pub routing_approach: Option<GatewayDeciderApproach>,
    pub filter_wise_gateways: Option<AValue>,
    pub error_info: UnifiedError,
    pub priority_logic_output: Option<GatewayPriorityLogicOutput>,
    pub is_dynamic_mga_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugFilterEntry {
    pub filterName: String,
    pub gateways: Vec<String>,
}

pub type DebugFilterList = Vec<DebugFilterEntry>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GatewayScore {
    pub gateway: String,
    pub score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugScoringEntry {
    pub scoringName: String,
    pub gatewayScores: Vec<GatewayScore>,
}

pub type DebugScoringList = Vec<DebugScoringEntry>;

pub fn toListOfGatewayScore(m: GatewayScoreMap) -> Vec<GatewayScore> {
    m.into_iter()
        .map(|(k, v)| GatewayScore {
            gateway: k,
            score: v,
        })
        .collect()
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct DecidedGateway {
    pub decided_gateway: String,
    pub gateway_priority_map: Option<AValue>,
    pub filter_wise_gateways: Option<AValue>,
    pub priority_logic_tag: Option<String>,
    pub routing_approach: GatewayDeciderApproach,
    pub gateway_before_evaluation: Option<String>,
    pub priority_logic_output: Option<GatewayPriorityLogicOutput>,
    pub debit_routing_output: Option<network_decider::types::DebitRoutingOutput>,
    pub reset_approach: ResetApproach,
    pub routing_dimension: Option<String>,
    pub routing_dimension_level: Option<String>,
    pub is_scheduled_outage: bool,
    pub is_dynamic_mga_enabled: bool,
    pub gateway_mga_id_map: Option<AValue>,
    pub is_rust_based_decider: bool,
}

#[derive(Debug, Serialize, Clone, Deserialize)]
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
    pub dpEnforceGatewayList: Option<Vec<String>>,
    pub dpPriorityLogicOutput: Option<GatewayPriorityLogicOutput>,
    pub dpPriorityLogicScript: Option<String>,
    pub dpEDCCApplied: Option<bool>,
    pub dpShouldConsumeResult: Option<bool>,
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
pub enum EmiType {
    NO_COST_EMI,
    LOW_COST_EMI,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ValidationType {
    CARD_MANDATE,
    EMANDATE,
    TPV,
    TPV_MANDATE,
    REWARD,
    TPV_EMANDATE,
}

impl fmt::Display for DeciderScoringName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UpdateScoreForIssuer => write!(f, "UpdateScoreForIssuer"),
            Self::UpdateScoreForIsin => write!(f, "UpdateScoreForIsin"),
            Self::UpdateScoreForCardBrand => write!(f, "UpdateScoreForCardBrand"),
            Self::UpdateScoreWithHealth => write!(f, "UpdateScoreWithHealth"),
            Self::UpdateScoreIfLastTxnFailure => {
                write!(f, "UpdateScoreIfLastTxnFailure")
            }
            Self::UpdateScoreForOutage => write!(f, "UpdateScoreForOutage"),
            Self::ScoringByGatewayScoreBasedOnGlobalSuccessRate => {
                write!(f, "ScoringByGatewayScoreBasedOnGlobalSuccessRate")
            }
            Self::UpdateGatewayScoreBasedOnSuccessRate => {
                write!(f, "UpdateGatewayScoreBasedOnSuccessRate")
            }
            Self::FinalScoring => write!(f, "FinalScoring"),
            Self::GetScoreWithPriority => write!(f, "GetScoreWithPriority"),
            Self::GetCachedScoresBasedOnSuccessRate => {
                write!(f, "GetCachedScoresBasedOnSuccessRate")
            }
            Self::GetCachedScoresBasedOnSrV3 => {
                write!(f, "GetCachedScoresBasedOnSrV3")
            }
        }
    }
}

impl fmt::Display for DetailedGatewayScoringType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ELIMINATION_PENALISE => write!(f, "ELIMINATION_PENALISE"),
            Self::ELIMINATION_REWARD => write!(f, "ELIMINATION_REWARD"),
            Self::SRV2_PENALISE => write!(f, "SRV2_PENALISE"),
            Self::SRV2_REWARD => write!(f, "SRV2_REWARD"),
            Self::SRV3_PENALISE => write!(f, "SRV3_PENALISE"),
            Self::SRV3_REWARD => write!(f, "SRV3_REWARD"),
        }
    }
}

impl fmt::Display for RoutingFlowType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ELIMINATION_FLOW => write!(f, "ELIMINATION_FLOW"),
            Self::SRV2_FLOW => write!(f, "SRV2_FLOW"),
            Self::SRV3_FLOW => write!(f, "SRV3_FLOW"),
        }
    }
}

impl fmt::Display for ScoreUpdateStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PENALISED => write!(f, "PENALISED"),
            Self::REWARDED => write!(f, "REWARDED"),
            Self::NOT_INITIATED => write!(f, "NOT_INITIATED"),
        }
    }
}

impl fmt::Display for ScoreKeyType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ELIMINATION_GLOBAL_KEY => write!(f, "ELIMINATION_GLOBAL_KEY"),
            Self::ELIMINATION_MERCHANT_KEY => write!(f, "ELIMINATION_MERCHANT_KEY"),
            Self::OUTAGE_GLOBAL_KEY => write!(f, "OUTAGE_GLOBAL_KEY"),
            Self::OUTAGE_MERCHANT_KEY => write!(f, "OUTAGE_MERCHANT_KEY"),
            Self::SR_V2_KEY => write!(f, "SR_V2_KEY"),
            Self::SR_V3_KEY => write!(f, "SR_V3_KEY"),
        }
    }
}

impl fmt::Display for GatewayDeciderApproach {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SR_SELECTION => write!(f, "SR_SELECTION"),
            Self::SR_SELECTION_V2_ROUTING => write!(f, "SR_SELECTION_V2_ROUTING"),
            Self::SR_SELECTION_V3_ROUTING => write!(f, "SR_SELECTION_V3_ROUTING"),
            Self::PRIORITY_LOGIC => write!(f, "PRIORITY_LOGIC"),
            Self::DEFAULT => write!(f, "DEFAULT"),
            Self::NONE => write!(f, "NONE"),
            Self::MERCHANT_PREFERENCE => write!(f, "MERCHANT_PREFERENCE"),
            Self::PL_ALL_DOWNTIME_ROUTING => write!(f, "PL_ALL_DOWNTIME_ROUTING"),
            Self::PL_DOWNTIME_ROUTING => write!(f, "PL_DOWNTIME_ROUTING"),
            Self::PL_GLOBAL_DOWNTIME_ROUTING => {
                write!(f, "PL_GLOBAL_DOWNTIME_ROUTING")
            }
            Self::SR_V2_ALL_DOWNTIME_ROUTING => {
                write!(f, "SR_V2_ALL_DOWNTIME_ROUTING")
            }
            Self::SR_V2_DOWNTIME_ROUTING => write!(f, "SR_V2_DOWNTIME_ROUTING"),
            Self::SR_V2_GLOBAL_DOWNTIME_ROUTING => {
                write!(f, "SR_V2_GLOBAL_DOWNTIME_ROUTING")
            }
            Self::SR_V2_HEDGING => write!(f, "SR_V2_HEDGING"),
            Self::SR_V2_ALL_DOWNTIME_HEDGING => {
                write!(f, "SR_V2_ALL_DOWNTIME_HEDGING")
            }
            Self::SR_V2_DOWNTIME_HEDGING => write!(f, "SR_V2_DOWNTIME_HEDGING"),
            Self::SR_V2_GLOBAL_DOWNTIME_HEDGING => {
                write!(f, "SR_V2_GLOBAL_DOWNTIME_HEDGING")
            }
            Self::SR_V3_ALL_DOWNTIME_ROUTING => {
                write!(f, "SR_V3_ALL_DOWNTIME_ROUTING")
            }
            Self::SR_V3_DOWNTIME_ROUTING => write!(f, "SR_V3_DOWNTIME_ROUTING"),
            Self::SR_V3_GLOBAL_DOWNTIME_ROUTING => {
                write!(f, "SR_V3_GLOBAL_DOWNTIME_ROUTING")
            }
            Self::SR_V3_HEDGING => write!(f, "SR_V3_HEDGING"),
            Self::SR_V3_ALL_DOWNTIME_HEDGING => {
                write!(f, "SR_V3_ALL_DOWNTIME_HEDGING")
            }
            Self::SR_V3_DOWNTIME_HEDGING => write!(f, "SR_V3_DOWNTIME_HEDGING"),
            Self::SR_V3_GLOBAL_DOWNTIME_HEDGING => {
                write!(f, "SR_V3_GLOBAL_DOWNTIME_HEDGING")
            }
            Self::NTW_BASED_ROUTING => {
                write!(f, "NTW_BASED_ROUTING")
            }
        }
    }
}

impl fmt::Display for DownTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ALL_DOWNTIME => write!(f, "ALL_DOWNTIME"),
            Self::GLOBAL_DOWNTIME => write!(f, "GLOBAL_DOWNTIME"),
            Self::DOWNTIME => write!(f, "DOWNTIME"),
            Self::NO_DOWNTIME => write!(f, "NO_DOWNTIME"),
        }
    }
}

impl fmt::Display for ResetApproach {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ELIMINATION_RESET => write!(f, "ELIMINATION_RESET"),
            Self::SRV2_RESET => write!(f, "SRV2_RESET"),
            Self::SRV3_RESET => write!(f, "SRV3_RESET"),
            Self::NO_RESET => write!(f, "NO_RESET"),
            Self::SRV2_ELIMINATION_RESET => write!(f, "SRV2_ELIMINATION_RESET"),
            Self::SRV3_ELIMINATION_RESET => write!(f, "SRV3_ELIMINATION_RESET"),
        }
    }
}

impl fmt::Display for ValidationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CARD_MANDATE => write!(f, "CARD_MANDATE"),
            Self::EMANDATE => write!(f, "EMANDATE"),
            Self::TPV => write!(f, "TPV"),
            Self::TPV_MANDATE => write!(f, "TPV_MANDATE"),
            Self::REWARD => write!(f, "REWARD"),
            Self::TPV_EMANDATE => write!(f, "TPV_EMANDATE"),
        }
    }
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SUCCESS => write!(f, "SUCCESS"),
            Self::FAILURE => write!(f, "FAILURE"),
        }
    }
}

impl fmt::Display for PriorityLogicFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NO_ERROR => write!(f, "NO_ERROR"),
            Self::CONNECTION_FAILED => write!(f, "CONNECTION_FAILED"),
            Self::COMPILATION_ERROR => write!(f, "COMPILATION_ERROR"),
            Self::MEMORY_EXCEEDED => write!(f, "MEMORY_EXCEEDED"),
            Self::GATEWAY_NAME_PARSE_FAILURE => {
                write!(f, "GATEWAY_NAME_PARSE_FAILURE")
            }
            Self::RESPONSE_CONTENT_TYPE_NOT_SUPPORTED => {
                write!(f, "RESPONSE_CONTENT_TYPE_NOT_SUPPORTED")
            }
            Self::RESPONSE_DECODE_FAILURE => write!(f, "RESPONSE_DECODE_FAILURE"),
            Self::RESPONSE_PARSE_ERROR => write!(f, "RESPONSE_PARSE_ERROR"),
            Self::PL_EVALUATION_FAILED => write!(f, "PL_EVALUATION_FAILED"),
            Self::NULL_AFTER_ENFORCE => write!(f, "NULL_AFTER_ENFORCE"),
            Self::UNHANDLED_EXCEPTION => write!(f, "UNHANDLED_EXCEPTION"),
            Self::CODE_TOO_LARGE => write!(f, "CODE_TOO_LARGE"),
        }
    }
}

impl fmt::Display for Dimension {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FIRST => write!(f, "FIRST"),
            Self::SECOND => write!(f, "SECOND"),
            Self::THIRD => write!(f, "THIRD"),
            Self::FOURTH => write!(f, "FOURTH"),
        }
    }
}

impl fmt::Display for EmiType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NO_COST_EMI => write!(f, "NO_COST_EMI"),
            Self::LOW_COST_EMI => write!(f, "LOW_COST_EMI"),
        }
    }
}

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

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct GatewayPriorityLogicOutput {
    pub isEnforcement: bool,
    pub gws: Vec<String>,
    pub priorityLogicTag: Option<String>,
    pub gatewayReferenceIds: HMap<String, String>,
    pub primaryLogic: Option<PriorityLogicData>,
    pub fallbackLogic: Option<PriorityLogicData>,
}

impl GatewayPriorityLogicOutput {
    pub fn new(
        isEnforcement: bool,
        gws: Vec<String>,
        priorityLogicTag: Option<String>,
        gatewayReferenceIds: HMap<String, String>,
        primaryLogic: Option<PriorityLogicData>,
        fallbackLogic: Option<PriorityLogicData>,
    ) -> Self {
        Self {
            isEnforcement,
            gws,
            priorityLogicTag,
            gatewayReferenceIds,
            primaryLogic,
            fallbackLogic,
        }
    }
    pub fn setIsEnforcement(&mut self, isEnforcement: bool) -> &mut Self {
        self.isEnforcement = isEnforcement;
        self
    }
    pub fn setPriorityLogicTag(&mut self, priorityLogicTag: Option<String>) -> &mut Self {
        self.priorityLogicTag = priorityLogicTag;
        self
    }
    pub fn setPrimaryLogic(&mut self, primaryLogic: Option<PriorityLogicData>) -> &mut Self {
        self.primaryLogic = primaryLogic;
        self
    }
    pub fn setGws(&mut self, setGws: Vec<String>) -> &mut Self {
        self.gws = setGws;
        self
    }
    pub fn build(&self) -> Self {
        Self {
            isEnforcement: self.isEnforcement,
            gws: self.gws.clone(),
            priorityLogicTag: self.priorityLogicTag.clone(),
            gatewayReferenceIds: self.gatewayReferenceIds.clone(),
            primaryLogic: self.primaryLogic.clone(),
            fallbackLogic: self.fallbackLogic.clone(),
        }
    }
}

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub struct PriorityLogicData {
    pub name: Option<String>,
    pub status: Status,
    pub failure_reason: PriorityLogicFailure,
}

#[derive(Debug, PartialEq, Clone, Eq, Serialize, Deserialize)]
// #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PriorityLogicFailure {
    NO_ERROR,
    CONNECTION_FAILED,
    COMPILATION_ERROR,
    MEMORY_EXCEEDED,
    GATEWAY_NAME_PARSE_FAILURE,
    RESPONSE_CONTENT_TYPE_NOT_SUPPORTED,
    RESPONSE_DECODE_FAILURE,
    RESPONSE_PARSE_ERROR,
    PL_EVALUATION_FAILED,
    NULL_AFTER_ENFORCE,
    UNHANDLED_EXCEPTION,
    CODE_TOO_LARGE,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
// #[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Status {
    SUCCESS,
    FAILURE,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum Dimension {
    FIRST,
    SECOND,
    THIRD,
    FOURTH,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ResetGatewayInput {
    pub gateway: String,
    pub eliminationThreshold: Option<f64>,
    pub eliminationMaxCount: Option<i64>,
    pub gatewayEliminationThreshold: Option<f64>,
    pub gatewayReferenceId: Option<String>,
    pub key: Option<String>,
    pub hardTtl: u128,
    pub softTtl: f64,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub enabledPaymentMethodTypes: Option<Vec<String>>,
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
    #[serde(rename = "transaction_count")]
    pub transactionCount: i64,
    #[serde(rename = "current_score")]
    pub currentScore: f64,
    #[serde(rename = "merchant_id")]
    pub merchantId: MerchantId,
    #[serde(rename = "elimination_threshold")]
    pub eliminationThreshold: f64,
    #[serde(rename = "elimination_max_count_threshold")]
    pub eliminationMaxCountThreshold: i64,
    pub gateway: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SRStaleScoreLog {
    pub score_key: String,
    pub merchant_id: String,
    pub gateway_scores: Vec<(String, f64)>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuccessRate1AndNConfig {
    pub successRate: f64,
    pub nValue: f64,
    pub paymentMethodType: String,
    pub paymentMethod: Option<String>,
    pub txnObjectType: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FilterLevel {
    TXN_OBJECT_TYPE,
    PAYMENT_METHOD,
    PAYMENT_METHOD_TYPE,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum ConfigSource {
    GLOBAL_DEFAULT,
    MERCHANT_DEFAULT,
    SERVICE_CONFIG,
    REDIS,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BooleanOrString;

#[derive(Debug, Serialize, Deserialize)]
pub struct EMIAccountDetails {
    pub emiTenure: Option<i32>,
    pub isEmi: Option<AValue>,
}

pub struct DeciderFlow<'a> {
    pub reader: Reader<DeciderParams>,
    pub logger: &'a mut HashMap<String, String>,
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

pub async fn initial_decider_flow<'a>(
    decider_params: DeciderParams,
    logger: &'a mut HashMap<String, String>,
    writer: &'a mut DeciderState,
) -> DeciderFlow<'a> {
    let app_state = get_tenant_app_state().await;
    let reader = Reader {
        reader: decider_params,
        tenant_state: (*app_state).clone(),
    };
    DeciderFlow {
        reader,
        logger,
        writer,
    }
}

struct Reader<T> {
    reader: T,
    tenant_state: TenantAppState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SrRoutingDimensions {
    pub card_network: Option<String>,
    pub card_isin: Option<String>,
    pub currency: Option<String>,
    pub country: Option<String>,
    pub auth_type: Option<String>,
}
