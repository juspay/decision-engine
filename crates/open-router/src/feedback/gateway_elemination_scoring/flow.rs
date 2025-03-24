// Automatically converted from Haskell to Rust
// Generated on 2025-03-23 12:10:32


// Local Imports  
use feedback::constants::{  
    ecRedis,  
    ecRedis2,  
    kvRedis,  
    kvRedis2,  
    globalGatewayScoringEnabledMerchants,  
    globalOutageGatewayScoringEnabledMerchants,  
    defaultGWScoringRewardFactor,  
    defaultScoreKeysTTL,  
    defaultScoreGlobalKeysTTL,  
    minimumGatewayScore,  
    defaultMinimumGatewayScore,  
    gatewayRewardFactor,  
    outageRewardFactor,  
    gatewayScoreOutageKeyTTL,  
    gatewayScoreGlobalKeyTTL,  
    gatewayScoreGlobalOutageKeyTTL,  
    enforceGwScoreKvRedis,  
    srScoreRedisFallbackLookupDisable,  
    gatewayScoreMerchantArrMaxLength,  
    defaultMerchantArrMaxLength,  
    defaultGWScoringPenaltyFactor,  
    outagePenaltyFactor,  
    gatewayPenaltyFactor,  
    GATEWAY_SCORE_THIRD_DIMENSION_TTL,  
};  
  
use gateway_decider::constants::{enableEliminationV2, enableEliminationV2ForOutage};  
  
use types::gateway_routing_input as ETGRI;  
  
use gateway_decider::utils::decode_and_log_error;  
  
use gateway_decider::gw_scoring::get_sr1_and_sr2_and_n;  
  
use db::storage::types::merchant_account as MerchantAccount;  
  
use feedback::utils as EulerTransforms;  
use feedback::utils::GatewayScoringType as GatewayScoreType;  
  
use feedback::types::{  
    TxnCardInfo,  
    TxnDetail,  
    CachedGatewayScore,  
    MerchantScoringDetails,  
    KeyType,  
    ScoringDimension,  
    ScoreType,  
    GatewayScoringKeyType,  
};  
  
use eulerhs::language::get_current_date_in_millis;  
use eulerhs::language as EL;  
  
use utils::redis::cache::find_by_name_from_redis;  
use utils::redis::feature::is_feature_enabled;  
  
use types::merchant as Merchant;  
use types::tenant_config as TenantConfig;  
  
use db::common::types::payment_flows as PF;  
use utils::config::merchant_config as MerchantConfig;  
  
use gateway_decider::types::{GatewayScoringData, ScoreKeyType};  
  
// use utils::utils::common as WebUtils;  
  
// Prelude functions like fromIntegral, Foldable::length, and mapM are part of Rust's standard traits and methods.  
  
// Haskell's Double corresponds to Rust's f64, which is built into the language.  
  
use bytes::Bytes;  
use encoding_rs as TE;  
  
use lens::set;  
use lens::view;  
  
