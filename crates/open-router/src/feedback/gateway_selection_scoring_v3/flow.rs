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
use crate::{
    app,
    decider::gatewaydecider::types::GatewayScoringData,
    feedback::{
        constants as C,
        types::SrV3DebugBlock,
        utils::{
            dateInIST, getCurrentIstDateWithFormat,
            getProducerKey, getTrueString, isKeyExistsRedis, logGatewayScoreType,
            GatewayScoringType,updateMovingWindow, updateScore,
        },
       
    },
    redis::{feature::isFeatureEnabled, types::ServiceConfigKey},
    types::{
        card::txn_card_info::TxnCardInfo,
        payment_flow::PaymentFlow as PF,
        merchant::{
            merchant_account::MerchantAccount, merchant_gateway_account::MerchantGatewayAccount,
        },
        payment::payment_method::PaymentMethodType as PMT,
        txn_details::types::TxnDetail,
        merchant::id as MID
    },
    decider::{
        gatewaydecider::constants as DC,
        gatewaydecider::types::{SrV3InputConfig},
        gatewaydecider::types::ScoreKeyType as SK,
        gatewaydecider::types::RoutingFlowType as RF,
    },
    utils as U,
};
use crate::redis::cache::findByNameFromRedis;
use crate::merchant_config_util as MC;


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
    let is_merchant_enabled_globally = MC::isMerchantEnabledForPaymentFlows(merchant_acc.id, [PF::SR_BASED_ROUTING].to_vec()).await;
    if is_merchant_enabled_globally {
        let cutover_result = isFeatureEnabled(
            C::SR_V3_BASED_FLOW_CUTOVER.get_key(),
            MID::merchant_id_to_text(txn_detail.merchantId.clone()),
            C::kvRedis(),
        ).await;
        if cutover_result {
                match (txn_detail.gateway.clone()) {
                    (None) => {
                        // LogUtils::logInfoT(
                        //     "gateway not found",
                        //     "gateway not found for this transaction having id",
                        // );
                    }
                    (Some(gateway)) => {
                        let unified_sr_v3_key =
                            getProducerKey(txn_detail.clone(), mb_gateway_scoring_data, SK::SR_V3_KEY, false, gateway_reference_id.clone())
                                .await;
                        let key_for_gateway_selection = unified_sr_v3_key.clone().unwrap_or_else(|| "".to_string());
                        let payment_method_type = txn_card_info.paymentMethodType.clone();
                        // LogUtils::logInfoT(
                        //     "SR V3 Based oneD Producer Key",
                        //     &key_for_gateway_selection,
                        // );
                        let key_for_gateway_selection_queue =
                            format!("{}_{}queue", key_for_gateway_selection, "_");
                        let key_for_gateway_selection_score =
                            format!("{}_{}score", key_for_gateway_selection, "_");
                        updateScoreAndQueue(
                            key_for_gateway_selection_queue,
                            key_for_gateway_selection_score,
                            gateway_scoring_type.clone(),
                            txn_detail.clone(),
                            txn_card_info.clone(),
                        );
                        if [PMT::Card, PMT::UPI].contains(&payment_method_type) {
                            let key3d_for_gateway_selection =
                                unified_sr_v3_key.clone().unwrap_or_else(|| "".to_string());
                            if key3d_for_gateway_selection != key_for_gateway_selection {
                                // LogUtils::logInfoT(
                                //     "SR V3 Based threeD Producer Key",
                                //     &key3d_for_gateway_selection,
                                // );
                                let key3d_for_gateway_selection_queue =
                                    format!("{}_{}queue", key3d_for_gateway_selection, "_");
                                let key3d_for_gateway_selection_score =
                                    format!("{}_{}score", key3d_for_gateway_selection, "_");
                                updateScoreAndQueue(
                                    key3d_for_gateway_selection_queue,
                                    key3d_for_gateway_selection_score,
                                    gateway_scoring_type.clone(),
                                    txn_detail.clone(),
                                    txn_card_info.clone(),
                                );
                            }
                        }
                        logGatewayScoreType(gateway_scoring_type, RF::SRV3_FLOW, txn_detail);
                    }
                }
            
        } else {
            // LogUtils::logInfoT(
            //     "updateSrV3Score",
            //     &format!(
            //         "SR V3 based gateway flow cutover is not enabled for {}",
            //         txn_detail.merchantId.merchantId
            //     ),
            // );
        }
    }
}

