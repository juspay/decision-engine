// Automatically converted from Haskell to Rust
// Generated on 2025-03-23 10:24:42

// Converted imports
use gateway_decider::constants as c::{enable_elimination_v2, gateway_scoring_data, ENABLE_EXPLORE_AND_EXPLOIT_ON_SRV3, SR_V3_INPUT_CONFIG, GATEWAY_SCORE_FIRST_DIMENSION_SOFT_TTL};
use feedback::constants as c;
use data::text::encoding as de::encode_utf8;
use db::storage::types::merchant_account as merchant_account;
use types::gateway_routing_input as etgri;
use gateway_decider::utils::decode_and_log_error;
use gateway_decider::gw_scoring::get_sr1_and_sr2_and_n;
use feedback::utils as euler_transforms;
use feedback::types::*;
use feedback::types::txn_card_info;
use eulerhs::prelude::*;
use eulerhs::language::get_current_date_in_millis;
use data::text as t;
use feedback::utils::*;
use feedback::gateway_selection_scoring_v3::flow;
use feedback::gateway_elimination_scoring::flow;
use eulerhs::language as el;
use eulerhs::types as et;
use eulerhs::tenant_redis_layer as rc;
use utils::redis::cache as cutover;
use utils::redis::feature as cutover::is_feature_enabled;
use prelude::{from_integral, foldable::length, map_m, error};
use data::foldable::{for_, foldl};
use data::text::is_infix_of;
use data::byte_string::lazy as bsl;
use data::text::encoding as te;
use control::monad::extra::maybe_m;
use control::category;
use types::merchant as merchant;
use utils::redis as redis;
use db::common::types::payment_flows as pf;
use utils::config::merchant_config as merchant_config;
use gateway_decider::utils as gu::{get_sr_v3_latency_threshold, get_payment_method};
use gateway_decider::types::{routing_flow_type, gateway_scoring_data};
use gateway_decider::types as update_status;
use types::tenant_config as tenant_config;
use prelude::float;
use utils::config::service_configuration as sc;
use feedback::utils::*;
use eulerhs::language as l;
use data::aeson as a;
use types::merchant as etm;


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
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Ord)]
pub struct SrMetrics {
    #[serde(rename = "dimension")]
    pub dimension: String,
    
    #[serde(rename = "value")]
    pub value: MetricEntry,
}


// Original Haskell data type: MerchantSrMetrics
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct MerchantSrMetrics {
    #[serde(rename = "merchant_id")]
    pub merchant_id: String,
    
    #[serde(rename = "sr_metrics")]
    pub sr_metrics: Vec<SrMetrics>,
}


// Original Haskell data type: ResetGatewayScoreRequest
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


// Original Haskell data type: ResetGatewayScoreBulkRequest
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


// Original Haskell data type: InternalMetadata
#[derive(Debug, Serialize, Deserialize)]
pub struct InternalMetadata {
    #[serde(rename = "internal_tracking_info")]
    pub internal_tracking_info: InternalTrackingInfo,
}


// Original Haskell data type: InternalTrackingInfo
#[derive(Debug, Serialize, Deserialize)]
pub struct InternalTrackingInfo {
    #[serde(rename = "routing_approach")]
    pub routing_approach: String,
}


// Converted functions
// Original Haskell function: defaultGwLatencyCheckInMins
pub fn defaultGwLatencyCheckInMins() -> GatewayLatencyForScoring {
    GatewayLatencyForScoring {
        defaultLatencyThreshold: 10.0,
        merchantLatencyGatewayWiseInput: None,
    }
}


// Original Haskell function: txnSuccessStates
pub fn txnSuccessStates() -> Vec<TxnStatus> {
    vec![
        TxnStatus::CHARGED,
        TxnStatus::AUTHORIZED,
        TxnStatus::COD_INITIATED,
        TxnStatus::VOIDED,
        TxnStatus::VOID_INITIATED,
        TxnStatus::CAPTURE_INITIATED,
        TxnStatus::CAPTURE_FAILED,
        TxnStatus::VOID_FAILED,
        TxnStatus::AUTO_REFUNDED,
        TxnStatus::PARTIAL_CHARGED,
        TxnStatus::TO_BE_CHARGED,
    ]
}


// Original Haskell function: txnFailureStates
pub fn txnFailureStates() -> Vec<TxnStatus> {
    vec![
        TxnStatus::AUTHENTICATION_FAILED,
        TxnStatus::AUTHORIZATION_FAILED,
        TxnStatus::JUSPAY_DECLINED,
        TxnStatus::FAILURE,
    ]
}


