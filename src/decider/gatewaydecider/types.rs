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
use serde::{Deserialize, Deserializer, Serialize};
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
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DetailedGatewayScoringType {
    EliminationPenalise,
    EliminationReward,
    Srv2Penalise,
    Srv2Reward,
    Srv3Penalise,
    Srv3Reward,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RoutingFlowType {
    EliminationFlow,
    Srv2Flow,
    Srv3Flow,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ScoreUpdateStatus {
    Penalised,
    Rewarded,
    NotInitiated,
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
    pub is_udf_consumed: Option<bool>
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
        gwDeciderApproach: GatewayDeciderApproach::None,
        srElminiationApproachInfo: vec![],
        allMgas: None,
        paymentFlowList: vec![],
        internalMetaData: None,
        topGatewayBeforeSRDowntimeEvaluation: None,
        isOptimizedBasedOnSRMetricEnabled: false,
        isSrV3MetricEnabled: false,
        isPrimaryGateway: Some(true),
        experiment_tag: None,
        reset_approach: ResetApproach::NoReset,
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
            is_legacy_decider_flow: true,
            udfs: None,
            udfs_consumed_for_routing: None,
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
    pub udfs: Option<UDFs>,
    pub udfs_consumed_for_routing: Option<bool>,
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
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ScoreKeyType {
    EliminationGlobalKey,
    EliminationMerchantKey,
    OutageGlobalKey,
    OutageMerchantKey,
    SrV2Key,
    SrV3Key,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GatewayDeciderApproach {
    SrSelection,
    SrSelectionV2Routing,
    SrSelectionV3Routing,
    PriorityLogic,
    Default,
    None,
    MerchantPreference,
    PlAllDowntimeRouting,
    PlDowntimeRouting,
    PlGlobalDowntimeRouting,
    SrV2AllDowntimeRouting,
    SrV2DowntimeRouting,
    SrV2GlobalDowntimeRouting,
    SrV2Hedging,
    SrV2AllDowntimeHedging,
    SrV2DowntimeHedging,
    SrV2GlobalDowntimeHedging,
    SrV3AllDowntimeRouting,
    SrV3DowntimeRouting,
    SrV3GlobalDowntimeRouting,
    SrV3Hedging,
    SrV3AllDowntimeHedging,
    SrV3DowntimeHedging,
    SrV3GlobalDowntimeHedging,
    NtwBasedRouting,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DownTime {
    AllDowntime,
    GlobalDowntime,
    Downtime,
    NoDowntime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ResetApproach {
    EliminationReset,
    Srv2Reset,
    Srv3Reset,
    NoReset,
    Srv2EliminationReset,
    Srv3EliminationReset,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RankingAlgorithm {
    SrBasedRouting,
    PlBasedRouting,
    NtwBasedRouting,
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
#[serde(rename_all = "camelCase")]
pub struct DomainDeciderRequestForApiCallV2 {
    pub payment_info: PaymentInfo,
    pub merchant_id: String,
    pub eligible_gateway_list: Option<Vec<String>>,
    pub ranking_algorithm: Option<RankingAlgorithm>,
    pub elimination_enabled: Option<bool>,
}

pub fn deserialize_optional_udfs_to_hashmap<'de, D>(
    deserializer: D,
) -> Result<Option<UDFs>, D::Error>
where
    D: Deserializer<'de>,
{
    // First try to deserialize as Option<Vec<Option<String>>>
    let opt_raw_vec: Option<Vec<Option<String>>> = Option::deserialize(deserializer)?;

    match opt_raw_vec {
        None => Ok(None),
        Some(raw_vec) => {
            // Convert the Vec<Option<String>> to a HashMap<i32, String>
            let hashmap: HashMap<i32, String> = raw_vec
                .into_iter()
                .enumerate()
                .filter_map(|(index, value)| value.map(|v| (index as i32, v)))
                .collect();

            Ok(Some(UDFs(hashmap)))
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentInfo {
    payment_id: String,
    pub amount: f64,
    currency: Currency,
    country: Option<CountryISO2>,
    customer_id: Option<ETCu::CustomerId>,
    #[serde(deserialize_with = "deserialize_optional_udfs_to_hashmap")]
    udfs: Option<UDFs>,
    preferred_gateway: Option<String>,
    payment_type: TxnObjectType,
    pub metadata: Option<String>,
    internal_metadata: Option<String>,
    is_emi: Option<bool>,
    emi_bank: Option<String>,
    emi_tenure: Option<i32>,
    payment_method_type: String,
    payment_method: String,
    payment_source: Option<String>,
    auth_type: Option<ETCa::txn_card_info::AuthType>,
    card_issuer_bank_name: Option<String>,
    pub card_isin: Option<String>,
    card_type: Option<ETCa::card_type::CardType>,
    card_switch_provider: Option<Secret<String>>,
}

// write a function to transfer DomainDeciderRequestForApiCallV2 to DomainDeciderRequest

impl DomainDeciderRequestForApiCallV2 {
    pub async fn to_domain_decider_request(&self) -> DomainDeciderRequest {
        DomainDeciderRequest {
            orderReference: ETO::Order {
                id: ETO::id::to_order_prim_id(1),
                amount: ETMo::Money::from_double(self.payment_info.amount),
                currency: self.payment_info.currency.clone(),
                dateCreated: OffsetDateTime::now_utc(),
                merchantId: ETM::id::to_merchant_id(self.merchant_id.clone()),
                orderId: ETO::id::to_order_id(self.payment_info.payment_id.clone()),
                status: ETO::OrderStatus::Created,
                description: None,
                customerId: self.payment_info.customer_id.clone(),
                udfs: self
                    .payment_info
                    .udfs
                    .clone()
                    .unwrap_or(UDFs(HashMap::new())),
                preferredGateway: self.payment_info.preferred_gateway.clone(),
                productId: None,
                orderType: ETO::OrderType::from_txn_object_type(
                    self.payment_info.payment_type.clone(),
                ),
                metadata: self.payment_info.metadata.clone(),
                internalMetadata: self.payment_info.internal_metadata.clone(),
            },
            shouldConsumeResult: None,
            orderMetadata: ETOMV2::OrderMetadataV2 {
                id: ETOMV2::to_order_metadata_v2_pid(1),
                date_created: OffsetDateTime::now_utc(),
                last_updated: OffsetDateTime::now_utc(),
                metadata: self.payment_info.metadata.clone(),
                order_reference_id: 1,
                ip_address: None,
                partition_key: None,
            },
            txnDetail: ETTD::TxnDetail {
                id: ETTD::to_txn_detail_id(1),
                orderId: ETO::id::to_order_id(self.payment_info.payment_id.clone()),
                status: ETTD::TxnStatus::Started,
                txnId: ETId::to_transaction_id(self.payment_info.payment_id.clone()),
                txnType: Some("NOT_DEFINED".to_string()),
                dateCreated: OffsetDateTime::now_utc(),
                addToLocker: Some(false),
                merchantId: ETM::id::to_merchant_id(self.merchant_id.clone()),
                gateway: None,
                expressCheckout: Some(false),
                isEmi: Some(self.payment_info.is_emi.clone().unwrap_or(false)),
                emiBank: self.payment_info.emi_bank.clone(),
                emiTenure: self.payment_info.emi_tenure.clone(),
                txnUuid: self.payment_info.payment_id.clone(),
                merchantGatewayAccountId: None,
                txnAmount: Some(ETMo::Money::from_double(self.payment_info.amount)),
                txnObjectType: Some(self.payment_info.payment_type.clone()),
                sourceObject: Some(self.payment_info.payment_method.clone()),
                sourceObjectId: None,
                currency: self.payment_info.currency.clone(),
                country: self.payment_info.country.clone(),
                netAmount: Some(ETMo::Money::from_double(self.payment_info.amount)),
                surchargeAmount: None,
                taxAmount: None,
                internalMetadata: self.payment_info.internal_metadata.clone().map(Secret::new),
                metadata: self.payment_info.metadata.clone().map(Secret::new),
                offerDeductionAmount: None,
                internalTrackingInfo: None,
                partitionKey: None,
                txnAmountBreakup: None,
            },
            txnOfferDetails: None,
            txnCardInfo: ETCa::txn_card_info::TxnCardInfo {
                id: ETCa::txn_card_info::to_txn_card_info_pid(1),
                card_isin: self.payment_info.card_isin.clone(),
                cardIssuerBankName: self.payment_info.card_issuer_bank_name.clone(),
                cardSwitchProvider: self.payment_info.card_switch_provider.clone(),
                card_type: self.payment_info.card_type.clone(),
                nameOnCard: None,
                dateCreated: OffsetDateTime::now_utc(),
                paymentMethodType: self.payment_info.payment_method_type.to_string(),
                paymentMethod: self.payment_info.payment_method.clone(),
                paymentSource: self.payment_info.payment_source.clone(),
                authType: self.payment_info.auth_type.clone(),
                partitionKey: None,
            },
            merchantAccount: ETM::merchant_account::load_merchant_by_merchant_id(
                self.merchant_id.clone(),
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
    pub latency: Option<u64>,
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
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EmiType {
    NoCostEmi,
    LowCostEmi,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ValidationType {
    CardMandate,
    Emandate,
    Tpv,
    TpvMandate,
    Reward,
    TpvEmandate,
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
            Self::EliminationPenalise => write!(f, "ELIMINATION_PENALISE"),
            Self::EliminationReward => write!(f, "ELIMINATION_REWARD"),
            Self::Srv2Penalise => write!(f, "SRV2_PENALISE"),
            Self::Srv2Reward => write!(f, "SRV2_REWARD"),
            Self::Srv3Penalise => write!(f, "SRV3_PENALISE"),
            Self::Srv3Reward => write!(f, "SRV3_REWARD"),
        }
    }
}

impl fmt::Display for RoutingFlowType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EliminationFlow => write!(f, "ELIMINATION_FLOW"),
            Self::Srv2Flow => write!(f, "SRV2_FLOW"),
            Self::Srv3Flow => write!(f, "SRV3_FLOW"),
        }
    }
}

impl fmt::Display for ScoreUpdateStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Penalised => write!(f, "PENALISED"),
            Self::Rewarded => write!(f, "REWARDED"),
            Self::NotInitiated => write!(f, "NOT_INITIATED"),
        }
    }
}

impl fmt::Display for ScoreKeyType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EliminationGlobalKey => write!(f, "ELIMINATION_GLOBAL_KEY"),
            Self::EliminationMerchantKey => write!(f, "ELIMINATION_MERCHANT_KEY"),
            Self::OutageGlobalKey => write!(f, "OUTAGE_GLOBAL_KEY"),
            Self::OutageMerchantKey => write!(f, "OUTAGE_MERCHANT_KEY"),
            Self::SrV2Key => write!(f, "SR_V2_KEY"),
            Self::SrV3Key => write!(f, "SR_V3_KEY"),
        }
    }
}

impl fmt::Display for GatewayDeciderApproach {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SrSelection => write!(f, "SR_SELECTION"),
            Self::SrSelectionV2Routing => write!(f, "SR_SELECTION_V2_ROUTING"),
            Self::SrSelectionV3Routing => write!(f, "SR_SELECTION_V3_ROUTING"),
            Self::PriorityLogic => write!(f, "PRIORITY_LOGIC"),
            Self::Default => write!(f, "DEFAULT"),
            Self::None => write!(f, "NONE"),
            Self::MerchantPreference => write!(f, "MERCHANT_PREFERENCE"),
            Self::PlAllDowntimeRouting => write!(f, "PL_ALL_DOWNTIME_ROUTING"),
            Self::PlDowntimeRouting => write!(f, "PL_DOWNTIME_ROUTING"),
            Self::PlGlobalDowntimeRouting => {
                write!(f, "PL_GLOBAL_DOWNTIME_ROUTING")
            }
            Self::SrV2AllDowntimeRouting => {
                write!(f, "SR_V2_ALL_DOWNTIME_ROUTING")
            }
            Self::SrV2DowntimeRouting => write!(f, "SR_V2_DOWNTIME_ROUTING"),
            Self::SrV2GlobalDowntimeRouting => {
                write!(f, "SR_V2_GLOBAL_DOWNTIME_ROUTING")
            }
            Self::SrV2Hedging => write!(f, "SR_V2_HEDGING"),
            Self::SrV2AllDowntimeHedging => {
                write!(f, "SR_V2_ALL_DOWNTIME_HEDGING")
            }
            Self::SrV2DowntimeHedging => write!(f, "SR_V2_DOWNTIME_HEDGING"),
            Self::SrV2GlobalDowntimeHedging => {
                write!(f, "SR_V2_GLOBAL_DOWNTIME_HEDGING")
            }
            Self::SrV3AllDowntimeRouting => {
                write!(f, "SR_V3_ALL_DOWNTIME_ROUTING")
            }
            Self::SrV3DowntimeRouting => write!(f, "SR_V3_DOWNTIME_ROUTING"),
            Self::SrV3GlobalDowntimeRouting => {
                write!(f, "SR_V3_GLOBAL_DOWNTIME_ROUTING")
            }
            Self::SrV3Hedging => write!(f, "SR_V3_HEDGING"),
            Self::SrV3AllDowntimeHedging => {
                write!(f, "SR_V3_ALL_DOWNTIME_HEDGING")
            }
            Self::SrV3DowntimeHedging => write!(f, "SR_V3_DOWNTIME_HEDGING"),
            Self::SrV3GlobalDowntimeHedging => {
                write!(f, "SR_V3_GLOBAL_DOWNTIME_HEDGING")
            }
            Self::NtwBasedRouting => {
                write!(f, "NTW_BASED_ROUTING")
            }
        }
    }
}

impl fmt::Display for DownTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AllDowntime => write!(f, "ALL_DOWNTIME"),
            Self::GlobalDowntime => write!(f, "GLOBAL_DOWNTIME"),
            Self::Downtime => write!(f, "DOWNTIME"),
            Self::NoDowntime => write!(f, "NO_DOWNTIME"),
        }
    }
}

