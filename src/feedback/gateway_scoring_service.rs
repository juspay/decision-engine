// Automatically converted from Haskell to Rust
// Generated on 2025-03-23 10:24:42

use crate::redis::cache::findByNameFromRedis;
use crate::redis::feature::is_feature_enabled;
use masking::PeekInterface;
// Converted imports
// use gateway_decider::constants as c::{enable_elimination_v2, gateway_scoring_data, EnableExploreAndExploitOnSrv3, SrV3InputConfig, GatewayScoreFirstDimensionSoftTtl};
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
use crate::app::{get_tenant_app_state, APP_STATE};
use crate::decider::gatewaydecider::constants::{self as DC, SR_V3_DEFAULT_INPUT_CONFIG};
use crate::decider::gatewaydecider::types as T;
use crate::decider::gatewaydecider::types::GatewayScoringData;
use crate::decider::gatewaydecider::types::{RoutingFlowType as RF, SrRoutingDimensions};
use crate::decider::gatewaydecider::utils::{
    self as GU, get_m_id, get_payment_method, get_sr_v3_latency_threshold,
};
use crate::feedback::gateway_selection_scoring_v3 as GSSV3;
use crate::feedback::types as FT;
use crate::feedback::utils as Fbu;
use crate::feedback::utils::GatewayScoringType as GST;
use crate::merchant_config_util as MerchantConfig;
use crate::redis::{feature as Cutover, types::ServiceConfigKey};
use crate::types::card::txn_card_info::TxnCardInfo;
use crate::types::gateway_routing_input::GatewaySuccessRateBasedRoutingInput;
use crate::types::merchant::id as MID;
use crate::types::merchant::merchant_account as MA;
use crate::types::merchant::merchant_account::MerchantAccount;
// use utils::redis::feature as cutover::is_feature_enabled;
// use prelude::{from_integral, foldable::length, map_m, error};
// use data::foldable::{for_, foldl};
// use data::text::is_infix_of;
// use data::byte_string::lazy as bsl;
// use data::text::encoding as te;
// use control::monad::extra::maybe_m;
// use control::category;
use crate::types::merchant as ETM;
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
        constants as C,
        gateway_elimination_scoring::flow as GEF,
        utils::{isPennyMandateRegTxn, isRecurringTxn, GatewayScoringType},
    },
    types::txn_details::types::{TransactionLatency, TxnDetail, TxnStatus, TxnStatus as TS},
};

use super::constants::{
    default_sr_v3_latency_threshold_in_secs, SrV3InputConfig, UpdateGatewayScoreLockFlagTtl,
    UpdateScoreLockFeatureEnabledMerchant,
};
use super::utils::get_time_from_txn_created_in_mills;
use crate::logger;
use crate::types::payment::payment_method_type_const::*;
// Converted data types
// Original Haskell data type: GatewayLatencyForScoring
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct GatewayLatencyForScoring {
    #[serde(rename = "defaultLatencyThreshold")]
    pub default_latency_threshold: f64,

    #[serde(rename = "merchantLatencyGatewayWiseInput")]
    pub merchant_latency_gateway_wise_input: Option<Vec<GatewayWiseLatencyInput>>,
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
    pub reset_gateway_score_req_arr: Vec<ResetGatewayScoreRequest>,
}

pub fn default_gw_latency_check_in_mins() -> GatewayLatencyForScoring {
    GatewayLatencyForScoring {
        default_latency_threshold: 10.0,
        merchant_latency_gateway_wise_input: None,
    }
}