// Original Haskell function: getKeyForGatewaySelection
// pub fn getKeyForGatewaySelection(
//     payment_method_type: PaymentMethodType,
//     merchant_id: String,
//     gateway_name: String,
//     txn_detail: TxnDetail,
//     txn_card_info: TxnCardInfo,
// ) -> String {
//     let two_d_key = {
//         let order_type = txn_detail.txnObjectType;
//         let base_key = vec![
//             C.gatewaySelectionV3OrderTypeKeyPrefix.to_string(),
//             merchant_id.clone(),
//             order_type.to_string(),
//             payment_method_type.to_string(),
//         ];

//         match payment_method_type {
//             PaymentMethodType::Upi => {
//                 let source_object = vec![txn_card_info
//                     .paymentMethod
//                     .as_ref()
//                     .filter(|pm| pm == &&"UPI".to_string())
//                     .map_or_else(|| txn_detail.sourceObject.clone(), |pm| pm.clone())
//                     .unwrap_or_default()];
//                 GU::intercalateWithoutEmptyString("_", &[base_key, source_object].concat())
//             }
//             PaymentMethodType::Card => {
//                 let card_key = vec![
//                     txn_card_info.paymentMethod.clone(),
//                     txn_card_info.card_type.clone(),
//                 ];
//                 GU::intercalateWithoutEmptyString("_", &[base_key, card_key].concat())
//             }
//             _ => {
//                 let inter_key = vec![txn_card_info.paymentMethod.clone()];
//                 GU::intercalateWithoutEmptyString("_", &[base_key, inter_key].concat())
//             }
//         }
//     };

//     let gri_s_rv2_cutover = isFeatureEnabled(
//         C.isGriEnabledMerchantSRv2Producer,
//         txn_detail.merchantId.merchantId.clone(),
//         C.kvRedis,
//     );

//     if gri_s_rv2_cutover {
//         let merchant_gateway_account: Option<MerchantGatewayAccount> = maybeM(
//             async { None },
//             |mga_id| async { None }, // Replace with actual implementation
//             async { txn_detail.merchantGatewayAccountId.clone() },
//         );

//         let gw_ref_id = merchant_gateway_account
//             .as_ref()
//             .and_then(|mga| mga.referenceId.clone())
//             .unwrap_or_else(|| "NULL".to_string());

//         format!("{}_{}_{}", two_d_key, gw_ref_id, gateway_name)
//     } else {
//         format!("{}_{}", two_d_key, gateway_name)
//     }
// }

// Original Haskell function: get3DKeyForGatewaySelection
// pub fn get3DKeyForGatewaySelection(
//     payment_method_type: PaymentMethodType,
//     merchant_id: String,
//     gateway_name: String,
//     txn_detail: TxnDetail,
//     txn_card_info: TxnCardInfo,
// ) -> String {
//     let order_type = txn_detail.txnObjectType;
//     let base_key = vec![
//         C.gatewaySelectionV3OrderTypeKeyPrefix,
//         merchant_id.clone(),
//         order_type.to_string(),
//         payment_method_type.to_string(),
//     ];
//     let m_source_object = if txn_card_info.paymentMethod == "UPI".to_string() {
//         txn_detail.sourceObject.clone()
//     } else {
//         Some(txn_card_info.paymentMethod.clone())
//     };

