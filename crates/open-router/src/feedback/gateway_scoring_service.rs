// Automatically converted from Haskell to Rust
// Generated on 2025-03-23 10:24:42

use crate::redis::cache::findByNameFromRedis;
use crate::redis::feature::isFeatureEnabled;
// Converted imports
// use gateway_decider::constants as c::{enable_elimination_v2, gateway_scoring_data, ENABLE_EXPLORE_AND_EXPLOIT_ON_SRV3, SR_V3_INPUT_CONFIG, GATEWAY_SCORE_FIRST_DIMENSION_SOFT_TTL};
// use feedback::constants as c;
// use data::text::encoding as de::encode_utf8;
// use db::storage::types::merchant_account as merchant_account;
// use types::gateway_routing_input as etgri;
// use gateway_decider::utils::decode_and_log_error;
// use gateway_decider::gw_scoring::get_sr1_and_sr2_and_n;
// use feedback::utils as euler_transforms;
// use feedback::types::*;
// use feedback::types::txn_card_info;
// use eulerhs::prelude::*;
// use eulerhs::language::get_current_date_in_millis;
// use data::text as t;
// use feedback::utils::*;
// use feedback::gateway_selection_scoring_v3::flow;
// use feedback::gateway_elimination_scoring::flow;
// use eulerhs::language as el;
// use eulerhs::types as et;
// use eulerhs::tenant_redis_layer as rc;
use crate::redis::{feature as Cutover, types::ServiceConfigKey};
use crate::types::gateway::gateway_to_text;
use crate::types::payment::payment_method::PaymentMethodType as PMT;
use crate::types::merchant::merchant_account::MerchantAccount;
use crate::types::merchant::merchant_account as MA;
use crate::types::card::txn_card_info::TxnCardInfo;
use crate::types::merchant::id as MID;
use crate::feedback::utils::GatewayScoringType as GST;
use crate::feedback::utils as Fbu;
use crate::decider::gatewaydecider::constants::{self as DC, srV3DefaultInputConfig};
use crate::decider::gatewaydecider::types::GatewayScoringData;
use crate::app::{get_tenant_app_state, APP_STATE};
use crate::feedback::gateway_selection_scoring_v3 as GSSV3;
use crate::merchant_config_util as MerchantConfig;
use crate::decider::gatewaydecider::types::RoutingFlowType as RF;
use crate::decider::gatewaydecider::utils::{self as GU, get_m_id, get_payment_method, get_sr_v3_latency_threshold};
use crate::feedback::types as FT;
// use utils::redis::feature as cutover::is_feature_enabled;
// use prelude::{from_integral, foldable::length, map_m, error};
// use data::foldable::{for_, foldl};
// use data::text::is_infix_of;
// use data::byte_string::lazy as bsl;
// use data::text::encoding as te;
// use control::monad::extra::maybe_m;
// use control::category;
use crate::types::merchant as ETM;
use crate::types::transaction::id::transaction_id_to_text;
// use utils::redis as redis;
// use db::common::types::payment_flows as pf;
// use utils::config::merchant_config as merchant_config;
// use gateway_decider::utils as gu::{get_sr_v3_latency_threshold, get_payment_method};
// use gateway_decider::types::{routing_flow_type, gateway_scoring_data};
// use gateway_decider::types as update_status;
// use types::tenant_config as tenant_config;
// use prelude::float;
// use utils::config::service_configuration as sc;
// use feedback::utils::*;
// use eulerhs::language as l;
// use data::aeson as a;
// use types::merchant as etm;


use fred::types::SetOptions;
use serde::{Deserialize, Serialize};

use crate::{
    feedback::{
        utils::{isRecurringTxn, GatewayScoringType, isPennyMandateRegTxn},
        constants as C,
        gateway_elimination_scoring::flow as GEF,
        
    },
    types::txn_details::types::TxnDetail,
    types::txn_details::types::TxnStatus,
    types::txn_details::types::TxnStatus as TS,
};

use super::constants::{defaultSrV3LatencyThresholdInSecs, SR_V3_INPUT_CONFIG, UPDATE_GATEWAY_SCORE_LOCK_FLAG_TTL, UPDATE_SCORE_LOCK_FEATURE_ENABLED_MERCHANT};
use super::utils::getTimeFromTxnCreatedInMills;