// Converted functions
// Original Haskell function: updateKeyScoreForKeysFromConsumer
pub fn updateKeyScoreForKeysFromConsumer(
    txn_detail: TxnDetail,
    txn_card_info: TxnCardInfo,
    gateway_scoring_type: GatewayScoreType::GatewayScoringType,
    mer_acc_p_id: Merchant::MerchantPId,
    mer_acc: MerchantAccount::MerchantAccount,
    gateway_scoring_key: (ScoreKeyType, Option<String>),
) -> Option<((ScoreKeyType, String), CachedGatewayScore)> {
    let merchant_id = txn_detail.merchant_id.unwrap_or_else(|| "".to_string());
    let (score_key_type, m_key) = gateway_scoring_key;
    match m_key {
        Some(key) => {
            let gateway = txn_detail.gateway.unwrap_or_else(|| "".to_string());
            let hard_key_ttl = getTTLForKey(score_key_type);
            let timestamp = getCurrentDateInMillis();
            let should_enforce_kv_redis = isFeatureEnabled(C.enforceGwScoreKvRedis, &merchant_id, C.kvRedis);
            let should_disable_fallback = isFeatureEnabled(C.srScoreRedisFallbackLookupDisable, &merchant_id, C.kvRedis);
            let m_cached_gateway_score: Option<CachedGatewayScore> = readFromCacheWithFallback(should_enforce_kv_redis, should_disable_fallback, &key);
            let gw_score_to_be_updated = match m_cached_gateway_score {
                None => getNewCachedGatewayScore(&key, gateway_scoring_type, score_key_type, txn_detail, txn_card_info),
                Some(cached_gateway_score) => {
                    if (timestamp - cached_gateway_score.timestamp) > hard_key_ttl - 1000 {
                        EL::logDebugV::<String>("updateKeyScore", &format!("{} has persisted longer than hardTTL", key));
                        getNewCachedGatewayScore(&key, gateway_scoring_type, score_key_type, txn_detail, txn_card_info)
                    } else {
                        cached_gateway_score
                    }
                }
            };
            let updated_cached_gateway_score = {
                let updated_merchant_details_array = getUpdatedMerchantDetailsForGlobalKey(&gw_score_to_be_updated, score_key_type, gateway_scoring_type, txn_detail, txn_card_info);
                let updated_score = match gw_score_to_be_updated.score {
                    None => None,
                    Some(score) => Some(updateKeyScoreForTxnStatus(txn_detail, txn_card_info, &merchant_id, gateway_scoring_type, score, score_key_type)),
                };
                let transaction_count = getTransactionCount(gw_score_to_be_updated.transaction_count, score_key_type, gateway_scoring_type);
                CachedGatewayScore {
                    score: updated_score,
                    timestamp: gw_score_to_be_updated.timestamp,
                    merchants: updated_merchant_details_array,
                    last_reset_timestamp: gw_score_to_be_updated.last_reset_timestamp,
                    transaction_count,
                }
            };
            let remaining_ttl = hard_key_ttl - (timestamp - updated_cached_gateway_score.timestamp).max(0);
            let safe_remaining_ttl = if remaining_ttl < 1000 { hard_key_ttl } else { remaining_ttl as u64 };
            match writeToCacheWithTTL(should_enforce_kv_redis, should_disable_fallback, &key, &updated_cached_gateway_score, safe_remaining_ttl) {
                Ok(_) => EL::logDebugV::<String>("updateKeyScore", &format!("Updated score for key {}", key)),
                Err(_) => EL::logDebugV::<String>("updateKeyScore", &format!("Unable to update score for key {}", key)),
            }
            Some(((score_key_type, key), updated_cached_gateway_score))
        }
        None => None,
    }
}

fn getTransactionCount(
    previous_transaction_count: Option<u64>,
    score_key_type: ScoreKeyType,
    gateway_scoring_type: GatewayScoreType::GatewayScoringType,
) -> Option<u64> {
    if isGlobalKey(score_key_type) {
        None
    } else {
        match previous_transaction_count {
            None => Some(1),
            Some(transaction_count) => {
                if gateway_scoring_type == GatewayScoreType::PENALISE {
                    Some(transaction_count + 1)
                } else {
                    Some(transaction_count)
                }
            }
        }
    }
}


// Original Haskell function: updateKeyScoreForTxnStatus
pub fn updateKeyScoreForTxnStatus(
    txn_detail: TxnDetail,
    txn_card_info: TxnCardInfo,
    merchant_id: String,
    gateway_scoring_type: GatewayScoreType,
    current_key_score: f64,
    score_key_type: ScoreKeyType,
) -> f64 {
        let is_elimination_v2_enabled = isFeatureEnabled(C.enableEliminationV2, &merchant_id, C.kvRedis);
        let is_elimination_v2_enabled_for_outage = isFeatureEnabled(C.enableEliminationV2ForOutage, &merchant_id, C.kvRedis);
        let is_outage_key = isKeyOutage(&score_key_type);
        EL.logDebugT("IS_ELIMINATION_V2_ENABLED", &format!("{}", is_elimination_v2_enabled));

        match gateway_scoring_type {
            GatewayScoreType::PENALISE => {
                updateScoreWithPenalty(
                    is_elimination_v2_enabled,
                    is_outage_key,
                    is_elimination_v2_enabled_for_outage,
                    &merchant_id,
                    &txn_card_info,
                    &txn_detail,
                    current_key_score,
                    &score_key_type,
                );
            }
            GatewayScoreType::REWARD => {
                updateScoreWithReward(
                    is_elimination_v2_enabled,
                    is_outage_key,
                    is_elimination_v2_enabled_for_outage,
                    &merchant_id,
                    &txn_card_info,
                    &txn_detail,
                    current_key_score,
                    &score_key_type,
                );
            }
            _ => current_key_score,
        }
}