//     let three_d_key = match payment_method_type {
//         PaymentMethodType::Card => {
//             let card_key = vec![
//                 txn_card_info.paymentMethod.clone(),
//                 txn_card_info.card_type.clone(),
//             ];
//             let auth_type_sr_routing_producer_enabled = isFeatureEnabled(
//                 C.authTypeSrRoutingProducerEnabledMerchant,
//                 txn_detail.merchantId.clone(),
//                 C.kvRedis,
//             );
//             let bank_level_sr_routing_producer_enabled = isFeatureEnabled(
//                 C.bankLevelSrRoutingProducerEnabledMerchant,
//                 txn_detail.merchantId.clone(),
//                 C.kvRedis,
//             );
//             if auth_type_sr_routing_producer_enabled {
//                 GU::intercalateWithoutEmptyString(
//                     "_",
//                     &[
//                         base_key.clone(),
//                         card_key.clone(),
//                         vec![txn_card_info.authType.clone().unwrap_or_default()],
//                     ]
//                     .concat(),
//                 )
//             } else if bank_level_sr_routing_producer_enabled {
//                 let top_bank_list = GU::getRoutingTopBankList();
//                 let maybe_bank_code = fetchJuspayBankCodeFromPaymentSource(&txn_card_info)
//                     .unwrap_or_else(|| "UNKNOWN".to_string());
//                 let append_bank_code = if top_bank_list.contains(&maybe_bank_code) {
//                     maybe_bank_code
//                 } else {
//                     "".to_string()
//                 };
//                 GU::intercalateWithoutEmptyString(
//                     "_",
//                     &[base_key.clone(), card_key.clone(), vec![append_bank_code]].concat(),
//                 )
//             } else {
//                 GU::intercalateWithoutEmptyString(
//                     "_",
//                     &base_key
//                         .iter()
//                         .chain(card_key.iter())
//                         .cloned()
//                         .collect::<Vec<_>>(),
//                 )
//             }
//         }
//         PaymentMethodType::UPI => {
//             if matches!(
//                 m_source_object.as_deref(),
//                 Some("UPI_COLLECT") | Some("COLLECT")
//             ) {
//                 let handle_list = GU::getUPIHandleList();
//                 let source_object = m_source_object.unwrap_or_default();
//                 let upi_handle = txn_card_info
//                     .paymentSource
//                     .as_ref()
//                     .and_then(|ps| getTrueString(ps))
//                     .map(|ps| {
//                         T::split_on(&ps, "@")
//                             .last()
//                             .map(|s| s.to_uppercase())
//                             .unwrap_or_default()
//                     })
//                     .unwrap_or_default();
//                 let append_handle = if handle_list.contains(&upi_handle) {
//                     upi_handle
//                 } else {
//                     "".to_string()
//                 };
//                 GU::intercalateWithoutEmptyString(
//                     "_",
//                     &[base_key.clone(), vec![source_object, append_handle]].concat(),
//                 )
//             } else if matches!(m_source_object.as_deref(), Some("UPI_PAY") | Some("PAY")) {
//                 let source_object = m_source_object.unwrap_or_default();
//                 let psp_app_sr_routing_producer_enabled = isFeatureEnabled(
//                     C.pspAppSrRoutingProducerEnabledMerchant,
//                     txn_detail.merchantId.clone(),
//                     C.kvRedis,
//                 );
//                 let psp_package_sr_routing_producer_enabled = isFeatureEnabled(
//                     C.pspPackageSrRoutingProducerEnabledMerchant,
//                     txn_detail.merchantId.clone(),
//                     C.kvRedis,
//                 );
//                 if psp_app_sr_routing_producer_enabled {
//                     let psp_list = GU::getUPIPspList();
//                     let juspay_bank_code = getJuspayBankCodeFromInternalMetadata(&txn_detail)
//                         .unwrap_or_else(|| "UNKNOWN".to_string());
//                     let append_psp_bank_code = if psp_list.contains(&juspay_bank_code) {
//                         juspay_bank_code
//                     } else {
//                         "".to_string()
//                     };
//                     GU::intercalateWithoutEmptyString(
//                         "_",
//                         &[base_key.clone(), vec![source_object, append_psp_bank_code]].concat(),
//                     )
//                 } else if psp_package_sr_routing_producer_enabled {
//                     let package_list = GU::getUPIPackageList();
//                     let upi_package = txn_card_info
//                         .paymentSource
//                         .as_ref()
//                         .and_then(|ps| getTrueString(ps))
//                         .map(|ps| ps.to_uppercase())
//                         .unwrap_or_default();
//                     let append_package = if package_list.contains(&upi_package) {
//                         upi_package
//                     } else {
//                         "".to_string()
//                     };
//                     GU::intercalateWithoutEmptyString(
//                         "_",
//                         &[base_key.clone(), vec![source_object, append_package]].concat(),
//                     )
//                 } else {
//                     GU::intercalateWithoutEmptyString(
//                         "_",
//                         &[base_key.clone(), vec![source_object]].concat(),
//                     )
//                 }
//             } else {
//                 GU::intercalateWithoutEmptyString(
//                     "_",
//                     &[
//                         base_key.clone(),
//                         vec![txn_detail.sourceObject.clone().unwrap_or_default()],
//                     ]
//                     .concat(),
//                 )
//             }
//         }
//         _ => GU::intercalateWithoutEmptyString(
//             "_",
//             &[
//                 base_key.clone(),
//                 vec![txn_card_info.paymentMethod.clone().unwrap_or_default()],
//             ]
//             .concat(),
//         ),
//     };

//     let gri_sr_v2_cutover = isFeatureEnabled(
//         C.isGriEnabledMerchantSRv2Producer,
//         txn_detail.merchantId.clone().unwrap_or_default(),
//         C.kvRedis,
//     );
//     if gri_sr_v2_cutover {
//         let merchant_gateway_account =
//             txn_detail
//                 .merchantGatewayAccountId
//                 .as_ref()
//                 .and_then(|mga_id| {
//                     // Assuming getMerchantGatewayAccountFromId is an async function
//                     getMerchantGatewayAccountFromId(
//                         mga_id,
//                         OptionalParameters {
//                             disableDecryption: None,
//                         },
//                     )
//                     .ok()
//                 });
//         let gw_ref_id = merchant_gateway_account
//             .as_ref()
//             .and_then(|mga| mga.referenceId.clone())
//             .unwrap_or_else(|| "NULL".to_string());
//         T::intercalate("_", &[three_d_key, gw_ref_id, gateway_name])
//     } else {
//         T::intercalate("_", &[three_d_key, gateway_name])
//     }
// }