pub fn txn_success_states() -> Vec<TxnStatus> {
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

pub fn txn_failure_states() -> Vec<TxnStatus> {
    vec![
        TxnStatus::AuthenticationFailed,
        TxnStatus::AuthorizationFailed,
        TxnStatus::JuspayDeclined,
        TxnStatus::Failure,
    ]
}

pub async fn check_and_send_should_update_gateway_score(
    lock_key: String,
    lock_key_ttl: i32,
) -> bool {
    let app_state = get_tenant_app_state().await;
    let is_set_either = app_state
        .redis_conn
        .setXWithOption(
            lock_key.as_str(),
            "true",
            lock_key_ttl as i64,
            SetOptions::NX,
        )
        .await;

    match is_set_either {
        Ok(value) => value,
        Err(_) => false,
    }
}

pub fn is_transaction_success(txn_status: TxnStatus) -> bool {
    txn_success_states().contains(&txn_status)
}

pub fn is_transaction_failure(txn_status: TxnStatus) -> bool {
    txn_failure_states().contains(&txn_status)
}

pub fn isGwLatencyWithinConfiguredThreshold(
    txn_latency: Option<f64>,
    merchant_latency_threshold: Option<f64>,
) -> bool {
    logger::info!(
        action = "txn_latency_within_threshold",
        tag = "txn_latency_within_threshold",
        "Latency & Threshold: {:?} {:?}",
        txn_latency,
        merchant_latency_threshold
    );
    if let Some((latency, threshold)) = txn_latency.zip(merchant_latency_threshold) {
        latency <= threshold
    } else {
        true
    }
}

pub async fn get_gateway_scoring_type(
    txn_detail: TxnDetail,
    txn_card_info: TxnCardInfo,
    flag: bool,
) -> GatewayScoringType {
    if flag {
        return GatewayScoringType::PenaliseSrv3;
    }

    let txn_status = txn_detail.status.clone();
    let merchant_id = txn_detail.merchantId.clone();
    let is_success = is_transaction_success(txn_status.clone());
    let is_failure = is_transaction_failure(txn_status.clone());
    let time_difference = get_time_from_txn_created_in_mills(txn_detail.clone());
    let merchant_sr_v3_input_config =
        findByNameFromRedis(SrV3InputConfig(get_m_id(merchant_id)).get_key()).await;
    let pmt = txn_card_info.paymentMethodType;
    let pm = get_payment_method(
        pmt.to_string(),
        txn_card_info.paymentMethod,
        txn_detail.sourceObject.unwrap_or_default(),
    );
    // Extract the new parameters from txn_card_info

    let sr_routing_dimesions = SrRoutingDimensions {
        card_network: txn_card_info
            .cardSwitchProvider
            .as_ref()
            .map(|s| s.peek().to_string()),
        card_isin: txn_card_info.card_isin.clone(),
        currency: Some(txn_detail.currency.to_string()),
        country: txn_detail.country.as_ref().map(|c| c.to_string()),
        auth_type: txn_card_info.authType.as_ref().map(|a| a.to_string()),
    };

    let maybe_latency_threshold = get_sr_v3_latency_threshold(
        merchant_sr_v3_input_config.clone(),
        &pmt,
        &pm,
        &sr_routing_dimesions,
    );

    let time_difference_threshold = match maybe_latency_threshold {
        None => {
            let default_sr_v3_input_config =
                findByNameFromRedis(SR_V3_DEFAULT_INPUT_CONFIG.get_key()).await;
            let maybe_default_latency_threshold = get_sr_v3_latency_threshold(
                default_sr_v3_input_config,
                &pmt,
                &pm,
                &sr_routing_dimesions,
            );
            maybe_default_latency_threshold.unwrap_or(default_sr_v3_latency_threshold_in_secs())
        }
        Some(latency_threshold) => latency_threshold,
    };

    logger::info!(
        action = "sr_v3_latency_threshold",
        tag = "sr_v3_latency_threshold",
        "Latency Threshold: {} Time Difference: {}",
        time_difference_threshold,
        time_difference
    );

    if is_success {
        GatewayScoringType::Reward
    } else if is_failure {
        GatewayScoringType::PenaliseSrv3
    } else if time_difference < ((time_difference_threshold * 1000.0) as u128) {
        GatewayScoringType::Penalise
    } else {
        GatewayScoringType::PenaliseSrv3
    }
}

pub fn update_gateway_score_lock(
    gateway_scoring_type: GatewayScoringType,
    txn_uuid: String,
    gateway: String,
) -> String {
    match (gateway_scoring_type) {
        (GatewayScoringType::Penalise) => {
            format!("gateway_scores_lock_PENALISE_{}_{}", txn_uuid, gateway)
        }
        (GatewayScoringType::PenaliseSrv3) => {
            format!("gateway_scores_lock_PENALISE_SRV3_{}_{}", txn_uuid, gateway)
        }
        (GatewayScoringType::Reward) => {
            format!("gateway_scores_lock_REWARD_{}_{}", txn_uuid, gateway)
        }
        _ => String::new(),
    }
}

pub fn invalid_request_error(detail: &str, e: &impl std::fmt::Display) -> T::ErrorResponse {
    T::ErrorResponse {
        status: "400".to_string(),
        error_code: "INVALID_REQUEST".to_string(),
        error_message: format!("Failed to extract {}: {}", detail, e),
        priority_logic_tag: None,
        routing_approach: None,
        filter_wise_gateways: None,
        error_info: T::UnifiedError {
            code: "INVALID_REQUEST".to_string(),
            user_message: "Invalid request data provided".to_string(),
            developer_message: format!("Error extracting {}: {}", detail, e),
        },
        priority_logic_output: None,
        is_dynamic_mga_enabled: false,
    }
}

pub async fn check_and_update_gateway_score_(
    api_payload: FT::UpdateScorePayload,
) -> Result<String, T::ErrorResponse> {
    let redis_key = format!(
        "{}{}",
        C::GATEWAY_SCORING_DATA,
        api_payload.clone().payment_id
    );
    let app_state = get_tenant_app_state().await;

    // Attempt to fetch gateway scoring data from Redis
    let m_gateway_scoring_data: Result<
        GatewayScoringData,
        error_stack::Report<redis_interface::errors::RedisError>,
    > = app_state
        .redis_conn
        .get_key(&redis_key, "GatewayScoringData")
        .await;

    match m_gateway_scoring_data {
        Ok(gateway_scoring_data) => {
            // Extract transaction details and card info from the API payload
            let txn_detail: TxnDetail = match Fbu::get_txn_detail_from_api_payload(
                api_payload.clone(),
                gateway_scoring_data.clone(),
            ) {
                Ok(detail) => detail,
                Err(e) => {
                    return Err(invalid_request_error("transaction details", &e));
                }
            };
            let txn_card_info: TxnCardInfo = Fbu::get_txn_card_info_from_api_payload(
                api_payload.clone(),
                gateway_scoring_data.clone(),
            );

            let log_message = "update_gateway_score";
            let enforce_failure = api_payload.enforce_dynamic_routing_failure.unwrap_or(false);

            // Call the function to check and update the gateway score
            check_and_update_gateway_score(
                txn_detail,
                txn_card_info,
                log_message,
                enforce_failure,
                api_payload.gateway_reference_id.clone(),
                api_payload.txn_latency.clone(),
            )
            .await;

            // Return success response
            Ok("Success".to_string())
        }
        Err(e) => {
            // Return error response if gateway scoring data is not found
            Err(T::ErrorResponse {
                status: "400".to_string(),
                error_code: "GATEWAY_SCORING_DATA_NOT_FOUND".to_string(),
                error_message: "GatewayScoringData is not found in redis".to_string(),
                priority_logic_tag: None,
                routing_approach: None,
                filter_wise_gateways: None,
                error_info: T::UnifiedError {
                    code: "GATEWAY_SCORING_DATA_NOT_FOUND".to_string(),
                    user_message:
                        "GatewayScoringData is not in redis. Please create the transaction."
                            .to_string(),
                    developer_message: e.to_string(),
                },
                priority_logic_output: None,
                is_dynamic_mga_enabled: false,
            })
        }
    }
}

pub async fn check_and_update_gateway_score(
    txn_detail: TxnDetail,
    txn_card_info: TxnCardInfo,
    log_message: &str,
    enforce_failure: bool,
    gateway_reference_id: Option<String>,
    txn_latency: Option<TransactionLatency>,
) -> () {
    // Get gateway scoring type
    let gateway_scoring_type =
        get_gateway_scoring_type(txn_detail.clone(), txn_card_info.clone(), enforce_failure).await;

    let gateway_in_string = txn_detail.gateway.clone().unwrap_or_default();

    // Create update score lock key
    let update_score_lock_key = update_gateway_score_lock(
        gateway_scoring_type.clone(),
        txn_detail.txnUuid.clone(), // This is Maybe type in haskell and here it is not Option
        gateway_in_string,          // Convert Option to String
    );

    let lock_key_ttl = findByNameFromRedis(UpdateGatewayScoreLockFlagTtl.get_key())
        .await
        .unwrap_or(300);

    let should_compute_gw_score =
        check_and_send_should_update_gateway_score(update_score_lock_key, lock_key_ttl).await;

    // Check if feature is enabled for merchant
    let feature_enabled = is_feature_enabled(
        UpdateScoreLockFeatureEnabledMerchant.get_key(),
        get_m_id(txn_detail.merchantId.clone()),
        "kv_redis".to_string(),
    )
    .await;

    // Logging and score update logic
    if feature_enabled {
        if should_compute_gw_score {
            logger::info!(
                action = "UPDATE_GATEWAY_SCORE_LOCK",
                tag = "UPDATE_GATEWAY_SCORE_LOCK",
                "Updating Gateway Score in {} flow with status as {:?} and scoring type as {:?}",
                log_message,
                txn_detail.status,
                gateway_scoring_type
            );
            update_gateway_score(
                gateway_scoring_type.clone(),
                txn_detail.clone(),
                txn_card_info.clone(),
                gateway_reference_id.clone(),
                txn_latency.clone(),
            )
            .await;
        }
    } else {
        logger::info!(
            action = "GW_SCORE_LOCK_FEATURE_NOT_ENABLED",
            tag = "GW_SCORE_LOCK_FEATURE_NOT_ENABLED",
            "Updating Gateway Score in {} flow with status as {:?} and scoring type as {:?}",
            log_message,
            txn_detail.status,
            gateway_scoring_type
        );
        update_gateway_score(
            gateway_scoring_type.clone(),
            txn_detail,
            txn_card_info.clone(),
            gateway_reference_id.clone(),
            txn_latency.clone(),
        )
        .await;
    }

    ()
}

// Original Haskell function: updateGatewayScore
pub async fn update_gateway_score(
    gateway_scoring_type: GatewayScoringType,
    txn_detail: TxnDetail,
    txn_card_info: TxnCardInfo,
    gateway_reference_id: Option<String>,
    txn_latency: Option<TransactionLatency>,
) -> () {
    let mer_acc: MerchantAccount =
        MA::load_merchant_by_merchant_id(MID::merchant_id_to_text(txn_detail.clone().merchantId))
            .await
            .expect("Merchant account not found");

    //let mer_acc =
    let routing_approach = get_routing_approach(txn_detail.clone());
    logger::info!(
        action = "routing_approach_value",
        tag = "routing_approach_value",
        "{:?}",
        routing_approach
    );

    let should_update_gateway_score = if gateway_scoring_type.clone() == GST::PenaliseSrv3 {
        false
    } else if gateway_scoring_type.clone() == GST::Penalise {
        is_transaction_pending(txn_detail.clone().status)
    } else {
        true
    };

    //let is_pm_and_pmt_present = Fbu::isTrueString(txn_card_info.paymentMethod) && txn_card_info.paymentMethodType.is_some();
    let should_update_srv3_gateway_score = if gateway_scoring_type.clone() == GST::Penalise {
        false
    } else {
        true
    };

    let is_update_within_window = is_update_within_latency_window(
        txn_detail.clone(),
        txn_card_info.clone(),
        gateway_scoring_type.clone(),
        mer_acc.clone(),
        txn_latency.clone(),
    )
    .await;

    let should_isolate_srv3_producer = if Cutover::is_feature_enabled(
        C::SrV3ProducerIsolation.get_key(),
        MID::merchant_id_to_text(txn_detail.clone().merchantId),
        C::kvRedis(),
    )
    .await
    {
        if is_routing_approach_in_srv3(routing_approach.clone()) {
            true
        } else {
            false
        }
    } else {
        true
    };

    let should_update_explore_txn = if Cutover::is_feature_enabled(
        DC::EnableExploreAndExploitOnSrv3(txn_card_info.clone().paymentMethodType.to_string())
            .get_key(),
        MID::merchant_id_to_text(txn_detail.clone().merchantId),
        C::kvRedis(),
    )
    .await
    {
        if is_routing_approach_in_explore(routing_approach.clone()) {
            true
        } else {
            false
        }
    } else {
        true
    };

    let redis_key = format!("{}{}", C::GATEWAY_SCORING_DATA, txn_detail.clone().txnUuid);
    let redis_gateway_score_data = if should_update_srv3_gateway_score
        && is_update_within_window
        && should_isolate_srv3_producer
        && should_update_explore_txn
    {
        logger::info!(
            action = "updateGatewayScore",
            tag = "updateGatewayScore",
            "Updating sr v3 score for the txn with scoring type as {:?} and status as {:?}",
            gateway_scoring_type,
            txn_detail.status
        );
        let app_state = get_tenant_app_state().await;
        let mb_gateway_scoring_data: Option<GatewayScoringData> = app_state
            .redis_conn
            .get_key(&redis_key, "GatewayScoringData")
            .await
            .ok();
        GSSV3::flow::update_sr_v3_score(
            gateway_scoring_type.clone(),
            txn_detail.clone(),
            txn_card_info.clone(),
            mer_acc.clone(),
            mb_gateway_scoring_data.clone(),
            gateway_reference_id.clone(),
        )
        .await;
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
                let redis_data: Option<GatewayScoringData> = app_state
                    .redis_conn
                    .get_key(&redis_key, "GatewayScoringData")
                    .await
                    .ok();
                redis_data
            }
            Some(_) => redis_gateway_score_data,
        };
        logger::info!(tag = "GatewayScoringData", "{:?}", mb_gateway_scoring_data);
        match mb_gateway_scoring_data {
            None => {
                logger::error!(
                    action = "GATEWAY_SCORING_DATA_NOT_FOUND_FOR_ELIMINATION",
                    tag = "GATEWAY_SCORING_DATA_NOT_FOUND_FOR_ELIMINATION",
                    "Gateway scoring data is not found in redis"
                );
            }
            Some(gateway_scoring_data) => {
                logger::info!(
                    action = "Downtime-EmailNotification",
                    tag = "Downtime-EmailNotification",
                    "Proceed to updateKeyScoreForKeysFromConsumer"
                );
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
                logger::info!(
                    action = "Downtime-EmailNotification",
                    tag = "Downtime-EmailNotification",
                    "{:?}",
                    key_array
                );
                for key in key_array {
                    tokio::spawn(GEF::updateKeyScoreForKeysFromConsumer(
                        txn_detail.clone(),
                        txn_card_info.clone(),
                        gateway_scoring_type.clone(),
                        mer_acc_p_id,
                        mer_acc.clone(),
                        key,
                    ));
                }
            }
        }
        Fbu::log_gateway_score_type(
            gateway_scoring_type,
            RF::EliminationFlow,
            txn_detail.clone(),
        );
    } else {
        if !should_update_gateway_score {
            logger::debug!(
                tag = "updateGatewayScore",
                action = "updateGatewayScore",
                "Gateway Scoring Type {:?} does not match with txn status {:?}",
                gateway_scoring_type,
                txn_detail.status
            );
        }
        // if !is_pm_and_pmt_present {
        // logger::debug!(
        //     tag = "updateGatewayScore",
        //     "Payment Method or Payment Method Type is null for txn_detail.id {}",
        //     txn_detail._id.as_deref().unwrap_or("")
        // );
        // }
        if !is_update_within_window {
            logger::debug!(
                tag = "updateGatewayScore",
                action = "updateGatewayScore",
                "Update GW Score call received outside Update Window"
            );
        }
    }
}