async fn updateScoreWithPenalty(
    is_elimination_v2_enabled: bool,
    is_outage_key: bool,
    is_elimination_v2_enabled_for_outage: bool,
    merchant_id: &str,
    txn_card_info: &TxnCardInfo,
    txn_detail: &TxnDetail,
    current_key_score: f64,
    score_key_type: &ScoreKeyType,
) -> f64 {
    match (is_elimination_v2_enabled, is_outage_key, is_elimination_v2_enabled_for_outage) {
        (true, true, true) | (true, _, _) => {
            let m_reward_factor = eliminationV2RewardFactor(merchant_id, txn_card_info, txn_detail);
            match m_reward_factor {
                None => getFailureKeyScore(false, current_key_score, getPenaltyFactor(score_key_type)),
                Some(factor) => getFailureKeyScore(true, current_key_score, 1.0 - factor),
            }
        }
        _ => getFailureKeyScore(false, current_key_score, getPenaltyFactor(score_key_type)),
    }
}

async fn updateScoreWithReward(
    is_elimination_v2_enabled: bool,
    is_outage_key: bool,
    is_elimination_v2_enabled_for_outage: bool,
    merchant_id: &str,
    txn_card_info: &TxnCardInfo,
    txn_detail: &TxnDetail,
    current_key_score: f64,
    score_key_type: &ScoreKeyType,
) -> f64 {
    match (is_elimination_v2_enabled, is_outage_key, is_elimination_v2_enabled_for_outage) {
        (true, true, true) | (true, _, _) => {
            let m_reward_factor = eliminationV2RewardFactor(merchant_id, txn_card_info, txn_detail);
            match m_reward_factor {
                None => getSuccessKeyScore(false, current_key_score, getRewardFactor(score_key_type)),
                Some(factor) => getSuccessKeyScore(true, current_key_score, factor),
            }
        }
        _ => getSuccessKeyScore(false, current_key_score, getRewardFactor(score_key_type)),
    }
}


// Original Haskell function: getSuccessKeyScore
pub fn getSuccessKeyScore(
    use_elimination_v2: bool,
    current_score: f64,
    reward_factor: f64,
) -> f64 {
    let score = if use_elimination_v2 {
        current_score + reward_factor
    } else {
        current_score + (current_score * (reward_factor / 100.0))
    };
    if score > 1.0 {
        1.0
    } else {
        score
    }
}


// Original Haskell function: getFailureKeyScore
pub fn getFailureKeyScore(
    use_elimination_v2: bool,
    current_score: f64,
    penalty_factor: f64,
) -> f64 {
    let m_score = Cutover::findByNameFromRedis(C::minimumGatewayScore);
    let minimum_failure_score = m_score.unwrap_or(C::defaultMinimumGatewayScore);
    let score = if use_elimination_v2 {
        current_score * penalty_factor
    } else {
        current_score - (current_score * (penalty_factor / 100.0))
    };
    if score < minimum_failure_score {
        minimum_failure_score
    } else {
        score
    }
}


// Original Haskell function: getPenaltyFactor
pub fn getPenaltyFactor(scoreKeyType: ScoreKeyType) -> Double {
    fromMaybe(C.defaultGWScoringPenaltyFactor, {
        if isKeyOutage(scoreKeyType) {
            Cutover::findByNameFromRedis(C.outagePenaltyFactor)
        } else {
            Cutover::findByNameFromRedis(C.gatewayPenaltyFactor)
        }
    })
}


