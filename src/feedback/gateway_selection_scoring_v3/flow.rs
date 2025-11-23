// Automatically converted from Haskell to Rust
// Generated on 2025-03-23 12:02:17

// Converted imports
// use std::string::String as T;
// use feedback::utils::get_current_ist_date_with_format;
// use eulerhs::prelude::*;
// use std::vec::Vec;
// use std::vec::Vec;
// use gateway_decider::utils as GU;
// use gateway_decider::types::{RoutingFlowType, GatewayScoringData, ScoreKeyType};
// use utils::config::merchant_config as MC;
// use feedback::types::*;
// use feedback::utils as U;
// use feedback::utils::*;
// use std::string::String as TE;
// use eulerhs::tenant_redis_layer as RC;
// use utils::redis::cache as Cutover;
// use eulerhs::language as LogUtils;
// use control::monad::extra::maybe_m;
// use control::monad::except::run_except;
// use db::storage::types::merchant_account as MerchantAccount;
// use utils::redis as Redis;
// use feedback::utils::{log_gateway_score_type, get_producer_key};
// use gateway_decider::types as UpdateStatus;
// use feedback::utils::*;
// use utils::redis::feature::is_feature_enabled;
// use eulerhs::language as L;
// use serde_json as A;
// use std::vec::Vec as BSL;
// use feedback::types::{TxnCardInfo, PaymentMethodType, MerchantGatewayAccount};
use crate::decider::gatewaydecider::utils as GU;
use crate::logger;
use crate::redis::cache::findByNameFromRedis;
use crate::types::payment::payment_method_type_const::*;
use crate::{
    decider::gatewaydecider::types::{GatewayScoringData, SrRoutingDimensions},
    decider::{
        gatewaydecider::constants as DC, gatewaydecider::types::RoutingFlowType as RF,
        gatewaydecider::types::ScoreKeyType as SK, gatewaydecider::types::SrV3InputConfig,
    },
    feedback::{
        constants as C,
        types::SrV3DebugBlock,
        utils::{
            dateInIST, getCurrentIstDateWithFormat, getProducerKey, isKeyExistsRedis,
            log_gateway_score_type, updateMovingWindow, updateScore, GatewayScoringType,
        },
    },
    redis::types::ServiceConfigKey,
    types::{
        card::txn_card_info::TxnCardInfo, merchant::id as MID,
        merchant::merchant_account::MerchantAccount, txn_details::types::TxnDetail,
    },
};
use masking::PeekInterface;
use serde_json;

// Converted functions
// Original Haskell function: updateSrV3Score
pub async fn update_sr_v3_score(
    gateway_scoring_type: GatewayScoringType,
    txn_detail: TxnDetail,
    txn_card_info: TxnCardInfo,
    merchant_acc: MerchantAccount,
    mb_gateway_scoring_data: Option<GatewayScoringData>,
    gateway_reference_id: Option<String>,
) {
    // let is_merchant_enabled_globally = MC::isMerchantEnabledForPaymentFlows(merchant_acc.id, [PF::SrBasedRouting].to_vec()).await;
    match txn_detail.gateway.clone() {
        None => {
            logger::debug!(
                action = "gateway not found",
                tag = "gateway not found",
                "gateway not found for this transaction having id"
            );
        }
        Some(gateway) => {
            let unified_sr_v3_key = getProducerKey(
                txn_detail.clone(),
                mb_gateway_scoring_data,
                SK::SrV3Key,
                false,
                gateway_reference_id.clone(),
            )
            .await;
            let key_for_gateway_selection =
                unified_sr_v3_key.clone().unwrap_or_else(|| "".to_string());
            let payment_method_type = txn_card_info.paymentMethodType.clone();
            let key_for_gateway_selection_queue =
                format!("{}_{}queue", key_for_gateway_selection, "}");
            let key_for_gateway_selection_score =
                format!("{}_{}score", key_for_gateway_selection, "}");
            updateScoreAndQueue(
                key_for_gateway_selection_queue,
                key_for_gateway_selection_score,
                gateway_scoring_type.clone(),
                txn_detail.clone(),
                txn_card_info.clone(),
            )
            .await;
            if [CARD, UPI].contains(&payment_method_type.as_str()) {
                let key3d_for_gateway_selection =
                    unified_sr_v3_key.clone().unwrap_or_else(|| "".to_string());
                if key3d_for_gateway_selection != key_for_gateway_selection {
                    logger::debug!(
                        tag = "SR V3 Based threeD Producer Key",
                        action = "SR V3 Based threeD Producer Key",
                        "{:?}",
                        key3d_for_gateway_selection
                    );
                    let key3d_for_gateway_selection_queue =
                        format!("{}_{}queue", key3d_for_gateway_selection, "}");
                    let key3d_for_gateway_selection_score =
                        format!("{}_{}score", key3d_for_gateway_selection, "}");
                    updateScoreAndQueue(
                        key3d_for_gateway_selection_queue,
                        key3d_for_gateway_selection_score,
                        gateway_scoring_type.clone(),
                        txn_detail.clone(),
                        txn_card_info.clone(),
                    )
                    .await;
                }
            }
            log_gateway_score_type(gateway_scoring_type, RF::Srv3Flow, txn_detail);
        }
    }
}

