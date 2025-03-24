// Automatically converted from Haskell to Rust
// Generated on 2025-03-23 12:02:17

// Converted imports
use feedback::constants as C;
use std::string::String as T;
use feedback::utils::get_current_ist_date_with_format;
use eulerhs::prelude::*;
use std::vec::Vec;
use std::vec::Vec;
use gateway_decider::utils as GU;
use gateway_decider::types::{RoutingFlowType, GatewayScoringData, ScoreKeyType};
use utils::config::merchant_config as MC;
use feedback::types::*;
use feedback::utils as U;
use feedback::utils::*;
use std::string::String as TE;
use eulerhs::tenant_redis_layer as RC;
use utils::redis::cache as Cutover;
use eulerhs::language as LogUtils;
use control::monad::extra::maybe_m;
use control::monad::except::run_except;
use db::storage::types::merchant_account as MerchantAccount;
use utils::redis as Redis;
use feedback::utils::{log_gateway_score_type, get_producer_key};
use gateway_decider::types as UpdateStatus;
use feedback::utils::*;
use utils::redis::feature::is_feature_enabled;
use eulerhs::language as L;
use serde_json as A;
use std::vec::Vec as BSL;
use feedback::types::{TxnCardInfo, PaymentMethodType, MerchantGatewayAccount};


// Converted functions
// Original Haskell function: updateSrV3Score
pub fn updateSrV3Score(
    gateway_scoring_type: GatewayScoringType,
    txn_detail: TxnDetail,
    txn_card_info: TxnCardInfo,
    merchant_acc: MerchantAccount::MerchantAccount,
    mb_gateway_scoring_data: Option<GatewayScoringData>,
) {
    let is_merchant_enabled_globally = undefined; // MC.isPaymentFlowsEnabledAtMerchantConfig(merchant_acc, &["SR_BASED_ROUTING"], Redis::Enforce);
    if is_merchant_enabled_globally {
        let cutover_result = isFeatureEnabled(
            C::srV3BasedFlowCutover,
            txn_detail.merchantId.unwrap_or_default(),
            C::kvRedis,
        );
        if cutover_result {
            match txn_card_info.paymentMethodType {
                None => {
                    LogUtils::logInfoT("PMT not found", "pmt is not present for this transaction having id");
                }
                Some(payment_method_type) => {
                    match (txn_detail.merchantId, txn_detail.gateway) {
                        (None, _) => {
                            LogUtils::logInfoT("merchantId not found", "merchantId not found for this transaction having id");
                        }
                        (_, None) => {
                            LogUtils::logInfoT("gateway not found", "gateway not found for this transaction having id");
                        }
                        (Some(merchant_id), Some(gateway)) => {
                            let unified_sr_v3_key = getProducerKey(txn_detail, mb_gateway_scoring_data, SR_V3_KEY, false);
                            let key_for_gateway_selection = unified_sr_v3_key.unwrap_or_else(|| {
                                getKeyForGatewaySelection(payment_method_type, merchant_id, gateway, txn_detail, txn_card_info)
                            });
                            LogUtils::logInfoT("SR V3 Based oneD Producer Key", &key_for_gateway_selection);
                            let key_for_gateway_selection_queue = format!("{}_{}queue", key_for_gateway_selection, "_");
                            let key_for_gateway_selection_score = format!("{}_{}score", key_for_gateway_selection, "_");
                            updateScoreAndQueue(
                                &key_for_gateway_selection_queue,
                                &key_for_gateway_selection_score,
                                gateway_scoring_type,
                                txn_detail,
                                txn_card_info,
                            );
                            if [CARD, UPI].contains(&payment_method_type) {
                                let key3d_for_gateway_selection = unified_sr_v3_key.unwrap_or_else(|| {
                                    get3DKeyForGatewaySelection(payment_method_type, merchant_id, gateway, txn_detail, txn_card_info)
                                });
                                if key3d_for_gateway_selection != key_for_gateway_selection {
                                    LogUtils::logInfoT("SR V3 Based threeD Producer Key", &key3d_for_gateway_selection);
                                    let key3d_for_gateway_selection_queue = format!("{}_{}queue", key3d_for_gateway_selection, "_");
                                    let key3d_for_gateway_selection_score = format!("{}_{}score", key3d_for_gateway_selection, "_");
                                    updateScoreAndQueue(
                                        &key3d_for_gateway_selection_queue,
                                        &key3d_for_gateway_selection_score,
                                        gateway_scoring_type,
                                        txn_detail,
                                        txn_card_info,
                                    );
                                }
                            }
                            logGatewayScoreType(gateway_scoring_type, SRV3_FLOW, txn_detail);
                        }
                    }
                }
            }
        } else {
            LogUtils::logInfoT("updateSrV3Score", &format!(
                "SR V3 based gateway flow cutover is not enabled for {}",
                txn_detail.merchantId.unwrap_or_default()
            ));
        }
    }
}