// Original Haskell function: getRewardFactor
pub fn getRewardFactor(score_key_type: ScoreKeyType) -> Double {
    let reward_factor = if isKeyOutage(score_key_type) {
        Cutover::findByNameFromRedis(C.outageRewardFactor)
    } else {
        Cutover::findByNameFromRedis(C.gatewayRewardFactor)
    };
    reward_factor.unwrap_or(C.defaultGWScoringRewardFactor)
}


// Original Haskell function: getUpdatedMerchantDetailsForGlobalKey
pub fn getUpdatedMerchantDetailsForGlobalKey(
    cached_gateway_score: CachedGatewayScore,
    score_key_type: ScoreKeyType,
    gateway_scoring_type: GatewayScoreType::GatewayScoringType,
    txn_detail: TxnDetail,
    txn_card_info: TxnCardInfo,
) -> Option<Vec<MerchantScoringDetails>> {
    let merchant_id = txn_detail.merchantId.unwrap_or_else(|| "".to_string());
    if isGlobalKey(score_key_type) {
        match cached_gateway_score.merchants {
            Some(merchant_details_array) => {
                let filtered_merchant_details_array = findMerchantFromMerchantArray(&merchant_id, &merchant_details_array);
                if filtered_merchant_details_array.is_empty() {
                    let arr_max_length = getMerchantArrMaxLength();
                    if merchant_details_array.len() >= arr_max_length {
                        return (Some(merchant_details_array));
                    } else {
                        let merchant_detail = getDefaultMerchantScoringDetailsArray(&merchant_id, 1.0, 1, None);
                        return (Some([merchant_details_array, vec![merchant_detail]].concat()));
                    }
                } else {
                    return  Some(
                        merchant_details_array
                            .iter()
                            .map(replace_transaction_count)
                            .collect(),
                    );
                }
            }
            None => {
                let merchant_scoring_details = getDefaultMerchantScoringDetailsArray(&merchant_id, 1.0, 1, None);
                return (Some(vec![merchant_scoring_details]));
            }
        }
    } else {
        return (None);
    }
}

fn replaceTransactionCount(
    merchant_scoring_details: MerchantScoringDetails,
    txn_detail: &TxnDetail,
    txn_card_info: &TxnCardInfo,
    gateway_scoring_type: GatewayScoreType::GatewayScoringType,
    score_key_type: ScoreKeyType,
) -> MerchantScoringDetails {
    if merchant_scoring_details.merchantId == txn_detail.merchantId.unwrap_or_else(|| "".to_string()) {
        let updated_score = updateKeyScoreForTxnStatus(
            txn_detail,
            txn_card_info,
            &merchant_scoring_details.merchantId,
            gateway_scoring_type,
            merchant_scoring_details.score,
            score_key_type,
        );
        let new_count = if gateway_scoring_type == GatewayScoreType::PENALISE {
            merchant_scoring_details.transactionCount + 1
        } else {
            merchant_scoring_details.transactionCount
        };
        (MerchantScoringDetails {
            score: updated_score,
            transactionCount: new_count,
            ..merchant_scoring_details
        })
    } else {
       (merchant_scoring_details)
    }
}


// Original Haskell function: getNewCachedGatewayScore
pub fn getNewCachedGatewayScore(
    key: String,
    gateway_scoring_type: GatewayScoreType,
    score_key_type: ScoreKeyType,
    txn_detail: TxnDetail,
    txn_card_info: TxnCardInfo,
) -> CachedGatewayScore {
    let merchant_id = txn_detail.merchantId.unwrap_or_else(|| "".to_string());
    let current_date = getCurrentDateInMillis();
    if isGlobalKey(score_key_type) {
        let merchant_scoring_details = getDefaultMerchantScoringDetailsArray(merchant_id, 1.0, 0, None);
        CachedGatewayScore {
            score: None,
            timestamp: current_date,
            last_reset_timestamp: None,
            merchants: Some(vec![merchant_scoring_details]),
            transaction_count: None,
        }
    } else {
        CachedGatewayScore {
            score: Some(1.0),
            timestamp: current_date,
            last_reset_timestamp: Some(current_date),
            merchants: None,
            transaction_count: Some(0),
        }
    }
}