// Original Haskell function: createKeysIfNotExist
pub async fn createKeysIfNotExist(
    key_for_gateway_selection_queue: String,
    key_for_gateway_selection_score: String,
    txn_detail: TxnDetail,
    txn_card_info: TxnCardInfo,
) {
    let is_queue_key_exists = isKeyExistsRedis(key_for_gateway_selection_queue.clone()).await;
    let is_score_key_exists = isKeyExistsRedis(key_for_gateway_selection_score.clone()).await;
    logger::debug!(
        tag = "createKeysIfNotExist",
        action = "createKeysIfNotExist",
        "Value for isQueueKeyExists is {} and isScoreKeyExists is {}",
        is_queue_key_exists,
        is_score_key_exists
    );
    if is_queue_key_exists && is_score_key_exists {
        return;
    } else {
        let merchant_bucket_size = getSrV3MerchantBucketSize(txn_detail, txn_card_info).await;
        logger::debug!(
            tag = "createKeysIfNotExist",
            action = "createKeysIfNotExist",
            "Creating keys with bucket size as {}",
            merchant_bucket_size
        );
        let score_list = vec!["1".to_string(); merchant_bucket_size.clone().try_into().unwrap()];
        let redis = C::kvRedis();
        GU::create_moving_window_and_score(
            redis,
            key_for_gateway_selection_queue,
            key_for_gateway_selection_score,
            merchant_bucket_size,
            score_list,
        )
        .await;
    }
}