impl fmt::Display for ResetApproach {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EliminationReset => write!(f, "ELIMINATION_RESET"),
            Self::Srv2Reset => write!(f, "SRV2_RESET"),
            Self::Srv3Reset => write!(f, "SRV3_RESET"),
            Self::NoReset => write!(f, "NO_RESET"),
            Self::Srv2EliminationReset => write!(f, "SRV2_ELIMINATION_RESET"),
            Self::Srv3EliminationReset => write!(f, "SRV3_ELIMINATION_RESET"),
        }
    }
}

impl fmt::Display for ValidationType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CardMandate => write!(f, "CARD_MANDATE"),
            Self::Emandate => write!(f, "EMANDATE"),
            Self::Tpv => write!(f, "TPV"),
            Self::TpvMandate => write!(f, "TPV_MANDATE"),
            Self::Reward => write!(f, "REWARD"),
            Self::TpvEmandate => write!(f, "TPV_EMANDATE"),
        }
    }
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Success => write!(f, "SUCCESS"),
            Self::Failure => write!(f, "FAILURE"),
        }
    }
}

impl fmt::Display for PriorityLogicFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoError => write!(f, "NO_ERROR"),
            Self::ConnectionFailed => write!(f, "CONNECTION_FAILED"),
            Self::CompilationError => write!(f, "COMPILATION_ERROR"),
            Self::MemoryExceeded => write!(f, "MEMORY_EXCEEDED"),
            Self::GatewayNameParseFailure => {
                write!(f, "GATEWAY_NAME_PARSE_FAILURE")
            }
            Self::ResponseContentTypeNotSupported => {
                write!(f, "RESPONSE_CONTENT_TYPE_NOT_SUPPORTED")
            }
            Self::ResponseDecodeFailure => write!(f, "RESPONSE_DECODE_FAILURE"),
            Self::ResponseParseError => write!(f, "RESPONSE_PARSE_ERROR"),
            Self::PlEvaluationFailed => write!(f, "PL_EVALUATION_FAILED"),
            Self::NullAfterEnforce => write!(f, "NULL_AFTER_ENFORCE"),
            Self::UnhandledException => write!(f, "UNHANDLED_EXCEPTION"),
            Self::CodeTooLarge => write!(f, "CODE_TOO_LARGE"),
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
            Self::NoCostEmi => write!(f, "NO_COST_EMI"),
            Self::LowCostEmi => write!(f, "LOW_COST_EMI"),
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
#[serde(rename_all = "camelCase")]
pub struct GatewayPriorityLogicOutput {
    pub is_enforcement: bool,
    pub gws: Vec<String>,
    pub priority_logic_tag: Option<String>,
    pub gateway_reference_ids: HMap<String, String>,
    pub primary_logic: Option<PriorityLogicData>,
    pub fallback_logic: Option<PriorityLogicData>,
}