// Original Haskell function: getDefaultMerchantScoringDetailsArray
pub fn getDefaultMerchantScoringDetailsArray(
    merchant_id: String,
    score: f64,
    transaction_count: i32,
    m_last_reset_timestamp: Option<i32>,
) -> MerchantScoringDetails {
    let current_date = getCurrentDateInMillis();
    MerchantScoringDetails {
        score: score,
        merchantId: merchant_id,
        transactionCount: transaction_count,
        lastResetTimestamp: m_last_reset_timestamp.unwrap_or(current_date),
    }
}


// Original Haskell function: getAllUnifiedKeys
pub fn getAllUnifiedKeys(
    txn_detail: TxnDetail,
    txn_card_info: TxnCardInfo,
    mer_acc_p_id: Merchant::MerchantPId,
    m_pf_mc_config: Option<Redis::PfMcConfig>,
    mer_acc: MerchantAccount::MerchantAccount,
    gateway_scoring_data: GatewayScoringData,
) ->  Vec<(ScoreKeyType, Option<String>)> {
    let merchant_id = txn_detail.merchantId.unwrap_or_else(|| "".to_string());
    let is_key_enabled_for_global_gateway_scoring = isFeatureEnabled(
        C::globalGatewayScoringEnabledMerchants,
        &merchant_id,
        C::kvRedis,
    );
    let is_key_enabled_for_merchant_gateway_scoring = async { None }; // Placeholder for MerchantConfig.isPaymentFlowEnabledWithHierarchyCheck
    let is_gateway_scoring_enabled_for_global_outage = isFeatureEnabled(
        C::globalOutageGatewayScoringEnabledMerchants,
        &merchant_id,
        C::kvRedis,
    );
    let is_gateway_scoring_enabled_for_merchant_outage = None ; // Placeholder for MerchantConfig.isPaymentFlowEnabledWithHierarchyCheck

        let global_key = if is_key_enabled_for_global_gateway_scoring {
            let key = getProducerKey(
                &txn_detail,
                Some(&gateway_scoring_data),
                ELIMINATION_GLOBAL_KEY,
                false,
            );
            vec![(ELIMINATION_GLOBAL_KEY, key)]
        } else {
            EL::logDebugV::<String>(
                "getGlobalKeys",
                &format!("Global gateway scoring not enabled for merchant {}", merchant_id),
            );
            vec![(ELIMINATION_GLOBAL_KEY, None)]
        };

        let merchant_key = if is_key_enabled_for_merchant_gateway_scoring.is_some() {
            let key = getProducerKey(
                &txn_detail,
                Some(&gateway_scoring_data),
                ELIMINATION_MERCHANT_KEY,
                false,
            );
            vec![(ELIMINATION_MERCHANT_KEY, key)]
        } else {
            EL::logDebugV::<String>(
                "getMerchantBasedKeys",
                &format!("Merchant gateway scoring not enabled for merchant {}", merchant_id),
            );
            vec![(ELIMINATION_MERCHANT_KEY, None)]
        };

        let global_outage_keys = if is_gateway_scoring_enabled_for_global_outage {
            let key = getProducerKey(
                &txn_detail,
                Some(&gateway_scoring_data),
                OUTAGE_GLOBAL_KEY,
                false,
            );
            vec![(OUTAGE_GLOBAL_KEY, key)]
        } else {
            EL::logDebugV::<String>(
                "getGlobalKeys",
                &format!("Global gateway scoring not enabled for merchant {}", merchant_id),
            );
            vec![(OUTAGE_GLOBAL_KEY, None)]
        };

        let merchant_outage_keys = if is_gateway_scoring_enabled_for_merchant_outage.is_some() {
            let key = getProducerKey(
                &txn_detail,
                Some(&gateway_scoring_data),
                OUTAGE_MERCHANT_KEY,
                false,
            );
            vec![(OUTAGE_MERCHANT_KEY, key)]
        } else {
            EL::logDebugV::<String>(
                "getMerchantScopedOutageKeys",
                &format!("Outage scoring not enabled for merchant {}", merchant_id),
            );
            vec![(OUTAGE_MERCHANT_KEY, None)]
        };

        global_key
            .into_iter()
            .chain(merchant_key)
            .chain(global_outage_keys)
            .chain(merchant_outage_keys)
            .collect()
}