// Original Haskell function: getRoutingApproach
pub fn get_routing_approach(txn_detail: TxnDetail) -> Option<String> {
    let internal_meta: Option<FT::InternalMetadata> = get_value_from_meta_data(&txn_detail);
    match internal_meta {
        Some(meta) => Some(meta.internal_tracking_info.routing_approach),
        None => None,
    }
}

// Original Haskell function: getValueFromMetaData
pub fn get_value_from_meta_data<T: serde::de::DeserializeOwned>(txn_detail: &TxnDetail) -> Option<T> {
    let metadata = txn_detail.internalMetadata.clone()?;
    serde_json::from_str(metadata.peek()).ok()
}

// Original Haskell function: isRoutingApproachInSRV2
pub fn isRoutingApproachInSRV2(maybe_text: Option<String>) -> bool {
    match maybe_text {
        Some(text) => text.contains("V2"),
        None => false,
    }
}

// Original Haskell function: isRoutingApproachInSRV3
pub fn is_routing_approach_in_srv3(maybe_text: Option<String>) -> bool {
    match maybe_text {
        Some(text) => text.contains("V3"),
        None => false,
    }
}

// Original Haskell function: isRoutingApproachInExplore
pub fn is_routing_approach_in_explore(maybe_text: Option<String>) -> bool {
    match maybe_text {
        Some(text) => text.contains("HEDGING"),
        None => false,
    }
}