// Original Haskell function: getKeyForGatewaySelection
pub fn getKeyForGatewaySelection(
    payment_method_type: PaymentMethodType,
    merchant_id: String,
    gateway_name: String,
    txn_detail: TxnDetail,
    txn_card_info: TxnCardInfo,
) -> String {
        let two_d_key = {
            let order_type = txn_detail.txnObjectType.as_ref().map_or("", |ot| &ot.to_string());
            let base_key = vec![
                C.gatewaySelectionV3OrderTypeKeyPrefix.to_string(),
                merchant_id.clone(),
                order_type.to_string(),
                payment_method_type.to_string(),
            ];

            match payment_method_type {
                PaymentMethodType::UPI => {
                    let source_object = vec![txn_card_info
                        .paymentMethod
                        .as_ref()
                        .filter(|pm| pm == &&"UPI".to_string())
                        .map_or_else(|| txn_detail.sourceObject.clone(), |pm| pm.clone())
                        .unwrap_or_default()];
                    GU::intercalateWithoutEmptyString("_", &[base_key, source_object].concat())
                }
                PaymentMethodType::CARD => {
                    let card_key = vec![
                        txn_card_info.paymentMethod.clone().unwrap_or_default(),
                        txn_card_info.cardType.clone().unwrap_or_default(),
                    ];
                    GU::intercalateWithoutEmptyString("_", &[base_key, card_key].concat())
                }
                _ => {
                    let inter_key = vec![txn_card_info.paymentMethod.clone().unwrap_or_default()];
                    GU::intercalateWithoutEmptyString("_", &[base_key, inter_key].concat())
                }
            }
        };

        let gri_s_rv2_cutover = isFeatureEnabled(
            C.isGriEnabledMerchantSRv2Producer,
            txn_detail.merchantId.clone().unwrap_or_default(),
            C.kvRedis,
        );

        if gri_s_rv2_cutover {
            let merchant_gateway_account: Option<MerchantGatewayAccount> = maybeM(
                async { None },
                |mga_id| async { None }, // Replace with actual implementation
                async { txn_detail.merchantGatewayAccountId.clone() },
            );

            let gw_ref_id = merchant_gateway_account
                .as_ref()
                .and_then(|mga| mga.referenceId.clone())
                .unwrap_or_else(|| "NULL".to_string());

            format!("{}_{}_{}", two_d_key, gw_ref_id, gateway_name)
        } else {
            format!("{}_{}", two_d_key, gateway_name)
        }
}