// Original Haskell function: getTTLForKey
pub fn getTTLForKey(score_key_type: ScoreKeyType) -> Int {
    let is_key_global = isGlobalKey(&score_key_type);
    let is_outage_key = isKeyOutage(&score_key_type);
    let key: Option<f64> = match (is_key_global, is_outage_key) {
        (true, true) => Cutover::findByNameFromRedis(C.gatewayScoreGlobalOutageKeyTTL),
        (false, true) => Cutover::findByNameFromRedis(C.gatewayScoreOutageKeyTTL),
        (true, false) => Cutover::findByNameFromRedis(C.gatewayScoreGlobalKeyTTL),
        _ => Cutover::findByNameFromRedis(C.GATEWAY_SCORE_THIRD_DIMENSION_TTL),
    };
    key.map_or_else(
        || getDefaultTTL(&score_key_type),
        |k| k.floor() as Int,
    )
}

fn getDefaultTTL(score_key_type: &ScoreKeyType) -> Int {
    if isGlobalKey(score_key_type) {
        C.defaultScoreGlobalKeysTTL
    } else {
        C.defaultScoreKeysTTL
    }
}

pub fn readFromCacheWithFallback<T>(  
    enforce_kv_redis: bool,  
    disable_fallback: bool,  
    key: Text,  
) -> Option<T> {  
    if enforce_kv_redis {  
        let m_kv_val = getCachedVal(C.kvRedis, C.kvRedis2, &key);  
        match m_kv_val {  
            Some(kv_val) => Some(kv_val),  
            None => {  
                if disable_fallback {  
                    None  
                } else {  
                    getCachedVal(C.ecRedis, C.ecRedis2, &key)  
                }  
            }  
        }  
    } else {  
        let m_ec_val = getCachedVal(C.ecRedis, C.ecRedis2, &key);  
        match m_ec_val {  
            Some(ec_val) => Some(ec_val),  
            None => {  
                if disable_fallback {  
                    None  
                } else {  
                    getCachedVal(C.kvRedis, C.kvRedis2, &key)  
                }  
            }  
        }  
    }  
}  


// Original Haskell function: getMerchantScore
pub fn getMerchantScore(
    merchant_id: Text,
    merchants_array: Vec<MerchantScoringDetails>,
) -> Option<f64> {
    let details = merchants_array.into_iter().find(|msd| msd.merchantId == merchant_id)?;
    Some(details.score)
}


// Original Haskell function: eliminationV2RewardFactor
pub fn eliminationV2RewardFactor(
    merchant_id: Text,
    txn_card_info: TxnCardInfo,
    txn_detail: TxnDetail,
) -> Option<f64> {
        let merch_acc: MerchantAccount = unimplemented!(); // MerchantAccount.findMerchantAccount(merchant_id).await;
        let m_gateway_success_rate_merchant_input: Option<ETGRI.GatewaySuccessRateBasedRoutingInput> = decodeAndLogError(
            "Gateway Decider Input Decode Error",
            &BSL::from_slice(&TE::encode_utf8(&merch_acc.gateway_success_rate_based_decider_input.unwrap_or_default())),
        );

        let txn_card_info = EulerTransforms::transform_ectxncard_info_to_eulertxncard_info(txn_card_info);
        let txn_detail = EulerTransforms::transform_ectxn_detail_to_euler_txn_detail(txn_detail);

        let sr1_and_sr2_and_n = get_sr1_and_sr2_and_n(
            m_gateway_success_rate_merchant_input,
            merchant_id,
            txn_card_info,
            txn_detail,
        );

        match sr1_and_sr2_and_n {
            Some((sr1, sr2, n, m_pmt, m_pm, m_txn_object_type, source)) => {
                EL::log_info_t(
                    "CALCULATING_ALPHA:SR1_SR2_N_PMT_PM_TXNOBJECTTYPE_CONFIGSOURCE",
                    &format!(
                        "{} {} {} {} {} {} {}",
                        sr1,
                        sr2,
                        n,
                        m_pmt.unwrap_or_else(|| "Nothing".to_string()),
                        m_pm.unwrap_or_else(|| "Nothing".to_string()),
                        m_txn_object_type.unwrap_or_else(|| "Nothing".to_string()),
                        source,
                    ),
                );
                EL::log_info_t("ALPHA_VALUE", &format!("{}", calculate_alpha(sr1, sr2, n)));

                Some(calculate_alpha(sr1, sr2, n))
            }
            None => {
                EL::log_info_v(
                    "ELIMINATION_V2_VALUES_NOT_FOUND:ALPHA:PMT_PM_TXNOBJECTTYPE_SOUCREOBJECT",
                    &format!(
                        "{} {} {} {}",
                        txn_card_info.payment_method_type,
                        txn_card_info.payment_method.unwrap_or_else(|| "Nothing".to_string()),
                        txn_detail.txn_object_type,
                        txn_detail.source_object.unwrap_or_else(|| "Nothing".to_string()),
                    ),
                );
                None
            }
        }
}