// Converted data types
// Original Haskell data type: GatewayLatencyForScoring
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct GatewayLatencyForScoring {
    #[serde(rename = "defaultLatencyThreshold")]
    pub defaultLatencyThreshold: f64,

    #[serde(rename = "merchantLatencyGatewayWiseInput")]
    pub merchantLatencyGatewayWiseInput: Option<Vec<GatewayWiseLatencyInput>>,
}

// Original Haskell data type: GatewayWiseLatencyInput
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct GatewayWiseLatencyInput {
    #[serde(rename = "gateway")]
    pub gateway: String,

    #[serde(rename = "paymentMethodType")]
    pub paymentMethodType: Option<String>,

    #[serde(rename = "paymentMethod")]
    pub paymentMethod: Option<String>,

    #[serde(rename = "latencyThreshold")]
    pub latencyThreshold: f64,
}

// Original Haskell data type: UpdateGatewayScoreRequest
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct UpdateGatewayScoreRequest {
    #[serde(rename = "command")]
    pub command: GatewayScoringType,

    #[serde(rename = "txn_detail_id")]
    pub txn_detail_id: Option<String>,

    #[serde(rename = "txn_id")]
    pub txn_id: Option<String>,

    #[serde(rename = "merchant_id")]
    pub merchant_id: Option<String>,

    #[serde(rename = "order_id")]
    pub order_id: Option<String>,

    #[serde(rename = "txn_status")]
    pub txn_status: TxnStatus,
}

// Original Haskell data type: MetricEntry
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct MetricEntry {
    #[serde(rename = "total_volume")]
    pub total_volume: f32,

    #[serde(rename = "success_rate")]
    pub success_rate: f32,

    #[serde(rename = "sigma_factor")]
    pub sigma_factor: f32,

    #[serde(rename = "average_latency")]
    pub average_latency: f32,

    #[serde(rename = "tp99_latency")]
    pub tp99_latency: f32,
}

// Original Haskell data type: SrMetrics
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct SrMetrics {
    #[serde(rename = "dimension")]
    pub dimension: String,

    #[serde(rename = "value")]
    pub value: MetricEntry,
}


#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct MerchantSrMetrics {
    #[serde(rename = "merchant_id")]
    pub merchant_id: String,

    #[serde(rename = "sr_metrics")]
    pub sr_metrics: Vec<SrMetrics>,
}


#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct ResetGatewayScoreRequest {
    #[serde(rename = "gateway")]
    pub gateway: String,

    #[serde(rename = "eliminationThreshold")]
    pub eliminationThreshold: f64,

    #[serde(rename = "gatewayEliminationThreshold")]
    pub gatewayEliminationThreshold: Option<f64>,

    #[serde(rename = "eliminationMaxCount")]
    pub eliminationMaxCount: i32,

    #[serde(rename = "gatewayReferenceId")]
    pub gatewayReferenceId: Option<String>,
}


#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct ResetGatewayScoreBulkRequest {
    #[serde(rename = "txn_detail_id")]
    pub txn_detail_id: Option<String>,

    #[serde(rename = "txn_id")]
    pub txn_id: Option<String>,

    #[serde(rename = "merchant_id")]
    pub merchant_id: Option<String>,

    #[serde(rename = "order_id")]
    pub order_id: Option<String>,

    #[serde(rename = "resetGatewayScoreReqArr")]
    pub resetGatewayScoreReqArr: Vec<ResetGatewayScoreRequest>,
}

pub fn defaultGwLatencyCheckInMins() -> GatewayLatencyForScoring {
    GatewayLatencyForScoring {
        defaultLatencyThreshold: 10.0,
        merchantLatencyGatewayWiseInput: None,
    }
}


pub fn txnSuccessStates() -> Vec<TxnStatus> {
    vec![
        TS::Charged,
        TS::Authorized,
        TS::CODInitiated,
        TS::Voided,
        TS::VoidInitiated,
        TS::CaptureInitiated,
        TS::CaptureFailed,
        TS::VoidFailed,
        TS::AutoRefunded,
        TS::PartialCharged,
        TS::ToBeCharged,
    ]
}

pub fn txnFailureStates() -> Vec<TxnStatus> {
    vec![
        TxnStatus::AuthenticationFailed,
        TxnStatus::AuthorizationFailed,
        TxnStatus::JuspayDeclined,
        TxnStatus::Failure,
    ]
}