// Original Haskell function: createKeysIfNotExist
pub async fn createKeysIfNotExist(
    key_for_gateway_selection_queue: String,
    key_for_gateway_selection_score: String,
    txn_detail: TxnDetail,
    txn_card_info: TxnCardInfo,
) {
    let is_queue_key_exists = isKeyExistsRedis(key_for_gateway_selection_queue.clone()).await;
    let is_score_key_exists = isKeyExistsRedis(key_for_gateway_selection_score.clone()).await;
    // LogUtils::logInfoT(
    //     "createKeysIfNotExist",
    //     &format!(
    //         "Value for isQueueKeyExists is {} and isScoreKeyExists is {}",
    //         is_queue_key_exists, is_score_key_exists
    //     ),
    // );
    if is_queue_key_exists && is_score_key_exists {
        return;
    } else {
        let merchant_bucket_size = getSrV3MerchantBucketSize(txn_detail, txn_card_info).await;
        // LogUtils::logInfoT(
        //     "createKeysIfNotExist",
        //     &format!("Creating keys with bucket size as {}", merchant_bucket_size),
        // );
        let score_list = vec!["1".to_string(); merchant_bucket_size.clone().try_into().unwrap()];
        let redis = C::kvRedis();
        GU::create_moving_window_and_score(
            redis,
            key_for_gateway_selection_queue,
            key_for_gateway_selection_score,
            merchant_bucket_size,
            score_list
        ).await;
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
    //LogUtils::logInfoT("updateScoreAndQueue", "Updating sr v3 score and queue");
    createKeysIfNotExist(
        key_for_gateway_selection_queue.clone(),
        key_for_gateway_selection_score.clone(),
        txn_detail.clone(),
        txn_card_info,
    );
    let (value, should_score_increase) : (String, bool) = match gateway_scoring_type {
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
    ).await;
    // LogUtils::logInfoT(
    //     "updateScoreAndQueue",
    //     &format!("Popped Redis Value {}", popped_status),
    // );
    let returned_value = match serde_json::from_slice::<Option<SrV3DebugBlock>>(popped_status.as_bytes()){
            Ok(maybe_popped_status_block) => {
                get_status(maybe_popped_status_block, popped_status)
            },
            Err(_) => popped_status
        };
    
    // LogUtils::logInfoT(
    //     "updateScoreAndQueue",
    //     &format!("Popped Returned Value {}", returned_value),
    // );
    if returned_value == value {
        return;
    } else {
        updateScore(
            C::kvRedis(),
            key_for_gateway_selection_score.clone(),
            should_score_increase,
        );
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
    let merchant_sr_v3_input_config:Option<SrV3InputConfig>  =
        findByNameFromRedis(C::SR_V3_INPUT_CONFIG(MID::merchant_id_to_text(txn_detail.merchantId)).get_key()).await;
    let pmt = txn_card_info.paymentMethodType.to_text();
    let pm = GU::get_payment_method(
        (&pmt).to_string(),
        txn_card_info.paymentMethod,
        txn_detail.sourceObject.unwrap_or_default(),
    );
    let maybe_bucket_size = GU::get_sr_v3_bucket_size(merchant_sr_v3_input_config, &pmt, &pm);
    let merchant_bucket_size = match maybe_bucket_size {
        None => {
            let default_sr_v3_input_config:Option<SrV3InputConfig> =
                findByNameFromRedis(DC::srV3DefaultInputConfig.get_key()).await;
            GU::get_sr_v3_bucket_size(default_sr_v3_input_config, &pmt, &pm)
                .unwrap_or(C::defaultSrV3BasedBucketSize)
        }
        Some(bucket_size) => bucket_size,
    };
    // LogUtils::logInfoT(
    //     "sr_v3_bucket_size",
    //     &format!("Bucket Size: {}", merchant_bucket_size),
    // );
    merchant_bucket_size
}

fn get_status(maybe_popped_status_block: Option<SrV3DebugBlock>, default_status: String) -> String {
    maybe_popped_status_block
        .map(|block| block.txn_status)
        .unwrap_or(default_status)
}