fn calculate_alpha(sr1: f64, sr2: f64, n: f64) -> f64 {
    ((sr1 - sr2) * (sr1 - sr2)) / ((n * n) * (sr1 * (100.0 - sr1)))
}


// Original Haskell function: findMerchantFromMerchantArray
pub fn findMerchantFromMerchantArray(
    merchant_id: Text,
    merchants_array: Vec<MerchantScoringDetails>,
) -> Vec<MerchantScoringDetails> {
    merchants_array
        .into_iter()
        .filter(|msd| msd.merchantId == merchant_id)
        .collect()
}


// Original Haskell function: getMerchantArrMaxLength
pub fn getMerchantArrMaxLength() -> Int {
    fromMaybe(C.defaultMerchantArrMaxLength, Cutover.findByNameFromRedis(C.gatewayScoreMerchantArrMaxLength))
}


// Original Haskell function: isGlobalKey
pub fn isGlobalKey(scoreKeyType: ScoreKeyType) -> bool {
    scoreKeyType == ELIMINATION_GLOBAL_KEY || scoreKeyType == OUTAGE_GLOBAL_KEY
}


// Original Haskell function: isKeyOutage
pub fn isKeyOutage(scoreKeyType: ScoreKeyType) -> bool {
    scoreKeyType == OUTAGE_GLOBAL_KEY || scoreKeyType == OUTAGE_MERCHANT_KEY
}


// Original Haskell function: filterAndTransformOutageKeys
pub fn filterAndTransformOutageKeys(
    txn_detail: TxnDetail,
    updated_scores: Vec<((ScoreKeyType, String), CachedGatewayScore)>,
) -> Vec<(GatewayScoringKeyType, CachedGatewayScore)> {
    let outage_scores: Vec<_> = updated_scores
        .into_iter()
        .filter(|((score_key_type, _), _)| isKeyOutage(score_key_type))
        .collect();

    let transformed_scores: Vec<_> = outage_scores
        .into_iter()
        .map(|(key_type, score)| {
            let transformed_key = transformOutageKey(key_type, txn_detail);
            (transformed_key, score)
        })
        .collect();

    transformed_scores
}


// Original Haskell function: transformOutageKey
pub fn transformOutageKey(
    key_type: (ScoreKeyType, String),
    txn_detail: TxnDetail,
) -> GatewayScoringKeyType {
    let (score_key_type, key) = key_type;
    let ttl = getTTLForKey(score_key_type);
    GatewayScoringKeyType {
        key: Some(key),
        ttl: Some(ttl),
        downThreshold: None,
        eliminationMaxCount: None,
        dimension: None,
        merchantId: txn_detail.merchantId.unwrap_or_else(|| "".to_string()),
        gateway: txn_detail.gateway.unwrap_or_else(|| "".to_string()),
        authType: None,
        cardBin: None,
        cardIssuerBankName: None,
        paymentMethodType: None,
        paymentMethod: None,
        sourceObject: None,
        paymentSource: None,
        cardType: None,
        keyType: if isGlobalKey(score_key_type) {
            KeyType::GLOBAL
        } else {
            KeyType::MERCHANT
        },
        scoreType: ScoreType::OUTAGE,
        softTTL: None,
    }
}