pub async fn checkAndSendShouldUpdateGatewayScore(
    lock_key: String,
    lock_key_ttl: i32,
) -> bool {
    let app_state = get_tenant_app_state().await;
    let is_set_either = 
        app_state.redis_conn
        .setXWithOption(lock_key.as_str(), "true", lock_key_ttl as i64, SetOptions::NX)
        .await;
    
    match is_set_either {
        Ok(value) => value,
        Err(_) => false,
    }
}


pub fn isTransactionSuccess(txn_status: TxnStatus) -> bool {
    
    txnSuccessStates().contains(&txn_status)
}


pub fn isTransactionFailure(txn_status: TxnStatus) -> bool {
    txnFailureStates().contains(&txn_status)
}


pub async fn getGatewayScoringType(  
    txn_detail: TxnDetail,  
    txn_card_info: TxnCardInfo,  
    flag: bool,  
) -> GatewayScoringType {  
    if flag {  
        return GatewayScoringType::PENALISE_SRV3;  
    }  
  
    let txn_status = txn_detail.status.clone();  
    let merchant_id = txn_detail.merchantId.clone();  
    let is_success = isTransactionSuccess(txn_status.clone());  
    let is_failure = isTransactionFailure(txn_status.clone());  
    let time_difference = getTimeFromTxnCreatedInMills(txn_detail.clone());  
    let merchant_sr_v3_input_config = findByNameFromRedis(SR_V3_INPUT_CONFIG(get_m_id(merchant_id)).get_key()).await;  
    let pmt = txn_card_info.paymentMethodType.to_text();  
    let pm = get_payment_method(pmt.to_string(), txn_card_info.paymentMethod, txn_detail.sourceObject.unwrap_or_default());  
    let maybe_latency_threshold = get_sr_v3_latency_threshold(merchant_sr_v3_input_config, &pmt, &pm);  
    
    let time_difference_threshold = match maybe_latency_threshold {  
        None => {  
            let default_sr_v3_input_config = findByNameFromRedis(srV3DefaultInputConfig.get_key()).await;  
            let maybe_default_latency_threshold = get_sr_v3_latency_threshold(default_sr_v3_input_config, &pmt, &pm);  
            maybe_default_latency_threshold.unwrap_or( defaultSrV3LatencyThresholdInSecs())  
        }  
        Some(latency_threshold) => latency_threshold,  
    };  
  
    // EL.logInfoT("sr_v3_latency_threshold", &format!("Latency Threshold: {}", time_difference_threshold));  
  
    if is_success {  
        GatewayScoringType::REWARD  
    } else if is_failure {  
        GatewayScoringType::PENALISE_SRV3  
    } else if time_difference < ((time_difference_threshold * 1000.0) as u128) {  
        GatewayScoringType::PENALISE  
    } else {  
        GatewayScoringType::PENALISE_SRV3  
    }  
}  

pub fn updateGatewayScoreLock(
    gateway_scoring_type: GatewayScoringType,
    txn_uuid: String,
    gateway: String,
) -> String {  
    match (gateway_scoring_type) {
        (GatewayScoringType::PENALISE) => {
            format!("gateway_scores_lock_PENALISE_{}_{}", txn_uuid, gateway)
        }
        (GatewayScoringType::PENALISE_SRV3) => {
            format!("gateway_scores_lock_PENALISE_SRV3_{}_{}", txn_uuid, gateway)
        }
        (GatewayScoringType::REWARD) => {
            format!("gateway_scores_lock_REWARD_{}_{}", txn_uuid, gateway)
        }
        _ => String::new(),
    }
}

pub async fn check_and_update_gateway_score_(
  apiPayload: FT::UpdateScorePayload,
) -> () {
    let redis_key = format!("{}{}", C::gatewayScoringData, apiPayload.clone().paymentId);
    let app_state = get_tenant_app_state().await;
    let gateway_scoring_data: GatewayScoringData = app_state.redis_conn.get_key(&redis_key, "GatewayScoringData").await.expect("GatewayScoringData Not Found");
    let txn_detail: TxnDetail = Fbu::getTxnDetailFromApiPayload(apiPayload.clone() ,gateway_scoring_data.clone());
    let txn_card_info = Fbu::getTxnCardInfoFromApiPayload(apiPayload.clone() ,gateway_scoring_data.clone());
    let log_message = "update_gateway_score";
    let enforce_failure = apiPayload.enforceDynamicRoutingFailure.unwrap_or(false);
    check_and_update_gateway_score(
        txn_detail,
        txn_card_info,
        log_message,
        enforce_failure,
        apiPayload.gatewayReferenceId.clone(),
    ).await;
}