// Original Haskell function: isUpdateWithinLatencyWindow
pub async fn is_update_within_latency_window(
    txn_detail: TxnDetail,
    txn_card_info: TxnCardInfo,
    gateway_scoring_type: GatewayScoringType,
    mer_acc: MerchantAccount,
    txn_latency: Option<TransactionLatency>,
) -> bool {
    match gateway_scoring_type {
        GatewayScoringType::Penalise => true,
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
                // let m_auto_refund_conflict_threshold_in_mins: Option<i32> = None; // Placeholder for actual implementation
                let gw_latency_check_threshold =
                    findByNameFromRedis(C::GatewayScoreLatencyCheckInMins.get_key())
                        .await
                        .unwrap_or(C::defaultGatewayScoreLatencyCheckInMins());
                /// check if the transaction latency calculated by orchestration is within the configured threshold
                let is_gw_latency_within_threshold = isGwLatencyWithinConfiguredThreshold(
                    txn_latency.and_then(|m| m.gateway_latency),
                    GatewaySuccessRateBasedRoutingInput::from_str(
                        &mer_acc.gatewaySuccessRateBasedDeciderInput,
                    )
                    .ok()
                    .and_then(|m| m.txnLatency.and_then(|l| l.gatewayLatency)),
                );
                // Cutover::findByNameFromRedis(C.gatewayScoreLatencyCheckInMins)
                //     .await
                //     .unwrap_or(C.defaultGatewayScoreLatencyCheckInMins);
                let merchant_id = MID::merchant_id_to_text(txn_detail.merchantId.clone());
                let pmt = txn_card_info.paymentMethodType;
                let pm = GU::get_payment_method(
                    pmt,
                    txn_card_info.paymentMethod,
                    txn_detail.sourceObject.clone().unwrap_or_default(),
                );

                let gw_score_update_latency =
                    Fbu::get_time_from_txn_created_in_mills(txn_detail.clone());
                logger::info!(
                    action = "gwLatencyCheckThreshold",
                    tag = "gwLatencyCheckThreshold",
                    "gwLatencyCheckThreshold: {}",
                    gw_latency_check_threshold
                );
                if (gw_score_update_latency < gw_latency_check_threshold * 60000u128)
                    && is_gw_latency_within_threshold
                {
                    true
                } else {
                    false
                }
            }
        }
    }
}

async fn checkExemptIfMandateTxn(txn_detail: &TxnDetail, txn_card_info: &TxnCardInfo) -> bool {
    let is_recurring = isRecurringTxn(txn_detail.txnObjectType.clone());
    let is_nb_pmt = txn_card_info.paymentMethodType == (NB);
    let is_penny_reg_txn = isPennyMandateRegTxn(txn_detail.clone());
    is_recurring || (is_nb_pmt && is_penny_reg_txn)
}

// Original Haskell function: isTransactionPending
pub fn is_transaction_pending(txn_status: TxnStatus) -> bool {
    txn_status == TS::PendingVBV || txn_status == TS::Started
}