// Original Haskell function: get3DKeyForGatewaySelection
pub fn get3DKeyForGatewaySelection(
    payment_method_type: PaymentMethodType,
    merchant_id: Text,
    gateway_name: Text,
    txn_detail: TxnDetail,
    txn_card_info: TxnCardInfo,
) -> Text {
        let order_type = txn_detail.txnObjectType.as_ref().map_or("", |obj| &obj.to_string());
        let base_key = vec![
            C.gatewaySelectionV3OrderTypeKeyPrefix,
            merchant_id.clone(),
            order_type.to_string(),
            payment_method_type.to_string(),
        ];
        let m_source_object = if txn_card_info.paymentMethod == Some("UPI".to_string()) {
            txn_detail.sourceObject.clone()
        } else {
            txn_card_info.paymentMethod.clone()
        };

        let three_d_key = match payment_method_type {
            PaymentMethodType::CARD => {
                let card_key = vec![
                    txn_card_info.paymentMethod.clone().unwrap_or_default(),
                    txn_card_info.cardType.clone().unwrap_or_default(),
                ];
                let auth_type_sr_routing_producer_enabled = isFeatureEnabled(
                    C.authTypeSrRoutingProducerEnabledMerchant,
                    txn_detail.merchantId.clone().unwrap_or_default(),
                    C.kvRedis,
                );
                let bank_level_sr_routing_producer_enabled = isFeatureEnabled(
                    C.bankLevelSrRoutingProducerEnabledMerchant,
                    txn_detail.merchantId.clone().unwrap_or_default(),
                    C.kvRedis,
                );
                if auth_type_sr_routing_producer_enabled {
                    GU::intercalateWithoutEmptyString(
                        "_",
                        &[
                            base_key.clone(),
                            card_key.clone(),
                            vec![txn_card_info.authType.clone().unwrap_or_default()],
                        ]
                        .concat(),
                    )
                } else if bank_level_sr_routing_producer_enabled {
                    let top_bank_list = GU::getRoutingTopBankList();
                    let maybe_bank_code = fetchJuspayBankCodeFromPaymentSource(&txn_card_info)
                        .unwrap_or_else(|| "UNKNOWN".to_string());
                    let append_bank_code = if top_bank_list.contains(&maybe_bank_code) {
                        maybe_bank_code
                    } else {
                        "".to_string()
                    };
                    GU::intercalateWithoutEmptyString(
                        "_",
                        &[
                            base_key.clone(),
                            card_key.clone(),
                            vec![append_bank_code],
                        ]
                        .concat(),
                    )
                } else {
                    GU::intercalateWithoutEmptyString(
                        "_",
                        &base_key.iter().chain(card_key.iter()).cloned().collect::<Vec<_>>(),
                    )
                }
            }
            PaymentMethodType::UPI => {
                if matches!(m_source_object.as_deref(), Some("UPI_COLLECT") | Some("COLLECT")) {
                    let handle_list = GU::getUPIHandleList();
                    let source_object = m_source_object.unwrap_or_default();
                    let upi_handle = txn_card_info
                        .paymentSource
                        .as_ref()
                        .and_then(|ps| getTrueString(ps))
                        .map(|ps| {
                            T::split_on(&ps, "@")
                                .last()
                                .map(|s| s.to_uppercase())
                                .unwrap_or_default()
                        })
                        .unwrap_or_default();
                    let append_handle = if handle_list.contains(&upi_handle) {
                        upi_handle
                    } else {
                        "".to_string()
                    };
                    GU::intercalateWithoutEmptyString(
                        "_",
                        &[
                            base_key.clone(),
                            vec![source_object, append_handle],
                        ]
                        .concat(),
                    )
                } else if matches!(m_source_object.as_deref(), Some("UPI_PAY") | Some("PAY")) {
                    let source_object = m_source_object.unwrap_or_default();
                    let psp_app_sr_routing_producer_enabled = isFeatureEnabled(
                        C.pspAppSrRoutingProducerEnabledMerchant,
                        txn_detail.merchantId.clone().unwrap_or_default(),
                        C.kvRedis,
                    );
                    let psp_package_sr_routing_producer_enabled = isFeatureEnabled(
                        C.pspPackageSrRoutingProducerEnabledMerchant,
                        txn_detail.merchantId.clone().unwrap_or_default(),
                        C.kvRedis,
                    );
                    if psp_app_sr_routing_producer_enabled {
                        let psp_list = GU::getUPIPspList();
                        let juspay_bank_code = getJuspayBankCodeFromInternalMetadata(&txn_detail)
                            .unwrap_or_else(|| "UNKNOWN".to_string());
                        let append_psp_bank_code = if psp_list.contains(&juspay_bank_code) {
                            juspay_bank_code
                        } else {
                            "".to_string()
                        };
                        GU::intercalateWithoutEmptyString(
                            "_",
                            &[
                                base_key.clone(),
                                vec![source_object, append_psp_bank_code],
                            ]
                            .concat(),
                        )
                    } else if psp_package_sr_routing_producer_enabled {
                        let package_list = GU::getUPIPackageList();
                        let upi_package = txn_card_info
                            .paymentSource
                            .as_ref()
                            .and_then(|ps| getTrueString(ps))
                            .map(|ps| ps.to_uppercase())
                            .unwrap_or_default();
                        let append_package = if package_list.contains(&upi_package) {
                            upi_package
                        } else {
                            "".to_string()
                        };
                        GU::intercalateWithoutEmptyString(
                            "_",
                            &[
                                base_key.clone(),
                                vec![source_object, append_package],
                            ]
                            .concat(),
                        )
                    } else {
                        GU::intercalateWithoutEmptyString(
                            "_",
                            &[
                                base_key.clone(),
                                vec![source_object],
                            ]
                            .concat(),
                        )
                    }
                } else {
                    GU::intercalateWithoutEmptyString(
                        "_",
                        &[
                            base_key.clone(),
                            vec![txn_detail.sourceObject.clone().unwrap_or_default()],
                        ]
                        .concat(),
                    )
                }
            }
            _ => GU::intercalateWithoutEmptyString(
                "_",
                &[
                    base_key.clone(),
                    vec![txn_card_info.paymentMethod.clone().unwrap_or_default()],
                ]
                .concat(),
            ),
        };

        let gri_sr_v2_cutover = isFeatureEnabled(
            C.isGriEnabledMerchantSRv2Producer,
            txn_detail.merchantId.clone().unwrap_or_default(),
            C.kvRedis,
        );
        if gri_sr_v2_cutover {
            let merchant_gateway_account = txn_detail
                .merchantGatewayAccountId
                .as_ref()
                .and_then(|mga_id| {
                    // Assuming getMerchantGatewayAccountFromId is an async function
                    getMerchantGatewayAccountFromId(mga_id, OptionalParameters { disableDecryption: None })
                        .ok()
                });
            let gw_ref_id = merchant_gateway_account
                .as_ref()
                .and_then(|mga| mga.referenceId.clone())
                .unwrap_or_else(|| "NULL".to_string());
            T::intercalate("_", &[three_d_key, gw_ref_id, gateway_name])
        } else {
            T::intercalate("_", &[three_d_key, gateway_name])
        }
}