pub async fn check_and_update_gateway_score(
    txn_detail: TxnDetail,
    txn_card_info: TxnCardInfo,
    log_message: &str,
    enforce_failure: bool,
    gateway_reference_id: Option<String>,
) -> () {
    // Get gateway scoring type
    let gateway_scoring_type = getGatewayScoringType(txn_detail.clone(), txn_card_info.clone(), enforce_failure).await;

    let gateway_in_string = match txn_detail.gateway.clone() {
        Some(gw) => gateway_to_text(&gw),
        None => "".to_string(),
    };

    // Create update score lock key
    let update_score_lock_key = updateGatewayScoreLock(
        gateway_scoring_type.clone(), 
        txn_detail.txnUuid.clone(),  // This is Maybe type in haskell and here it is not Option 
        gateway_in_string// Convert Option to String
    );

    let lock_key_ttl = findByNameFromRedis(UPDATE_GATEWAY_SCORE_LOCK_FLAG_TTL.get_key())
        .await
        .unwrap_or(300);


    let should_compute_gw_score = checkAndSendShouldUpdateGatewayScore(
        update_score_lock_key, 
        lock_key_ttl
    ).await;

    // Check if feature is enabled for merchant
    let feature_enabled = isFeatureEnabled(UPDATE_SCORE_LOCK_FEATURE_ENABLED_MERCHANT.get_key(), get_m_id(txn_detail.merchantId.clone()), "kv_redis".to_string()).await;

    // Logging and score update logic
    if feature_enabled {
        if should_compute_gw_score {
            // info!(
            //     "Updating Gateway Score in {} flow with status as {} and scoring type as {:?}",
            //     log_message, txn_detail.status, gateway_scoring_type
            // );
            updateGatewayScore(
                gateway_scoring_type.clone(), 
                txn_detail.clone(), 
                txn_card_info.clone(),
                gateway_reference_id.clone()
            ).await;
        }
    } else {
        // info!(
        //     "GW_SCORE_LOCK_FEATURE_NOT_ENABLED: Updating Gateway Score in {} flow with status as {} and scoring type as {:?}",
        //     log_message, txn_detail.status, gateway_scoring_type
        // );
        updateGatewayScore(
            gateway_scoring_type.clone(), 
            txn_detail, 
            txn_card_info.clone(),
            gateway_reference_id.clone()
        ).await;
    }

    ()
}