// Original Haskell function: updateGatewayScore
pub async fn updateGatewayScore(
    gateway_scoring_type: GatewayScoringType,
    txn_detail: TxnDetail,
    txn_card_info: TxnCardInfo,
) -> impl L.MonadFlow<()> {
    let mer_acc = None; // MerchantAccount.findMerchantAccount(txn_detail._merchantId);
    let routing_approach = getRoutingApproach(&txn_detail);
    EL.logInfoV::<String>("routing_approach_value", &routing_approach);

    let should_update_gateway_score = if gateway_scoring_type == PENALISE_SRV3 {
        false
    } else if gateway_scoring_type == PENALISE {
        isTransactionPending(txn_detail.status)
    } else {
        true
    };

    let is_pm_and_pmt_present = isTrueString(&txn_card_info.paymentMethod) && txn_card_info.paymentMethodType.is_some();
    let should_update_srv3_gateway_score = if gateway_scoring_type == PENALISE {
        false
    } else {
        true
    };

    let is_update_within_window = if is_pm_and_pmt_present {
        isUpdateWithinLatencyWindow(&txn_detail, &txn_card_info, gateway_scoring_type, mer_acc).await
    } else {
        false
    };

    let should_isolate_srv3_producer = if Cutover.isFeatureEnabled(
        C.shouldIsolateProducerVolumesSRv3,
        txn_detail.merchantId.as_deref().unwrap_or(""),
        C.kvRedis,
    )
    .await
    {
        if isRoutingApproachInSRV3(&routing_approach) {
            true
        } else {
            false
        }
    } else {
        true
    };

    let should_update_explore_txn = if Cutover.isFeatureEnabled(
        C.ENABLE_EXPLORE_AND_EXPLOIT_ON_SRV3(&txn_card_info.paymentMethodType.map_or("".to_string(), |pmt| pmt.to_string())),
        txn_detail.merchantId.as_deref().unwrap_or(""),
        C.kvRedis,
    )
    .await
    {
        if isRoutingApproachInExplore(&routing_approach) {
            true
        } else {
            false
        }
    } else {
        true
    };

    let redis_key = format!("{}{}", C.gatewayScoringData, txn_detail.txnUuid.as_deref().unwrap_or(""));
    let redis_gateway_score_data = if should_update_srv3_gateway_score
        && is_pm_and_pmt_present
        && is_update_within_window
        && should_isolate_srv3_producer
        && should_update_explore_txn
    {
        EL.logInfoV::<String>(
            "updateGatewayScore",
            &format!(
                "Updating sr v3 score for the txn with scoring type as {} and status as {}",
                gateway_scoring_type,
                txn_detail.status
            ),
        );
        let mb_gateway_scoring_data: Option<GatewayScoringData> = RC.rGet(C.kvRedis, &redis_key).await;
        updateSrV3Score(gateway_scoring_type, &txn_detail, &txn_card_info, mer_acc, mb_gateway_scoring_data.clone()).await;
        mb_gateway_scoring_data
    } else {
        None
    };

    if should_update_gateway_score && is_pm_and_pmt_present && is_update_within_window {
        let mer_acc_p_id: ETM.MerchantPId = None; // MerchantAccount.getMerchantPIdFromMerchantAccount(mer_acc);
        let m_pf_mc_config = MerchantConfig.getMerchantConfigEntityLevelLookupConfig().await;
        let mb_gateway_scoring_data = match redis_gateway_score_data {
            None => {
                let redis_data: Option<GatewayScoringData> = RC.rGet(C.kvRedis, &redis_key).await;
                redis_data
            }
            Some(_) => redis_gateway_score_data,
        };
        EL.logInfoV::<String>("GatewayScoringData", &format!("{:?}", mb_gateway_scoring_data));
        match mb_gateway_scoring_data {
            None => {
                EL.logErrorV::<String>(
                    "GATEWAY_SCORING_DATA_NOT_FOUND_FOR_ELIMINATION",
                    "Gateway scoring data is not found in redis",
                );
            }
            Some(gateway_scoring_data) => {
                let key_array = getAllUnifiedKeys(
                    &txn_detail,
                    &txn_card_info,
                    mer_acc_p_id,
                    m_pf_mc_config,
                    mer_acc,
                    gateway_scoring_data,
                )
                .await;
                let scores = futures::future::join_all(
                    key_array
                        .into_iter()
                        .map(|key| updateKeyScoreForKeysFromConsumer(&txn_detail, &txn_card_info, gateway_scoring_type, mer_acc_p_id, mer_acc, key)),
                )
                .await
                .into_iter()
                .filter_map(|x| x)
                .collect::<Vec<_>>();
                ()
            }
        }
        logGatewayScoreType(gateway_scoring_type, ELIMINATION_FLOW, &txn_detail).await;
    } else {
        if !should_update_gateway_score {
            L.logDebugV::<String>(
                "updateGatewayScore",
                &format!(
                    "Gateway Scoring Type {} does not match with txn status {}",
                    gateway_scoring_type,
                    txn_detail.status
                ),
            );
        }
        if !is_pm_and_pmt_present {
            L.logDebugV::<String>(
                "updateGatewayScore",
                &format!(
                    "Payment Method or Payment Method Type is null for txn_detail.id {}",
                    txn_detail._id.as_deref().unwrap_or("")
                ),
            );
        }
        if !is_update_within_window {
            L.logDebugV::<String>(
                "updateGatewayScore",
                "Update GW Score call received outside Update Window",
            );
        }
    }
}


