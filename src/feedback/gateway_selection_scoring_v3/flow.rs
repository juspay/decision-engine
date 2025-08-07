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
use crate::euclid::errors::EuclidErrors;
use crate::logger;
use crate::merchant_config_util as MC;
use crate::redis::cache::findByNameFromRedis;
use crate::types::payment::payment_method_type_const::*;
use crate::types::service_configuration::find_config_by_name;
use crate::{
    app,
    decider::gatewaydecider::types::{GatewayScoringData, SrRoutingDimensions},
    decider::{
        gatewaydecider::constants as DC, gatewaydecider::types::RoutingFlowType as RF,
        gatewaydecider::types::ScoreKeyType as SK, gatewaydecider::types::SrV3InputConfig,
    },
    feedback::{
        constants as C,
        types::SrV3DebugBlock,
        utils::{
            dateInIST, findKeysByPattern, getCurrentIstDateWithFormat, getProducerKey, getScore,
            getScoreList, getTrueString, isKeyExistsRedis, logGatewayScoreType, updateMovingWindow,
            updateScore, GatewayScoringType,
        },
    },
    redis::{feature::isFeatureEnabled, types::ServiceConfigKey},
    types::{
        card::txn_card_info::TxnCardInfo,
        merchant::id as MID,
        merchant::{
            merchant_account::MerchantAccount, merchant_gateway_account::MerchantGatewayAccount,
        },
        payment_flow::PaymentFlow as PF,
        txn_details::types::TxnDetail,
    },
    utils as U,
};
use error_stack::ResultExt;
use masking::PeekInterface;