// Original Haskell function: updateGatewayScore
pub async fn updateGatewayScore(
    gateway_scoring_type: GatewayScoringType,
    txn_detail: TxnDetail,
    txn_card_info: TxnCardInfo,
    gateway_reference_id: Option<String>,
) -> () {
    let mer_acc: MerchantAccount =  MA::load_merchant_by_merchant_id(MID::merchant_id_to_text(txn_detail.clone().merchantId)).await.expect("Merchant account not found");
       
    //let mer_acc = 
    let routing_approach = getRoutingApproach(txn_detail.clone());
   // EL.logInfoV::<String>("routing_approach_value", &routing_approach);

    let should_update_gateway_score = if gateway_scoring_type.clone() == GST::PENALISE_SRV3 {
        false
    } else if gateway_scoring_type.clone() == GST::PENALISE {
        isTransactionPending(txn_detail.clone().status)
    } else {
        true
    };

    //let is_pm_and_pmt_present = Fbu::isTrueString(txn_card_info.paymentMethod) && txn_card_info.paymentMethodType.is_some();
    let should_update_srv3_gateway_score = if gateway_scoring_type.clone() == GST::PENALISE {
        false
    } else {
        true
    };

    let is_update_within_window = isUpdateWithinLatencyWindow(txn_detail.clone(), txn_card_info.clone(), gateway_scoring_type.clone(), mer_acc.clone()).await;
    

    let should_isolate_srv3_producer = if Cutover::isFeatureEnabled(
        C::SR_V3_PRODUCER_ISOLATION.get_key(),
        MID::merchant_id_to_text( txn_detail.clone().merchantId),
        C::kvRedis(),
    ).await
    {
        if isRoutingApproachInSRV3(routing_approach.clone()) {
            true
        } else {
            false
        }
    } else {
        true
    };

    let should_update_explore_txn = if Cutover::isFeatureEnabled(
        DC::ENABLE_EXPLORE_AND_EXPLOIT_ON_SRV3(txn_card_info.clone().paymentMethodType.to_text().to_string()).get_key(),
        MID::merchant_id_to_text( txn_detail.clone().merchantId),
        C::kvRedis(),
    ).await
    {
        if isRoutingApproachInExplore(routing_approach.clone()) {
            true
        } else {
            false
        }
    } else {
        true
    };

    let redis_key = format!("{}{}", C::gatewayScoringData, txn_detail.clone().txnUuid);
    let redis_gateway_score_data = if should_update_srv3_gateway_score
        && is_update_within_window
        && should_isolate_srv3_producer
        && should_update_explore_txn
    {
        // EL.logInfoV::<String>(
        //     "updateGatewayScore",
        //     &format!(
        //         "Updating sr v3 score for the txn with scoring type as {} and status as {}",
        //         gateway_scoring_type,
        //         txn_detail.status
        //     ),
        // );
        let app_state = get_tenant_app_state().await;
        let mb_gateway_scoring_data: Option<GatewayScoringData>   = app_state.redis_conn.get_key(&redis_key, "GatewayScoringData").await.ok();
            GSSV3::flow::updateSrV3Score(gateway_scoring_type.clone(), txn_detail.clone(), txn_card_info.clone(), mer_acc.clone(), mb_gateway_scoring_data.clone(), gateway_reference_id.clone()).await;
        mb_gateway_scoring_data
    } else {
        None
    };

    if should_update_gateway_score && is_update_within_window {
        let mer_acc_p_id: ETM::id::MerchantPId = mer_acc.id.clone();
        let m_pf_mc_config = MerchantConfig::getMerchantConfigEntityLevelLookupConfig().await;
        let mb_gateway_scoring_data = match redis_gateway_score_data {
            None => {
                let app_state = get_tenant_app_state().await;
                let redis_data: Option<GatewayScoringData> = app_state.redis_conn.get_key(&redis_key, "GatewayScoringData").await.ok();
                redis_data
            }
            Some(_) => redis_gateway_score_data,
        };
        //EL.logInfoV::<String>("GatewayScoringData", &format!("{:?}", mb_gateway_scoring_data));
        match mb_gateway_scoring_data {
            None => {
                // EL.logErrorV::<String>(
                //     "GATEWAY_SCORING_DATA_NOT_FOUND_FOR_ELIMINATION",
                //     "Gateway scoring data is not found in redis",
                // );
            }
            Some(gateway_scoring_data) => {
                let key_array = GEF::getAllUnifiedKeys(
                    txn_detail.clone(),
                    txn_card_info.clone(),
                    mer_acc_p_id.clone(),
                    m_pf_mc_config.clone(),
                    mer_acc.clone(),
                    gateway_scoring_data.clone(),
                    gateway_reference_id.clone(),
                )
                .await;
            for key in key_array {
                tokio::spawn(
                    GEF::updateKeyScoreForKeysFromConsumer(
                        txn_detail.clone(),
                        txn_card_info.clone(),
                        gateway_scoring_type.clone(),
                        mer_acc_p_id,
                        mer_acc.clone(),
                        key
                    )
                );
            }
        }
        }
        Fbu::logGatewayScoreType(gateway_scoring_type, RF::ELIMINATION_FLOW, txn_detail.clone());
    } else {
        if !should_update_gateway_score {
            // L.logDebugV::<String>(
            //     "updateGatewayScore",
            //     &format!(
            //         "Gateway Scoring Type {} does not match with txn status {}",
            //         gateway_scoring_type,
            //         txn_detail.status
            //     ),
            // );
        }
        // if !is_pm_and_pmt_present {
        //     L.logDebugV::<String>(
        //         "updateGatewayScore",
        //         &format!(
        //             "Payment Method or Payment Method Type is null for txn_detail.id {}",
        //             txn_detail._id.as_deref().unwrap_or("")
        //         ),
        //     );
        // }
        if !is_update_within_window {
            // L.logDebugV::<String>(
            //     "updateGatewayScore",
            //     "Update GW Score call received outside Update Window",
            // );
        }
    }
}

// Original Haskell function: getRoutingApproach
pub fn getRoutingApproach(txnDetail: TxnDetail) -> Option<String> {
    let internalMeta: Option<FT::InternalMetadata> = getValueFromMetaData(&txnDetail);
    match internalMeta {
        Some(meta) => Some(meta.internal_tracking_info.routing_approach),
        None => None,    
    }
}