// Original Haskell function: getRoutingApproach
pub fn getRoutingApproach(txnDetail: TxnDetail) -> Option<String> {
    let internalMeta: Option<InternalMetadata> = getValueFromMetaData(txnDetail);
    routing_approach(internal_tracking_info(internalMeta))
}


// Original Haskell function: getValueFromMetaData
pub fn getValueFromMetaData<T: serde::de::DeserializeOwned>(
    txn_detail: &TxnDetail,
) -> Option<T> {
    let metadata = txn_detail.internalMetadata()?;
    match serde_json::from_slice(metadata.as_bytes()) {
        Ok(val) => Some(val),
        Err(_) => None,
    }
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
            let exempt_gws = Cutover::findByNameFromRedis(C.gatewayScoreLatencyCheckExemptGateways)
                .await
                .unwrap_or_else(|| vec![]);
            let exempt_for_mandate_txn = checkExemptIfMandateTxn(&txn_detail, &txn_card_info).await;
            if exempt_for_mandate_txn
                || txn_detail.gateway.as_ref().map_or(true, |gw| exempt_gws.contains(gw))
            {
                true
            } else {
                let m_auto_refund_conflict_threshold_in_mins: Option<i32> = None; // Placeholder for actual implementation
                let gw_score_latency_threshold = Cutover::findByNameFromRedis(C.gatewayScoreLatencyCheckInMins)
                    .await
                    .unwrap_or(C.defaultGatewayScoreLatencyCheckInMins);
                let merchant_id = txn_detail.merchantId.clone().unwrap_or_default();
                let latency_threshold_from_redis = Cutover::findByNameFromRedis(
                    C.DEFAULT_GW_SCORE_LATENCY_THRESHOLD(Some(merchant_id.clone())),
                )
                .await
                .or_else(|| {
                    Cutover::findByNameFromRedis(C.DEFAULT_GW_SCORE_LATENCY_THRESHOLD(None)).await
                })
                .unwrap_or(defaultGwLatencyCheckInMins);
                let pmt = txn_card_info.paymentMethodType.to_string();
                let pm = GU::getPaymentMethod(
                    &pmt,
                    txn_card_info.paymentMethod.as_deref().unwrap_or(""),
                    txn_detail.sourceObject.as_deref().unwrap_or(""),
                );

                let gw_score_update_latency = getTimeFromTxnCreatedInMills(&txn_detail).await;
                let gw_latency_check_threshold = gw_score_latency_threshold as f64;
                EL::logInfoT("gwLatencyCheckThreshold", &gw_latency_check_threshold.to_string()).await;
                if gw_score_update_latency < gw_latency_check_threshold * 60000.0 {
                    true
                } else {
                    let gateway = txn_detail.gateway.clone().unwrap_or_else(|| "NA".into());
                    let merchant_id = txn_detail.merchantId.clone().unwrap_or_else(|| "NA".into());
                    let payment_method_type = txn_card_info.paymentMethodType
                        .as_ref()
                        .map_or_else(|| "NA".into(), |pmt| pmt.to_string());
                    let payment_method = txn_card_info.paymentMethod.clone().unwrap_or_else(|| "NA".into());
                    let source_object = txn_detail.sourceObject.clone().unwrap_or_else(|| "NA".into());
                    L::logDebugV(
                        "isUpdateWithinLatencyWindow",
                        &format!(
                            "TxnId {} blocked due to a latency of {} mins.",
                            txn_detail.txnId,
                            gw_score_update_latency / 60000.0
                        ),
                    )
                    .await;
                    false
                }
            }
        }
    }
}

async fn checkExemptIfMandateTxn(txn_detail: &TxnDetail, txn_card_info: &TxnCardInfo) -> bool {
    let is_recurring = isRecurringTxn(txn_detail.txn_object_type);
    let is_nb_pmt = txn_card_info.payment_method_type == Some(PaymentMethodType::NB);
    let is_penny_reg_txn = isPennyMandateRegTxn(txn_detail).await;
    is_recurring || (is_nb_pmt && is_penny_reg_txn)
}


// Original Haskell function: isTransactionPending
pub fn isTransactionPending(txnStatus: TxnStatus) -> bool {
    txnStatus == TxnStatus::PENDING_VBV
}