// Converted functions
// Original Haskell function: updateSrV3Score
pub async fn updateSrV3Score(
    gateway_scoring_type: GatewayScoringType,
    txn_detail: TxnDetail,
    txn_card_info: TxnCardInfo,
    merchant_acc: MerchantAccount,
    mb_gateway_scoring_data: Option<GatewayScoringData>,
    gateway_reference_id: Option<String>,
) {
    // let is_merchant_enabled_globally = MC::isMerchantEnabledForPaymentFlows(merchant_acc.id, [PF::SR_BASED_ROUTING].to_vec()).await;
    match (txn_detail.gateway.clone()) {
        (None) => {
            logger::info!(
                action = "gateway not found",
                tag = "gateway not found",
                "gateway not found for this transaction having id"
            );
        }
        (Some(gateway)) => {
            let unified_sr_v3_key = getProducerKey(
                txn_detail.clone(),
                mb_gateway_scoring_data,
                SK::SR_V3_KEY,
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
                    logger::info!(
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
            logGatewayScoreType(gateway_scoring_type, RF::SRV3_FLOW, txn_detail);
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
    logger::info!(
        tag = "createKeysIfNotExist",
        action = "createKeysIfNotExist",
        "Value for isQueueKeyExists is {} and isScoreKeyExists is {}",
        is_queue_key_exists,
        is_score_key_exists
    );
    if is_queue_key_exists && is_score_key_exists {
        return;
    } else {
        println!(
            ">>>>Creating keys as they do not exist in Redis: key_for_gateway_selection_queue:{} and key_for_gateway_selection_score: {}",
            key_for_gateway_selection_queue, key_for_gateway_selection_score
        );
        let merchant_bucket_size =
            getSrV3MerchantBucketSize(txn_detail.clone(), txn_card_info).await;
        logger::info!(
            tag = "createKeysIfNotExist",
            action = "createKeysIfNotExist",
            "Creating keys with bucket size as {}",
            merchant_bucket_size
        );
        //here check if longest possible subset is present in the redis
        const PREFIX: &str = "{gw_sr_v3_score__";
        const SUFFIX: &str = "_}queue";

        let mut processed_string = key_for_gateway_selection_queue.clone();
        processed_string = processed_string[PREFIX.len()..].to_string();
        processed_string = processed_string[..(processed_string.len() - SUFFIX.len())].to_string();

        let first_delimiter_idx = processed_string.find("__");
        let last_delimiter_idx = processed_string.rfind("__");

        let mid = MID::merchant_id_to_text(txn_detail.merchantId);
        let name = format!("SR_DIMENSION_CONFIG_{}", mid);

        let service_config = find_config_by_name(name.clone())
            .await
            .change_context(EuclidErrors::StorageError);

        let enable_global_info = match service_config {
            Ok(Some(config)) => match config.value {
                Some(json_value) => match serde_json::from_str::<serde_json::Value>(&json_value) {
                    Ok(parsed_json) => parsed_json
                        .get("enable_global_info")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    Err(e) => {
                        logger::error!(
                            tag = "createKeysIfNotExist",
                            action = "parse_service_config_json",
                            "Failed to parse service config JSON: {}",
                            e
                        );
                        false
                    }
                },
                None => {
                    logger::warn!(
                        tag = "createKeysIfNotExist",
                        action = "service_config_value_missing",
                        "Service config value is None for name: {}",
                        name
                    );
                    false
                }
            },
            Ok(None) => {
                logger::warn!(
                    tag = "createKeysIfNotExist",
                    action = "service_config_not_found",
                    "Service config not found for name: {}",
                    name
                );
                false
            }
            Err(e) => {
                logger::error!(
                    tag = "createKeysIfNotExist",
                    action = "service_config_fetch_error",
                    "Error fetching service config: {:?}",
                    e
                );
                false
            }
        };

        let sr_dimensions_key = processed_string[first_delimiter_idx.unwrap_or(0)..].to_string();

        let gateway = processed_string[last_delimiter_idx.unwrap_or(0)..].to_string();

        // Loop through sr_dimensions_key from back, removing chars till we find "__"
        let mut current_key = sr_dimensions_key.clone();
        let gateway_name = gateway.clone();

        loop {
            if let Some(last_delimiter_pos) = current_key.rfind("__") {
                // Remove everything after the last "__"
                current_key = current_key[..last_delimiter_pos].to_string();

                let key_to_check = format!("{}{}", current_key, gateway_name);

                let mut queue_key_pattern = format!("{}{}{}{}", PREFIX, mid, key_to_check, SUFFIX);
                let mut score_key_pattern =
                    format!("{}{}{}{}score", PREFIX, mid, key_to_check, "_}");

                if enable_global_info == true {
                    queue_key_pattern = format!("{}*{}{}", PREFIX, key_to_check, SUFFIX);
                    score_key_pattern = format!("{}*{}{}score", PREFIX, key_to_check, "_}");
                }

                logger::info!(
                    tag = "createKeysIfNotExist",
                    action = "checking_parent_key_pattern",
                    "Checking if parent key exists with pattern: {}",
                    queue_key_pattern
                );

                // Use pattern matching to find keys
                let found_queue_keys = findKeysByPattern(&queue_key_pattern).await;
                let found_score_keys = findKeysByPattern(&score_key_pattern).await;

                if !found_queue_keys.is_empty() && !found_score_keys.is_empty() {
                    // Key exists in Redis, use the first matching key
                    let first_queue_key = &found_queue_keys[0];
                    let first_score_key = &found_score_keys[0];

                    logger::info!(
                        tag = "createKeysIfNotExist",
                        action = "parent_key_found",
                        "Found existing subset keys in Redis - Queue: {}, Score: {}",
                        first_queue_key,
                        first_score_key
                    );

                    println!(
                        ">>>>found subset keys in Redis - Queue: {}, Score: {}",
                        first_queue_key, first_score_key
                    );

                    // Implement logic to copy/inherit from subset key

                    let mut subset_score_list = getScoreList(first_queue_key.clone()).await;
                    let mut new_score = getScore(first_score_key.clone()).await;

                    subset_score_list.reverse();
                    let subset_bucket_size = subset_score_list.len() as i32;

                    println!(
                        ">>>>subset bucket size: {}, subset score list: {:?}",
                        subset_bucket_size, subset_score_list
                    );

                    logger::info!(
                        tag = "createKeysIfNotExist",
                        action = "parent_bucket_info",
                        "subset bucket size: {}, Our bucket size: {}, score of the subset: {}",
                        subset_bucket_size,
                        merchant_bucket_size,
                        new_score
                    );

                    let final_score_list = if merchant_bucket_size == subset_bucket_size {
                        subset_score_list
                    } else if merchant_bucket_size < subset_bucket_size {
                        let start_idx = subset_score_list
                            .len()
                            .saturating_sub(merchant_bucket_size as usize);
                        subset_score_list[start_idx..].to_vec()
                    } else {
                        let mut extended_list = subset_score_list;
                        let additional_ones = merchant_bucket_size - subset_bucket_size;
                        for _ in 0..additional_ones {
                            extended_list.push("1".to_string());
                        }
                        extended_list
                    };

                    logger::info!(
                        tag = "createKeysIfNotExist",
                        action = "creating_keys_with_parent_data",
                        "Creating keys with inherited score list of length: {}",
                        final_score_list.len()
                    );
                    new_score = final_score_list
                        .iter()
                        .filter_map(|s| s.parse::<i32>().ok())
                        .sum();
                    let redis = C::kvRedis();
                    GU::create_moving_window_and_score(
                        redis,
                        key_for_gateway_selection_queue,
                        key_for_gateway_selection_score,
                        new_score,
                        final_score_list,
                    )
                    .await;

                    return;
                } else {
                    // Key doesn't exist, continue to next iteration
                    logger::info!(
                        tag = "createKeysIfNotExist",
                        action = "parent_key_not_found",
                        "Parent key not found in Redis with pattern: {}, continuing search",
                        queue_key_pattern
                    );
                    println!(
                        ">>>>Not found Parent key in Redis with pattern: {}",
                        queue_key_pattern
                    );
                }

                // If we've removed everything except the first "__", break
                if current_key.matches("__").count() <= 1 {
                    logger::info!(
                        tag = "createKeysIfNotExist",
                        action = "no_parent_key_found",
                        "No parent key found in Redis hierarchy"
                    );
                    let score_list =
                        vec!["1".to_string(); merchant_bucket_size.clone().try_into().unwrap()];
                    let redis = C::kvRedis();
                    GU::create_moving_window_and_score(
                        redis,
                        key_for_gateway_selection_queue,
                        key_for_gateway_selection_score,
                        merchant_bucket_size,
                        score_list,
                    )
                    .await;
                    break;
                }
            } else {
                // No more "__" found, break the loop
                logger::info!(
                    tag = "createKeysIfNotExist",
                    action = "no_more_delimiters",
                    "No more delimiters found in key"
                );
                let score_list =
                    vec!["1".to_string(); merchant_bucket_size.clone().try_into().unwrap()];
                let redis = C::kvRedis();
                GU::create_moving_window_and_score(
                    redis,
                    key_for_gateway_selection_queue,
                    key_for_gateway_selection_score,
                    merchant_bucket_size,
                    score_list,
                )
                .await;
                break;
            }
        }
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
    logger::info!(
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
        GatewayScoringType::PENALISE_SRV3 => ("0".into(), false),
        GatewayScoringType::REWARD => ("1".into(), true),
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
    logger::info!(
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
    logger::info!(
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

fn debugBlock(
    txn_detail: TxnDetail,
    current_time: String,
    date_created: String,
    value: String,
) -> SrV3DebugBlock {
    SrV3DebugBlock {
        txn_uuid: txn_detail.txnUuid,
        order_id: txn_detail.orderId.0,
        date_created,
        current_time,
        txn_status: value,
    }
}

fn getStatus(maybe_popped_status_block: Option<SrV3DebugBlock>, popped_status: String) -> String {
    match maybe_popped_status_block {
        Some(popped_status_block) => popped_status_block.txn_status.clone(),
        None => popped_status,
    }
}

//Original Haskell function: getSrV3MerchantBucketSize
pub async fn getSrV3MerchantBucketSize(txn_detail: TxnDetail, txn_card_info: TxnCardInfo) -> i32 {
    let merchant_sr_v3_input_config: Option<SrV3InputConfig> = findByNameFromRedis(
        C::SR_V3_INPUT_CONFIG(MID::merchant_id_to_text(txn_detail.merchantId)).get_key(),
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
                findByNameFromRedis(DC::srV3DefaultInputConfig.get_key()).await;
            GU::get_sr_v3_bucket_size(default_sr_v3_input_config, &pmt, &pm, &sr_routing_dimesions)
                .unwrap_or(C::defaultSrV3BasedBucketSize)
        }
        Some(bucket_size) => bucket_size,
    };
    logger::info!(
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