// Original Haskell function: getValueFromMetaData
pub fn getValueFromMetaData<T: serde::de::DeserializeOwned>(txn_detail: &TxnDetail) -> Option<T> {
    let metadata = txn_detail.internalMetadata.clone()?;
    serde_json::from_str(&metadata).ok()
}

// Original Haskell function: isRoutingApproachInSRV2
pub fn isRoutingApproachInSRV2(maybe_text: Option<String>) -> bool {
    match maybe_text {
        Some(text) => text.contains("V2"),
        None => false,
    }
}

// Original Haskell function: isRoutingApproachInSRV3
pub fn isRoutingApproachInSRV3(maybe_text: Option<String>) -> bool {
    match maybe_text {
        Some(text) => text.contains("V3"),
        None => false,
    }
}

// Original Haskell function: isRoutingApproachInExplore
pub fn isRoutingApproachInExplore(maybe_text: Option<String>) -> bool {
    match maybe_text {
        Some(text) => text.contains("HEDGING"),
        None => false,
    }
}

// Original Haskell function: isUpdateWithinLatencyWindow
pub async fn isUpdateWithinLatencyWindow(
    txn_detail: TxnDetail,
    txn_card_info: TxnCardInfo,
    gateway_scoring_type: GatewayScoringType,
    mer_acc: MerchantAccount,
) -> bool {
    match gateway_scoring_type {
        GatewayScoringType::PENALISE => true,
        _ => {
            let exempt_for_mandate_txn = checkExemptIfMandateTxn(&txn_detail, &txn_card_info).await;
            if exempt_for_mandate_txn
                // || txn_detail
                //     .gateway
                //     .as_ref()
                //     .map_or(true, |gw| exempt_gws.contains(gw))
            {
                true
            } else {
                let m_auto_refund_conflict_threshold_in_mins: Option<i32> = None; // Placeholder for actual implementation
                let gw_score_latency_threshold = C::defaultGatewayScoreLatencyCheckInMins();
                    // Cutover::findByNameFromRedis(C.gatewayScoreLatencyCheckInMins)
                    //     .await
                    //     .unwrap_or(C.defaultGatewayScoreLatencyCheckInMins);
                let merchant_id = MID::merchant_id_to_text( txn_detail.merchantId.clone());
                let pmt = txn_card_info.paymentMethodType.to_text().to_string();
                let pm = GU::get_payment_method(
                    pmt,
                    txn_card_info.paymentMethod,
                    txn_detail.sourceObject.clone().unwrap_or_default(),
                );

                let gw_score_update_latency = Fbu::getTimeFromTxnCreatedInMills(txn_detail.clone());
                let gw_latency_check_threshold = gw_score_latency_threshold as u128;
                // EL::logInfoT(
                //     "gwLatencyCheckThreshold",
                //     &gw_latency_check_threshold.to_string(),
                // )
               // .await;
                if gw_score_update_latency < gw_latency_check_threshold * 60000u128 {
                    true
                 } else {
                //     let gateway = txn_detail.gateway.clone().unwrap_or_else(|| "NA".into());
                //     let merchant_id = MID::merchant_id_to_text( txn_detail.merchantId.clone());
                //     let pmt = txn_card_info.paymentMethodType.to_text().to_string();
                //     let payment_method = txn_card_info
                //         .paymentMethod
                //         .clone()
                //         .unwrap_or_else(|| "NA".into());
                //     let source_object = txn_detail
                //         .sourceObject
                //         .clone()
                //         .unwrap_or_else(|| "NA".into());
                    // L::logDebugV(
                    //     "isUpdateWithinLatencyWindow",
                    //     &format!(
                    //         "TxnId {} blocked due to a latency of {} mins.",
                    //         txn_detail.txnId,
                    //         gw_score_update_latency / 60000.0
                    //     ),
                    // )
                    // .await;
                    false
                }
            }
        }
    }
}

async fn checkExemptIfMandateTxn(txn_detail: &TxnDetail, txn_card_info: &TxnCardInfo) -> bool {
    let is_recurring = isRecurringTxn(Some(txn_detail.txnObjectType.clone()));
    let is_nb_pmt = txn_card_info.paymentMethodType == (PMT::NB);
    let is_penny_reg_txn = isPennyMandateRegTxn(txn_detail.clone());
    is_recurring || (is_nb_pmt && is_penny_reg_txn)
}

// Original Haskell function: isTransactionPending
pub fn isTransactionPending(txnStatus: TxnStatus) -> bool {
    txnStatus == TS::PendingVBV
}