// Original Haskell function: updateScoreAndQueue
pub async fn updateScoreAndQueue(
    key_for_gateway_selection_queue: String,
    key_for_gateway_selection_score: String,
    gateway_scoring_type: GatewayScoringType,
    txn_detail: TxnDetail,
    txn_card_info: TxnCardInfo,
) {
    logger::debug!(
        action = "updateScoreAndQueue",
        tag = "updateScoreAndQueue",
        "Updating sr v3 score and queue"
    );
    createKeysIfNotExist(
        key_for_gateway_selection_queue.clone(),
        key_for_gateway_selection_score.clone(),
        txn_detail.clone(),
        txn_card_info,
    )
    .await;
    let (value, should_score_increase): (String, bool) = match gateway_scoring_type {
        GatewayScoringType::PenaliseSrv3 => ("0".into(), false),
        GatewayScoringType::Reward => ("1".into(), true),
        _ => ("0".into(), false),
    };
    // let is_debug_mode_enabled = isFeatureEnabled(
    //     DC::enableDebugModeOnSrV3.get_key(),
    //     MID::merchant_id_to_text(txn_detail.merchantId),
    //     C::kvRedis(),
    // ).await;
    // if is_debug_mode_enabled {
    //     let _ = RC::rHDelB(
    //         C::kvRedis,
    //         &format!(
    //             "{}{}",
    //             C::pendingTxnsKeyPrefix,
    //             txn_detail.merchantId.clone()
    //         ),
    //         &[txn_detail.txnUuid.clone()],
    //     );
    // } else {
    //     match app::get_tenant_app_state()
    //         .await
    //         .redis_conn
    //         .conn
    //         .delete_key(&[format!(
    //             "{}{}",
    //             C::pendingTxnsKeyPrefix,
    //             txn_detail.merchantId.clone()
    //         )])
    //         .await
    //     {
    //         Ok(res) => (),
    //         Err(err) => {
    //             // Log an error if there's an issue deleting the score key
    //             // L::log_error_v(
    //             //     "deleteScoreKeyIfBucketSizeChanges",
    //             //     "Error while deleting score key in redis",
    //             //     err
    //             // ).await;
    //             ()
    //         }
    //     }
    // }
    let current_ist_time = getCurrentIstDateWithFormat("YYYY-MM-DD HH:mm:SS.sss".to_string());
    let date_created = dateInIST(
        txn_detail.clone().dateCreated.to_string(),
        "YYYY-MM-DD HH:mm:SS.sss".to_string(),
    )
    .unwrap_or_default();
    // let updated_value = if is_debug_mode_enabled {
    //     TE::decodeUtf8(&BSL::toStrict(&A::encode(&debugBlock(
    //         txn_detail.clone(),
    //         current_ist_time.clone(),
    //         date_created.clone(),
    //         value.clone(),
    //     ))))
    //     .unwrap()
    // } else {
    //     value.clone()
    // };
    let popped_status = updateMovingWindow(
        C::kvRedis(),
        key_for_gateway_selection_queue.clone(),
        key_for_gateway_selection_score.clone(),
        value.clone(),
    )
    .await;
    logger::debug!(
        action = "updateScoreAndQueue",
        tag = "updateScoreAndQueue",
        "Popped Redis Value {}",
        popped_status
    );
    let returned_value =
        match serde_json::from_slice::<Option<SrV3DebugBlock>>(popped_status.as_bytes()) {
            Ok(maybe_popped_status_block) => get_status(maybe_popped_status_block, popped_status),
            Err(_) => popped_status,
        };
    logger::debug!(
        action = "updateScoreAndQueue",
        tag = "updateScoreAndQueue",
        "Popped Returned Value {}",
        returned_value
    );
    if returned_value == value {
        return;
    } else {
        updateScore(
            C::kvRedis(),
            key_for_gateway_selection_score.clone(),
            should_score_increase,
        )
        .await;
    }
}

//Original Haskell function: getSrV3MerchantBucketSize
pub async fn getSrV3MerchantBucketSize(txn_detail: TxnDetail, txn_card_info: TxnCardInfo) -> i32 {
    let merchant_sr_v3_input_config: Option<SrV3InputConfig> = findByNameFromRedis(
        C::SrV3InputConfig(MID::merchant_id_to_text(txn_detail.merchantId)).get_key(),
    )
    .await;
    let pmt = txn_card_info.paymentMethodType;
    let pm = GU::get_payment_method(
        (&pmt).to_string(),
        txn_card_info.paymentMethod,
        txn_detail.sourceObject.unwrap_or_default(),
    );
    // Extract the new parameters from txn_card_info

    let sr_routing_dimesions = SrRoutingDimensions {
        card_network: txn_card_info
            .cardSwitchProvider
            .as_ref()
            .map(|s| s.peek().to_string()),
        card_isin: txn_card_info.card_isin,
        currency: Some(txn_detail.currency.to_string()),
        country: txn_detail.country.as_ref().map(|c| c.to_string()),
        auth_type: txn_card_info.authType.as_ref().map(|a| a.to_string()),
    };

    let maybe_bucket_size = GU::get_sr_v3_bucket_size(
        merchant_sr_v3_input_config,
        &pmt,
        &pm,
        &sr_routing_dimesions,
    );
    let merchant_bucket_size = match maybe_bucket_size {
        None => {
            let default_sr_v3_input_config: Option<SrV3InputConfig> =
                findByNameFromRedis(DC::SR_V3_DEFAULT_INPUT_CONFIG.get_key()).await;
            GU::get_sr_v3_bucket_size(default_sr_v3_input_config, &pmt, &pm, &sr_routing_dimesions)
                .unwrap_or(C::DEFAULT_SR_V3_BASED_BUCKET_SIZE)
        }
        Some(bucket_size) => bucket_size,
    };
    logger::debug!(
        action = "sr_v3_bucket_size",
        tag = "sr_v3_bucket_size",
        "Bucket Size: {}",
        merchant_bucket_size
    );
    merchant_bucket_size
}

fn get_status(maybe_popped_status_block: Option<SrV3DebugBlock>, default_status: String) -> String {
    maybe_popped_status_block
        .map(|block| block.txn_status)
        .unwrap_or(default_status)
}