// Original Haskell function: createKeysIfNotExist
pub fn createKeysIfNotExist(
    key_for_gateway_selection_queue: Text,
    key_for_gateway_selection_score: Text,
    txn_detail: TxnDetail,
    txn_card_info: TxnCardInfo,
) {
    let is_queue_key_exists = isKeyExistsRedis(C.kvRedis, key_for_gateway_selection_queue.clone());
    let is_score_key_exists = isKeyExistsRedis(C.kvRedis, key_for_gateway_selection_score.clone());
    LogUtils::logInfoT(
        "createKeysIfNotExist",
        &format!(
            "Value for isQueueKeyExists is {} and isScoreKeyExists is {}",
            is_queue_key_exists, is_score_key_exists
        ),
    );
    if is_queue_key_exists && is_score_key_exists {
        return;
    } else {
        let merchant_bucket_size = getSrV3MerchantBucketSize(txn_detail, txn_card_info);
        LogUtils::logInfoT(
            "createKeysIfNotExist",
            &format!(
                "Creating keys with bucket size as {}",
                merchant_bucket_size
            ),
        );
        GU::createMovingWindowAndScore(
            C.kvRedis,
            key_for_gateway_selection_queue,
            key_for_gateway_selection_score,
            merchant_bucket_size,
            vec!["1".to_string(); merchant_bucket_size],
        );
    }
}