impl GatewayPriorityLogicOutput {
    pub fn new(
        is_enforcement: bool,
        gws: Vec<String>,
        priority_logic_tag: Option<String>,
        gateway_reference_ids: HMap<String, String>,
        primary_logic: Option<PriorityLogicData>,
        fallback_logic: Option<PriorityLogicData>,
    ) -> Self {
        Self {
            is_enforcement,
            gws,
            priority_logic_tag,
            gateway_reference_ids,
            primary_logic,
            fallback_logic,
        }
    }
    pub fn set_is_enforcement(&mut self, is_enforcement: bool) -> &mut Self {
        self.is_enforcement = is_enforcement;
        self
    }
    pub fn set_priority_logic_tag(&mut self, priorityLogicTag: Option<String>) -> &mut Self {
        self.priority_logic_tag = priorityLogicTag;
        self
    }
    pub fn set_primary_logic(&mut self, primaryLogic: Option<PriorityLogicData>) -> &mut Self {
        self.primary_logic = primaryLogic;
        self
    }
    pub fn set_gws(&mut self, setGws: Vec<String>) -> &mut Self {
        self.gws = setGws;
        self
    }
    pub fn build(&self) -> Self {
        Self {
            is_enforcement: self.is_enforcement,
            gws: self.gws.clone(),
            priority_logic_tag: self.priority_logic_tag.clone(),
            gateway_reference_ids: self.gateway_reference_ids.clone(),
            primary_logic: self.primary_logic.clone(),
            fallback_logic: self.fallback_logic.clone(),
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
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PriorityLogicFailure {
    NoError,
    ConnectionFailed,
    CompilationError,
    MemoryExceeded,
    GatewayNameParseFailure,
    ResponseContentTypeNotSupported,
    ResponseDecodeFailure,
    ResponseParseError,
    PlEvaluationFailed,
    NullAfterEnforce,
    UnhandledException,
    CodeTooLarge,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Status {
    Success,
    Failure,
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
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FilterLevel {
    TxnObjectType,
    PaymentMethod,
    PaymentMethodType,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConfigSource {
    GlobalDefault,
    MerchantDefault,
    ServiceConfig,
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