// Original Haskell function: updateScoreAndQueue
pub fn updateScoreAndQueue(
    key_for_gateway_selection_queue: Text,
    key_for_gateway_selection_score: Text,
    gateway_scoring_type: GatewayScoringType,
    txn_detail: TxnDetail,
    txn_card_info: TxnCardInfo,
) {
    LogUtils::logInfoT("updateScoreAndQueue", "Updating sr v3 score and queue");
    createKeysIfNotExist(
        key_for_gateway_selection_queue,
        key_for_gateway_selection_score,
        txn_detail,
        txn_card_info,
    );
    let (value, should_score_increase) = match gateway_scoring_type {
        GatewayScoringType::PENALISE_SRV3 => ("0".into(), false),
        GatewayScoringType::REWARD => ("1".into(), true),
        _ => ("0".into(), false),
    };
    let is_debug_mode_enabled = isFeatureEnabled(
        C::enableDebugModeOnSrV3,
        txn_detail.merchantId.clone().unwrap_or_default(),
        C::kvRedis,
    );
    if is_debug_mode_enabled {
        let _ = RC::rHDelB(
            C::kvRedis,
            &format!(
                "{}{}",
                C::pendingTxnsKeyPrefix,
                txn_detail.merchantId.clone().unwrap_or_default()
            ),
            &[txn_detail.txnUuid.clone().unwrap_or_default()],
        );
    } else {
        let _ = RC::rDel(
            C::kvRedis,
            &[format!(
                "{}{}",
                C::pendingTxnsKeyPrefix,
                txn_detail.merchantId.clone().unwrap_or_default()
            )],
        );
    }
    let current_ist_time = getCurrentIstDateWithFormat("YYYY-MM-DD HH:mm:SS.sss");
    let date_created = dateInIST(
        &txn_detail.dateCreated.to_string(),
        "YYYY-MM-DD HH:mm:SS.sss",
    )
    .unwrap_or_default();
    let updated_value = if is_debug_mode_enabled {
        TE::decodeUtf8(
            &BSL::toStrict(&A::encode(&debugBlock(
                txn_detail.clone(),
                current_ist_time.clone(),
                date_created.clone(),
                value.clone(),
            ))),
        )
        .unwrap()
    } else {
        value.clone()
    };
    let popped_status = updateMovingWindow(
        C::kvRedis,
        key_for_gateway_selection_queue,
        key_for_gateway_selection_score,
        updated_value.clone(),
    );
    LogUtils::logInfoT("updateScoreAndQueue", &format!("Popped Redis Value {}", popped_status));
    let returned_value = match A::eitherDecode::<Option<SrV3DebugBlock>>(
        &BSL::fromStrict(&TE::encodeUtf8(&popped_status)),
    ) {
        Ok(Some(maybe_popped_status_block)) => getStatus(maybe_popped_status_block, popped_status.clone()),
        _ => popped_status.clone(),
    };
    LogUtils::logInfoT("updateScoreAndQueue", &format!("Popped Returned Value {}", returned_value));
    if returned_value == value {
        return;
    } else {
        updateScore(C::kvRedis, key_for_gateway_selection_score, should_score_increase);
    }
}

fn debugBlock(
    txn_detail: TxnDetail,
    current_time: Text,
    date_created: Text,
    value: Text,
) -> SrV3DebugBlock {
    SrV3DebugBlock {
        txn_uuid: txn_detail.txnUuid.clone().unwrap_or_default(),
        order_id: txn_detail.orderId.clone(),
        date_created,
        current_time,
        txn_status: value,
    }
}

fn getStatus(maybe_popped_status_block: Option<SrV3DebugBlock>, popped_status: Text) -> Text {
    match maybe_popped_status_block {
        Some(popped_status_block) => popped_status_block.txn_status.clone(),
        None => popped_status,
    }
}


// Original Haskell function: getSrV3MerchantBucketSize
pub fn getSrV3MerchantBucketSize(
    txn_detail: TxnDetail,
    txn_card_info: TxnCardInfo,
) -> Int {
        let merchant_sr_v3_input_config = Cutover::findByNameFromRedis(
            C::SR_V3_INPUT_CONFIG(&txn_detail.merchantId.unwrap_or_default()),
        );
        let pmt = match &txn_card_info.paymentMethodType {
            Some(x) => x.to_string(),
            None => String::new(),
        };
        let pm = GU::getPaymentMethod(
            &pmt,
            &txn_card_info.paymentMethod.unwrap_or_default(),
            &txn_detail.sourceObject.unwrap_or_default(),
        );
        let maybe_bucket_size = GU::getSrV3BucketSize(&merchant_sr_v3_input_config, &pmt, &pm);
        let merchant_bucket_size = match maybe_bucket_size {
            None => {
                let default_sr_v3_input_config =
                    Cutover::findByNameFromRedis(C::srV3DefaultInputConfig);
                GU::getSrV3BucketSize(&default_sr_v3_input_config, &pmt, &pm)
                    .unwrap_or(C::defaultSrV3BasedBucketSize)
            }
            Some(bucket_size) => bucket_size,
        };
        LogUtils::logInfoT("sr_v3_bucket_size", &format!("Bucket Size: {}", merchant_bucket_size));
        merchant_bucket_size
}

